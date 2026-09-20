use std::error::Error;
use std::sync::Arc;

use crate::{
    client::api::LlmClient, message::Message, plan::Plan, state::AgentState,
    tokencalculator::estimateTokens, toolregistry::ToolRegistry, tools::UpdateArgs,
};

pub struct Agent {
    llm: Arc<LlmClient>,
    state: AgentState,
    registry: Arc<ToolRegistry>,
    max_steps: usize,
    compaction_threshold: i64,
    system_prompt: Message,
}

impl Agent {
    pub fn new(llm: Arc<LlmClient>, registry: Arc<ToolRegistry>, max_steps: usize) -> Self {
        Self {
            llm,
            state: AgentState::new(),
            registry,
            max_steps,
            compaction_threshold: 3500,
            system_prompt: Message::System { content: "You are a helpful assistant that plans and executes tasks methodically.\n\
RULES:\n\
1. On your first turn, call `update_plan` once to create the plan. Mark Task 1 as `in_progress` and others as `pending`.\n\
2. After creating the plan, IMMEDIATELY call an execution tool (such as `run_python`) to execute Task 1. DO NOT call `update_plan` again until you have executed a tool and observed its output.\n\
3. Once you obtain tool output, call `update_plan` to mark that task `completed` and the next task `in_progress`.\n\
4. When all tasks are completed, call `complete_goal` with your final summary."
                    .to_string() }
        }
    }

    pub fn needs_compaction(&self) -> bool {
        self.state.current_tokens >= self.compaction_threshold
    }

    pub async fn run(
        &mut self,
        user_message: String,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.state.add_message(self.system_prompt.clone());
        let tool_defs = self.registry.definitions();

        //add the user message in the goal in Agent's state
        self.state.goal = user_message.clone();

        //inject the user's message into the context window
        self.state.add_message(Message::User {
            content: user_message,
        });

        let mut consecutive_plan_updates = 0;

        for i in 0..self.max_steps {
            if self.needs_compaction() {
                println!("==================Compaction required!!======================");
            }

            let response = self.llm.complete(&self.state.messages, &tool_defs).await?;
            println!("Iteration: {i}\nResponse: \n{response:#?}");
            println!(
                "Usage:\nInput Tokens: {} | Output Tokens: {}",
                response.usage.prompt_tokens, response.usage.completion_tokens
            );
            //update total tokens
            self.state.current_tokens = response.usage.total_tokens;
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
                    self.state.add_message(choice.message);
                    for call in tool_calls_clone {
                        let tool_name = call.function.name;
                        let tool_id = call.id;

                        if tool_name == "update_plan" {
                            consecutive_plan_updates += 1;
                            let result = if consecutive_plan_updates > 1 {
                                "Plan is already saved and unchanged. Do not call update_plan again. Proceed immediately to execute the current task using run_python.".to_string()
                            } else {
                                if let Ok(args) =
                                    serde_json::from_str::<UpdateArgs>(&call.function.arguments)
                                {
                                    self.state.plan = Some(Plan { tasks: args.tasks });
                                }
                                match self
                                    .registry
                                    .execute(&tool_name, call.function.arguments)
                                    .await
                                {
                                    Ok(output) => output,
                                    Err(error) => format!("Error executing {tool_name}: {error}"),
                                }
                            };
                            println!("Update Tool output: {}", result);
                            self.state.add_message(Message::Tool {
                                tool_call_id: tool_id,
                                content: result,
                            });
                        } else if tool_name == "complete_goal" {
                            consecutive_plan_updates = 0;
                            match self
                                .registry
                                .execute(&tool_name, call.function.arguments)
                                .await
                            {
                                Ok(summary) => {
                                    println!("Complete Goal output: {}", summary);
                                    self.state.add_message(Message::Tool {
                                        tool_call_id: tool_id,
                                        content: summary.clone(),
                                    });
                                    return Ok(summary);
                                }
                                Err(e) => {
                                    let error_msg = format!("Error executing {tool_name}: {e}");
                                    println!("{error_msg}");
                                    self.state.add_message(Message::Tool {
                                        tool_call_id: tool_id,
                                        content: error_msg,
                                    });
                                }
                            }
                        } else {
                            consecutive_plan_updates = 0;
                            let result = match self
                                .registry
                                .execute(&tool_name, call.function.arguments)
                                .await
                            {
                                Ok(output) => output,
                                Err(e) => format!("Error executing {tool_name}: {e}"),
                            };
                            println!("{tool_name} tool output: {}", result);
                            self.state.add_message(Message::Tool {
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
                    self.state.add_message(choice.message);
                    return Ok(final_text);
                }

                _ => return Err("Unexpected message variant returned by model".into()),
            }
        }
        Err("Agent reached max iterations without concluding".into())
    }
}
