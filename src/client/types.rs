use crate::{message::Message, tool::ToolSpec};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: &'a [Message],
    pub tools: &'a [ToolSpec],
    pub temperature: f64,
    pub top_k: i64,
    pub top_p: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub model: String,
    pub choices: Vec<Choices>,
    pub usage: Usage,
}

#[derive(Deserialize, Debug)]
pub struct Choices {
    pub index: i64,
    pub message: Message,
}

#[derive(Deserialize, Debug)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}
