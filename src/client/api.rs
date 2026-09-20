use super::types::{ChatRequest, ChatResponse};
use crate::{message::Message, tool::ToolDefinition};
use reqwest::{Client, Url};
use std::error::Error;

#[derive(Clone)]
pub struct LlmClient {
    client: Client,
    endpoint: Url,
    model: String,
}

impl LlmClient {
    pub fn new(endpoint: &str, model: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self {
            client: Client::new(),
            endpoint: Url::parse(endpoint)?,
            model: model,
        })
    }

    pub async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, Box<dyn Error + Send + Sync>> {
        let payload = ChatRequest {
            model: &self.model,
            messages,
            temperature: 0.0,
            tools: tools,
            top_k: 2,
            top_p: 2,
        };
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&payload)
            .send()
            .await?
            .json::<ChatResponse>()
            .await?;
        Ok(response)
    }
}
