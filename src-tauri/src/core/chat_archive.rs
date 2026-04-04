use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::core::data_store::{
    ChatArchiveSettingRecord, DataStore, NewChatCaptureRecord, StoredChatArchiveSetting,
    StoredChatCaptureRecord,
};

const DEFAULT_SCOPE_TYPE: &str = "runtime";
const DEFAULT_SCOPE_REF: &str = "Entrance";

#[derive(Debug, Clone)]
pub struct ChatArchivePolicyRequest {
    pub scope_type: Option<String>,
    pub scope_ref: Option<String>,
    pub archive_policy: String,
}

#[derive(Debug, Clone)]
pub struct ChatCaptureRequest {
    pub session_ref: Option<String>,
    pub role: String,
    pub content: String,
    pub summary: Option<String>,
    pub scope_type: Option<String>,
    pub scope_ref: Option<String>,
    pub linked_decision_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatArchivePolicyReport {
    pub setting: StoredChatArchiveSetting,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCaptureReport {
    pub policy: String,
    pub stored: bool,
    pub record: Option<StoredChatCaptureRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCaptureListReport {
    pub capture_count: usize,
    pub captures: Vec<StoredChatCaptureRecord>,
}

pub fn set_chat_archive_policy(
    data_store: &DataStore,
    request: ChatArchivePolicyRequest,
) -> Result<ChatArchivePolicyReport> {
    let policy = normalize_policy(&request.archive_policy)?;
    let scope_type = normalize_scope(request.scope_type.as_deref(), DEFAULT_SCOPE_TYPE);
    let scope_ref = normalize_scope(request.scope_ref.as_deref(), DEFAULT_SCOPE_REF);

    let setting = data_store.upsert_chat_archive_setting(ChatArchiveSettingRecord {
        scope_type: &scope_type,
        scope_ref: &scope_ref,
        archive_policy: policy,
    })?;
    Ok(ChatArchivePolicyReport { setting })
}

pub fn get_chat_archive_policy(
    data_store: &DataStore,
    scope_type: Option<&str>,
    scope_ref: Option<&str>,
) -> Result<ChatArchivePolicyReport> {
    let scope_type = normalize_scope(scope_type, DEFAULT_SCOPE_TYPE);
    let scope_ref = normalize_scope(scope_ref, DEFAULT_SCOPE_REF);
    let setting = data_store
        .get_chat_archive_setting(&scope_type, &scope_ref)?
        .unwrap_or(StoredChatArchiveSetting {
            id: 0,
            scope_type,
            scope_ref,
            archive_policy: "off".to_string(),
            updated_at: String::new(),
        });

    Ok(ChatArchivePolicyReport { setting })
}

pub fn capture_chat_message(
    data_store: &DataStore,
    request: ChatCaptureRequest,
) -> Result<ChatCaptureReport> {
    let scope_type = normalize_scope(request.scope_type.as_deref(), DEFAULT_SCOPE_TYPE);
    let scope_ref = normalize_scope(request.scope_ref.as_deref(), DEFAULT_SCOPE_REF);
    let setting = get_chat_archive_policy(data_store, Some(&scope_type), Some(&scope_ref))?.setting;
    let policy = normalize_policy(&setting.archive_policy)?.to_string();

    if policy == "off" {
        return Ok(ChatCaptureReport {
            policy,
            stored: false,
            record: None,
        });
    }

    let role = request.role.trim().to_string();
    if role.is_empty() {
        return Err(anyhow!("`role` must not be empty"));
    }

    let content = request.content.trim().to_string();
    if content.is_empty() {
        return Err(anyhow!("`content` must not be empty"));
    }

    let summary = request
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| summarize_chat(&content));
    let (capture_mode, stored_content, stored_summary) = if policy == "summary" {
        ("summary_capture", String::new(), summary)
    } else {
        ("raw_chat_capture", content.clone(), summary)
    };

    let record = data_store.insert_chat_capture_record(NewChatCaptureRecord {
        session_ref: request.session_ref.as_deref().unwrap_or(""),
        role: &role,
        capture_mode,
        archive_policy: &policy,
        content: &stored_content,
        summary: &stored_summary,
        scope_type: &scope_type,
        scope_ref: &scope_ref,
        linked_decision_id: request.linked_decision_id,
        status: "captured",
    })?;

    Ok(ChatCaptureReport {
        policy,
        stored: true,
        record: Some(record),
    })
}

pub fn list_chat_captures(data_store: &DataStore) -> Result<ChatCaptureListReport> {
    let captures = data_store.list_chat_capture_records()?;
    Ok(ChatCaptureListReport {
        capture_count: captures.len(),
        captures,
    })
}

fn normalize_policy(value: &str) -> Result<&str> {
    match value.trim() {
        "off" => Ok("off"),
        "summary" => Ok("summary"),
        "full" => Ok("full"),
        other => Err(anyhow!(
            "unsupported chat archive policy `{other}`, expected `off`, `summary`, or `full`"
        )),
    }
}

fn normalize_scope(value: Option<&str>, fallback: &str) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn summarize_chat(content: &str) -> String {
    const MAX_CHARS: usize = 160;
    let trimmed = content.trim();
    if trimmed.len() <= MAX_CHARS {
        return trimmed.to_string();
    }

    let mut summary = trimmed.chars().take(MAX_CHARS).collect::<String>();
    summary.push_str("...");
    summary
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::core::data_store::MigrationPlan;

    use super::{
        capture_chat_message, list_chat_captures, set_chat_archive_policy,
        ChatArchivePolicyRequest, ChatCaptureRequest,
    };

    #[test]
    fn chat_archive_policy_and_capture_modes_are_separate_from_decisions() -> Result<()> {
        let store = crate::core::data_store::DataStore::in_memory(MigrationPlan::new(&[]))?;

        set_chat_archive_policy(
            &store,
            ChatArchivePolicyRequest {
                scope_type: None,
                scope_ref: None,
                archive_policy: "summary".to_string(),
            },
        )?;
        let summary_capture = capture_chat_message(
            &store,
            ChatCaptureRequest {
                session_ref: Some("session-1".to_string()),
                role: "human".to_string(),
                content: "Need NOTA to remember that raw chat is not equal to decision."
                    .to_string(),
                summary: None,
                scope_type: None,
                scope_ref: None,
                linked_decision_id: None,
            },
        )?;
        assert!(summary_capture.stored);
        assert_eq!(
            summary_capture
                .record
                .as_ref()
                .map(|record| record.capture_mode.as_str()),
            Some("summary_capture")
        );
        assert_eq!(
            summary_capture
                .record
                .as_ref()
                .map(|record| record.content.as_str()),
            Some("")
        );

        set_chat_archive_policy(
            &store,
            ChatArchivePolicyRequest {
                scope_type: None,
                scope_ref: None,
                archive_policy: "full".to_string(),
            },
        )?;
        let full_capture = capture_chat_message(
            &store,
            ChatCaptureRequest {
                session_ref: Some("session-2".to_string()),
                role: "nota".to_string(),
                content: "Checkpoint created and decision persisted.".to_string(),
                summary: Some("Checkpoint and decision persisted.".to_string()),
                scope_type: None,
                scope_ref: None,
                linked_decision_id: None,
            },
        )?;
        assert_eq!(
            full_capture
                .record
                .as_ref()
                .map(|record| record.capture_mode.as_str()),
            Some("raw_chat_capture")
        );
        assert_eq!(list_chat_captures(&store)?.capture_count, 2);

        Ok(())
    }
}
