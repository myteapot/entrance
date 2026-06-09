pub fn create(store: &Store, request: HiveLoopCreateRequest) -> Result<HiveLoopReport> {
    let loop_id = store.insert_hive_loop_contract(HiveLoopContractCreate {
        title: request.title.clone(),
        goal: request.goal.clone(),
        boundary: default_text(request.boundary, "No explicit boundary supplied."),
        approach_space: default_vec(request.approach_space, "Explore the smallest runnable MVP"),
        eval_space: default_vec(
            request.eval_space,
            "CLI loop run produces a keep/reject/block verdict",
        ),
        review_surface: default_text(request.review_surface, "local-hive-panel"),
        autonomy_level: default_text(request.autonomy_level, "run-approved-candidates"),
        runtime: default_text(request.runtime, "local"),
    })?;
    seed_default_policies(store, loop_id)?;

    let issue_id = store.insert_hive_issue(HiveIssueCreate {
        loop_id: Some(loop_id),
        title: format!("Loop #{loop_id}: {}", request.title),
        status: "Todo".to_string(),
        summary: Some("Loop contract created; waiting for Explorer.".to_string()),
    })?;

    store.insert_hive_comment(HiveCommentCreate {
        issue_id,
        author: "compiler".to_string(),
        body: format!(
            "Loop contract admitted into Hive with {} active policies.",
            DEFAULT_LOOP_POLICIES.len()
        ),
        payload: system_comment_payload(
            "compiler",
            serde_json::json!({
                "loop_id": loop_id,
                "goal": request.goal,
                "next_phase": "explorer",
                "policy_count": DEFAULT_LOOP_POLICIES.len()
            }),
        ),
    })?;

    report(store, loop_id)
}

pub fn run(store: &Store, request: HiveLoopRunRequest) -> Result<HiveLoopReport> {
    let mut contract = store
        .get_hive_loop_contract(request.loop_id)?
        .with_context(|| format!("unknown hive loop `{}`", request.loop_id))?;
    let runtime = request.runtime.unwrap_or_else(|| contract.runtime.clone());
    let worker_timeout_secs = worker_timeout_secs(request.worker_timeout_secs)?;
    let worker_attempts = worker_attempts(request.worker_attempts)?;
    let issues = store.list_hive_issues_for_loop(contract.id)?;
    let issue_id = issues.first().map(|issue| issue.id);

    if contract.status != "todo" {
        return report(store, contract.id);
    }

    if runtime != contract.runtime {
        store.update_hive_loop_contract_runtime(contract.id, &runtime)?;
        contract.runtime = runtime.clone();
    }

    let runtime_probe = probe_runtime(&runtime);
    let preflight_admission = emit_and_admit(
        store,
        &contract,
        "PREFLIGHT_PACKET",
        "kernel",
        "kernel",
        "explorer",
        runtime_preflight_payload(&contract, &runtime, &runtime_probe),
    )?;
    if preflight_admission.result != "admitted" {
        let kernel_stage = insert_stage(
            store,
            &contract,
            "kernel",
            "Kernel preflight rejected the loop before spawning agent workers.",
            serde_json::json!({
                "runtime": runtime,
                "runtime_probe": runtime_probe
            }),
            serde_json::json!({
                "admission": preflight_admission.result,
                "reason": preflight_admission.reason
            }),
        )?;
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "kernel",
            Some(kernel_stage),
            &preflight_admission,
        );
    }

    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(
            issue_id,
            "Doing",
            Some("Explorer, Developer, and Reviewer are running."),
        )?;
        add_system_comment(
            store,
            issue_id,
            "Loop run started.",
            serde_json::json!({ "loop_id": contract.id, "runtime": runtime }),
        )?;
    }

    store.update_hive_loop_contract_state(
        contract.id,
        "running",
        "explorer",
        contract.current_round,
    )?;
    let accepted_candidate = "Run a local MVP loop through Hive";
    let explorer_worker = run_role_worker(
        &runtime,
        "explorer",
        &contract,
        &runtime_probe,
        worker_timeout_secs,
        worker_attempts,
    );
    let explorer_stage = insert_stage(
        store,
        &contract,
        "explorer",
        "Explorer compiled the goal into a runnable candidate.",
        serde_json::json!({
            "goal": contract.goal,
            "boundary": contract.boundary,
            "approach_space": contract.approach_space
        }),
        serde_json::json!({
            "candidate": accepted_candidate,
            "role_worker": explorer_worker,
            "constraints": [
                "keep work in SQLite/Hive",
                "separate explorer, developer, reviewer stages",
                "record issue/status/comment evidence"
            ]
        }),
    )?;
    let explorer_admission = emit_and_admit(
        store,
        &contract,
        "EXPLORATION_PACKET",
        "explorer",
        "explorer",
        "developer",
        serde_json::json!({
            "candidate": accepted_candidate,
            "role_worker": explorer_worker,
            "constraints": [
                "keep work in SQLite/Hive",
                "separate explorer, developer, reviewer stages",
                "record issue/status/comment evidence"
            ]
        }),
    )?;
    if explorer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "explorer",
            Some(explorer_stage),
            &explorer_admission,
        );
    }
    let explorer_evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(explorer_stage),
        round: contract.current_round,
        kind: "exploration_packet".to_string(),
        summary: "Explorer produced a concrete local-loop candidate.".to_string(),
        path: None,
        payload: serde_json::json!({
            "candidate": accepted_candidate,
            "candidate_id": "local-loop-mvp",
            "approach_count": contract.approach_space.len(),
            "worker": explorer_worker,
            "admission": explorer_admission.result
        }),
    })?;
    if let Some(issue_id) = issue_id {
        add_stage_system_comment(
            store,
            issue_id,
            contract.id,
            contract.current_round,
            "explorer",
            "exploration_packet",
            explorer_evidence_id,
            "Explorer admitted a candidate for this round.",
            &explorer_admission.result,
            &explorer_worker,
        )?;
    }

    store.update_hive_loop_contract_state(
        contract.id,
        "running",
        "developer",
        contract.current_round,
    )?;
    let runtime_worker = run_role_worker(
        &runtime,
        "developer",
        &contract,
        &runtime_probe,
        worker_timeout_secs,
        worker_attempts,
    );
    let developer_stage = insert_stage(
        store,
        &contract,
        "developer",
        "Developer executed the accepted MVP action and captured runtime evidence.",
        serde_json::json!({
            "accepted_candidate": accepted_candidate,
            "candidate_id": "local-loop-mvp",
            "runtime": runtime
        }),
        serde_json::json!({
            "accepted_candidate": accepted_candidate,
            "runtime_probe": runtime_probe,
            "runtime_worker": runtime_worker,
            "role_worker": runtime_worker,
            "artifact": "hive-loop-ledger"
        }),
    )?;
    let developer_admission = emit_and_admit(
        store,
        &contract,
        "EXECUTION_PACKET",
        "developer",
        "developer",
        "reviewer",
        serde_json::json!({
            "accepted_candidate": accepted_candidate,
            "runtime": runtime,
            "runtime_probe": runtime_probe,
            "runtime_worker": runtime_worker,
            "role_worker": runtime_worker,
            "artifact": "hive-loop-ledger"
        }),
    )?;
    if developer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "developer",
            Some(developer_stage),
            &developer_admission,
        );
    }
    let developer_evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(developer_stage),
        round: contract.current_round,
        kind: "execution_packet".to_string(),
        summary: format!("Developer ran `{runtime}` runtime worker."),
        path: None,
        payload: serde_json::json!({
            "accepted_candidate": accepted_candidate,
            "candidate_id": "local-loop-mvp",
            "runtime": runtime,
            "probe": runtime_probe,
            "worker": runtime_worker,
            "admission": developer_admission.result
        }),
    })?;
    if let Some(issue_id) = issue_id {
        add_stage_system_comment(
            store,
            issue_id,
            contract.id,
            contract.current_round,
            "developer",
            "execution_packet",
            developer_evidence_id,
            "Developer admitted the execution packet.",
            &developer_admission.result,
            &runtime_worker,
        )?;
    }

    store.update_hive_loop_contract_state(
        contract.id,
        "evaluating",
        "reviewer",
        contract.current_round,
    )?;
    let evidence = store.list_hive_loop_evidence(contract.id)?;
    let reviewer_worker = run_role_worker(
        &runtime,
        "reviewer",
        &contract,
        &runtime_probe,
        worker_timeout_secs,
        worker_attempts,
    );
    let runtime_ready = runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        && worker_ok(&explorer_worker)
        && worker_ok(&runtime_worker)
        && worker_ok(&reviewer_worker);
    let runtime_failure = runtime_failure(&runtime_probe, &runtime_worker);
    let decision_override = parse_decision_override(request.decision.as_deref())?;
    let prior_verdicts = store.list_hive_loop_verdicts(contract.id)?;
    let prior_reviewer_invalid_rounds = reviewer_invalid_streak_from_verdicts(
        &prior_verdicts,
        contract.current_round.saturating_sub(1),
    );
    let stages = store.list_hive_loop_stages(contract.id)?;
    let packets = store.list_hive_loop_packets(contract.id)?;
    let admissions = store.list_hive_loop_admissions(contract.id)?;
    let round_stage_evidence_count = evidence
        .iter()
        .filter(|row| row.round == contract.current_round && stage_bound_evidence_kind(&row.kind))
        .count();
    let reviewer_assessment = reviewer_gate_assessment(
        &contract,
        runtime_ready,
        &stages,
        &evidence,
        &packets,
        &admissions,
        &reviewer_worker,
    );
    let typed_verdict = build_verdict(
        decision_override,
        runtime_ready,
        runtime_failure,
        &runtime,
        round_stage_evidence_count,
        reviewer_assessment,
        prior_reviewer_invalid_rounds,
    );
    let reviewer_stage = insert_stage(
        store,
        &contract,
        "reviewer",
        &typed_verdict.summary,
        serde_json::json!({
            "evidence_count": round_stage_evidence_count,
            "eval_space": contract.eval_space
        }),
        serde_json::json!({
            "decision": typed_verdict.decision.as_str(),
            "role_worker": reviewer_worker,
            "gates": {
                "three_stages_recorded": typed_verdict.assessment.three_stages_recorded,
                "evidence_recorded": typed_verdict.assessment.evidence_recorded,
                "runtime_ready": typed_verdict.runtime_ready,
                "admissions_clean": typed_verdict.assessment.admissions_clean,
                "target_bound": typed_verdict.assessment.target_bound,
                "semantic_gates_passed": typed_verdict.assessment.semantic_gates_passed,
                "review_gates_passed": typed_verdict.assessment.review_gates_passed
            },
            "semantic_scores": {
                "goal_alignment": typed_verdict.assessment.goal_alignment,
                "acceptance_evidence": typed_verdict.assessment.acceptance_evidence,
                "implementation_specificity": typed_verdict.assessment.implementation_specificity,
                "regression_risk": typed_verdict.assessment.regression_risk,
                "threshold": crate::reviewer_semantics::REVIEWER_SEMANTIC_THRESHOLD
            }
        }),
    )?;
    let reviewer_admission = emit_and_admit(
        store,
        &contract,
        "VERDICT_PACKET",
        "reviewer",
        "reviewer",
        "complete",
        typed_verdict.packet_payload(&reviewer_worker),
    )?;
    if reviewer_admission.result != "admitted" {
        return block_on_admission_rejection(
            store,
            &contract,
            issue_id,
            "reviewer",
            Some(reviewer_stage),
            &reviewer_admission,
        );
    }
    let reviewer_evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id: Some(reviewer_stage),
        round: contract.current_round,
        kind: "verdict_packet".to_string(),
        summary: typed_verdict.summary.clone(),
        path: None,
        payload: serde_json::json!({
            "decision": typed_verdict.decision.as_str(),
            "reason_code": typed_verdict.reason_code,
            "runtime_ready": typed_verdict.runtime_ready,
            "worker": reviewer_worker,
            "admission": reviewer_admission.result
        }),
    })?;
    if let Some(issue_id) = issue_id {
        add_stage_system_comment(
            store,
            issue_id,
            contract.id,
            contract.current_round,
            "reviewer",
            "verdict_packet",
            reviewer_evidence_id,
            "Reviewer admitted the verdict packet.",
            &reviewer_admission.result,
            &reviewer_worker,
        )?;
    }
    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: typed_verdict.decision.as_str().to_string(),
        summary: typed_verdict.summary.clone(),
        score: typed_verdict.score_payload(),
        evidence: typed_verdict.evidence_payload(&runtime, &reviewer_worker),
    })?;

    let final_status = typed_verdict.decision.contract_status();
    let issue_status = typed_verdict.decision.issue_status();
    store.update_hive_loop_contract_state(
        contract.id,
        final_status,
        "complete",
        contract.current_round,
    )?;
    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(issue_id, issue_status, Some(&typed_verdict.summary))?;
        let admission_summary = format!(
            "{} Workers: explorer={}, developer={}, reviewer={}. Admissions: explorer={}, developer={}, reviewer={}.",
            typed_verdict.summary,
            runtime_worker_summary(&explorer_worker),
            runtime_worker_summary(&runtime_worker),
            runtime_worker_summary(&reviewer_worker),
            explorer_admission.result,
            developer_admission.result,
            reviewer_admission.result
        );
        add_system_comment(
            store,
            issue_id,
            &admission_summary,
            serde_json::json!({
                "loop_id": contract.id,
                "decision": typed_verdict.decision.as_str(),
                "reason_code": typed_verdict.reason_code,
                "phase": "reviewer",
                "runtime_worker": runtime_worker,
                "role_workers": {
                    "explorer": explorer_worker,
                    "developer": runtime_worker,
                    "reviewer": reviewer_worker
                },
                "admissions": {
                    "explorer": explorer_admission.result,
                    "developer": developer_admission.result,
                    "reviewer": reviewer_admission.result
                }
            }),
        )?;
    }

    contract = store
        .get_hive_loop_contract(contract.id)?
        .expect("loop contract should exist after run");
    let mut output = report(store, contract.id)?;
    output.contract = contract;
    Ok(output)
}

