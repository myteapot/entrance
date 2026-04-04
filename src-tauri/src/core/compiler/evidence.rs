use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    TestResult,
    ReviewVerdict,
    IntegrationProbe,
    QualityMetric,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestResult => "test_result",
            Self::ReviewVerdict => "review_verdict",
            Self::IntegrationProbe => "integration_probe",
            Self::QualityMetric => "quality_metric",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerdict {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

impl EvidenceVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateEvidenceRef {
    pub evidence_id: i64,
    pub evidence_kind: String,
}

impl From<&StoredGateEvidence> for GateEvidenceRef {
    fn from(value: &StoredGateEvidence) -> Self {
        Self {
            evidence_id: value.id,
            evidence_kind: value.evidence_kind.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredGateEvidence {
    pub id: i64,
    pub allocation_id: i64,
    pub evidence_kind: String,
    pub verdict: String,
    pub summary: String,
    pub payload_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredAttemptReceipt {
    pub id: i64,
    pub evidence_id: i64,
    pub attempt_number: u8,
    pub passed: bool,
    pub reason: String,
    pub created_at: String,
}

/// Evidence data to insert into the gate evidence table.
#[derive(Debug, Clone)]
pub struct NewGateEvidence {
    pub allocation_id: i64,
    pub evidence_kind: EvidenceKind,
    pub summary: String,
    pub payload_json: String,
}

/// Derive the gate verdict from a Forge task's terminal state.
///
/// Only `Done` with `exit_code == Some(0)` yields `Accepted`.
/// All other terminal states (`Failed`, `Blocked`, `Cancelled`, non-zero exit)
/// yield `Rejected`.
pub fn derive_verdict(task_status: &str, exit_code: Option<i64>) -> EvidenceVerdict {
    match (task_status, exit_code) {
        ("Done", Some(0)) => EvidenceVerdict::Accepted,
        ("Done", None) => EvidenceVerdict::Accepted, // Done without exit_code defaults to success
        _ => EvidenceVerdict::Rejected,
    }
}

/// Build gate evidence from a completed Forge task's state.
///
/// This is a pure function — it reads inputs and produces a `NewGateEvidence`.
/// The caller is responsible for persisting via `DataStore::insert_gate_evidence`.
pub fn collect_task_evidence(
    allocation_id: i64,
    task_name: &str,
    task_status: &str,
    exit_code: Option<i64>,
    status_message: Option<&str>,
) -> NewGateEvidence {
    let summary = build_evidence_summary(task_name, task_status, exit_code, status_message);
    let payload = serde_json::json!({
        "task_status": task_status,
        "exit_code": exit_code,
        "status_message": status_message,
    });

    NewGateEvidence {
        allocation_id,
        evidence_kind: EvidenceKind::IntegrationProbe,
        summary,
        payload_json: payload.to_string(),
    }
}

fn build_evidence_summary(
    task_name: &str,
    task_status: &str,
    exit_code: Option<i64>,
    status_message: Option<&str>,
) -> String {
    let exit_part = match exit_code {
        Some(code) => format!(", exit_code={code}"),
        None => String::new(),
    };
    let message_part = match status_message {
        Some(msg) if !msg.is_empty() => format!(": {msg}"),
        _ => String::new(),
    };
    format!("Forge task \"{task_name}\" {task_status}{exit_part}{message_part}")
}

#[cfg(test)]
mod tests {
    use super::{collect_task_evidence, derive_verdict, EvidenceKind, EvidenceVerdict};
    use anyhow::Result;

    #[test]
    fn evidence_kinds_serialize_correctly() -> Result<()> {
        let kinds = serde_json::to_string(&[
            EvidenceKind::TestResult,
            EvidenceKind::ReviewVerdict,
            EvidenceKind::IntegrationProbe,
            EvidenceKind::QualityMetric,
        ])?;
        assert_eq!(
            kinds,
            r#"["test_result","review_verdict","integration_probe","quality_metric"]"#
        );

        let verdicts = serde_json::to_string(&[
            EvidenceVerdict::Pending,
            EvidenceVerdict::Accepted,
            EvidenceVerdict::Rejected,
            EvidenceVerdict::Expired,
        ])?;
        assert_eq!(verdicts, r#"["pending","accepted","rejected","expired"]"#);

        Ok(())
    }

    #[test]
    fn derive_verdict_done_with_zero_exit_is_accepted() {
        assert_eq!(derive_verdict("Done", Some(0)), EvidenceVerdict::Accepted);
    }

    #[test]
    fn derive_verdict_done_without_exit_code_is_accepted() {
        assert_eq!(derive_verdict("Done", None), EvidenceVerdict::Accepted);
    }

    #[test]
    fn derive_verdict_done_with_nonzero_exit_is_rejected() {
        assert_eq!(derive_verdict("Done", Some(1)), EvidenceVerdict::Rejected);
        assert_eq!(derive_verdict("Done", Some(7)), EvidenceVerdict::Rejected);
        assert_eq!(derive_verdict("Done", Some(-1)), EvidenceVerdict::Rejected);
    }

    #[test]
    fn derive_verdict_failed_is_rejected() {
        assert_eq!(derive_verdict("Failed", Some(7)), EvidenceVerdict::Rejected);
        assert_eq!(derive_verdict("Failed", None), EvidenceVerdict::Rejected);
    }

    #[test]
    fn derive_verdict_blocked_is_rejected() {
        assert_eq!(derive_verdict("Blocked", None), EvidenceVerdict::Rejected);
    }

    #[test]
    fn derive_verdict_cancelled_is_rejected() {
        assert_eq!(
            derive_verdict("Cancelled", None),
            EvidenceVerdict::Rejected
        );
    }

    #[test]
    fn collect_evidence_builds_integration_probe() {
        let evidence = collect_task_evidence(42, "My Task", "Done", Some(0), None);

        assert_eq!(evidence.allocation_id, 42);
        assert_eq!(evidence.evidence_kind, EvidenceKind::IntegrationProbe);
        assert!(evidence.summary.contains("My Task"));
        assert!(evidence.summary.contains("Done"));
    }

    #[test]
    fn collect_evidence_includes_exit_code_in_summary() {
        let evidence = collect_task_evidence(1, "Build", "Failed", Some(7), Some("exit 7"));

        assert!(evidence.summary.contains("exit_code=7"));
        assert!(evidence.summary.contains("exit 7"));
    }

    #[test]
    fn collect_evidence_payload_contains_task_state() -> Result<()> {
        let evidence = collect_task_evidence(1, "Test", "Done", Some(0), None);
        let payload: serde_json::Value = serde_json::from_str(&evidence.payload_json)?;

        assert_eq!(payload["task_status"], "Done");
        assert_eq!(payload["exit_code"], 0);
        assert!(payload["status_message"].is_null());

        Ok(())
    }
}
