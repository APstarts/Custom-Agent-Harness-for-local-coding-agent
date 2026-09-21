use std::error::Error;
use std::sync::Arc;

use crate::{
    client::api::LlmClient, compaction::ContextManager, message::Message, plan::Plan,
    state::AgentState, toolregistry::ToolRegistry, tools::UpdateArgs,
};

pub struct Agent {
    llm: Arc<LlmClient>,
    state: AgentState,
    registry: Arc<ToolRegistry>,
    max_steps: usize,
    context_mgr: ContextManager,
    system_prompt: Message,
}

impl Agent {
    pub fn new(llm: Arc<LlmClient>, registry: Arc<ToolRegistry>, max_steps: usize) -> Self {
        Self {
            llm,
            state: AgentState::new(),
            registry,
            max_steps,
            context_mgr: ContextManager::new(4096, 3000),
            system_prompt: Message::System {
                content: "You are a helpful assistant that plans and executes tasks methodically."
                    .to_string(),
            },
        }
    }

    pub fn needs_compaction(&self, messages: &[Message]) -> bool {
        self.context_mgr
            .needs_compaction(messages, self.state.current_tokens)
    }

    async fn generate_plan(&mut self) -> Result<Plan, Box<dyn Error + Send + Sync>> {
        let planning_tools: Vec<crate::tool::ToolSpec> = self
            .registry
            .definitions()
            .into_iter()
            .filter(|spec| spec.function.name == "update_plan")
            .collect();

        let prompt = format!(
            "You are a strategic planning agent. Break down the following user goal into 2 to 4 concrete, sequential sub-tasks.\n\
Goal: {}\n\n\
RULES:\n\
1. Focus on specific problem-domain steps.\n\
2. Do NOT create generic meta-steps like 'write code', 'create plan', or 'run code'.\n\
3. Call the `update_plan` tool to submit your tasks with initial status 'pending'.",
            self.state.goal
        );

        let mut messages = vec![
            Message::System {
                content: "You are a methodical planner that decomposes complex goals into distinct, actionable engineering tasks. You MUST call the `update_plan` tool to submit your tasks. Keep internal thinking concise.".to_string(),
            },
            Message::User { content: prompt },
        ];

        for attempt in 1..=3 {
            let response = self
                .llm
                .complete_with_max_tokens(&messages, &planning_tools, Some(800))
                .await?;
            self.state.current_tokens = response.usage.total_tokens;

            let choice = response
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| "Empty planning response".to_string())?;

            if let Message::Assistant {
                tool_calls: Some(ref calls),
                ..
            } = choice.message
            {
                for call in calls {
                    if call.function.name == "update_plan" {
                        match serde_json::from_str::<UpdateArgs>(&call.function.arguments) {
                            Ok(args) => {
                                if !args.tasks.is_empty() {
                                    return Ok(Plan { tasks: args.tasks });
                                } else {
                                    messages.push(choice.message.clone());
                                    messages.push(Message::Tool {
                                        content: "Error: `tasks` array cannot be empty. Please provide 2 to 4 tasks.".to_string(),
                                        tool_call_id: call.id.clone(),
                                    });
                                }
                            }
                            Err(e) => {
                                messages.push(choice.message.clone());
                                messages.push(Message::Tool {
                                    content: format!("Error: Failed to parse update_plan arguments: {}. Please provide valid JSON conforming to the schema.", e),
                                    tool_call_id: call.id.clone(),
                                });
                            }
                        }
                    }
                }
            } else {
                messages.push(choice.message);
                messages.push(Message::User {
                    content: "Error: You did not call the required `update_plan` tool. You MUST call the `update_plan` tool with your proposed tasks array.".to_string(),
                });
            }

            if attempt < 3 {
                println!(">>> [Planner] Attempt {}/3 did not yield a valid plan, retrying with feedback...", attempt);
            }
        }

