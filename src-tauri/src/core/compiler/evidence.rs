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

#[cfg(test)]
mod tests {
    use super::{EvidenceKind, EvidenceVerdict};
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
}
