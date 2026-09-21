use std::error::Error;
use std::sync::Arc;

use crate::{
    client::api::LlmClient, message::Message, tokencalculator::estimate_messages_tokens,
};

#[derive(Clone)]
pub struct ContextManager {
    pub max_tokens: usize,
    pub compaction_threshold: usize,
    pub max_tool_chars: usize,
}

impl ContextManager {
    pub fn new(max_tokens: usize, compaction_threshold: usize) -> Self {
        Self {
            max_tokens,
            compaction_threshold,
            max_tool_chars: 2000,
        }
    }

    pub fn sanitize_tool_output(&self, output: String) -> String {
        if output.len() <= self.max_tool_chars {
            return output;
        }

        let mut head_idx = 1200.min(output.len());
        while !output.is_char_boundary(head_idx) && head_idx > 0 {
            head_idx -= 1;
        }

        let mut tail_idx = output.len().saturating_sub(400);
        while !output.is_char_boundary(tail_idx) && tail_idx < output.len() {
            tail_idx += 1;
        }

        let head = &output[..head_idx];
        let tail = &output[tail_idx..];
        let omitted = output.len() - (head.len() + tail.len());

        format!("{head}\n\n[... Output truncated: omitted {omitted} characters ...]\n\n{tail}")
    }

    pub fn needs_compaction(&self, messages: &[Message], reported_tokens: i64) -> bool {
        reported_tokens as usize >= self.compaction_threshold
            || estimate_messages_tokens(messages) >= self.compaction_threshold
    }

    pub async fn compact(
        &self,
        llm: &Arc<LlmClient>,
        messages: &mut Vec<Message>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if messages.len() <= 4 {
            return Ok(());
        }

        let system_msg = messages[0].clone();
        let user_goal = messages[1].clone();
        let recent_messages = messages[messages.len() - 2..].to_vec();

        let middle_messages = &messages[2..messages.len() - 2];
        let mut middle_text = String::new();
        for msg in middle_messages {
            match msg {
                Message::Assistant {
                    content,
                    tool_calls,
                    ..
                } => {
                    if let Some(c) = content {
                        middle_text.push_str(&format!("Assistant: {c}\n"));
                    }
                    if let Some(calls) = tool_calls {
                        for call in calls {
                            middle_text.push_str(&format!(
                                "Tool Call: {} ({})\n",
                                call.function.name, call.function.arguments
                            ));
                        }
                    }
                }
                Message::Tool { content, .. } => {
                    let preview_len = content.len().min(300);
                    let mut preview_idx = preview_len;
                    while !content.is_char_boundary(preview_idx) && preview_idx > 0 {
                        preview_idx -= 1;
                    }
                    middle_text.push_str(&format!("Tool Output: {}\n", &content[..preview_idx]));
                }
                _ => {}
            }
        }

        let summary_prompt = vec![
            Message::System {
                content: "You are a context compaction engine. Summarize the key findings, discovered information, and progress from the intermediate execution log into a concise 100-word checkpoint.".to_string(),
            },
            Message::User {
                content: format!("Intermediate execution history to summarize:\n{middle_text}"),
            },
        ];

        let response = llm.complete(&summary_prompt, &[]).await?;
        let summary_content = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| match c.message {
                Message::Assistant { content, .. } => content,
                _ => None,
            })
            .unwrap_or_else(|| "Intermediate steps completed execution successfully.".to_string());

        let mut compacted = vec![
            system_msg,
            user_goal,
            Message::System {
                content: format!("[COMPACTED CONTEXT CHECKPOINT]:\n{summary_content}"),
            },
        ];
        compacted.extend(recent_messages);

        println!(
            "\n>>> [ContextManager] Compacted messages: reduced from {} to {} messages",
            messages.len(),
            compacted.len()
        );

        *messages = compacted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_tool_output() {
        let mgr = ContextManager::new(4096, 3000);
        let short = "Hello World".to_string();
        assert_eq!(mgr.sanitize_tool_output(short.clone()), short);

        let long = "A".repeat(5000);
        let sanitized = mgr.sanitize_tool_output(long);
        assert!(sanitized.len() < 2500);
        assert!(sanitized.contains("[... Output truncated: omitted"));
    }

    #[test]
    fn test_needs_compaction() {
        let mgr = ContextManager::new(4096, 3000);
        let messages = vec![Message::User {
            content: "short prompt".to_string(),
        }];
        assert!(!mgr.needs_compaction(&messages, 100));
        assert!(mgr.needs_compaction(&messages, 3100));

        let large_messages = vec![Message::User {
            content: "A".repeat(15000),
        }];
        assert!(mgr.needs_compaction(&large_messages, 100));
    }
}
