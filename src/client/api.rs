use super::types::{ChatRequest, ChatResponse};
use crate::{message::Message, tool::ToolSpec};
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
        tools: &[ToolSpec],
    ) -> Result<ChatResponse, Box<dyn Error + Send + Sync>> {
        self.complete_with_max_tokens(messages, tools, None).await
    }

    pub async fn complete_with_max_tokens(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
        max_tokens: Option<u32>,
    ) -> Result<ChatResponse, Box<dyn Error + Send + Sync>> {
        let payload = ChatRequest {
            model: &self.model,
            messages,
            temperature: 0.2,
            tools: tools,
            top_k: 40,
            top_p: 0.9,
            max_tokens,
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
