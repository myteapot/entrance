use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    action::ActorRole,
    data_store::{DataStore, StoredForgeTask},
};

#[derive(Debug, Clone, Serialize)]
pub struct PreparedDispatch {
    pub dispatch_role: ActorRole,
    pub dispatch_tool_name: String,
    pub issue_id: String,
    pub issue_status: String,
    pub issue_status_source: String,
    pub issue_title: Option<String>,
    pub project_root: String,
    pub worktree_path: String,
    pub prompt_source: String,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct DispatchReceiptRequest {
    pub parent_task_id: i64,
    pub supervision_strategy: crate::supervision::SupervisionStrategy,
    pub child_dispatch_role: ActorRole,
    pub child_dispatch_tool_name: String,
    pub child_slot: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateTaskRequest {
    pub name: String,
    pub command: String,
    pub args: String,
    pub working_dir: Option<String>,
    pub stdin_text: Option<String>,
    pub required_tokens: String,
    pub metadata: String,
    pub dispatch_receipt: Option<DispatchReceiptRequest>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForgeTaskMetadata {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub issue_id: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub dispatch_role: Option<ActorRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch_tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocator_role: Option<ActorRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allocator_surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_mode: Option<String>,
}

pub trait DispatchHost {
    fn prepare_agent_dispatch(
        &self,
        data_store: &DataStore,
        project_dir: Option<String>,
    ) -> Result<PreparedDispatch>;

    fn prepare_dev_dispatch(
        &self,
        data_store: &DataStore,
        project_dir: Option<String>,
    ) -> Result<PreparedDispatch>;

    fn build_agent_task_request(
        &self,
        dispatch: &PreparedDispatch,
        model: String,
        agent_command: Option<String>,
    ) -> Result<CreateTaskRequest>;

    fn build_dev_task_request(
        &self,
        dispatch: &PreparedDispatch,
        model: String,
        agent_command: Option<String>,
    ) -> Result<CreateTaskRequest>;

    fn create_task(&self, request: CreateTaskRequest) -> Result<i64>;
    fn get_task(&self, task_id: i64) -> Result<Option<StoredForgeTask>>;
    fn spawn_task(&self, task_id: i64) -> Result<()>;
}
