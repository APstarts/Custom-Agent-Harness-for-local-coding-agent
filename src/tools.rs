use std::{error::Error, fmt::Debug, time::Duration};

use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;

use crate::{
    plan::{Plan, PlanTask},
    tool::{Tool, ToolDefinition},
};

#[derive(Debug, Deserialize)]
pub struct CalculatorArgs {
    pub first_number: String,
    pub second_number: String,
}

pub struct Calculator;

#[async_trait::async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "Calculator".to_string(),
            description: "Calculates the sum of two numbers".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "first_number": {"type": "number", "description": "The first number to add"},
                    "second_number": {"type":"number", "description": "The second number to add"}
                },
                "required": ["number_1", "number_2"],
            }),
        }
    }

    async fn execute(&self, arguments: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let arguments: CalculatorArgs = serde_json::from_str(arguments.as_str())?;
        let number_1: f64 = arguments.first_number.parse()?;
        let number_2: f64 = arguments.second_number.parse()?;
        let result = number_1 + number_2;

        Ok(format!("The calculation result is {}", result))
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateArgs {
    pub tasks: Vec<PlanTask>,
}

pub struct UpdatePlan;

#[async_trait::async_trait]
impl Tool for UpdatePlan {
    fn name(&self) -> &str {
        "update_plan"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "update_plan".to_string(),
            description: r#"
Create or update the execution plan.

Use this when:
- starting a complex task
- completing a task
- moving to another task
- new information requires changing the plan

Keep the plan short and actionable.
Only one task should normally be in_progress at a time.
Do not mark a task completed unless there is evidence it has been completed.
"#
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "description": "Sequential engineering tasks needed to accomplish the goal",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "integer"
                                },
                                "description": {
                                    "type": "string"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": [
                                        "pending",
                                        "in_progress",
                                        "completed"
                                    ]
                                }
                            },
                            "required": ["id", "description", "status"]
                        }
                    }
                },
                "required": ["tasks"]
            }),
        }
    }

    async fn execute(&self, arguments: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let args: UpdateArgs = serde_json::from_str(&arguments)?;
        let plan = Plan { tasks: args.tasks };
        let json_plan = serde_json::to_string(&plan)?;

        Ok(format!(
            "{json_plan}\nPlan saved. Proceed immediately to execute the current task using the appropriate tool (such as run_python). Do not call update_plan again until you have executed code."
        ))
    }
}

#[derive(Debug, Deserialize)]
pub struct CompleteGoalArgs {
    pub summary: String,
}

pub struct CompleteGoal;

#[async_trait::async_trait]
impl Tool for CompleteGoal {
    fn name(&self) -> &str {
        "complete_goal"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "complete_goal".to_string(),
            description: r#"Signal that the overall user goal has been completed.

Only call this after every required plan task has been completed."#
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary":{
                        "type": "string",
                        "description": "Short summary of the completed work"
                    }
                },
                "required": ["summary"]
            }),
        }
    }

    async fn execute(&self, arguments: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let args: CompleteGoalArgs = serde_json::from_str(&arguments)?;
        Ok(args.summary)
    }
}

#[derive(Debug, Deserialize)]
pub struct RunPythonArgs {
    pub code: String,
}

pub struct RunPython;

#[async_trait::async_trait]
impl Tool for RunPython {
    fn name(&self) -> &str {
        "run_python"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "run_python".to_string(),
            description:
                r#"Executes a Python snippet in a subprocess and returns stdout and stderr.
Use this tool for mathematical computations, data transformations, or logic.
Remember to use `print(...)` to output results you want to observe."#
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Valid Python code to execute"
                    }
                },
                "required": ["code"]
            }),
        }
    }

    async fn execute(&self, arguments: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let args: RunPythonArgs = serde_json::from_str(&arguments)?;

        let execution = Command::new("python3").arg("-c").arg(&args.code).output();

        let output = match tokio::time::timeout(Duration::from_secs(15), execution).await {
            Ok(result) => result?,
            Err(_) => return Ok("Execution timed out after 15 seconds.".to_string()),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if stdout.trim().is_empty() && !stderr.trim().is_empty() {
                Ok(format!("Execution completed with warnings:\n{}", stderr))
            } else if stdout.trim().is_empty() {
                Ok(
                    "Execution succeeded with no output. (Tip: Use print() to display results)"
                        .to_string(),
                )
            } else {
                Ok(stdout)
            }
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            Ok(format!(
                "Execution failed with exit code {exit_code}:\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
            ))
        }
    }
}
