use std::error::Error;
mod agent;
mod client;
mod compaction;
mod message;
mod plan;
mod state;
mod tokencalculator;
mod tool;
mod toolregistry;
mod tools;
use std::sync::Arc;
use tools::{Calculator, CompleteGoal, RunPython, UpdatePlan};

use crate::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = Arc::new(client::api::LlmClient::new(
        "http://localhost:11434/v1/chat/completions",
        "qwen3.5:2b-q4_K_M".into(),
    )?);

    let mut registry = toolregistry::ToolRegistry::new();
    registry.register(Calculator);
    registry.register(UpdatePlan);
    registry.register(CompleteGoal);
    registry.register(RunPython);

    let mut agent = Agent::new(client, Arc::new(registry), 5);
    let answer = agent
        .run(
            "Write a python code that finds latest news from google rss related to iphone"
                .to_string(),
        )
        .await?;

    println!("Agent's response: {:#?}", answer);

    Ok(())
}
