use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum GraphUpdateEvent {
    NodeCreated {
        id: String,
        node_kind: GraphNodeKind,
        label: String,
        parent_id: Option<String>,
        detail: String,
        tone: String,
    },
    NodeStateChanged {
        id: String,
        tone: String,
        detail: String,
    },
    NodeArchived {
        id: String,
    },
    EdgeCreated {
        source_id: String,
        target_id: String,
        edge_kind: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum GraphNodeKind {
    Nota,
    Allocation,
    Receipt,
    Checkpoint,
    Supervision,
    Dialog,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaDialogEvent {
    pub dialog_id: String,
    pub kind: NotaDialogKind,
    pub title: String,
    pub body: String,
    pub context_json: String,
    pub allocation_id: Option<i64>,
    pub transaction_id: Option<i64>,
    pub actions: Vec<DialogAction>,
}

#[derive(Debug, Clone, Serialize)]
pub enum NotaDialogKind {
    ApprovalRequired,
    Escalation,
    BudgetWarning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct DialogAction {
    pub action_key: String,
    pub label: String,
    pub tone: String,
}
