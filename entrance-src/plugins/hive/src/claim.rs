use anyhow::{Context, Result};
use entrance_core::{HiveCommentCreate, HiveLoopEvidenceCreate, Store};
use serde::{Deserialize, Serialize};

use crate::IssueCard;

const ISSUE_CLAIM_SCHEMA_VERSION: &str = "entrance.hive.issue_claim.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueClaimRequest {
    pub issue_id: i64,
    pub agent: String,
    pub role: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueClaimReport {
    pub schema_version: String,
    pub issue: IssueCard,
    pub assignee: String,
    pub claim_role: String,
    pub claim_source: String,
    pub comment_id: i64,
    pub evidence_id: Option<i64>,
}

pub fn claim_issue(store: &Store, request: IssueClaimRequest) -> Result<IssueClaimReport> {
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let assignee = request.agent.trim().to_string();
    if assignee.is_empty() {
        anyhow::bail!("issue claim requires a non-empty agent");
    }
    let claim_role = request.role.unwrap_or_else(|| "developer".to_string());
    if !matches!(claim_role.as_str(), "developer" | "reviewer") {
        anyhow::bail!("issue claim role must be developer or reviewer");
    }
    let claim_source = request.source.unwrap_or_else(|| "local".to_string());

    store.claim_hive_issue(issue.id, &assignee, &claim_role, &claim_source)?;
    let comment_id = store.insert_hive_comment(HiveCommentCreate {
        issue_id: issue.id,
        author: assignee.clone(),
        body: format!("Claimed issue as {claim_role}."),
        payload: serde_json::json!({
            "schema_version": ISSUE_CLAIM_SCHEMA_VERSION,
            "source": claim_source.clone(),
            "issue_id": issue.id,
            "loop_id": issue.loop_id,
            "assignee": assignee.clone(),
            "claim_role": claim_role.clone()
        }),
    })?;
    let evidence_id = match issue.loop_id {
        Some(loop_id) => {
            let contract = store
                .get_hive_loop_contract(loop_id)?
                .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
            Some(store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id,
                stage_id: None,
                round: contract.current_round,
                kind: "issue_claim".to_string(),
                summary: format!("{assignee} claimed issue #{} as {claim_role}.", issue.id),
                path: None,
                payload: serde_json::json!({
                    "schema_version": ISSUE_CLAIM_SCHEMA_VERSION,
                    "source": claim_source.clone(),
                    "issue": {
                        "id": issue.id,
                        "status": issue.status,
                        "comment_id": comment_id
                    },
                    "claim": {
                        "assignee": assignee.clone(),
                        "role": claim_role.clone()
                    }
                }),
            })?)
        }
        None => None,
    };
    let card = crate::loop_control::issue(store, issue.id)?;
    Ok(IssueClaimReport {
        schema_version: ISSUE_CLAIM_SCHEMA_VERSION.to_string(),
        issue: card,
        assignee,
        claim_role,
        claim_source,
        comment_id,
        evidence_id,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use entrance_core::Store;

    use super::*;
    use crate::HiveLoopCreateRequest;

    fn temp_store(name: &str) -> (std::path::PathBuf, Store) {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-claim-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");
        (root, store)
    }

    #[test]
    fn issue_claim_persists_assignee_fields_and_comment_evidence() {
        let (root, store) = temp_store("persist");
        let issue = crate::kernel::create(
            &store,
            HiveLoopCreateRequest {
                title: "Claim persistence".to_string(),
                goal: "Persist assignee metadata on local issues".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: "local-hive-panel".to_string(),
                autonomy_level: "developer-reviewer".to_string(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created")
        .issues
        .into_iter()
        .next()
        .expect("loop should create an issue");

        let report = claim_issue(
            &store,
            IssueClaimRequest {
                issue_id: issue.issue.id,
                agent: "developer-agent".to_string(),
                role: Some("developer".to_string()),
                source: Some("test".to_string()),
            },
        )
        .expect("issue should be claimed");

        let persisted = store
            .get_hive_issue(issue.issue.id)
            .expect("issue should load")
            .expect("issue should exist");
        assert_eq!(persisted.assignee.as_deref(), Some("developer-agent"));
        assert_eq!(persisted.claim_role.as_deref(), Some("developer"));
        assert_eq!(persisted.claim_source.as_deref(), Some("test"));
        assert!(persisted.claimed_at.is_some());
        assert_eq!(
            report.issue.issue.assignee.as_deref(),
            Some("developer-agent")
        );
        assert!(report.evidence_id.is_some());
        assert!(store
            .list_hive_comments(issue.issue.id)
            .expect("comments should load")
            .iter()
            .any(|comment| comment.id == report.comment_id
                && comment
                    .payload
                    .pointer("/schema_version")
                    .and_then(|value| value.as_str())
                    == Some(ISSUE_CLAIM_SCHEMA_VERSION)));
        assert!(store
            .list_hive_loop_evidence(issue.issue.loop_id.expect("issue should be loop-linked"))
            .expect("evidence should load")
            .iter()
            .any(|evidence| evidence.kind == "issue_claim"
                && evidence
                    .payload
                    .pointer("/claim/assignee")
                    .and_then(|value| value.as_str())
                    == Some("developer-agent")));

        let _ = fs::remove_dir_all(root);
    }
}
