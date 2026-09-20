use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub tasks: Vec<PlanTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: usize,
    pub description: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

impl Plan {
    pub fn is_complete(&self) -> bool {
        !self.tasks.is_empty()
            && self
                .tasks
                .iter()
                .all(|task| task.status == TaskStatus::Completed)
    }

    pub fn current_task(&self) -> Option<&PlanTask> {
        self.tasks
            .iter()
            .find(|task| task.status == TaskStatus::InProgress)
    }

    pub fn mark_in_progress(&mut self, task_id: usize) {
        for task in &mut self.tasks {
            if task.id == task_id {
                task.status = TaskStatus::InProgress;
            }
        }
    }

    pub fn mark_completed(&mut self, task_id: usize) {
        for task in &mut self.tasks {
            if task.id == task_id {
                task.status = TaskStatus::Completed;
            }
        }
    }
}
