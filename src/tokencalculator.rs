use crate::message::Message;

pub fn estimate_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    (char_count + 3) / 4
}

pub fn estimate_messages_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|msg| match msg {
            Message::System { content } | Message::User { content } => estimate_tokens(content),
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let mut count = content.as_deref().map(estimate_tokens).unwrap_or(0);
                if let Some(calls) = tool_calls {
                    for call in calls {
                        count += estimate_tokens(&call.function.name);
                        count += estimate_tokens(&call.function.arguments);
                    }
                }
                count
            }
            Message::Tool { content, .. } => estimate_tokens(content),
        })
        .sum()
}
