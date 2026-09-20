use std::error::Error;
mod agent;
mod client;
mod message;
mod plan;
mod state;
mod tool;
mod toolregistry;
mod tools;
use state::AgentState;
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

    let agent = Agent::new(client, Arc::new(registry), 10);
    let answer = agent
        .run(
            &mut AgentState::new(),
            "What is the sum of 1231290734 , 21349087321, and 109872340?".to_string(),
        )
        .await?;

    println!("Agent's response: {:#?}", answer);

    Ok(())
}
