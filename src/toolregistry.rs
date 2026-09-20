use crate::tool::{Tool, ToolDefinition};
use std::{collections::HashMap, error::Error, sync::Arc};

#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool + Send + Sync>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: impl Tool + Send + Sync + 'static) {
        self.tools
            .insert(tool.name().to_lowercase(), Arc::new(tool));
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: String,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let tool = self
            .tools
            .get(&name.to_lowercase())
            .ok_or_else(|| format!("Tool {name} not found in registry"))?;
        tool.execute(args).await
    }
}