pub fn report(store: &Store, loop_id: i64) -> Result<HiveLoopReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issues = store
        .list_hive_issues_for_loop(loop_id)?
        .into_iter()
        .map(|issue| issue_card_from_issue(store, issue))
        .collect::<Result<Vec<_>>>()?;

    Ok(HiveLoopReport {
        policies: store.list_hive_loop_policies(loop_id)?,
        packets: store.list_hive_loop_packets(loop_id)?,
        admissions: store.list_hive_loop_admissions(loop_id)?,
        stages: store.list_hive_loop_stages(loop_id)?,
        evidence: store.list_hive_loop_evidence(loop_id)?,
        verdicts: store.list_hive_loop_verdicts(loop_id)?,
        contract,
        issues,
    })
}

pub fn list(store: &Store) -> Result<Vec<HiveLoopContract>> {
    store.list_hive_loop_contracts()
}

impl VerdictDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Reject => "reject",
            Self::NeedsReview => "needs-review",
            Self::Blocked => "blocked",
        }
    }

    fn contract_status(self) -> &'static str {
        match self {
            Self::Keep => "kept",
            Self::Reject => "rejected",
            Self::NeedsReview => "needs-review",
            Self::Blocked => "blocked",
        }
    }

    fn issue_status(self) -> &'static str {
        match self {
            Self::Keep => "Done",
            Self::Reject => "Canceled",
            Self::NeedsReview => "Needs Review",
            Self::Blocked => "Blocked",
        }
    }

    fn operator_review_required(self) -> bool {
        !matches!(self, Self::Keep)
    }

    fn gates_passed(self) -> bool {
        matches!(self, Self::Keep)
    }

    fn human_options(self) -> Vec<&'static str> {
        match self {
            Self::Keep => vec!["comment"],
            Self::Reject => vec!["comment", "retry"],
            Self::NeedsReview => vec!["comment", "retry", "cancel"],
            Self::Blocked => vec!["comment", "retry", "request-review", "cancel"],
        }
    }
}