        Err("Planner failed to call update_plan after retries".into())
    }

    async fn execute_task(
        &mut self,
        task: &crate::plan::PlanTask,
        task_index: usize,
        total_tasks: usize,
        context_summary: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let execution_tools: Vec<crate::tool::ToolSpec> = self
            .registry
            .definitions()
            .into_iter()
            .filter(|spec| {
                spec.function.name != "update_plan" && spec.function.name != "complete_goal"
            })
            .collect();

        let context_block = if context_summary.is_empty() {
            String::new()
        } else {
            format!("=== PREVIOUS WORK & CONTEXT ===\n{}\n", context_summary)
        };

        let user_prompt = format!(
            "Overall Goal: {}\n\n{}\
=== CURRENT TASK ({} of {}) ===\n\
Task ID: {}\n\
Objective: {}\n\n\
Execute this task using the available tools (such as `run_python`). Always print results. When finished, provide a concise summary.",
            self.state.goal,
            context_block,
            task_index + 1,
            total_tasks,
            task.id,
            task.description
        );

        let mut task_messages = vec![
            Message::System {
                content: "You are an autonomous engineering agent executing a specific sub-task. \
Use available tools (like `run_python` or `calculator`) to accomplish the objective. \
Always use print() in your code to output results. When done, explain what was accomplished."
                    .to_string(),
            },
            Message::User {
                content: user_prompt,
            },
        ];

        let max_task_steps = self.max_steps;
        let mut final_output = String::new();

        for step in 0..max_task_steps {
            if self.needs_compaction(&task_messages) {
                println!(
                    ">>> [ContextManager] Compacting task messages (current tokens: {})...",
                    self.state.current_tokens
                );
                self.context_mgr.compact(&self.llm, &mut task_messages).await?;
            }

            let response = self
                .llm
                .complete_with_max_tokens(&task_messages, &execution_tools, Some(600))
                .await?;
            self.state.current_tokens = response.usage.total_tokens;

            let choice = response
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| "Empty task execution response".to_string())?;

            match choice.message {
                Message::Assistant {
                    tool_calls: Some(ref calls),
                    ..
                } if !calls.is_empty() => {
                    let tool_calls_clone = calls.clone();
                    task_messages.push(choice.message);
                    for call in tool_calls_clone {
                        let tool_name = call.function.name;
                        let tool_id = call.id;

                        let result = match self
                            .registry
                            .execute(&tool_name, call.function.arguments)
                            .await
                        {
                            Ok(output) => output,
                            Err(e) => format!("Error executing {tool_name}: {e}"),
                        };
                        println!(
                            "[Task {} Step {}] {tool_name} output:\n{}",
                            task.id,
                            step + 1,
                            result
                        );
                        final_output = result.clone();
                        let sanitized_result = self.context_mgr.sanitize_tool_output(result);
                        task_messages.push(Message::Tool {
                            tool_call_id: tool_id,
                            content: sanitized_result,
                        });
                    }
                }
                Message::Assistant {
                    content: Some(ref content),
                    ..
                } => {
                    let content_str = content.clone();
                    task_messages.push(choice.message);
                    if !content_str.trim().is_empty() {
                        return Ok(content_str);
                    }
                    if !final_output.trim().is_empty() {
                        return Ok(final_output);
                    }
                    return Ok("Task completed successfully.".to_string());
                }
                _ => return Err("Unexpected message variant returned by model".into()),
            }
        }

        if !final_output.trim().is_empty() {
            Ok(final_output)
        } else {
            Ok("Task completed.".to_string())
        }
    }

    async fn synthesize_results(
        &mut self,
        task_results: &[(crate::plan::PlanTask, String)],
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut results_text = String::new();
        for (task, output) in task_results {
            results_text.push_str(&format!(
                "- Task {} ({}): {}\n",
                task.id,
                task.description,
                output.trim()
            ));
        }

        let messages = vec![
            Message::System {
                content: "You are a helpful assistant. Synthesize the completed task results into a comprehensive final answer for the user."
                    .to_string(),
            },
            Message::User {
                content: format!(
                    "Goal: {}\n\nExecution Results:\n{}\nPlease provide the final response to the user's goal based on these results.",
                    self.state.goal, results_text
                ),
            },
        ];

        let response = self
            .llm
            .complete_with_max_tokens(&messages, &[], Some(800))
            .await?;
        self.state.current_tokens = response.usage.total_tokens;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| "Empty synthesis response".to_string())?;

        match choice.message {
            Message::Assistant {
                content: Some(content),
                ..
            } if !content.trim().is_empty() => Ok(content),
            Message::Assistant {
                reasoning: Some(reasoning),
                ..
            } if !reasoning.trim().is_empty() => Ok(reasoning),
            _ => Err("Synthesis failed to produce text content".into()),
        }
    }

    pub async fn run(
        &mut self,
        user_message: String,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.state.goal = user_message.clone();

        println!("============================================================");
        println!("=== PHASE 1: GENERATING EXECUTION PLAN =====================");
        println!("============================================================");
        let mut plan = self.generate_plan().await?;
        println!("\nGenerated plan with {} tasks:", plan.tasks.len());
        for task in &plan.tasks {
            println!("  [Task {}] {}", task.id, task.description);
        }
        self.state.plan = Some(plan.clone());

        println!("\n============================================================");
        println!("=== PHASE 2: EXECUTING TASKS ===============================");
        println!("============================================================");
        let mut task_results: Vec<(crate::plan::PlanTask, String)> = Vec::new();
        let total_tasks = plan.tasks.len();

        for i in 0..total_tasks {
            plan.mark_in_progress(plan.tasks[i].id);
            self.state.plan = Some(plan.clone());
            println!(
                "\n>>> STARTING TASK {}/{}: {}",
                i + 1,
                total_tasks,
                plan.tasks[i].description
            );

            let mut context_summary = String::new();
            for (prev_task, prev_output) in &task_results {
                let sanitized_prev = self
                    .context_mgr
                    .sanitize_tool_output(prev_output.trim().to_string());
                context_summary.push_str(&format!(
                    "- Task {}: {}\n  Output: {}\n",
                    prev_task.id,
                    prev_task.description,
                    sanitized_prev
                ));
            }

            let task_output = self
                .execute_task(&plan.tasks[i], i, total_tasks, &context_summary)
                .await?;
            println!(
                ">>> FINISHED TASK {}: {}\nResult:\n{}",
                plan.tasks[i].id,
                plan.tasks[i].description,
                task_output.trim()
            );

            plan.mark_completed(plan.tasks[i].id);
            self.state.plan = Some(plan.clone());
            task_results.push((plan.tasks[i].clone(), task_output));
        }

        println!("\n============================================================");
        println!("=== PHASE 3: FINAL SYNTHESIS ===============================");
        println!("============================================================");
        let final_answer = self.synthesize_results(&task_results).await?;
        self.state.add_message(Message::Assistant {
            content: Some(final_answer.clone()),
            reasoning: None,
            tool_calls: None,
        });

        println!("\nPlan completed: {}", self.state.is_completed());
        Ok(final_answer)
    }
}
