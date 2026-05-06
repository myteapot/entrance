use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskState {
    Todo,
    Running,
    Waiting,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCheckpoint {
    pub round: u32,
    pub state: TaskState,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundState {
    pub current: u32,
    pub checkpoints: Vec<TaskCheckpoint>,
}

#[derive(Debug, Clone, Default)]
pub struct Scheduler;

impl Scheduler {
    pub fn start(&self) -> RoundState {
        RoundState {
            current: 1,
            checkpoints: Vec::new(),
        }
    }

    pub fn checkpoint(
        &self,
        mut round: RoundState,
        state: TaskState,
        summary: impl Into<String>,
    ) -> RoundState {
        round.checkpoints.push(TaskCheckpoint {
            round: round.current,
            state,
            summary: summary.into(),
        });
        round
    }

    pub fn next_round(&self, mut round: RoundState) -> RoundState {
        round.current += 1;
        round
    }
}