fn reviewer_gate_assessment(
    contract: &HiveLoopContract,
    runtime_ready: bool,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
    packets: &[HiveLoopPacket],
    admissions: &[HiveLoopAdmission],
    reviewer_worker: &serde_json::Value,
) -> ReviewerGateAssessment {
    let mut observed_stage_roles = stages
        .iter()
        .filter(|stage| stage.round == contract.current_round)
        .filter(|stage| CURRENT_LOOP_ROLES.contains(&stage.role.as_str()))
        .map(|stage| stage.role.clone())
        .collect::<BTreeSet<_>>();
    if reviewer_worker.get("role").and_then(|value| value.as_str()) == Some("reviewer") {
        observed_stage_roles.insert("reviewer".to_string());
    }
    let observed_stage_roles = observed_stage_roles.into_iter().collect::<Vec<_>>();
    let missing_stage_roles = CURRENT_LOOP_ROLES
        .iter()
        .filter(|role| {
            !observed_stage_roles
                .iter()
                .any(|observed| observed.as_str() == **role)
        })
        .map(|role| (*role).to_string())
        .collect::<Vec<_>>();
    let stage_completeness = bounded_ratio(observed_stage_roles.len(), CURRENT_LOOP_ROLES.len());
    let prior_stage_evidence_count = evidence
        .iter()
        .filter(|row| row.round == contract.current_round)
        .filter(|row| matches!(row.kind.as_str(), "exploration_packet" | "execution_packet"))
        .count();
    let expected_prior_stage_evidence_count = 2;
    let evidence_presence = bounded_ratio(
        prior_stage_evidence_count,
        expected_prior_stage_evidence_count,
    );
    let packet_rounds = packets
        .iter()
        .map(|packet| (packet.id, packet.round))
        .collect::<HashMap<_, _>>();
    let current_admissions = admissions
        .iter()
        .filter(|admission| {
            packet_rounds
                .get(&admission.packet_id)
                .is_some_and(|round| *round == contract.current_round)
        })
        .collect::<Vec<_>>();
    let current_round_admission_count = current_admissions.len();
    let rejected_admission_count = current_admissions
        .iter()
        .filter(|admission| admission.result != "admitted")
        .count();
    let receipt_missing_count = current_admissions
        .iter()
        .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
        .sum();
    let clean_admission_count = current_admissions
        .iter()
        .filter(|admission| admission.result == "admitted")
        .filter(|admission| receipt_array_len(&admission.policy, "/receipt/missing") == 0)
        .count();
    let admission_integrity = bounded_ratio(clean_admission_count, current_round_admission_count);
    let gate_context = GateEvaluationContext {
        packets,
        admissions,
    };
    let target_binding = packets
        .iter()
        .filter(|packet| packet.loop_id == contract.id)
        .filter(|packet| packet.round == contract.current_round)
        .filter(|packet| packet.object_kind == "EXECUTION_PACKET")
        .filter(|packet| packet.writer_role == "developer")
        .filter(|packet| packet.route_to == "reviewer")
        .last()
        .map(|packet| candidate_binding_status(&packet.payload, gate_context))
        .unwrap_or_else(|| CandidateBindingStatus {
            passed: false,
            reason: "missing_developer_execution_packet".to_string(),
            expected_candidate: None,
            accepted_candidate: None,
            explorer_packet_id: None,
            explorer_candidate_count: 0,
        });
    let target_alignment = if target_binding.passed { 1.0 } else { 0.0 };
    let runtime_readiness = if runtime_ready { 1.0 } else { 0.0 };
    let three_stages_recorded = missing_stage_roles.is_empty();
    let evidence_recorded = prior_stage_evidence_count >= expected_prior_stage_evidence_count;
    let admissions_clean = current_round_admission_count > 0
        && rejected_admission_count == 0
        && receipt_missing_count == 0;
    let target_bound = target_binding.passed;
    let has_execution_packet = evidence
        .iter()
        .filter(|row| row.round == contract.current_round)
        .any(|row| row.kind == "execution_packet");
    let semantic_assessment = crate::reviewer_semantics::assess_reviewer_semantics(
        target_bound,
        &contract.goal,
        evidence_presence,
        has_execution_packet,
        runtime_ready,
    );
    let semantic_gates_passed = semantic_assessment.passed();
    let mut failure_reasons = Vec::new();
    if !three_stages_recorded {
        failure_reasons.push(format!(
            "missing_stage_roles={}",
            missing_stage_roles.join(",")
        ));
    }
    if !evidence_recorded {
        failure_reasons.push(format!(
            "prior_stage_evidence={prior_stage_evidence_count}/{expected_prior_stage_evidence_count}"
        ));
    }
    if !runtime_ready {
        failure_reasons.push("runtime_not_ready".to_string());
    }
    if !admissions_clean {
        failure_reasons.push(format!(
            "admissions_clean=false rejected={} missing_receipts={} observed={}",
            rejected_admission_count, receipt_missing_count, current_round_admission_count
        ));
    }
    if !target_bound {
        failure_reasons.push(format!("target_binding={}", target_binding.reason));
    }
    failure_reasons.extend(semantic_assessment.failures.clone());
    let review_gates_passed = three_stages_recorded
        && evidence_recorded
        && runtime_ready
        && admissions_clean
        && target_bound
        && semantic_gates_passed;

    ReviewerGateAssessment {
        stage_completeness,
        runtime_readiness,
        evidence_presence,
        admission_integrity,
        target_alignment,
        goal_alignment: semantic_assessment.goal_alignment,
        acceptance_evidence: semantic_assessment.acceptance_evidence,
        implementation_specificity: semantic_assessment.implementation_specificity,
        regression_risk: semantic_assessment.regression_risk,
        three_stages_recorded,
        evidence_recorded,
        runtime_ready,
        admissions_clean,
        target_bound,
        semantic_gates_passed,
        review_gates_passed,
        observed_stage_roles,
        missing_stage_roles,
        expected_candidate: target_binding.expected_candidate,
        accepted_candidate: target_binding.accepted_candidate,
        target_binding_reason: target_binding.reason,
        current_round_admission_count,
        rejected_admission_count,
        receipt_missing_count,
        prior_stage_evidence_count,
        expected_prior_stage_evidence_count,
        failure_reasons,
    }
}

fn bounded_ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    ((numerator as f64) / (denominator as f64)).clamp(0.0, 1.0)
}

impl TypedVerdict {
    fn score_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": VERDICT_SCHEMA_VERSION,
            "decision": self.decision.as_str(),
            "reason_code": self.reason_code,
            "gates_passed": self.decision.gates_passed(),
            "operator_review_needed": self.decision.operator_review_required(),
            "reviewer_invalid_rounds_used": self.reviewer_invalid_rounds_used,
            "reviewer_invalid_round_budget": REVIEWER_INVALID_ROUND_BUDGET,
            "reviewer_invalid_budget_exhausted": self.reviewer_invalid_budget_exhausted,
            "score_vector": {
                "stage_completeness": self.assessment.stage_completeness,
                "runtime_readiness": self.assessment.runtime_readiness,
                "evidence_presence": self.assessment.evidence_presence,
                "admission_integrity": self.assessment.admission_integrity,
                "target_alignment": self.assessment.target_alignment,
                "goal_alignment": self.assessment.goal_alignment,
                "acceptance_evidence": self.assessment.acceptance_evidence,
                "implementation_specificity": self.assessment.implementation_specificity,
                "regression_risk": self.assessment.regression_risk
            },
            "gate_results": {
                "three_stages_recorded": self.assessment.three_stages_recorded,
                "evidence_recorded": self.assessment.evidence_recorded,
                "runtime_ready": self.assessment.runtime_ready,
                "admissions_clean": self.assessment.admissions_clean,
                "target_bound": self.assessment.target_bound,
                "semantic_gates_passed": self.assessment.semantic_gates_passed,
                "semantic_threshold": crate::reviewer_semantics::REVIEWER_SEMANTIC_THRESHOLD,
                "review_gates_passed": self.assessment.review_gates_passed,
                "observed_stage_roles": self.assessment.observed_stage_roles.clone(),
                "missing_stage_roles": self.assessment.missing_stage_roles.clone(),
                "expected_candidate": self.assessment.expected_candidate.clone(),
                "accepted_candidate": self.assessment.accepted_candidate.clone(),
                "target_binding_reason": self.assessment.target_binding_reason.clone(),
                "current_round_admission_count": self.assessment.current_round_admission_count,
                "rejected_admission_count": self.assessment.rejected_admission_count,
                "receipt_missing_count": self.assessment.receipt_missing_count,
                "prior_stage_evidence_count": self.assessment.prior_stage_evidence_count,
                "expected_prior_stage_evidence_count": self.assessment.expected_prior_stage_evidence_count,
                "failure_reasons": self.assessment.failure_reasons.clone(),
                "reviewer_invalid_rounds_used": self.reviewer_invalid_rounds_used,
                "reviewer_invalid_round_budget": REVIEWER_INVALID_ROUND_BUDGET,
                "reviewer_invalid_budget_exhausted": self.reviewer_invalid_budget_exhausted,
                "decision_allowed": matches!(
                    self.decision,
                    VerdictDecision::Keep
                        | VerdictDecision::Reject
                        | VerdictDecision::NeedsReview
                        | VerdictDecision::Blocked
                )
            },
            "human_options": self.decision.human_options()
        })
    }

    fn evidence_payload(
        &self,
        runtime: &str,
        reviewer_worker: &serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "schema_version": VERDICT_SCHEMA_VERSION,
            "decision": self.decision.as_str(),
            "reason_code": self.reason_code,
            "evidence_count": self.evidence_count + 1,
            "runtime": runtime,
            "runtime_ready": self.assessment.runtime_ready,
            "review_gates_passed": self.assessment.review_gates_passed,
            "semantic_gates_passed": self.assessment.semantic_gates_passed,
            "semantic_threshold": crate::reviewer_semantics::REVIEWER_SEMANTIC_THRESHOLD,
            "semantic_scores": {
                "goal_alignment": self.assessment.goal_alignment,
                "acceptance_evidence": self.assessment.acceptance_evidence,
                "implementation_specificity": self.assessment.implementation_specificity,
                "regression_risk": self.assessment.regression_risk
            },
            "review_gate_failures": self.assessment.failure_reasons.clone(),
            "target_bound": self.assessment.target_bound,
            "target_binding_reason": self.assessment.target_binding_reason.clone(),
            "reviewer_invalid_rounds_used": self.reviewer_invalid_rounds_used,
            "reviewer_invalid_round_budget": REVIEWER_INVALID_ROUND_BUDGET,
            "reviewer_invalid_budget_exhausted": self.reviewer_invalid_budget_exhausted,
            "role_worker": reviewer_worker,
            "source": {
                "reviewer": "hive-loop-control",
                "round_evidence_before_verdict": self.evidence_count
            }
        })
    }

    fn packet_payload(&self, reviewer_worker: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "decision": self.decision.as_str(),
            "summary": self.summary,
            "reason_code": self.reason_code,
            "score": self.score_payload(),
            "role_worker": reviewer_worker
        })
    }
}

