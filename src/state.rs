use crate::message::Message;
use crate::plan::Plan;

#[derive(Debug, Clone)]
pub struct AgentState {
    pub goal: String,
    pub plan: Option<Plan>,
    pub messages: Vec<Message>,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            goal: String::new(),
            plan: None,
            messages: Vec::new(),
        }
    }
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn is_completed(&self) -> bool {
        match &self.plan {
            Some(plan) => plan.is_complete(),
            None => false,
        }
    }
}
