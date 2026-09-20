use std::{error::Error, fmt::Debug};

use serde::Deserialize;
use serde_json::json;

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
    pub goal: String,
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
                "type": "objecct",
                "properties": {
                    "goal": {
                        "type": "string"
                    },
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "integer"
                                },
                                "description": {
                                    "type": "string",
                                },
                                "status": {
                                    "type": "string",
                                    "enum": [
                                        "pending",
                                        "in_progress",
                                        "completed"
                                    ]
                                }
                            }
                        }
                    }
                },
                "required": ["goal", "tasks"]
            }),
        }
    }

    async fn execute(&self, arguments: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let args: UpdateArgs = serde_json::from_str(&arguments)?;
        let plan = Plan {
            goal: args.goal,
            tasks: args.tasks,
        };

        Ok(serde_json::to_string(&plan)?)
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