fn parse_decision_override(value: Option<&str>) -> Result<Option<VerdictDecision>> {
    value
        .map(|value| match value {
            "keep" => Ok(VerdictDecision::Keep),
            "reject" => Ok(VerdictDecision::Reject),
            "needs-review" => Ok(VerdictDecision::NeedsReview),
            "blocked" => Ok(VerdictDecision::Blocked),
            other => anyhow::bail!(
                "unsupported reviewer decision `{other}`; expected keep, reject, needs-review, or blocked"
            ),
        })
        .transpose()
}

fn build_verdict(
    decision_override: Option<VerdictDecision>,
    runtime_ready: bool,
    runtime_failure: Option<RuntimeFailure>,
    runtime: &str,
    evidence_count: usize,
    assessment: ReviewerGateAssessment,
    prior_reviewer_invalid_rounds: i64,
) -> TypedVerdict {
    if !runtime_ready {
        let reason_code = runtime_failure
            .unwrap_or(RuntimeFailure::Worker)
            .reason_code();
        return TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code,
            summary: format!(
                "Reviewer blocked the candidate: `{runtime}` {}.",
                runtime_failure
                    .unwrap_or(RuntimeFailure::Worker)
                    .summary_fragment()
            ),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: 0,
            reviewer_invalid_budget_exhausted: false,
        };
    }

    let current_invalid_rounds =
        (prior_reviewer_invalid_rounds + 1).min(REVIEWER_INVALID_ROUND_BUDGET);
    let requested_decision = decision_override.unwrap_or(VerdictDecision::Keep);
    let forced_reject =
        requested_decision == VerdictDecision::Keep && !assessment.review_gates_passed;
    let invalid_decision = requested_decision == VerdictDecision::Reject || forced_reject;
    if invalid_decision && current_invalid_rounds >= REVIEWER_INVALID_ROUND_BUDGET {
        return TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code: "review_budget_exhausted",
            summary: format!(
                "Reviewer blocked the issue: candidate was still invalid after {REVIEWER_INVALID_ROUND_BUDGET} review rounds."
            ),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: current_invalid_rounds,
            reviewer_invalid_budget_exhausted: true,
        };
    }

    match requested_decision {
        VerdictDecision::Keep if forced_reject => TypedVerdict {
            decision: VerdictDecision::Reject,
            reason_code: "review_gates_failed",
            summary: format!(
                "Reviewer rejected the candidate: required ledger gates failed ({}); invalid review round {current_invalid_rounds}/{REVIEWER_INVALID_ROUND_BUDGET}.",
                assessment.failure_reasons.join(", ")
            ),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: current_invalid_rounds,
            reviewer_invalid_budget_exhausted: false,
        },
        VerdictDecision::Keep => TypedVerdict {
            decision: VerdictDecision::Keep,
            reason_code: "all_gates_passed",
            summary: "Reviewer kept the candidate: all MVP gates passed.".to_string(),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: 0,
            reviewer_invalid_budget_exhausted: false,
        },
        VerdictDecision::Reject => TypedVerdict {
            decision: VerdictDecision::Reject,
            reason_code: "quality_gate_failed",
            summary: format!(
                "Reviewer rejected the candidate: quality gate failed; invalid review round {current_invalid_rounds}/{REVIEWER_INVALID_ROUND_BUDGET}."
            ),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: current_invalid_rounds,
            reviewer_invalid_budget_exhausted: false,
        },
        VerdictDecision::NeedsReview => TypedVerdict {
            decision: VerdictDecision::NeedsReview,
            reason_code: "human_review_required",
            summary: "Reviewer requested human review for this candidate.".to_string(),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: 0,
            reviewer_invalid_budget_exhausted: false,
        },
        VerdictDecision::Blocked => TypedVerdict {
            decision: VerdictDecision::Blocked,
            reason_code: "operator_blocked",
            summary: "Reviewer blocked the candidate by operator decision.".to_string(),
            runtime_ready,
            evidence_count,
            assessment,
            reviewer_invalid_rounds_used: 0,
            reviewer_invalid_budget_exhausted: false,
        },
    }
}

impl RuntimeFailure {
    fn reason_code(self) -> &'static str {
        match self {
            Self::Probe => "runtime_probe_failed",
            Self::Worker => "runtime_worker_failed",
        }
    }

    fn summary_fragment(self) -> &'static str {
        match self {
            Self::Probe => "runtime probe failed",
            Self::Worker => "worker execution failed",
        }
    }
}

impl GateCheck {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReceiptRequirementsSatisfied => "receipt_requirements_satisfied",
            Self::BodyFieldPresent(_) => "body_field_present",
            Self::DecisionPresent => "decision_present",
            Self::RuntimePolicyReady => "runtime_policy_ready",
            Self::AcceptedCandidateBound => "accepted_candidate_bound",
        }
    }
}

impl From<GateSpec> for PolicyGateSpec {
    fn from(spec: GateSpec) -> Self {
        Self {
            schema_version: POLICY_SCHEMA_VERSION.to_string(),
            name: spec.name.to_string(),
            description: spec.description.to_string(),
            expected_object_kind: spec.expected_object_kind.map(ToOwned::to_owned),
            required_receipts: spec
                .required_receipts
                .iter()
                .map(|receipt| (*receipt).to_string())
                .collect(),
            check: spec.check.as_str().to_string(),
        }
    }
}

fn runtime_failure(
    runtime_probe: &serde_json::Value,
    runtime_worker: &serde_json::Value,
) -> Option<RuntimeFailure> {
    if !runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        Some(RuntimeFailure::Probe)
    } else if !runtime_worker
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        Some(RuntimeFailure::Worker)
    } else {
        None
    }
}

fn runtime_worker_summary(runtime_worker: &serde_json::Value) -> String {
    let kind = runtime_worker
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let mode = runtime_worker
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let ok = runtime_worker
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let role = runtime_worker
        .get("role")
        .and_then(|value| value.as_str())
        .unwrap_or("worker");
    format!("{role}:{kind}/{mode} ok={ok}")
}

