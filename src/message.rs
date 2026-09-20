use serde::{Deserialize, Serialize};

use crate::tool::ToolCall;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System {
        content: String,
    },
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    User {
        content: String,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}
