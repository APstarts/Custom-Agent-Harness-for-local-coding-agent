use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

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

use crate::agent::Agent;
use tools::{Calculator, CompleteGoal, RunPython, UpdatePlan};

pub fn extract_python_code(response: &str) -> Option<String> {
    if let Some(start) = response.find("```python") {
        let code_start = start + "```python".len();
        if let Some(end) = response[code_start..].find("```") {
            return Some(response[code_start..code_start + end].trim().to_string());
        }
        return Some(response[code_start..].trim().to_string());
    }

    if let Some(start) = response.find("```") {
        let code_start = start + "```".len();
        if let Some(end) = response[code_start..].find("```") {
            return Some(response[code_start..code_start + end].trim().to_string());
        }
        return Some(response[code_start..].trim().to_string());
    }

    None
}

fn parse_cli_args() -> (String, PathBuf) {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut output_path = PathBuf::from("output.py");
    let mut prompt_words = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if (args[i] == "-o" || args[i] == "--output") && i + 1 < args.len() {
            output_path = PathBuf::from(&args[i + 1]);
            i += 2;
        } else {
            prompt_words.push(args[i].clone());
            i += 1;
        }
    }

    let user_prompt = if prompt_words.is_empty() {
        "Write a python code that finds latest news from google rss related to iphone".to_string()
    } else {
        prompt_words.join(" ")
    };

    (user_prompt, output_path)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (user_prompt, output_path) = parse_cli_args();

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
    let answer = agent.run(user_prompt).await?;

    println!("Agent's response:\n{}", answer);

    if let Some(code) = extract_python_code(&answer) {
        let verified_code = agent.verify_and_repair(code, &output_path, 3).await?;
        fs::write(&output_path, &verified_code)?;
        println!("\n============================================================");
        println!(" Successfully saved verified Python script to: {}", output_path.display());
        println!("============================================================");
    } else {
        println!("\n⚠️ No Python code block found in response to save.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_python_code() {
        let md = "Here is the code:\n```python\nprint('hello')\n```\nEnjoy!";
        assert_eq!(extract_python_code(md), Some("print('hello')".to_string()));

        let unclosed = "```python\nprint('unclosed')";
        assert_eq!(extract_python_code(unclosed), Some("print('unclosed')".to_string()));

        let generic = "```\nx = 1\n```";
        assert_eq!(extract_python_code(generic), Some("x = 1".to_string()));

        let no_code = "Just some text with no code";
        assert_eq!(extract_python_code(no_code), None);
    }
}