fn worker_ok(worker: &serde_json::Value) -> bool {
    worker
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn worker_timeout_secs(requested: Option<u64>) -> Result<u64> {
    let value = match requested {
        Some(value) => value,
        None => match std::env::var("ENTRANCE_HIVE_WORKER_TIMEOUT_SECS") {
            Ok(value) if !value.trim().is_empty() => value
                .trim()
                .parse::<u64>()
                .with_context(|| "ENTRANCE_HIVE_WORKER_TIMEOUT_SECS must be a positive integer")?,
            _ => DEFAULT_WORKER_TIMEOUT_SECS,
        },
    };
    if value == 0 {
        anyhow::bail!("worker timeout must be at least 1 second");
    }
    if value > MAX_WORKER_TIMEOUT_SECS {
        anyhow::bail!(
            "worker timeout must be <= {} seconds",
            MAX_WORKER_TIMEOUT_SECS
        );
    }
    Ok(value)
}

fn worker_attempts(requested: Option<u64>) -> Result<u64> {
    let value = match requested {
        Some(value) => value,
        None => match std::env::var("ENTRANCE_HIVE_WORKER_ATTEMPTS") {
            Ok(value) if !value.trim().is_empty() => value
                .trim()
                .parse::<u64>()
                .with_context(|| "ENTRANCE_HIVE_WORKER_ATTEMPTS must be a positive integer")?,
            _ => DEFAULT_WORKER_ATTEMPTS,
        },
    };
    if value == 0 {
        anyhow::bail!("worker attempts must be at least 1");
    }
    if value > MAX_WORKER_ATTEMPTS {
        anyhow::bail!("worker attempts must be <= {}", MAX_WORKER_ATTEMPTS);
    }
    Ok(value)
}

fn seed_default_policies(store: &Store, loop_id: i64) -> Result<()> {
    for policy in DEFAULT_LOOP_POLICIES {
        store.insert_hive_loop_policy(HiveLoopPolicyCreate {
            loop_id,
            object_kind: policy.object_kind.to_string(),
            writer_role: policy.writer_role.to_string(),
            route_from: policy.route_from.to_string(),
            route_to: policy.route_to.to_string(),
            gate: policy.gate.to_string(),
            status: "active".to_string(),
        })?;
    }
    Ok(())
}

fn emit_and_admit(
    store: &Store,
    contract: &HiveLoopContract,
    object_kind: &str,
    writer_role: &str,
    route_from: &str,
    route_to: &str,
    payload: serde_json::Value,
) -> Result<HiveLoopAdmission> {
    let packet_payload = typed_packet_payload(
        contract,
        object_kind,
        writer_role,
        route_from,
        route_to,
        payload,
    );
    let packet_id = store.insert_hive_loop_packet(HiveLoopPacketCreate {
        loop_id: contract.id,
        round: contract.current_round,
        object_kind: object_kind.to_string(),
        writer_role: writer_role.to_string(),
        route_from: route_from.to_string(),
        route_to: route_to.to_string(),
        state_code: "submitted".to_string(),
        payload: packet_payload.clone(),
    })?;
    let packet = store
        .get_hive_loop_packet(packet_id)?
        .expect("newly created packet should exist");
    let context_packets = store.list_hive_loop_packets(contract.id)?;
    let context_admissions = store.list_hive_loop_admissions(contract.id)?;
    let gate_context = GateEvaluationContext {
        packets: &context_packets,
        admissions: &context_admissions,
    };
    let policies = store.list_hive_loop_policies(contract.id)?;
    let matching_policy = policies.iter().find(|policy| {
        policy.status == "active"
            && policy.object_kind == packet.object_kind
            && policy.writer_role == packet.writer_role
            && policy.route_from == packet.route_from
            && policy.route_to == packet.route_to
    });

    let (result, reason, gate_name, gate_passed) = match matching_policy {
        Some(policy) => {
            let passed = gate_passes_with_context(&policy.gate, &packet_payload, gate_context);
            let result = if passed { "admitted" } else { "rejected" };
            let reason = if passed {
                format!("{} passed", policy.gate)
            } else {
                gate_failure_reason_with_context(&policy.gate, &packet_payload, gate_context)
            };
            (
                result.to_string(),
                reason,
                Some(policy.gate.as_str()),
                Some(passed),
            )
        }
        None => (
            "rejected".to_string(),
            "no active policy matched packet writer and route".to_string(),
            None,
            None,
        ),
    };
    let admission_receipt = typed_admission_receipt(
        &packet,
        &packet_payload,
        matching_policy,
        &result,
        &reason,
        gate_name,
        gate_passed,
        gate_context,
    );

    let admission_id = store.insert_hive_loop_admission(HiveLoopAdmissionCreate {
        loop_id: contract.id,
        packet_id,
        result: result.clone(),
        reason: reason.clone(),
        policy: admission_receipt,
    })?;
    let admission = store
        .list_hive_loop_admissions(contract.id)?
        .into_iter()
        .find(|admission| admission.id == admission_id)
        .expect("newly created admission should exist");

    Ok(admission)
}

fn typed_packet_payload(
    contract: &HiveLoopContract,
    object_kind: &str,
    writer_role: &str,
    route_from: &str,
    route_to: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": PACKET_SCHEMA_VERSION,
        "loop_id": contract.id,
        "round": contract.current_round,
        "object_kind": object_kind,
        "writer": {
            "role": writer_role
        },
        "route": {
            "from": route_from,
            "to": route_to
        },
        "state_code": "submitted",
        "body": body,
        "receipt_requirements": receipt_requirements_for_packet(object_kind)
    })
}

fn receipt_requirements_for_packet(object_kind: &str) -> Vec<&'static str> {
    match object_kind {
        "PREFLIGHT_PACKET" => vec![
            "runtime",
            "runtime_probe",
            "runtime_policy",
            "capability_preview",
        ],
        "EXPLORATION_PACKET" => vec!["candidate", "constraints", "role_worker"],
        "EXECUTION_PACKET" => vec![
            "accepted_candidate",
            "runtime_probe",
            "runtime_worker",
            "artifact",
            "role_worker",
        ],
        "VERDICT_PACKET" => vec!["decision", "summary", "score", "role_worker"],
        _ => Vec::new(),
    }
}

