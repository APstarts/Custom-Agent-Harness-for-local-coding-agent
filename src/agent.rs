use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

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
Always use print() in your code to inspect results. \
Note: `bs4` / BeautifulSoup is NOT installed in this environment; use `requests`, `re`, `json`, or standard library `urllib` / `html.parser` instead. \
When working with websites or APIs, verify HTTP status codes and responses—do NOT assume an endpoint works if it returns 4xx/5xx errors or empty data; inspect URLs or HTML to find the correct endpoints. When finished, explain what was verified."
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
                .complete_with_max_tokens(&task_messages, &execution_tools, Some(1024))
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
                content: "You are an expert software engineer. Synthesize the completed task results into a comprehensive, fully functional, and complete final response for the user.\n\
CRITICAL REQUIREMENTS FOR PYTHON CODE:\n\
1. Provide complete, syntactically valid code including all imports, helper functions, and an executable entrypoint (`if __name__ == '__main__':`).\n\
2. Only use standard library modules (like `urllib.request`, `urllib.parse`, `re`, `json`, `html.parser`, `os`) or `requests`. Do NOT import `bs4` / BeautifulSoup as it is not installed in the environment.\n\
3. Ensure all URLs, scraping logic, and download routines are fully functional and complete.\n\
4. Never truncate code or leave incomplete blocks."
                    .to_string(),
            },
            Message::User {
                content: format!(
                    "Goal: {}\n\nExecution Results:\n{}\nPlease provide the complete and working final response to the user's goal based on these results.",
                    self.state.goal, results_text
                ),
            },
        ];

        let response = self
            .llm
            .complete(&messages, &[])
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

    pub async fn verify_and_repair(
        &mut self,
        mut current_code: String,
        output_path: &Path,
        max_attempts: usize,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        println!("\n============================================================");
        println!("=== PHASE 4: VERIFYING & AUTO-REPAIRING DELIVERABLE ========");
        println!("============================================================");

        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        for attempt in 1..=max_attempts {
            std::fs::write(output_path, &current_code)?;
            println!(
                "\n>>> [Verification Attempt {}/{}] Executing script: {}",
                attempt,
                max_attempts,
                output_path.display()
            );

            let execution = Command::new("python3").arg(output_path).output();

            let output = match timeout(Duration::from_secs(30), execution).await {
                Ok(res) => match res {
                    Ok(out) => out,
                    Err(e) => {
                        println!("⚠️ Subprocess execution error: {e}");
                        return Ok(current_code);
                    }
                },
                Err(_) => {
                    println!("⚠️ Execution timed out after 30 seconds (script is likely active or finished).");
                    return Ok(current_code);
                }
            };

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                println!("✅ Verification PASSED! Script executed cleanly with exit code 0.");
                if !stdout.trim().is_empty() {
                    let preview: String = stdout.lines().take(10).collect::<Vec<_>>().join("\n");
                    println!("Output preview:\n{}", preview);
                }
                return Ok(current_code);
            } else {
                let exit_code = output.status.code().unwrap_or(-1);
                println!("❌ Verification FAILED with exit code {exit_code}:");
                if !stderr.trim().is_empty() {
                    eprintln!("STDERR:\n{}", stderr.trim());
                } else if !stdout.trim().is_empty() {
                    println!("STDOUT:\n{}", stdout.trim());
                }

                if attempt == max_attempts {
                    println!("⚠️ Reached maximum repair attempts. Keeping latest version.");
                    return Ok(current_code);
                }

                println!(">>> Sending execution error back to LLM for auto-repair...");
                let repair_prompt = vec![
                    Message::System {
                        content: "You are an expert Python engineer. Fix the failing Python script based on the exact execution error.\n\
CRITICAL RULES:\n\
1. Output the complete, working Python script inside a single ```python ... ``` markdown block.\n\
2. Only use standard library modules (like `urllib.request`, `urllib.parse`, `json`, `re`, `html.parser`, `os`, `sys`) or `requests`. Do NOT import `bs4` or BeautifulSoup as it is not installed in the environment.\n\
3. Ensure all URLs, scraping logic, and error handling are fully implemented.\n\
4. Never truncate code or leave incomplete blocks."
                            .to_string(),
                    },
                    Message::User {
                        content: format!(
                            "User Goal: {}\n\nExecution Failure (Exit Code {}):\n{}\nSTDOUT:\n{}\n\nFailing Code:\n```python\n{}\n```\n\nPlease rewrite the complete, working Python script that fixes this error.",
                            self.state.goal, exit_code, stderr.trim(), stdout.trim(), current_code
                        ),
                    },
                ];

                let repair_response = self.llm.complete(&repair_prompt, &[]).await?;
                let choice = repair_response
                    .choices
                    .into_iter()
                    .next()
                    .ok_or_else(|| "Empty repair response".to_string())?;

                let response_text = match choice.message {
                    Message::Assistant {
                        content: Some(c), ..
                    } => c,
                    Message::Assistant {
                        reasoning: Some(r), ..
                    } => r,
                    _ => return Ok(current_code),
                };

                if let Some(new_code) = crate::extract_python_code(&response_text) {
                    current_code = new_code;
                } else {
                    println!("⚠️ Could not extract python code block from repair response. Retrying with existing code.");
                }
            }
        }

        Ok(current_code)
    }
}
