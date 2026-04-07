use tauri::State;

use crate::core::data_store::{DataStore, NewIssue, StoredIssue, StoredIssueComment};

#[tauri::command]
pub fn issue_list(
    status: Option<String>,
    data_store: State<'_, DataStore>,
) -> Result<Vec<StoredIssue>, String> {
    data_store
        .list_issues(status.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_get(
    issue_key: String,
    data_store: State<'_, DataStore>,
) -> Result<Option<StoredIssue>, String> {
    data_store
        .get_issue_by_key(&issue_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_create(
    title: String,
    description: Option<String>,
    priority: Option<String>,
    labels: Option<String>,
    assignee: Option<String>,
    data_store: State<'_, DataStore>,
) -> Result<StoredIssue, String> {
    data_store
        .create_issue(NewIssue {
            title: &title,
            description: description.as_deref().unwrap_or(""),
            status: "todo",
            priority: priority.as_deref().unwrap_or("none"),
            labels: labels.as_deref().unwrap_or("[]"),
            assignee: assignee.as_deref().unwrap_or(""),
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_update_status(
    issue_key: String,
    status: String,
    data_store: State<'_, DataStore>,
) -> Result<StoredIssue, String> {
    data_store
        .update_issue_status(&issue_key, &status)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_update(
    issue_key: String,
    title: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    labels: Option<String>,
    assignee: Option<String>,
    data_store: State<'_, DataStore>,
) -> Result<StoredIssue, String> {
    data_store
        .update_issue(
            &issue_key,
            title.as_deref(),
            description.as_deref(),
            priority.as_deref(),
            labels.as_deref(),
            assignee.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_delete(issue_key: String, data_store: State<'_, DataStore>) -> Result<(), String> {
    data_store
        .delete_issue(&issue_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_add_comment(
    issue_key: String,
    author: String,
    body: String,
    data_store: State<'_, DataStore>,
) -> Result<StoredIssueComment, String> {
    data_store
        .add_issue_comment(&issue_key, &author, &body)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn issue_list_comments(
    issue_key: String,
    data_store: State<'_, DataStore>,
) -> Result<Vec<StoredIssueComment>, String> {
    data_store
        .list_issue_comments(&issue_key)
        .map_err(|e| e.to_string())
}