fn gate_spec(gate: &str) -> Option<GateSpec> {
    match gate {
        "runtime_policy_ready" => Some(GateSpec {
            name: "runtime_policy_ready",
            description: "Kernel preflight must prove the selected runtime and external control surface capability are ready before spawning agent workers.",
            expected_object_kind: Some("PREFLIGHT_PACKET"),
            required_receipts: &[
                "runtime",
                "runtime_probe",
                "runtime_policy",
                "capability_preview",
            ],
            check: GateCheck::RuntimePolicyReady,
        }),
        "candidate_receipts_present" => Some(GateSpec {
            name: "candidate_receipts_present",
            description: "Explorer packets must carry the candidate, constraints, and role worker receipt.",
            expected_object_kind: Some("EXPLORATION_PACKET"),
            required_receipts: &["candidate", "constraints", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        "runtime_receipts_present" => Some(GateSpec {
            name: "runtime_receipts_present",
            description: "Developer packets must carry runtime probe, runtime worker, artifact, and role worker receipts.",
            expected_object_kind: Some("EXECUTION_PACKET"),
            required_receipts: &["runtime_probe", "runtime_worker", "artifact", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        ACCEPTED_CANDIDATE_BOUND_GATE => Some(GateSpec {
            name: ACCEPTED_CANDIDATE_BOUND_GATE,
            description: "Developer packets must carry runtime receipts and bind their accepted_candidate to the admitted Explorer candidate for the same loop round.",
            expected_object_kind: Some("EXECUTION_PACKET"),
            required_receipts: &[
                "accepted_candidate",
                "runtime_probe",
                "runtime_worker",
                "artifact",
                "role_worker",
            ],
            check: GateCheck::AcceptedCandidateBound,
        }),
        "verdict_receipts_present" => Some(GateSpec {
            name: "verdict_receipts_present",
            description: "Reviewer packets must carry decision, summary, score, and role worker receipts.",
            expected_object_kind: Some("VERDICT_PACKET"),
            required_receipts: &["decision", "summary", "score", "role_worker"],
            check: GateCheck::ReceiptRequirementsSatisfied,
        }),
        "candidate_present" => Some(GateSpec {
            name: "candidate_present",
            description: "Packet body must include a non-empty candidate.",
            expected_object_kind: None,
            required_receipts: &["candidate"],
            check: GateCheck::BodyFieldPresent("candidate"),
        }),
        "runtime_probe_present" => Some(GateSpec {
            name: "runtime_probe_present",
            description: "Packet body must include runtime probe evidence.",
            expected_object_kind: None,
            required_receipts: &["runtime_probe"],
            check: GateCheck::BodyFieldPresent("runtime_probe"),
        }),
        "decision_present" => Some(GateSpec {
            name: "decision_present",
            description: "Packet body must include an allowed reviewer decision.",
            expected_object_kind: None,
            required_receipts: &["decision"],
            check: GateCheck::DecisionPresent,
        }),
        _ => None,
    }
}

fn all_gate_specs() -> Vec<GateSpec> {
    [
        "runtime_policy_ready",
        "candidate_receipts_present",
        "runtime_receipts_present",
        ACCEPTED_CANDIDATE_BOUND_GATE,
        "verdict_receipts_present",
        "candidate_present",
        "runtime_probe_present",
        "decision_present",
    ]
    .into_iter()
    .filter_map(gate_spec)
    .collect()
}

fn gate_spec_payload(gate: &str) -> serde_json::Value {
    gate_spec(gate)
        .map(|spec| {
            serde_json::json!({
                "schema_version": POLICY_SCHEMA_VERSION,
                "name": spec.name,
                "description": spec.description,
                "expected_object_kind": spec.expected_object_kind,
                "required_receipts": spec.required_receipts,
                "check": spec.check.as_str()
            })
        })
        .unwrap_or(serde_json::Value::Null)
}

fn typed_admission_receipt(
    packet: &HiveLoopPacket,
    packet_payload: &serde_json::Value,
    policy: Option<&HiveLoopPolicy>,
    result: &str,
    reason: &str,
    gate_name: Option<&str>,
    gate_passed: Option<bool>,
    gate_context: GateEvaluationContext<'_>,
) -> serde_json::Value {
    let (required_receipts, missing_receipts) = receipt_requirement_status(packet_payload);
    let receipt_satisfied = missing_receipts.is_empty();
    let packet_envelope_errors = typed_packet_envelope_errors(packet_payload);
    let target_binding = target_binding_receipt(packet, packet_payload, gate_context);
    serde_json::json!({
        "schema_version": ADMISSION_SCHEMA_VERSION,
        "result": result,
        "reason": reason,
        "packet": {
            "id": packet.id,
            "schema_version": packet_payload
                .get("schema_version")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown"),
            "object_kind": &packet.object_kind,
            "writer_role": &packet.writer_role,
            "route_from": &packet.route_from,
            "route_to": &packet.route_to,
            "state_code": &packet.state_code,
            "envelope": {
                "valid": packet_envelope_errors.is_empty(),
                "errors": packet_envelope_errors
            }
        },
        "policy": policy.map(|policy| serde_json::json!({
            "schema_version": POLICY_SCHEMA_VERSION,
            "id": policy.id,
            "object_kind": &policy.object_kind,
            "writer_role": &policy.writer_role,
            "route_from": &policy.route_from,
            "route_to": &policy.route_to,
            "gate": &policy.gate,
            "status": &policy.status,
            "gate_spec": gate_spec_payload(&policy.gate)
        })),
        "gate": {
            "name": gate_name,
            "passed": gate_passed,
            "spec": gate_name
                .map(gate_spec_payload)
                .unwrap_or(serde_json::Value::Null)
        },
        "receipt": {
            "required": required_receipts,
            "missing": missing_receipts,
            "satisfied": receipt_satisfied
        },
        "target_binding": target_binding
    })
}

#[cfg(test)]
fn gate_passes(gate: &str, payload: &serde_json::Value) -> bool {
    gate_passes_with_context(
        gate,
        payload,
        GateEvaluationContext {
            packets: &[],
            admissions: &[],
        },
    )
}

fn gate_passes_with_context(
    gate: &str,
    payload: &serde_json::Value,
    context: GateEvaluationContext<'_>,
) -> bool {
    if !typed_packet_envelope_valid(payload) {
        return false;
    }
    let Some(spec) = gate_spec(gate) else {
        return false;
    };
    if spec
        .expected_object_kind
        .is_some_and(|expected| packet_object_kind(payload) != Some(expected))
    {
        return false;
    }
    let body = packet_body(payload);
    match spec.check {
        GateCheck::ReceiptRequirementsSatisfied => receipt_requirements_satisfied(payload),
        GateCheck::BodyFieldPresent(field) => body.get(field).is_some_and(|value| match value {
            serde_json::Value::Null => false,
            serde_json::Value::String(text) => !text.trim().is_empty(),
            serde_json::Value::Array(values) => !values.is_empty(),
            serde_json::Value::Object(values) => !values.is_empty(),
            serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
        }),
        GateCheck::DecisionPresent => body
            .get("decision")
            .and_then(|value| value.as_str())
            .is_some_and(|value| matches!(value, "keep" | "reject" | "needs-review" | "blocked")),
        GateCheck::RuntimePolicyReady => {
            receipt_requirements_satisfied(payload)
                && body
                    .pointer("/runtime_policy/supported")
                    .and_then(|value| value.as_bool())
                    == Some(true)
                && body
                    .pointer("/runtime_probe/ok")
                    .and_then(|value| value.as_bool())
                    == Some(true)
                && body
                    .pointer("/capability_preview/worker_spawn_ready")
                    .and_then(|value| value.as_bool())
                    == Some(true)
        }
        GateCheck::AcceptedCandidateBound => {
            receipt_requirements_satisfied(payload)
                && candidate_binding_status(payload, context).passed
        }
    }
}

fn receipt_requirements_satisfied(payload: &serde_json::Value) -> bool {
    let (_required, missing) = receipt_requirement_status(payload);
    missing.is_empty()
}

#[cfg(test)]
fn gate_failure_reason(gate: &str, payload: &serde_json::Value) -> String {
    gate_failure_reason_with_context(
        gate,
        payload,
        GateEvaluationContext {
            packets: &[],
            admissions: &[],
        },
    )
}

fn gate_failure_reason_with_context(
    gate: &str,
    payload: &serde_json::Value,
    context: GateEvaluationContext<'_>,
) -> String {
    let envelope_errors = typed_packet_envelope_errors(payload);
    if !envelope_errors.is_empty() {
        return format!(
            "{gate} failed: typed packet envelope invalid: {}",
            envelope_errors.join(", ")
        );
    }
    let (_required, missing) = receipt_requirement_status(payload);
    if missing.is_empty() {
        if gate == "runtime_policy_ready" {
            let body = packet_body(payload);
            let runtime = body
                .get("runtime")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let supported = body
                .pointer("/runtime_policy/supported")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let probe_ok = body
                .pointer("/runtime_probe/ok")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let capability_ready = body
                .pointer("/capability_preview/worker_spawn_ready")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let capability_blockers =
                string_array_at(body, "/capability_preview/worker_spawn_blockers");
            format!(
                "{gate} failed: runtime={runtime} supported={supported} probe_ok={probe_ok} capability_ready={capability_ready} capability_blockers={}",
                capability_blockers.join(",")
            )
        } else if gate == ACCEPTED_CANDIDATE_BOUND_GATE {
            let binding = candidate_binding_status(payload, context);
            format!(
                "{gate} failed: {} expected_candidate={} accepted_candidate={}",
                binding.reason,
                binding.expected_candidate.as_deref().unwrap_or("none"),
                binding.accepted_candidate.as_deref().unwrap_or("none")
            )
        } else {
            format!("{gate} failed")
        }
    } else {
        format!(
            "{gate} failed: missing or invalid receipts {}",
            missing.join(", ")
        )
    }
}

fn candidate_binding_status(
    payload: &serde_json::Value,
    context: GateEvaluationContext<'_>,
) -> CandidateBindingStatus {
    if packet_object_kind(payload) != Some("EXECUTION_PACKET") {
        return CandidateBindingStatus {
            passed: false,
            reason: "not_execution_packet".to_string(),
            expected_candidate: None,
            accepted_candidate: None,
            explorer_packet_id: None,
            explorer_candidate_count: 0,
        };
    }
    let loop_id = payload.get("loop_id").and_then(|value| value.as_i64());
    let round = payload.get("round").and_then(|value| value.as_i64());
    let admitted_packet_ids = context
        .admissions
        .iter()
        .filter(|admission| admission.result == "admitted")
        .map(|admission| admission.packet_id)
        .collect::<BTreeSet<_>>();
    let explorer_candidates = context
        .packets
        .iter()
        .filter(|packet| Some(packet.loop_id) == loop_id)
        .filter(|packet| Some(packet.round) == round)
        .filter(|packet| packet.object_kind == "EXPLORATION_PACKET")
        .filter(|packet| packet.writer_role == "explorer")
        .filter(|packet| packet.route_to == "developer")
        .filter(|packet| admitted_packet_ids.contains(&packet.id))
        .filter_map(|packet| {
            non_empty_body_string(&packet.payload, "candidate")
                .map(|candidate| (packet.id, candidate))
        })
        .collect::<Vec<_>>();
    let accepted_candidate = non_empty_body_string(payload, "accepted_candidate");
    if explorer_candidates.len() != 1 {
        return CandidateBindingStatus {
            passed: false,
            reason: if explorer_candidates.is_empty() {
                "missing_admitted_explorer_candidate".to_string()
            } else {
                "ambiguous_admitted_explorer_candidates".to_string()
            },
            expected_candidate: explorer_candidates
                .first()
                .map(|(_id, candidate)| candidate.clone()),
            accepted_candidate,
            explorer_packet_id: explorer_candidates.first().map(|(id, _candidate)| *id),
            explorer_candidate_count: explorer_candidates.len(),
        };
    }
    let (explorer_packet_id, expected_candidate) = explorer_candidates
        .into_iter()
        .next()
        .expect("one explorer candidate should exist");
    let passed = accepted_candidate.as_deref() == Some(expected_candidate.as_str());
    CandidateBindingStatus {
        passed,
        reason: if passed {
            "accepted_candidate_matches_explorer_candidate".to_string()
        } else if accepted_candidate.is_some() {
            "accepted_candidate_mismatch".to_string()
        } else {
            "accepted_candidate_missing".to_string()
        },
        expected_candidate: Some(expected_candidate),
        accepted_candidate,
        explorer_packet_id: Some(explorer_packet_id),
        explorer_candidate_count: 1,
    }
}

fn non_empty_body_string(payload: &serde_json::Value, field: &str) -> Option<String> {
    packet_body(payload)
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn target_binding_receipt(
    packet: &HiveLoopPacket,
    packet_payload: &serde_json::Value,
    context: GateEvaluationContext<'_>,
) -> serde_json::Value {
    if packet.object_kind != "EXECUTION_PACKET" {
        return serde_json::Value::Null;
    }
    let binding = candidate_binding_status(packet_payload, context);
    serde_json::json!({
        "schema_version": TARGET_BINDING_SCHEMA_VERSION,
        "name": "accepted_candidate_binding",
        "passed": binding.passed,
        "reason": binding.reason,
        "developer_packet_id": packet.id,
        "explorer_packet_id": binding.explorer_packet_id,
        "explorer_candidate_count": binding.explorer_candidate_count,
        "expected_candidate": binding.expected_candidate,
        "accepted_candidate": binding.accepted_candidate
    })
}

fn receipt_requirement_status(payload: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let required = packet_receipt_requirements(payload);
    let body = packet_body(payload);
    let expected_worker_role = payload
        .pointer("/writer/role")
        .and_then(|value| value.as_str());
    let missing = required
        .iter()
        .filter(|requirement| !receipt_value_present(body, requirement, expected_worker_role))
        .cloned()
        .collect::<Vec<_>>();
    (required, missing)
}

fn packet_receipt_requirements(payload: &serde_json::Value) -> Vec<String> {
    let declared = payload
        .get("receipt_requirements")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !declared.is_empty() {
        return declared;
    }
    packet_object_kind(payload)
        .map(receipt_requirements_for_packet)
        .unwrap_or_default()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

fn receipt_value_present(
    body: &serde_json::Value,
    requirement: &str,
    expected_worker_role: Option<&str>,
) -> bool {
    if matches!(requirement, "role_worker" | "runtime_worker") {
        return body
            .get(requirement)
            .is_some_and(|worker| worker_receipt_valid(worker, expected_worker_role));
    }
    body.get(requirement).is_some_and(|value| match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        serde_json::Value::Array(values) => !values.is_empty(),
        serde_json::Value::Object(values) => !values.is_empty(),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
    })
}

fn worker_receipt_valid(worker: &serde_json::Value, expected_role: Option<&str>) -> bool {
    worker_ok(worker)
        && worker_structured_receipt(worker).is_some_and(|receipt| {
            worker_receipt_contract_errors(&receipt, expected_role).is_empty()
        })
}

fn packet_object_kind(payload: &serde_json::Value) -> Option<&str> {
    payload.get("object_kind").and_then(|value| value.as_str())
}

fn typed_packet_envelope_valid(payload: &serde_json::Value) -> bool {
    typed_packet_envelope_errors(payload).is_empty()
}

fn typed_packet_envelope_errors(payload: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();
    if payload
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(PACKET_SCHEMA_VERSION)
    {
        errors.push("schema_version".to_string());
    }
    if payload
        .get("loop_id")
        .and_then(|value| value.as_i64())
        .is_none()
    {
        errors.push("loop_id".to_string());
    }
    if payload
        .get("round")
        .and_then(|value| value.as_i64())
        .is_none()
    {
        errors.push("round".to_string());
    }
    if payload
        .get("object_kind")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("object_kind".to_string());
    }
    if payload
        .pointer("/writer/role")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("writer.role".to_string());
    }
    if payload
        .pointer("/route/from")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("route.from".to_string());
    }
    if payload
        .pointer("/route/to")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("route.to".to_string());
    }
    if payload.get("state_code").and_then(|value| value.as_str()) != Some("submitted") {
        errors.push("state_code".to_string());
    }
    if payload.get("body").is_none() {
        errors.push("body".to_string());
    }
    errors
}

fn packet_body(payload: &serde_json::Value) -> &serde_json::Value {
    payload.get("body").unwrap_or(payload)
}

fn packet_role_worker(payload: &serde_json::Value) -> Option<&serde_json::Value> {
    packet_body(payload).get("role_worker")
}

fn runtime_preflight_payload(
    contract: &HiveLoopContract,
    runtime: &str,
    runtime_probe: &serde_json::Value,
) -> serde_json::Value {
    let registry = runtime_policy_registry();
    let supported_runtime = runtime_policy_spec(&registry, runtime);
    let probe_ok = runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let blocker = runtime_policy_blocker(supported_runtime, probe_ok);
    let capability_preview = runtime_capability_preview(
        contract,
        &registry,
        supported_runtime,
        runtime_probe,
        blocker,
    );
    let capability_ready = capability_preview.worker_spawn_ready;
    let capability_blockers = capability_preview.worker_spawn_blockers.clone();

    serde_json::json!({
        "runtime": runtime,
        "runtime_probe": runtime_probe,
        "capability_preview": capability_preview,
        "runtime_policy": {
            "schema_version": POLICY_SCHEMA_VERSION,
            "gate": "runtime_policy_ready",
            "supported": supported_runtime.is_some(),
            "probe_ok": probe_ok,
            "blocker": blocker,
            "capability_ready": capability_ready,
            "capability_blockers": capability_blockers,
            "supported_runtimes": registry
                .supported
                .iter()
                .map(|spec| spec.name.clone())
                .collect::<Vec<_>>(),
            "selected": supported_runtime.map(|spec| serde_json::json!({
                "name": &spec.name,
                "mode": &spec.mode,
                "command": &spec.command,
                "required_worker_context": &spec.required_worker_context,
                "sandbox": &spec.sandbox
            }))
        }
    })
}

fn probe_runtime(runtime: &str) -> serde_json::Value {
    match runtime {
        "local" => serde_json::json!({
            "ok": true,
            "kind": "local",
            "detail": "local deterministic runtime"
        }),
        "codex" => match Command::new("codex").arg("--version").output() {
            Ok(output) => serde_json::json!({
                "ok": output.status.success(),
                "kind": "codex",
                "status": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout).trim(),
                "stderr": String::from_utf8_lossy(&output.stderr).trim()
            }),
            Err(error) => serde_json::json!({
                "ok": false,
                "kind": "codex",
                "error": error.to_string()
            }),
        },
        other => serde_json::json!({
            "ok": false,
            "kind": "unsupported",
            "runtime": other,
            "error": "unsupported runtime"
        }),
    }
}

fn run_role_worker(
    runtime: &str,
    role: &str,
    contract: &HiveLoopContract,
    runtime_probe: &serde_json::Value,
    timeout_secs: u64,
    max_attempts: u64,
) -> serde_json::Value {
    match runtime {
        "local" => serde_json::json!({
            "ok": true,
            "kind": "local",
            "mode": "deterministic-worker",
            "role": role,
            "receipt_schema_version": WORKER_RECEIPT_SCHEMA_VERSION,
            "receipt_ok": true,
            "duration_ms": 0,
            "timeout_secs": timeout_secs,
            "attempt_count": 1,
            "max_attempts": max_attempts,
            "last_message": format!(
                "Local {role} worker accepted loop #{} round {}.",
                contract.id,
                contract.current_round
            ),
            "packet": {
                "loop_id": contract.id,
                "round": contract.current_round,
                "role": role,
                "action": role_worker_action(role)
            },
            "receipt": {
                "ok": true,
                "role": role,
                "action": role_worker_action(role),
                "evidence_summary": format!(
                    "Local {role} worker accepted loop #{} round {}.",
                    contract.id,
                    contract.current_round
                ),
                "gates": {
                    "packet_received": true,
                    "role_bound": true,
                    "deterministic_runtime": true
                }
            }
        }),
        "codex" => {
            if !runtime_probe
                .get("ok")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                return serde_json::json!({
                    "ok": false,
                    "kind": "codex",
                    "mode": "codex-exec",
                    "role": role,
                    "skipped": true,
                    "timeout_secs": timeout_secs,
                    "attempt_count": 0,
                    "max_attempts": max_attempts,
                    "error": "codex probe failed"
                });
            }
            run_codex_worker(contract, role, timeout_secs, max_attempts)
        }
        other => serde_json::json!({
            "ok": false,
            "kind": "unsupported",
            "mode": "unsupported",
            "role": role,
            "runtime": other,
            "skipped": true,
            "timeout_secs": timeout_secs,
            "attempt_count": 0,
            "max_attempts": max_attempts,
            "error": "unsupported runtime"
        }),
    }
}

fn role_worker_action(role: &str) -> &'static str {
    match role {
        "explorer" => "compile-candidate",
        "developer" => "implement-admitted-candidate",
        "reviewer" => "review-evidence-and-verdict-envelope",
        "doer" => "record-local-loop-ledger",
        "evaluator" => "check-gates-and-verdict-envelope",
        _ => "unknown-role-action",
    }
}

