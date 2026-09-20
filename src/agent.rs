use std::error::Error;
use std::sync::Arc;

use crate::{
    client::api::LlmClient,
    message::Message,
    state::AgentState,
    toolregistry::ToolRegistry,
    tools::{CompleteGoalArgs, UpdateArgs},
};

pub struct Agent {
    llm: Arc<LlmClient>,
    registry: Arc<ToolRegistry>,
    max_steps: usize,
}

impl Agent {
    pub fn new(llm: Arc<LlmClient>, registry: Arc<ToolRegistry>, max_steps: usize) -> Self {
        Self {
            llm,
            registry,
            max_steps,
        }
    }

    pub async fn run(
        &self,
        state: &mut AgentState,
        user_message: String,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let tool_defs = self.registry.definitions();
        state.goal = user_message.clone();
        state.add_message(Message::User {
            content: user_message,
        });

        for i in 0..self.max_steps {
            let response = self.llm.complete(&state.messages, &tool_defs).await?;
            println!("Iteration: {i}\nResponse: \n{response:#?}");
            println!(
                "Usage:\nInput Tokens: {} | Output Tokens: {}",
                response.usage.prompt_tokens, response.usage.completion_tokens
            );
            let choice = response
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| format!("Empty response choices"))?;

            match choice.message {
                Message::Assistant {
                    tool_calls: Some(ref calls),
                    ..
                } if !calls.is_empty() => {
                    let tool_calls_clone = calls.clone();
                    println!("Assistant: {:#?}", choice.message);
                    state.add_message(choice.message);
                    for call in tool_calls_clone {
                        let tool_name = call.function.name;
                        let tool_id = call.id;

                        if tool_name == "update_plan" {
                            let result = self
                                .registry
                                .execute(&tool_name, call.function.arguments)
                                .await?;
                            println!("Update Tool output: {}", result);
                            state.add_message(Message::Tool {
                                tool_call_id: tool_id,
                                content: result,
                            });
                        } else {
                            let result = match self
                                .registry
                                .execute(&tool_name, call.function.arguments)
                                .await
                            {
                                Ok(output) => output,
                                Err(e) => format!("Error executing {tool_name}: {e}"),
                            };
                            println!("{tool_name} tool output: {}", result);
                            state.add_message(Message::Tool {
                                tool_call_id: tool_id,
                                content: result,
                            });
                        }
                    }
                }
                Message::Assistant {
                    content: Some(ref content),
                    ..
                } => {
                    let final_text = content.clone();
                    state.add_message(choice.message);
                    return Ok(final_text);
                }

                _ => return Err("Unexpected message variant returned by model".into()),
            }
        }
        Err("Agent reached max iterations without concluding".into())
    }
}
