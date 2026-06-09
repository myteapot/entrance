pub(crate) const REVIEWER_SEMANTIC_THRESHOLD: f64 = 0.70;

#[derive(Debug, Clone)]
pub(crate) struct ReviewerSemanticAssessment {
    pub goal_alignment: f64,
    pub acceptance_evidence: f64,
    pub implementation_specificity: f64,
    pub regression_risk: f64,
    pub failures: Vec<String>,
}

impl ReviewerSemanticAssessment {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

pub(crate) fn assess_reviewer_semantics(
    target_bound: bool,
    goal: &str,
    evidence_presence: f64,
    has_execution_packet: bool,
    runtime_ready: bool,
) -> ReviewerSemanticAssessment {
    let goal_alignment = if target_bound && goal.trim().len() > 3 {
        1.0
    } else {
        0.0
    };
    let acceptance_evidence = evidence_presence;
    let implementation_specificity = if has_execution_packet { 1.0 } else { 0.0 };
    let regression_risk = if runtime_ready { 1.0 } else { 0.0 };
    let failures = [
        ("goal_alignment", goal_alignment),
        ("acceptance_evidence", acceptance_evidence),
        ("implementation_specificity", implementation_specificity),
        ("regression_risk", regression_risk),
    ]
    .into_iter()
    .filter(|(_, score)| *score < REVIEWER_SEMANTIC_THRESHOLD)
    .map(|(name, score)| format!("{name}={score:.2}<{}", REVIEWER_SEMANTIC_THRESHOLD))
    .collect();

    ReviewerSemanticAssessment {
        goal_alignment,
        acceptance_evidence,
        implementation_specificity,
        regression_risk,
        failures,
    }
}