fn run_codex_worker(
    contract: &HiveLoopContract,
    role: &str,
    timeout_secs: u64,
    max_attempts: u64,
) -> serde_json::Value {
    let mut attempts = Vec::new();
    for attempt in 1..=max_attempts {
        let attempt_result = run_codex_worker_attempt(contract, role, timeout_secs, attempt);
        let attempt_ok = worker_ok(&attempt_result);
        attempts.push(attempt_result);
        if attempt_ok {
            break;
        }
    }

    let mut worker = attempts.last().cloned().unwrap_or_else(|| {
        serde_json::json!({
            "ok": false,
            "kind": "codex",
            "mode": "codex-exec",
            "role": role,
            "timeout_secs": timeout_secs,
            "error": "no worker attempts were recorded"
        })
    });
    let attempt_count = attempts.len() as u64;
    let ok = worker_ok(&worker);
    if let Some(payload) = worker.as_object_mut() {
        payload.insert(
            "attempt_count".to_string(),
            serde_json::json!(attempt_count),
        );
        payload.insert("max_attempts".to_string(), serde_json::json!(max_attempts));
        payload.insert("attempts".to_string(), serde_json::json!(attempts));
        payload.insert(
            "retry_exhausted".to_string(),
            serde_json::json!(!ok && attempt_count >= max_attempts),
        );
    }
    worker
}

fn run_codex_worker_attempt(
    contract: &HiveLoopContract,
    role: &str,
    timeout_secs: u64,
    attempt: u64,
) -> serde_json::Value {
    let output_path = std::env::temp_dir().join(format!(
        "entrance-hive-codex-worker-{}-{}-{}-{}.txt",
        contract.id,
        role,
        attempt,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let prompt = codex_worker_prompt(contract, role);
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let command_display = codex_command_display(&cwd, &output_path);
    let prompt_chars = prompt.chars().count();

    let mut command = Command::new("codex");
    command
        .arg("-a")
        .arg("never")
        .arg("exec")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("read-only")
        .arg("-C")
        .arg(&cwd)
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("--json")
        .arg(prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let started_at = chrono::Utc::now().to_rfc3339();
    let result = run_command_with_timeout(command, Duration::from_secs(timeout_secs));
    let last_message = std::fs::read_to_string(&output_path).unwrap_or_default();
    let _ = std::fs::remove_file(&output_path);

    match result {
        Ok(output) => {
            let receipt = parse_worker_receipt(&last_message);
            let receipt_errors = receipt
                .as_ref()
                .map(|receipt| worker_receipt_contract_errors(receipt, None))
                .unwrap_or_else(|| vec!["receipt.parse".to_string()]);
            let receipt_ok = worker_receipt_ok(&last_message);
            let worker_ok = codex_worker_success(&output, receipt_ok);
            serde_json::json!({
                "ok": worker_ok,
                "kind": "codex",
                "mode": "codex-exec",
                "role": role,
                "receipt_schema_version": WORKER_RECEIPT_SCHEMA_VERSION,
                "attempt": attempt,
                "started_at": started_at,
                "completed_at": chrono::Utc::now().to_rfc3339(),
                "timed_out": output.timed_out,
                "status": output.status_code,
                "duration_ms": output.duration_ms,
                "timeout_secs": timeout_secs,
                "command": command_display,
                "cwd": cwd,
                "output_last_message_path": output_path.display().to_string(),
                "prompt_chars": prompt_chars,
                "receipt_ok": receipt_ok,
                "receipt": receipt,
                "receipt_errors": receipt_errors,
                "stdout": truncate_text(&output.stdout, 12000),
                "stderr": truncate_text(&output.stderr, 4000),
                "last_message": truncate_text(&last_message, 4000)
            })
        }
        Err(error) => serde_json::json!({
            "ok": false,
            "kind": "codex",
            "mode": "codex-exec",
            "role": role,
            "attempt": attempt,
            "started_at": started_at,
            "completed_at": chrono::Utc::now().to_rfc3339(),
            "timeout_secs": timeout_secs,
            "command": command_display,
            "cwd": cwd,
            "output_last_message_path": output_path.display().to_string(),
            "prompt_chars": prompt_chars,
            "error": error.to_string(),
            "last_message": truncate_text(&last_message, 4000)
        }),
    }
}

fn codex_command_display(cwd: &str, output_path: &std::path::Path) -> String {
    format!(
        "codex -a never exec --ephemeral --skip-git-repo-check --sandbox read-only -C {} --output-last-message {} --json <prompt>",
        cwd,
        output_path.display()
    )
}

fn codex_worker_success(output: &TimedCommandOutput, receipt_ok: Option<bool>) -> bool {
    output.status_success && !output.timed_out && receipt_ok == Some(true)
}

struct TimedCommandOutput {
    status_success: bool,
    status_code: Option<i32>,
    timed_out: bool,
    duration_ms: u64,
    stdout: String,
    stderr: String,
}

fn run_command_with_timeout(mut command: Command, timeout: Duration) -> Result<TimedCommandOutput> {
    let mut child = command.spawn().context("failed to spawn runtime worker")?;
    let started = Instant::now();
    let mut timed_out = false;

    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let output = child.wait_with_output()?;
    let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    Ok(TimedCommandOutput {
        status_success: output.status.success(),
        status_code: output.status.code(),
        timed_out,
        duration_ms,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn codex_worker_prompt(contract: &HiveLoopContract, role: &str) -> String {
    let expected_action = role_worker_action(role);
    let reviewer_semantic_receipt = if role == "reviewer" {
        r#"
- Reviewer-only: include `decision`, `scores`, `findings`, `acceptance_evidence`, and `risk_notes`.
- `scores` must include numeric goal_alignment, acceptance_evidence,
  implementation_specificity, and regression_risk values in [0, 1].
"#
    } else {
        ""
    };
    let role_duty = match role {
        "explorer" => {
            "compile the goal into a bounded candidate and confirm the constraints are explicit"
        }
        "developer" => {
            "validate that you received the admitted development packet and confirm the implementation path is bounded"
        }
        "reviewer" => {
            "inspect the evidence and gate contract, then confirm the verdict envelope can be reviewed"
        }
        "doer" => {
            "validate that you received the accepted execution packet and confirm it can be executed"
        }
        "evaluator" => {
            "inspect the gate contract and confirm the verdict envelope can be evaluated"
        }
        _ => "acknowledge the typed loop packet",
    };
    format!(
        r#"Entrance Hive {role_name} worker packet.

You are the {role_name} role inside a constrained Explorer -> Developer -> Reviewer loop.
Rules:
- Do not modify files.
- Do not make network calls.
- Keep the response compact.
- Return a single JSON object only. Do not use markdown fences or prose.
- The receipt schema is strict:
  {{
    "ok": true,
    "role": "{role_name}",
    "action": "{expected_action}",
    "evidence_summary": "one compact sentence",
    "gates": {{"packet_received": true, "role_bound": true}}
  }}
- `ok` must be a boolean.
- `role` must be the string "{role_name}".
- `action` must be a non-empty JSON string. Never return `action` as an object,
  array, or nested structure. Put details in `evidence_summary` instead.
- `evidence_summary` must be a non-empty string.
- `gates` must be a non-empty JSON object.
- Extra role-specific receipt fields:{reviewer_semantic_receipt}
- Your role duty is: {role_duty}.
- This is a runtime receipt: validate that you received the typed packet,
  summarize the accepted action for your role, and set ok=true unless you cannot process it.
- Do not set ok=false merely because the surrounding Hive runtime persists the
  evidence after you return.

Loop id: {id}
Round: {round}
Title: {title}
Goal: {goal}
Boundary: {boundary}
Approach space: {approach}
Eval space: {eval}
Accepted candidate: Run the local Hive loop MVP packet and report whether the runtime can execute it.
"#,
        role_name = role,
        expected_action = expected_action,
        reviewer_semantic_receipt = reviewer_semantic_receipt,
        role_duty = role_duty,
        id = contract.id,
        round = contract.current_round,
        title = contract.title,
        goal = contract.goal,
        boundary = contract.boundary,
        approach = serde_json::to_string(&contract.approach_space).unwrap_or_default(),
        eval = serde_json::to_string(&contract.eval_space).unwrap_or_default()
    )
}

fn worker_receipt_ok(value: &str) -> Option<bool> {
    let receipt = parse_worker_receipt(value)?;
    if receipt.get("ok").and_then(|value| value.as_bool()) != Some(true) {
        return Some(false);
    }
    Some(worker_receipt_contract_errors(&receipt, None).is_empty())
}

fn parse_worker_receipt(value: &str) -> Option<serde_json::Value> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .or_else(|_| {
            let start = trimmed.find('{').unwrap_or(0);
            let end = trimmed
                .rfind('}')
                .map(|index| index + 1)
                .unwrap_or(trimmed.len());
            serde_json::from_str::<serde_json::Value>(&trimmed[start..end])
        })
        .ok()
}

fn worker_structured_receipt(worker: &serde_json::Value) -> Option<serde_json::Value> {
    worker
        .get("receipt")
        .filter(|value| value.is_object())
        .cloned()
        .or_else(|| {
            worker
                .get("last_message")
                .and_then(|value| value.as_str())
                .and_then(parse_worker_receipt)
        })
}

fn worker_receipt_contract_errors(
    receipt: &serde_json::Value,
    expected_role: Option<&str>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if receipt
        .get("ok")
        .and_then(|value| value.as_bool())
        .is_none()
    {
        errors.push("ok".to_string());
    }
    let role = receipt.get("role").and_then(|value| value.as_str());
    if role.map_or(true, |value| value.trim().is_empty()) {
        errors.push("role".to_string());
    }
    if expected_role.is_some_and(|expected| role.is_some_and(|role| role != expected)) {
        errors.push("role_binding".to_string());
    }
    if receipt
        .get("action")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("action".to_string());
    }
    if receipt
        .get("evidence_summary")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("evidence_summary".to_string());
    }
    match receipt.get("gates") {
        Some(serde_json::Value::Object(gates)) if !gates.is_empty() => {}
        _ => errors.push("gates".to_string()),
    }
    errors
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            break;
        }
        output.push(ch);
    }
    output
}

fn default_text(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn default_vec(values: Vec<String>, fallback: &str) -> Vec<String> {
    let values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        vec![fallback.to_string()]
    } else {
        values
    }
}
