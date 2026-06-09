pub fn audit(store: &Store, loop_id: i64) -> Result<HiveLoopAuditReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let policies = store.list_hive_loop_policies(loop_id)?;
    let packets = store.list_hive_loop_packets(loop_id)?;
    let admissions = store.list_hive_loop_admissions(loop_id)?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let verdicts = store.list_hive_loop_verdicts(loop_id)?;
    let issues = store.list_hive_issues_for_loop(loop_id)?;
    let evidence = store.list_hive_loop_evidence(loop_id)?;
    let schema_status = store.schema_status()?;
    let packet_by_id = packets
        .iter()
        .map(|packet| (packet.id, packet))
        .collect::<HashMap<_, _>>();

    let active_policies = policies
        .iter()
        .filter(|policy| policy.status == "active")
        .collect::<Vec<_>>();
    let expected_active_policies = expected_loop_policies_for_active(&active_policies);
    let policy_errors = active_policy_audit_errors(&active_policies);
    let stage_sequence_errors = stage_sequence_audit_errors(&contract, &stages, &evidence);
    let packet_sequence_errors = packet_sequence_audit_errors(&packets);
    let packet_errors = packets
        .iter()
        .filter_map(|packet| {
            let mut errors = typed_packet_envelope_errors(&packet.payload);
            errors.extend(packet_row_binding_errors(packet));
            if errors.is_empty() {
                None
            } else {
                Some(serde_json::json!({
                    "packet_id": packet.id,
                    "object_kind": packet.object_kind,
                    "errors": errors
                }))
            }
        })
        .collect::<Vec<_>>();
    let mut admission_errors = admissions
        .iter()
        .filter_map(|admission| {
            admission_audit_errors(
                admission,
                &packet_by_id,
                GateEvaluationContext {
                    packets: &packets,
                    admissions: &admissions,
                },
            )
        })
        .collect::<Vec<_>>();
    admission_errors.extend(packet_admission_audit_errors(&packets, &admissions));
    let worker_errors = packets
        .iter()
        .filter_map(worker_receipt_audit_errors)
        .collect::<Vec<_>>();
    let runtime_policy_errors = runtime_policy_audit_errors(&contract, &packets);
    let mut verdict_errors = verdicts
        .iter()
        .filter_map(verdict_audit_errors)
        .collect::<Vec<_>>();
    verdict_errors.extend(verdict_sequence_audit_errors(
        &contract, &verdicts, &evidence,
    ));
    verdict_errors.extend(verdict_evidence_binding_audit_errors(
        &contract,
        &verdicts,
        &packets,
        &admissions,
        &evidence,
    ));
    let issue_surface = issue_surface_audit(store, &contract, &issues, &evidence)?;
    let issue_transition_policy_errors = issue_transition_policy_audit_errors(store, &issues)?;

    let mut stage_evidence_errors = stage_evidence_audit_errors(&contract, &stages, &evidence);
    stage_evidence_errors.extend(evidence_worker_policy_audit_errors(&stages, &evidence));
    let checks = vec![
        audit_check(
            "contract_loaded",
            true,
            format!("Loop #{} `{}` loaded.", contract.id, contract.title),
            serde_json::json!({
                "status": contract.status,
                "active_phase": contract.active_phase,
                "current_round": contract.current_round
            }),
        ),
        store_schema_audit_check(&schema_status),
        audit_check(
            "active_policy_registry",
            policy_errors.is_empty(),
            format!(
                "{} active policies inspected; {} policy contract issues.",
                active_policies.len(),
                policy_errors.len()
            ),
            serde_json::json!({
                "active_policy_count": active_policies.len(),
                "expected_policy_count": expected_active_policies.len(),
                "policy_errors": policy_errors
            }),
        ),
        audit_check(
            "stage_sequence",
            stage_sequence_errors.is_empty(),
            format!(
                "{} stages inspected; {} stage sequence issues.",
                stages.len(),
                stage_sequence_errors.len()
            ),
            serde_json::json!({ "stage_sequence_errors": stage_sequence_errors }),
        ),
        audit_check(
            "stage_evidence",
            stage_evidence_errors.is_empty(),
            format!(
                "{} evidence rows inspected; {} stage evidence issues.",
                evidence.len(),
                stage_evidence_errors.len()
            ),
            serde_json::json!({ "stage_evidence_errors": stage_evidence_errors }),
        ),
        audit_check(
            "packet_sequence",
            packet_sequence_errors.is_empty(),
            format!(
                "{} packets inspected; {} route cardinality issues.",
                packets.len(),
                packet_sequence_errors.len()
            ),
            serde_json::json!({ "packet_sequence_errors": packet_sequence_errors }),
        ),
        audit_check(
            "packet_envelopes",
            packet_errors.is_empty(),
            format!(
                "{} packets inspected; {} envelope or row-binding issues.",
                packets.len(),
                packet_errors.len()
            ),
            serde_json::json!({ "packet_errors": packet_errors }),
        ),
        audit_check(
            "admission_receipts",
            admission_errors.is_empty(),
            format!(
                "{} admissions inspected; {} receipt issues.",
                admissions.len(),
                admission_errors.len()
            ),
            serde_json::json!({ "admission_errors": admission_errors }),
        ),
        audit_check(
            "worker_receipts",
            worker_errors.is_empty(),
            format!(
                "{} packets inspected; {} worker receipt issues.",
                packets.len(),
                worker_errors.len()
            ),
            serde_json::json!({ "worker_errors": worker_errors }),
        ),
        audit_check(
            "runtime_policy",
            runtime_policy_errors.is_empty(),
            format!(
                "Runtime `{}` and current-round worker receipts inspected; {} runtime policy issues.",
                contract.runtime,
                runtime_policy_errors.len()
            ),
            serde_json::json!({
                "current_round": contract.current_round,
                "supported_runtimes": runtime_policy_registry()
                    .supported
                    .iter()
                    .map(|runtime| runtime.name.clone())
                    .collect::<Vec<_>>(),
                "runtime_policy_errors": runtime_policy_errors
            }),
        ),
        audit_check(
            "verdict_packets",
            verdict_errors.is_empty(),
            format!(
                "{} verdicts inspected; {} verdict issues.",
                verdicts.len(),
                verdict_errors.len()
            ),
            serde_json::json!({ "verdict_errors": verdict_errors }),
        ),
        audit_check(
            "issue_surface",
            issue_surface.errors.is_empty(),
            format!(
                "{} linked issues, {} comments, {} actions, and {} operator evidence rows inspected; {} issue surface issues.",
                issues.len(),
                issue_surface.comment_count,
                issue_surface.action_count,
                issue_surface.operator_evidence_count,
                issue_surface.errors.len()
            ),
            serde_json::json!({
                "issue_ids": issues.iter().map(|issue| issue.id).collect::<Vec<_>>(),
                "action_count": issue_surface.action_count,
                "comment_count": issue_surface.comment_count,
                "operator_evidence_count": issue_surface.operator_evidence_count,
                "issue_surface_errors": issue_surface.errors
            }),
        ),
        audit_check(
            "issue_transition_policy",
            issue_transition_policy_errors.is_empty(),
            format!(
                "{} linked issues inspected against status transition policy; {} transition policy issues.",
                issues.len(),
                issue_transition_policy_errors.len()
            ),
            serde_json::json!({
                "registry_owner": issue_transition_policy_registry().owner,
                "registry_scope": issue_transition_policy_registry().scope,
                "issue_transition_policy_errors": issue_transition_policy_errors
            }),
        ),
    ];
    let failed_count = checks.iter().filter(|check| !check.passed).count();
    Ok(HiveLoopAuditReport {
        schema_version: AUDIT_SCHEMA_VERSION.to_string(),
        loop_id,
        passed: failed_count == 0,
        failed_count,
        checks,
    })
}

pub fn doctor(store: &Store, loop_id: i64) -> Result<HiveLoopDoctorReport> {
    let trace_report = trace(store, loop_id)?;
    let audit_report = audit(store, loop_id)?;
    let contract = trace_report.contract;
    let issue_id = trace_report.issue.as_ref().map(|issue| issue.id);
    let issue_status = trace_report
        .issue
        .as_ref()
        .map(|issue| issue.status.clone());
    let trace_summary = trace_report.trace;
    let counts = doctor_counts(&trace_summary);
    let failed_checks = audit_report
        .checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.name.clone())
        .collect::<Vec<_>>();
    let audit_failure_details = audit_failure_details(&audit_report);
    let missing_receipts = doctor_missing_receipts(&trace_summary);
    let worker_failures = doctor_worker_failures(&trace_summary);
    let health = doctor_health(
        &contract.status,
        issue_status.as_deref(),
        trace_summary.last_decision.as_deref(),
        audit_report.passed,
        !worker_failures.is_empty(),
    )
    .to_string();
    let summary = doctor_summary(
        &contract,
        issue_status.as_deref(),
        &trace_summary,
        audit_report.passed,
        audit_report.failed_count,
        &health,
    );
    let next_actions = doctor_next_actions(
        &health,
        contract.id,
        issue_id,
        &contract.runtime,
        audit_report.passed,
    );
    let checks = audit_report
        .checks
        .into_iter()
        .map(|check| HiveLoopDoctorCheck {
            name: check.name,
            passed: check.passed,
            summary: check.summary,
        })
        .collect();

    Ok(HiveLoopDoctorReport {
        schema_version: DOCTOR_SCHEMA_VERSION.to_string(),
        loop_id: contract.id,
        health,
        summary,
        next_actions,
        status: contract.status,
        active_phase: contract.active_phase,
        current_round: contract.current_round,
        runtime: contract.runtime,
        issue_id,
        issue_status,
        decision: trace_summary.last_decision.clone(),
        reason_code: trace_summary.reason_code.clone(),
        counts,
        failed_checks,
        audit_failure_details,
        missing_receipts,
        worker_failures,
        checks,
        trace: trace_summary,
    })
}

pub fn worker_lifecycle(store: &Store, loop_id: i64) -> Result<HiveLoopWorkerLifecycleReport> {
    let trace_report = trace(store, loop_id)?;
    let contract = trace_report.contract;
    let issue_id = trace_report.issue.as_ref().map(|issue| issue.id);
    let issue_status = trace_report
        .issue
        .as_ref()
        .map(|issue| issue.status.clone());
    let trace_summary = trace_report.trace;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let evidence = store
        .list_hive_loop_evidence(loop_id)?
        .iter()
        .map(|row| issue_evidence_summary(row, &stage_roles))
        .collect::<Vec<_>>();
    let workers = evidence
        .iter()
        .filter_map(worker_lifecycle_worker)
        .collect::<Vec<_>>();
    let verdicts = store.list_hive_loop_verdicts(loop_id)?;
    let rounds = worker_lifecycle_rounds(&trace_summary, &workers, &verdicts);
    let current = rounds
        .iter()
        .find(|round| round.round == contract.current_round)
        .cloned()
        .unwrap_or_else(|| empty_worker_lifecycle_round(contract.current_round));
    let failures = worker_lifecycle_failures(&workers);
    let current_failures = current.failures.clone();
    let lifecycle_state = worker_lifecycle_state(
        &contract,
        issue_status.as_deref(),
        &trace_summary,
        &current,
        &current_failures,
    )
    .to_string();
    let next_actions =
        worker_lifecycle_next_actions(&lifecycle_state, contract.id, issue_id, &contract.runtime);
    let summary = worker_lifecycle_summary(
        &contract,
        issue_status.as_deref(),
        &lifecycle_state,
        &current,
    );

    Ok(HiveLoopWorkerLifecycleReport {
        schema_version: WORKER_LIFECYCLE_SCHEMA_VERSION.to_string(),
        loop_id: contract.id,
        issue_id,
        issue_status,
        status: contract.status,
        active_phase: contract.active_phase,
        current_round: contract.current_round,
        runtime: contract.runtime,
        lifecycle_state,
        summary,
        policy: worker_lifecycle_policy(),
        current,
        rounds,
        failures,
        next_actions,
    })
}

pub fn runtime_preflight(store: &Store, loop_id: i64) -> Result<HiveLoopRuntimePreflightReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issue = store.list_hive_issues_for_loop(loop_id)?.into_iter().next();
    let issue_id = issue.as_ref().map(|issue| issue.id);
    let issue_status = issue.as_ref().map(|issue| issue.status.clone());
    let packets = store.list_hive_loop_packets(loop_id)?;
    let admissions = store.list_hive_loop_admissions(loop_id)?;
    let current = runtime_preflight_observation(&contract, &packets, &admissions);
    let preview = runtime_preflight_preview(&contract);
    let preflight_state = runtime_preflight_state(&contract, &preview, current.as_ref());
    let failures = runtime_preflight_failures(&preview, current.as_ref());
    let next_actions =
        runtime_preflight_next_actions(preflight_state, contract.id, issue_id, &contract.runtime);
    let summary = runtime_preflight_summary(&contract, issue_status.as_deref(), preflight_state);

    Ok(HiveLoopRuntimePreflightReport {
        schema_version: RUNTIME_PREFLIGHT_SCHEMA_VERSION.to_string(),
        loop_id: contract.id,
        issue_id,
        issue_status,
        status: contract.status,
        active_phase: contract.active_phase,
        current_round: contract.current_round,
        runtime: contract.runtime,
        preflight_state: preflight_state.to_string(),
        summary,
        policy: runtime_preflight_policy(),
        preview,
        current,
        failures,
        next_actions,
    })
}

pub fn dashboard(store: &Store, loop_id: i64) -> Result<HiveLoopDashboardReport> {
    let preflight = runtime_preflight(store, loop_id)?;
    let lifecycle = worker_lifecycle(store, loop_id)?;
    let doctor = doctor(store, loop_id)?;
    let issue_card = store
        .list_hive_issues_for_loop(loop_id)?
        .into_iter()
        .next()
        .map(|issue| issue_card_from_issue(store, issue))
        .transpose()?;
    let issue = issue_card.as_ref().map(|card| card.issue.clone());
    let actions = issue_card
        .as_ref()
        .map(|card| card.actions.clone())
        .unwrap_or_default();
    let comments_count = issue_card
        .as_ref()
        .map(|card| card.comments.len())
        .unwrap_or_default();
    let latest_comment = issue_card
        .as_ref()
        .and_then(|card| card.comments.last())
        .map(|comment| HiveLoopDashboardComment {
            id: comment.id,
            author: comment.author.clone(),
            body: comment.body.clone(),
            created_at: comment.created_at.clone(),
        });
    let kernel = dashboard_kernel(&preflight);
    let agents = dashboard_agents(&lifecycle.current);
    let reviewer = dashboard_reviewer(&doctor.trace, &lifecycle);
    let human_decision = dashboard_human_decision(issue.as_ref(), &doctor.trace, &actions);
    let rounds = dashboard_rounds(store, loop_id, &doctor.trace)?;
    let health = HiveLoopDashboardHealth {
        health: doctor.health.clone(),
        audit_failed_count: doctor.counts.audit_failed_count,
        failed_checks: doctor.failed_checks.clone(),
        audit_failure_details: doctor.audit_failure_details.clone(),
        missing_receipts: doctor.missing_receipts.clone(),
        worker_failures: doctor.worker_failures.clone(),
    };
    let mut next_actions = Vec::new();
    push_unique(
        &mut next_actions,
        format!("entrance hive loop dashboard {loop_id}"),
    );
    for action in actions.iter().map(|action| action.command.clone()) {
        push_unique(&mut next_actions, action);
    }
    for action in preflight
        .next_actions
        .iter()
        .chain(lifecycle.next_actions.iter())
        .chain(doctor.next_actions.iter())
    {
        push_unique(&mut next_actions, action.clone());
    }
    let primary_next_action = next_actions
        .iter()
        .find(|action| !action.starts_with("entrance hive loop dashboard "))
        .cloned();
    let dashboard_state = dashboard_state(
        issue.as_ref(),
        &doctor,
        &preflight,
        &lifecycle.lifecycle_state,
    )
    .to_string();
    let summary = dashboard_summary(
        loop_id,
        issue.as_ref().map(|issue| issue.status.as_str()),
        &dashboard_state,
        &kernel,
        &lifecycle,
        &reviewer,
    );

    Ok(HiveLoopDashboardReport {
        schema_version: LOOP_DASHBOARD_SCHEMA_VERSION.to_string(),
        loop_id,
        issue,
        status: doctor.status,
        active_phase: doctor.active_phase,
        current_round: doctor.current_round,
        runtime: doctor.runtime,
        dashboard_state,
        summary,
        kernel,
        agents,
        reviewer,
        human_decision,
        health,
        rounds,
        comments_count,
        latest_comment,
        resources: HiveLoopDashboardResources {
            loop_dashboard: format!("entrance://loops/{loop_id}/dashboard"),
            evidence_drilldown: format!("entrance://loops/{loop_id}/evidence-drilldown"),
            evidence_manifest: format!("entrance://loops/{loop_id}/evidence-manifest"),
            runtime_preflight: format!("entrance://loops/{loop_id}/runtime-preflight"),
            worker_lifecycle: format!("entrance://loops/{loop_id}/worker-lifecycle"),
            issue: issue_card
                .as_ref()
                .map(|card| format!("entrance://issues/{}", card.issue.id)),
            issue_control: issue_card
                .as_ref()
                .map(|card| format!("entrance://issues/{}/control", card.issue.id)),
            review_queue: "entrance://review-queue".to_string(),
        },
        primary_next_action,
        next_actions,
    })
}

fn store_schema_audit_check(status: &StoreSchemaStatus) -> HiveLoopAuditCheck {
    let present_table_count = status.tables.iter().filter(|table| table.present).count();
    let present_index_count = status.indexes.iter().filter(|index| index.present).count();
    let errors = store_schema_audit_errors(status);
    let health = if status.healthy {
        "healthy"
    } else {
        "unhealthy"
    };
    audit_check(
        "store_schema",
        status.healthy,
        format!(
            "SQLite ledger schema is {health}: user_version {}/{}; tables {}/{}; indexes {}/{}.",
            status.user_version,
            status.expected_user_version,
            present_table_count,
            status.tables.len(),
            present_index_count,
            status.indexes.len()
        ),
        serde_json::json!({
            "schema_version": status.schema_version,
            "db_path": status.db_path,
            "user_version": status.user_version,
            "expected_user_version": status.expected_user_version,
            "present_table_count": present_table_count,
            "expected_table_count": status.tables.len(),
            "present_index_count": present_index_count,
            "expected_index_count": status.indexes.len(),
            "missing_tables": &status.missing_tables,
            "missing_columns": &status.missing_columns,
            "missing_indexes": &status.missing_indexes,
            "errors": errors
        }),
    )
}

fn store_schema_audit_errors(status: &StoreSchemaStatus) -> Vec<&'static str> {
    let mut errors = Vec::new();
    if status.user_version < status.expected_user_version {
        errors.push("schema.user_version");
    }
    if !status.missing_tables.is_empty() {
        errors.push("schema.missing_tables");
    }
    if !status.missing_columns.is_empty() {
        errors.push("schema.missing_columns");
    }
    if !status.missing_indexes.is_empty() {
        errors.push("schema.missing_indexes");
    }
    if !status.healthy && errors.is_empty() {
        errors.push("schema.healthy");
    }
    errors
}

fn audit_failure_details(report: &HiveLoopAuditReport) -> Vec<String> {
    let mut details = Vec::new();
    for check in report.checks.iter().filter(|check| !check.passed) {
        let before = details.len();
        collect_audit_failure_details(&check.name, &check.details, &mut details);
        if details.len() == before {
            details.push(check.name.clone());
        }
    }
    details.sort();
    details.dedup();
    details
}

fn collect_audit_failure_details(
    prefix: &str,
    value: &serde_json::Value,
    details: &mut Vec<String>,
) {
    if let Some(values) = value.as_array() {
        for value in values {
            collect_audit_failure_details(prefix, value, details);
        }
        return;
    }

    let Some(object) = value.as_object() else {
        return;
    };
    let scoped_prefix = object
        .get("scope")
        .and_then(|value| value.as_str())
        .map(|scope| format!("{prefix}:{scope}"))
        .unwrap_or_else(|| prefix.to_string());
    if let Some(errors) = object.get("errors").and_then(|value| value.as_array()) {
        for error in errors.iter().filter_map(|value| value.as_str()) {
            details.push(format!("{scoped_prefix}:{error}"));
        }
    }
    for (key, value) in object {
        if key != "errors" {
            collect_audit_failure_details(&scoped_prefix, value, details);
        }
    }
}

fn doctor_counts(trace: &IssueTraceSummary) -> HiveLoopDoctorCounts {
    HiveLoopDoctorCounts {
        packet_count: trace.packet_count,
        admission_count: trace.admission_count,
        evidence_count: trace.evidence_count,
        verdict_count: trace.verdict_count,
        round_packet_count: trace.round_packet_count,
        round_admission_count: trace.round_admission_count,
        round_evidence_count: trace.round_evidence_count,
        round_verdict_count: trace.round_verdict_count,
        receipt_required_count: trace.receipt_required_count,
        receipt_missing_count: trace.receipt_missing_count,
        round_receipt_required_count: trace.round_receipt_required_count,
        round_receipt_missing_count: trace.round_receipt_missing_count,
        role_worker_count: trace.role_worker_count,
        role_worker_ok_count: trace.role_worker_ok_count,
        round_role_worker_count: trace.round_role_worker_count,
        round_role_worker_ok_count: trace.round_role_worker_ok_count,
        round_worker_duration_ms: trace.round_worker_duration_ms,
        round_worker_timeout_count: trace.round_worker_timeout_count,
        round_worker_retry_exhausted_count: trace.round_worker_retry_exhausted_count,
        audit_failed_count: trace.audit_failed_count,
    }
}

fn doctor_missing_receipts(trace: &IssueTraceSummary) -> Vec<String> {
    let mut receipts = trace
        .evidence
        .iter()
        .flat_map(|evidence| evidence.missing_receipts.iter().cloned())
        .collect::<Vec<_>>();
    receipts.sort();
    receipts.dedup();
    receipts
}

fn doctor_worker_failures(trace: &IssueTraceSummary) -> Vec<String> {
    trace
        .evidence
        .iter()
        .filter(|evidence| {
            evidence.worker_ok == Some(false)
                || evidence.worker_receipt_ok == Some(false)
                || evidence.worker_timed_out == Some(true)
                || evidence.worker_retry_exhausted == Some(true)
                || !evidence.worker_receipt_errors.is_empty()
        })
        .map(|evidence| {
            let receipt_suffix = if evidence.worker_receipt_errors.is_empty() {
                String::new()
            } else {
                format!(
                    " receipt_errors={}",
                    evidence.worker_receipt_errors.join("|")
                )
            };
            format!(
                "{}:{} worker={} ok={} receipt={}{}{}",
                evidence.stage_role.as_deref().unwrap_or("loop"),
                evidence.kind,
                evidence.worker_kind.as_deref().unwrap_or("unknown"),
                doctor_bool_label(evidence.worker_ok),
                doctor_bool_label(evidence.worker_receipt_ok),
                if evidence.worker_retry_exhausted == Some(true) {
                    " retry_exhausted"
                } else if evidence.worker_timed_out == Some(true) {
                    " timeout"
                } else {
                    ""
                },
                receipt_suffix
            )
        })
        .collect()
}

fn doctor_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

fn doctor_health(
    contract_status: &str,
    issue_status: Option<&str>,
    decision: Option<&str>,
    audit_passed: bool,
    has_worker_failures: bool,
) -> &'static str {
    if contract_status == "needs-review"
        || issue_status == Some("Needs Review")
        || decision == Some("needs-review")
    {
        return "needs_review";
    }
    if contract_status == "blocked"
        || issue_status == Some("Blocked")
        || decision == Some("blocked")
    {
        return "blocked";
    }
    if contract_status == "rejected"
        || issue_status == Some("Canceled")
        || decision == Some("reject")
    {
        return "rejected";
    }
    if !audit_passed {
        return "audit_failed";
    }
    if has_worker_failures {
        return "worker_failed";
    }
    if contract_status == "kept" && decision == Some("keep") {
        return "ok";
    }
    if contract_status == "todo" || decision.is_none() {
        return "pending";
    }
    "unknown"
}

fn doctor_summary(
    contract: &HiveLoopContract,
    issue_status: Option<&str>,
    trace: &IssueTraceSummary,
    audit_passed: bool,
    audit_failed_count: usize,
    health: &str,
) -> String {
    let health_label = doctor_health_label(health);
    let audit_state = if audit_passed {
        "audit ok".to_string()
    } else {
        format!("audit failed {audit_failed_count} checks")
    };
    format!(
        "Loop #{} is {health_label} at {} round {}; issue {}; decision {}; {}; workers {}/{} current round; receipts missing {}/{} current round; worker time {} current round.",
        contract.id,
        contract.active_phase,
        contract.current_round,
        issue_status.unwrap_or("none"),
        trace.last_decision.as_deref().unwrap_or("pending"),
        audit_state,
        trace.round_role_worker_ok_count,
        trace.round_role_worker_count,
        trace.round_receipt_missing_count,
        trace.round_receipt_required_count,
        worker_duration_summary(trace.round_worker_duration_ms)
    )
}

fn doctor_health_label(health: &str) -> &str {
    match health {
        "needs_review" => "needs review",
        "audit_failed" => "audit failed",
        "worker_failed" => "worker failed",
        other => other,
    }
}

fn worker_duration_summary(duration_ms: u64) -> String {
    if duration_ms >= 1000 {
        format!("{:.1}s", duration_ms as f64 / 1000.0)
    } else {
        format!("{duration_ms}ms")
    }
}

fn doctor_next_actions(
    health: &str,
    loop_id: i64,
    issue_id: Option<i64>,
    runtime: &str,
    audit_passed: bool,
) -> Vec<String> {
    let mut actions = Vec::new();
    if !audit_passed {
        actions.push(compact_audit_command(loop_id));
        actions.push(format!("entrance hive loop evidence {loop_id}"));
    }
    match health {
        "ok" => {
            actions.push(compact_audit_command(loop_id));
            actions.push(format!("entrance hive loop trace {loop_id}"));
            actions.push(format!("entrance hive loop evidence {loop_id}"));
        }
        "pending" => {
            actions.push(pending_run_command(loop_id, issue_id, runtime));
        }
        "blocked" => {
            actions.push(format!("entrance hive loop evidence {loop_id}"));
            if let Some(issue_id) = issue_id {
                actions.push(retry_run_command(issue_id, runtime));
                actions.push(format!(
                    "entrance hive issue decide {issue_id} request-review --body <note> --human-confirmed --compact"
                ));
            }
        }
        "needs_review" => {
            if let Some(issue_id) = issue_id {
                actions.push(format!("entrance hive issue show {issue_id} --compact"));
                actions.push(retry_run_command(issue_id, runtime));
            }
        }
        "rejected" => {
            if let Some(issue_id) = issue_id {
                actions.push(format!("entrance hive issue show {issue_id} --compact"));
            }
        }
        "audit_failed" => {
            actions.push(compact_audit_command(loop_id));
            actions.push(format!("entrance hive loop evidence {loop_id}"));
        }
        "worker_failed" => {
            actions.push(format!("entrance hive loop evidence {loop_id}"));
            actions.push(format!("entrance hive loop doctor {loop_id}"));
            if let Some(issue_id) = issue_id {
                actions.push(retry_run_command(issue_id, runtime));
            }
        }
        _ => {
            actions.push(format!("entrance hive loop show {loop_id}"));
            actions.push(format!("entrance hive loop trace {loop_id}"));
        }
    }
    let mut deduped = Vec::new();
    for action in actions {
        if !deduped.contains(&action) {
            deduped.push(action);
        }
    }
    deduped
}

fn compact_audit_command(loop_id: i64) -> String {
    format!("entrance hive loop audit {loop_id} --compact")
}

fn pending_run_command(loop_id: i64, issue_id: Option<i64>, runtime: &str) -> String {
    match issue_id {
        Some(issue_id) => {
            format!("entrance hive issue run {issue_id} --runtime {runtime} --compact")
        }
        None => format!("entrance hive loop run {loop_id} --runtime {runtime} --compact"),
    }
}

fn worker_lifecycle_policy() -> HiveLoopWorkerLifecyclePolicy {
    HiveLoopWorkerLifecyclePolicy {
        schema_version: WORKER_LIFECYCLE_SCHEMA_VERSION.to_string(),
        expected_roles: CURRENT_LOOP_ROLES
            .iter()
            .map(|role| (*role).to_string())
            .collect(),
        compat_roles: COMPAT_LOOP_ROLES
            .iter()
            .map(|role| (*role).to_string())
            .collect(),
        default_timeout_secs: DEFAULT_WORKER_TIMEOUT_SECS,
        max_timeout_secs: MAX_WORKER_TIMEOUT_SECS,
        timeout_env: "ENTRANCE_HIVE_WORKER_TIMEOUT_SECS".to_string(),
        default_attempts: DEFAULT_WORKER_ATTEMPTS,
        max_attempts: MAX_WORKER_ATTEMPTS,
        attempts_env: "ENTRANCE_HIVE_WORKER_ATTEMPTS".to_string(),
        reviewer_invalid_round_budget: REVIEWER_INVALID_ROUND_BUDGET,
        fallback_status: "Blocked".to_string(),
        human_decision_statuses: vec!["Blocked".to_string(), "Needs Review".to_string()],
    }
}

fn worker_lifecycle_worker(
    evidence: &IssueEvidenceSummary,
) -> Option<HiveLoopWorkerLifecycleWorker> {
    if evidence.worker_kind.is_none()
        && evidence.worker_ok.is_none()
        && evidence.worker_attempt_count.is_none()
        && evidence.worker_receipt_ok.is_none()
    {
        return None;
    }

    Some(HiveLoopWorkerLifecycleWorker {
        evidence_id: evidence.id,
        round: evidence.round,
        role: worker_lifecycle_role(evidence),
        stage_role: evidence.stage_role.clone(),
        evidence_kind: evidence.kind.clone(),
        kind: evidence.worker_kind.clone(),
        mode: evidence.worker_mode.clone(),
        ok: evidence.worker_ok,
        receipt_ok: evidence.worker_receipt_ok,
        timed_out: evidence.worker_timed_out,
        status: evidence.worker_status,
        duration_ms: evidence.worker_duration_ms,
        timeout_secs: evidence.worker_timeout_secs,
        attempt_count: evidence.worker_attempt_count,
        max_attempts: evidence.worker_max_attempts,
        retry_exhausted: evidence.worker_retry_exhausted,
        command: evidence.worker_command.clone(),
        cwd: evidence.worker_cwd.clone(),
        action: evidence.worker_action.clone(),
        evidence_summary: evidence.worker_evidence_summary.clone(),
        gate_count: evidence.worker_gate_count,
        receipt_errors: evidence.worker_receipt_errors.clone(),
        transcript_excerpt: evidence.transcript_excerpt.clone(),
    })
}

fn worker_lifecycle_role(evidence: &IssueEvidenceSummary) -> String {
    evidence
        .stage_role
        .clone()
        .or_else(|| match evidence.kind.as_str() {
            "exploration_packet" => Some("explorer".to_string()),
            "execution_packet" => Some("developer".to_string()),
            "verdict_packet" => Some("reviewer".to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "loop".to_string())
}

fn worker_lifecycle_rounds(
    trace: &IssueTraceSummary,
    workers: &[HiveLoopWorkerLifecycleWorker],
    verdicts: &[HiveLoopVerdict],
) -> Vec<HiveLoopWorkerLifecycleRound> {
    let mut rounds = trace
        .rounds
        .iter()
        .map(|round| round.round)
        .collect::<Vec<_>>();
    rounds.push(trace.current_round);
    rounds.extend(workers.iter().map(|worker| worker.round));
    rounds.sort_unstable();
    rounds.dedup();
    rounds
        .into_iter()
        .map(|round| {
            let trace_round = trace.rounds.iter().find(|item| item.round == round);
            let round_workers = workers
                .iter()
                .filter(|worker| worker.round == round)
                .cloned()
                .collect::<Vec<_>>();
            let reviewer_invalid_rounds_used =
                reviewer_invalid_streak_from_verdicts(verdicts, round)
                    .min(REVIEWER_INVALID_ROUND_BUDGET);
            worker_lifecycle_round(
                round,
                trace_round,
                round_workers,
                reviewer_invalid_rounds_used,
            )
        })
        .collect()
}

fn worker_lifecycle_round(
    round: i64,
    trace_round: Option<&IssueRoundSummary>,
    workers: Vec<HiveLoopWorkerLifecycleWorker>,
    reviewer_invalid_rounds_used: i64,
) -> HiveLoopWorkerLifecycleRound {
    let expected_roles = CURRENT_LOOP_ROLES
        .iter()
        .map(|role| (*role).to_string())
        .collect::<Vec<_>>();
    let mut observed_roles = workers
        .iter()
        .map(|worker| worker.role.clone())
        .collect::<Vec<_>>();
    observed_roles.sort();
    observed_roles.dedup();
    let missing_roles = expected_roles
        .iter()
        .filter(|role| !observed_roles.contains(role))
        .cloned()
        .collect::<Vec<_>>();
    let worker_count = workers.len();
    let worker_ok_count = workers
        .iter()
        .filter(|worker| worker.ok == Some(true))
        .count();
    let worker_timeout_count = workers
        .iter()
        .filter(|worker| worker.timed_out == Some(true))
        .count();
    let worker_retry_exhausted_count = workers
        .iter()
        .filter(|worker| worker.retry_exhausted == Some(true))
        .count();
    let worker_duration_ms = workers.iter().filter_map(|worker| worker.duration_ms).sum();
    let failures = workers
        .iter()
        .filter_map(worker_lifecycle_worker_failure)
        .collect::<Vec<_>>();
    let decision = trace_round.and_then(|round| round.decision.clone());
    let reason_code = trace_round.and_then(|round| round.reason_code.clone());
    let reviewer_invalid_budget_exhausted = reviewer_invalid_rounds_used
        >= REVIEWER_INVALID_ROUND_BUDGET
        && reason_code.as_deref() == Some("review_budget_exhausted");

    HiveLoopWorkerLifecycleRound {
        round,
        status: trace_round
            .map(|round| round.status.clone())
            .unwrap_or_else(|| {
                issue_round_status(None, 0, 0, worker_count, worker_ok_count).to_string()
            }),
        decision,
        expected_roles,
        observed_roles,
        missing_roles,
        worker_count,
        worker_ok_count,
        worker_timeout_count,
        worker_retry_exhausted_count,
        worker_duration_ms,
        reviewer_invalid_rounds_used,
        reviewer_invalid_budget_exhausted,
        failures,
        workers,
    }
}

fn empty_worker_lifecycle_round(round: i64) -> HiveLoopWorkerLifecycleRound {
    worker_lifecycle_round(round, None, Vec::new(), 0)
}

fn worker_lifecycle_failures(workers: &[HiveLoopWorkerLifecycleWorker]) -> Vec<String> {
    workers
        .iter()
        .filter_map(worker_lifecycle_worker_failure)
        .collect()
}

fn worker_lifecycle_worker_failure(worker: &HiveLoopWorkerLifecycleWorker) -> Option<String> {
    if worker.ok != Some(false)
        && worker.receipt_ok != Some(false)
        && worker.timed_out != Some(true)
        && worker.retry_exhausted != Some(true)
        && worker.receipt_errors.is_empty()
    {
        return None;
    }

    let receipt_suffix = if worker.receipt_errors.is_empty() {
        String::new()
    } else {
        format!(" receipt_errors={}", worker.receipt_errors.join("|"))
    };
    Some(format!(
        "round={} role={} evidence={} worker={} ok={} receipt={}{}{}",
        worker.round,
        worker.role,
        worker.evidence_kind,
        worker.kind.as_deref().unwrap_or("unknown"),
        doctor_bool_label(worker.ok),
        doctor_bool_label(worker.receipt_ok),
        if worker.retry_exhausted == Some(true) {
            " retry_exhausted"
        } else if worker.timed_out == Some(true) {
            " timeout"
        } else {
            ""
        },
        receipt_suffix
    ))
}

fn worker_lifecycle_state(
    contract: &HiveLoopContract,
    issue_status: Option<&str>,
    trace: &IssueTraceSummary,
    current: &HiveLoopWorkerLifecycleRound,
    current_failures: &[String],
) -> &'static str {
    if matches!(contract.status.as_str(), "blocked")
        || issue_status == Some("Blocked")
        || trace.last_decision.as_deref() == Some("blocked")
    {
        return "blocked";
    }
    if matches!(contract.status.as_str(), "needs-review")
        || issue_status == Some("Needs Review")
        || trace.last_decision.as_deref() == Some("needs-review")
    {
        return "needs_review";
    }
    if matches!(contract.status.as_str(), "rejected") || issue_status == Some("Canceled") {
        return "canceled";
    }
    if !current_failures.is_empty() {
        return "worker_failed";
    }
    if contract.status == "kept"
        && issue_status == Some("Done")
        && trace.last_decision.as_deref() == Some("keep")
    {
        return "succeeded";
    }
    if contract.status == "running" || issue_status == Some("Doing") {
        return "running";
    }
    if current.worker_count == 0 {
        return "pending";
    }
    "observed"
}

fn worker_lifecycle_next_actions(
    lifecycle_state: &str,
    loop_id: i64,
    issue_id: Option<i64>,
    runtime: &str,
) -> Vec<String> {
    let mut actions = Vec::new();
    actions.push(format!("entrance hive loop worker-lifecycle {loop_id}"));
    actions.push(format!("entrance hive loop evidence {loop_id}"));
    match lifecycle_state {
        "pending" => actions.push(pending_run_command(loop_id, issue_id, runtime)),
        "blocked" | "needs_review" | "worker_failed" => {
            if let Some(issue_id) = issue_id {
                actions.push(retry_run_command(issue_id, runtime));
            }
        }
        "succeeded" => actions.push(format!("entrance hive loop trace {loop_id}")),
        _ => {}
    }
    let mut deduped = Vec::new();
    for action in actions {
        if !deduped.contains(&action) {
            deduped.push(action);
        }
    }
    deduped
}

fn worker_lifecycle_summary(
    contract: &HiveLoopContract,
    issue_status: Option<&str>,
    lifecycle_state: &str,
    current: &HiveLoopWorkerLifecycleRound,
) -> String {
    format!(
        "Loop #{} worker lifecycle is {} at round {}; issue {}; observed roles {}/{}; reviewer invalid budget {}/{}.",
        contract.id,
        lifecycle_state,
        current.round,
        issue_status.unwrap_or("none"),
        current.observed_roles.len(),
        current.expected_roles.len(),
        current.reviewer_invalid_rounds_used,
        REVIEWER_INVALID_ROUND_BUDGET
    )
}

fn runtime_preflight_policy() -> HiveLoopRuntimePreflightPolicy {
    let spec = gate_spec("runtime_policy_ready").expect("runtime preflight gate must exist");
    HiveLoopRuntimePreflightPolicy {
        schema_version: RUNTIME_PREFLIGHT_SCHEMA_VERSION.to_string(),
        gate: spec.name.to_string(),
        object_kind: spec
            .expected_object_kind
            .unwrap_or("PREFLIGHT_PACKET")
            .to_string(),
        route_from: "kernel".to_string(),
        route_to: "explorer".to_string(),
        required_receipts: spec
            .required_receipts
            .iter()
            .map(|receipt| (*receipt).to_string())
            .collect(),
        supported_runtimes: runtime_policy_registry()
            .supported
            .into_iter()
            .map(|runtime| runtime.name)
            .collect(),
    }
}

fn runtime_preflight_preview(contract: &HiveLoopContract) -> HiveLoopRuntimePreflightPreview {
    let registry = runtime_policy_registry();
    let runtime = contract.runtime.as_str();
    let selected_policy = runtime_policy_spec(&registry, runtime).cloned();
    let runtime_probe = probe_runtime(runtime);
    let probe_ok = runtime_probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let blocker = runtime_policy_blocker(selected_policy.as_ref(), probe_ok).map(ToOwned::to_owned);
    let capability_preview = runtime_capability_preview(
        contract,
        &registry,
        selected_policy.as_ref(),
        &runtime_probe,
        blocker.as_deref(),
    );
    HiveLoopRuntimePreflightPreview {
        runtime: runtime.to_string(),
        supported: selected_policy.is_some(),
        probe_ok,
        blocker,
        runtime_probe,
        selected_policy,
        capability_preview,
    }
}

fn runtime_policy_blocker(
    supported_runtime: Option<&RuntimePolicySpec>,
    probe_ok: bool,
) -> Option<&'static str> {
    if supported_runtime.is_none() {
        Some("runtime.unsupported")
    } else if !probe_ok {
        Some("runtime.probe_failed")
    } else {
        None
    }
}

fn runtime_capability_preview(
    contract: &HiveLoopContract,
    registry: &RuntimePolicyRegistry,
    selected_policy: Option<&RuntimePolicySpec>,
    runtime_probe: &serde_json::Value,
    runtime_blocker: Option<&str>,
) -> HiveLoopRuntimeCapabilityPreview {
    let runtime_ready = selected_policy.is_some()
        && runtime_probe
            .get("ok")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let worker_spawn_blockers = runtime_blocker
        .map(|blocker| vec![blocker.to_string()])
        .unwrap_or_default();
    let worker_spawn_ready = runtime_ready && worker_spawn_blockers.is_empty();
    let worker_mode = selected_policy.map(|policy| policy.mode.clone());

    HiveLoopRuntimeCapabilityPreview {
        schema_version: RUNTIME_CAPABILITY_PREVIEW_SCHEMA_VERSION.to_string(),
        runtime: contract.runtime.clone(),
        worker_spawn_ready,
        worker_spawn_blockers,
        admission_scope: vec![
            "packet.receipt_requirements".to_string(),
            "runtime_policy.supported".to_string(),
            "runtime_probe.ok".to_string(),
        ],
        worker_mode,
        sandbox: runtime_sandbox_preview(selected_policy),
        artifact_capture: runtime_artifact_capture_preview(contract.id, selected_policy),
        human_boundary: runtime_human_boundary_preview(
            &contract.review_surface,
            &contract.autonomy_level,
        ),
        worker_context: runtime_worker_context_preview(registry, selected_policy),
    }
}

fn runtime_sandbox_preview(
    selected_policy: Option<&RuntimePolicySpec>,
) -> HiveLoopRuntimeSandboxPreview {
    match selected_policy {
        Some(policy) => HiveLoopRuntimeSandboxPreview {
            filesystem: policy.sandbox.filesystem.clone(),
            network: policy.sandbox.network.clone(),
            writes_artifacts: policy.sandbox.writes_artifacts,
            process_isolation: if policy.command.is_some() {
                "external-process".to_string()
            } else {
                "in-process".to_string()
            },
            write_scope: if policy.sandbox.writes_artifacts {
                "worker transcript/output evidence only".to_string()
            } else {
                "none".to_string()
            },
        },
        None => HiveLoopRuntimeSandboxPreview {
            filesystem: "unknown".to_string(),
            network: "unknown".to_string(),
            writes_artifacts: false,
            process_isolation: "unknown".to_string(),
            write_scope: "unknown".to_string(),
        },
    }
}

fn runtime_artifact_capture_preview(
    loop_id: i64,
    selected_policy: Option<&RuntimePolicySpec>,
) -> HiveLoopRuntimeArtifactCapturePreview {
    let expected = selected_policy
        .map(|policy| policy.sandbox.writes_artifacts)
        .unwrap_or(false);
    HiveLoopRuntimeArtifactCapturePreview {
        expected,
        mode: if expected {
            "worker-transcript-evidence".to_string()
        } else {
            "ledger-only".to_string()
        },
        archive_ready: false,
        resource: format!("entrance://loops/{loop_id}/evidence-manifest"),
        next_action: if expected {
            format!("entrance hive loop evidence-manifest {loop_id}")
        } else {
            "no artifact capture required before worker spawn".to_string()
        },
    }
}

fn runtime_human_boundary_preview(
    review_surface: &str,
    autonomy_level: &str,
) -> HiveLoopRuntimeHumanBoundaryPreview {
    HiveLoopRuntimeHumanBoundaryPreview {
        review_surface: default_text(review_surface.to_string(), "local-hive-panel"),
        autonomy_level: default_text(autonomy_level.to_string(), "run-approved-candidates"),
        confirmation_arg: "human_confirmed".to_string(),
        human_decision_statuses: vec!["Blocked".to_string(), "Needs Review".to_string()],
        protected_actions: vec![
            "issue.retry".to_string(),
            "issue.request-review".to_string(),
            "issue.cancel".to_string(),
        ],
        reviewer_invalid_round_budget: REVIEWER_INVALID_ROUND_BUDGET,
        fallback_status: "Blocked".to_string(),
    }
}

fn runtime_worker_context_preview(
    registry: &RuntimePolicyRegistry,
    selected_policy: Option<&RuntimePolicySpec>,
) -> HiveLoopRuntimeWorkerContextPreview {
    let required = selected_policy
        .map(|policy| policy.required_worker_context.clone())
        .unwrap_or_default();
    HiveLoopRuntimeWorkerContextPreview {
        supplied_by_driver: required.clone(),
        required,
        missing_before_spawn: Vec::new(),
        required_receipt_fields: registry.worker.required_receipt_fields.clone(),
    }
}

fn runtime_preflight_observation(
    contract: &HiveLoopContract,
    packets: &[HiveLoopPacket],
    admissions: &[HiveLoopAdmission],
) -> Option<HiveLoopRuntimePreflightObservation> {
    let packet = packets
        .iter()
        .filter(|packet| {
            packet.object_kind == "PREFLIGHT_PACKET"
                && packet.writer_role == "kernel"
                && packet.route_from == "kernel"
                && packet.route_to == "explorer"
                && packet.round == contract.current_round
        })
        .max_by_key(|packet| packet.id)?;
    let admission = admissions
        .iter()
        .filter(|admission| admission.packet_id == packet.id)
        .max_by_key(|admission| admission.id);
    let body = packet_body(&packet.payload);
    let receipt_required = packet_receipt_requirements(&packet.payload);
    let receipt_missing = admission
        .map(|admission| string_array_at(&admission.policy, "/receipt/missing"))
        .unwrap_or_else(|| {
            let (_required, missing) = receipt_requirement_status(&packet.payload);
            missing
        });

    Some(HiveLoopRuntimePreflightObservation {
        packet_id: packet.id,
        admission_id: admission.map(|admission| admission.id),
        round: packet.round,
        result: admission.map(|admission| admission.result.clone()),
        reason: admission.map(|admission| admission.reason.clone()),
        gate: admission
            .and_then(|admission| admission.policy.pointer("/gate/name"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        gate_passed: admission
            .and_then(|admission| admission.policy.pointer("/gate/passed"))
            .and_then(|value| value.as_bool()),
        receipt_required,
        receipt_missing,
        runtime: body
            .get("runtime")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        supported: body
            .pointer("/runtime_policy/supported")
            .and_then(|value| value.as_bool()),
        probe_ok: body
            .pointer("/runtime_probe/ok")
            .and_then(|value| value.as_bool()),
        blocker: body
            .pointer("/runtime_policy/blocker")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        runtime_probe: body.get("runtime_probe").cloned(),
        capability_preview: body.get("capability_preview").cloned(),
    })
}

fn runtime_preflight_state(
    contract: &HiveLoopContract,
    preview: &HiveLoopRuntimePreflightPreview,
    current: Option<&HiveLoopRuntimePreflightObservation>,
) -> &'static str {
    if current.is_some_and(|current| current.result.as_deref() == Some("rejected")) {
        return "blocked";
    }
    if current.is_some_and(|current| current.result.as_deref() == Some("admitted")) {
        return "admitted";
    }
    if contract.status == "todo" && preview.capability_preview.worker_spawn_ready {
        return "ready";
    }
    if contract.status == "todo" {
        return "blocked";
    }
    "pending"
}

fn runtime_preflight_failures(
    preview: &HiveLoopRuntimePreflightPreview,
    current: Option<&HiveLoopRuntimePreflightObservation>,
) -> Vec<String> {
    let mut failures = Vec::new();
    if let Some(current) = current {
        if current.result.as_deref() == Some("rejected") {
            failures.push(
                current
                    .reason
                    .clone()
                    .unwrap_or_else(|| "runtime preflight rejected".to_string()),
            );
        }
    } else if let Some(blocker) = preview.blocker.as_ref() {
        failures.push(blocker.clone());
    }
    if !preview.supported
        && !failures
            .iter()
            .any(|failure| failure == "runtime.unsupported")
    {
        failures.push("runtime.unsupported".to_string());
    }
    if preview.supported
        && !preview.probe_ok
        && !failures
            .iter()
            .any(|failure| failure == "runtime.probe_failed")
    {
        failures.push("runtime.probe_failed".to_string());
    }
    for blocker in &preview.capability_preview.worker_spawn_blockers {
        if !failures.iter().any(|failure| failure == blocker) {
            failures.push(blocker.clone());
        }
    }
    failures
}

fn runtime_preflight_next_actions(
    preflight_state: &str,
    loop_id: i64,
    issue_id: Option<i64>,
    runtime: &str,
) -> Vec<String> {
    let mut actions = vec![format!("entrance hive loop preflight {loop_id}")];
    match preflight_state {
        "ready" => actions.push(pending_run_command(loop_id, issue_id, runtime)),
        "blocked" => {
            actions.push(format!("entrance hive loop audit {loop_id} --compact"));
            if let Some(issue_id) = issue_id {
                actions.push(retry_run_command(issue_id, runtime));
            }
        }
        "admitted" => actions.push(format!("entrance hive loop trace {loop_id}")),
        _ => {}
    }
    let mut deduped = Vec::new();
    for action in actions {
        if !deduped.contains(&action) {
            deduped.push(action);
        }
    }
    deduped
}

fn runtime_preflight_summary(
    contract: &HiveLoopContract,
    issue_status: Option<&str>,
    preflight_state: &str,
) -> String {
    format!(
        "Loop #{} runtime preflight is {} for `{}` at round {}; issue {}.",
        contract.id,
        preflight_state,
        contract.runtime,
        contract.current_round,
        issue_status.unwrap_or("none")
    )
}

fn dashboard_kernel(preflight: &HiveLoopRuntimePreflightReport) -> HiveLoopDashboardKernel {
    HiveLoopDashboardKernel {
        preflight_state: preflight.preflight_state.clone(),
        gate: preflight
            .current
            .as_ref()
            .and_then(|current| current.gate.clone())
            .unwrap_or_else(|| preflight.policy.gate.clone()),
        gate_passed: preflight
            .current
            .as_ref()
            .and_then(|current| current.gate_passed),
        route_from: preflight.policy.route_from.clone(),
        route_to: preflight.policy.route_to.clone(),
        object_kind: preflight.policy.object_kind.clone(),
        blocker: preflight
            .current
            .as_ref()
            .and_then(|current| current.blocker.clone())
            .or_else(|| preflight.preview.blocker.clone()),
        failures: preflight.failures.clone(),
    }
}

fn dashboard_agents(round: &HiveLoopWorkerLifecycleRound) -> Vec<HiveLoopDashboardAgent> {
    round
        .expected_roles
        .iter()
        .map(|role| {
            let worker = round.workers.iter().find(|worker| worker.role == *role);
            HiveLoopDashboardAgent {
                role: role.clone(),
                state: dashboard_agent_state(worker).to_string(),
                evidence_id: worker.map(|worker| worker.evidence_id),
                worker_kind: worker.and_then(|worker| worker.kind.clone()),
                worker_mode: worker.and_then(|worker| worker.mode.clone()),
                ok: worker.and_then(|worker| worker.ok),
                receipt_ok: worker.and_then(|worker| worker.receipt_ok),
                timed_out: worker.and_then(|worker| worker.timed_out),
                retry_exhausted: worker.and_then(|worker| worker.retry_exhausted),
                summary: worker
                    .and_then(|worker| worker.evidence_summary.clone())
                    .or_else(|| worker.and_then(|worker| worker.action.clone())),
            }
        })
        .collect()
}

fn dashboard_agent_state(worker: Option<&HiveLoopWorkerLifecycleWorker>) -> &'static str {
    let Some(worker) = worker else {
        return "pending";
    };
    if worker.retry_exhausted == Some(true) {
        return "retry_exhausted";
    }
    if worker.timed_out == Some(true) {
        return "timeout";
    }
    if worker.ok == Some(true) && worker.receipt_ok != Some(false) {
        return "ok";
    }
    if worker.ok == Some(false)
        || worker.receipt_ok == Some(false)
        || !worker.receipt_errors.is_empty()
    {
        return "blocked";
    }
    "observed"
}

fn dashboard_reviewer(
    trace: &IssueTraceSummary,
    lifecycle: &HiveLoopWorkerLifecycleReport,
) -> HiveLoopDashboardReviewer {
    HiveLoopDashboardReviewer {
        decision: trace.last_decision.clone(),
        reason_code: trace.reason_code.clone(),
        score_vector: trace.score_vector.clone(),
        human_options: trace.human_options.clone(),
        reviewer_invalid_rounds_used: lifecycle.current.reviewer_invalid_rounds_used,
        reviewer_invalid_round_budget: lifecycle.policy.reviewer_invalid_round_budget,
        reviewer_invalid_budget_exhausted: lifecycle.current.reviewer_invalid_budget_exhausted,
        fallback_status: lifecycle.policy.fallback_status.clone(),
    }
}

fn dashboard_human_decision(
    issue: Option<&HiveIssue>,
    trace: &IssueTraceSummary,
    actions: &[IssueAction],
) -> HiveLoopDashboardHumanDecision {
    let issue_status = issue.map(|issue| issue.status.clone());
    let required = issue_status
        .as_deref()
        .is_some_and(|status| matches!(status, "Blocked" | "Needs Review"))
        || actions.iter().any(|action| action.confirmation_required);
    HiveLoopDashboardHumanDecision {
        required,
        issue_status,
        options: trace.human_options.clone(),
        actions: actions.to_vec(),
    }
}

fn dashboard_rounds(
    store: &Store,
    loop_id: i64,
    trace: &IssueTraceSummary,
) -> Result<Vec<HiveLoopDashboardRound>> {
    let packets = store.list_hive_loop_packets(loop_id)?;
    let admissions = store.list_hive_loop_admissions(loop_id)?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let evidence = store.list_hive_loop_evidence(loop_id)?;
    let verdicts = store.list_hive_loop_verdicts(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let evidence_summaries = evidence
        .iter()
        .map(|row| issue_evidence_summary(row, &stage_roles))
        .collect::<Vec<_>>();
    let packet_rounds = packets
        .iter()
        .map(|packet| (packet.id, packet.round))
        .collect::<HashMap<_, _>>();
    let admission_by_packet = admissions
        .iter()
        .map(|admission| (admission.packet_id, admission))
        .collect::<HashMap<_, _>>();
    let failed_rounds = trace
        .rounds
        .iter()
        .filter(|round| dashboard_round_summary_failed(round))
        .map(|round| round.round)
        .collect::<Vec<_>>();

    Ok(trace
        .rounds
        .iter()
        .map(|round_summary| {
            let round = round_summary.round;
            let round_packets = packets
                .iter()
                .filter(|packet| packet.round == round)
                .map(|packet| dashboard_round_packet(packet, &admission_by_packet))
                .collect::<Vec<_>>();
            let round_admissions = admissions
                .iter()
                .filter(|admission| {
                    packet_rounds
                        .get(&admission.packet_id)
                        .is_some_and(|packet_round| *packet_round == round)
                })
                .map(dashboard_round_admission)
                .collect::<Vec<_>>();
            let round_evidence = evidence_summaries
                .iter()
                .filter(|row| row.round == round)
                .map(dashboard_round_evidence)
                .collect::<Vec<_>>();
            let round_verdicts = verdicts
                .iter()
                .filter(|verdict| verdict.round == round)
                .map(dashboard_round_verdict)
                .collect::<Vec<_>>();
            let groups = HiveLoopDashboardRoundGroups {
                packets: round_packets,
                admissions: round_admissions,
                evidence: round_evidence,
                verdicts: round_verdicts,
            };
            let blocker = dashboard_round_blocker(round_summary, &groups);
            let retry_lineage =
                dashboard_retry_lineage(round_summary, trace.current_round, &failed_rounds);
            HiveLoopDashboardRound {
                round,
                current: round == trace.current_round,
                status: round_summary.status.clone(),
                decision: round_summary.decision.clone(),
                reason_code: groups
                    .verdicts
                    .iter()
                    .rev()
                    .find_map(|verdict| verdict.reason_code.clone()),
                retry_lineage,
                blocker,
                packet_count: groups.packets.len(),
                admission_count: groups.admissions.len(),
                evidence_count: round_summary.evidence_count,
                verdict_count: groups.verdicts.len(),
                rejected_count: round_summary.rejected_count,
                receipt_missing_count: round_summary.receipt_missing_count,
                worker_count: round_summary.worker_count,
                worker_ok_count: round_summary.worker_ok_count,
                groups,
            }
        })
        .collect())
}

fn dashboard_round_packet(
    packet: &HiveLoopPacket,
    admission_by_packet: &HashMap<i64, &HiveLoopAdmission>,
) -> HiveLoopDashboardRoundPacket {
    HiveLoopDashboardRoundPacket {
        id: packet.id,
        object_kind: packet.object_kind.clone(),
        writer_role: packet.writer_role.clone(),
        route_from: packet.route_from.clone(),
        route_to: packet.route_to.clone(),
        state_code: packet.state_code.clone(),
        admission_result: admission_by_packet
            .get(&packet.id)
            .map(|admission| admission.result.clone()),
    }
}

fn dashboard_round_admission(admission: &HiveLoopAdmission) -> HiveLoopDashboardRoundAdmission {
    HiveLoopDashboardRoundAdmission {
        id: admission.id,
        packet_id: admission.packet_id,
        result: admission.result.clone(),
        gate: admission
            .policy
            .pointer("/gate/name")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        gate_passed: admission
            .policy
            .pointer("/gate/passed")
            .and_then(|value| value.as_bool()),
        reason: admission.reason.clone(),
        missing_receipts: string_array_at(&admission.policy, "/receipt/missing"),
    }
}

fn dashboard_round_evidence(evidence: &IssueEvidenceSummary) -> HiveLoopDashboardRoundEvidence {
    HiveLoopDashboardRoundEvidence {
        id: evidence.id,
        stage_role: evidence.stage_role.clone(),
        kind: evidence.kind.clone(),
        admission_result: evidence.admission_result.clone(),
        blocked_phase: evidence.blocked_phase.clone(),
        worker_ok: evidence.worker_ok,
        summary: evidence.summary.clone(),
    }
}

fn dashboard_round_verdict(verdict: &HiveLoopVerdict) -> HiveLoopDashboardRoundVerdict {
    HiveLoopDashboardRoundVerdict {
        id: verdict.id,
        decision: verdict.decision.clone(),
        reason_code: verdict_reason_code(verdict),
        score_vector: score_vector(&verdict.score),
        summary: verdict.summary.clone(),
    }
}

fn verdict_reason_code(verdict: &HiveLoopVerdict) -> Option<String> {
    verdict
        .score
        .get("reason_code")
        .and_then(|value| value.as_str())
        .or_else(|| {
            verdict
                .evidence
                .get("reason_code")
                .and_then(|value| value.as_str())
        })
        .map(ToOwned::to_owned)
}

fn verdict_reviewer_invalid_round(verdict: &HiveLoopVerdict) -> bool {
    verdict.decision == "reject"
        || verdict_reason_code(verdict).as_deref() == Some("review_budget_exhausted")
}

fn round_reviewer_invalid_round(round: &IssueRoundSummary) -> bool {
    round.decision.as_deref() == Some("reject")
        || round.reason_code.as_deref() == Some("review_budget_exhausted")
}

fn reviewer_invalid_streak_from_verdicts(verdicts: &[HiveLoopVerdict], through_round: i64) -> i64 {
    if through_round < 1 {
        return 0;
    }
    let mut by_round = BTreeMap::new();
    for verdict in verdicts {
        if verdict.round <= through_round {
            by_round.insert(verdict.round, verdict);
        }
    }
    let mut expected_round = through_round;
    let mut streak = 0;
    while expected_round >= 1 {
        match by_round.get(&expected_round) {
            Some(verdict) if verdict_reviewer_invalid_round(verdict) => {
                streak += 1;
                expected_round -= 1;
            }
            _ => break,
        }
    }
    streak
}

fn reviewer_invalid_streak_from_rounds(rounds: &[IssueRoundSummary], through_round: i64) -> i64 {
    if through_round < 1 {
        return 0;
    }
    let by_round = rounds
        .iter()
        .filter(|round| round.round <= through_round)
        .map(|round| (round.round, round))
        .collect::<BTreeMap<_, _>>();
    let mut expected_round = through_round;
    let mut streak = 0;
    while expected_round >= 1 {
        match by_round.get(&expected_round) {
            Some(round) if round_reviewer_invalid_round(round) => {
                streak += 1;
                expected_round -= 1;
            }
            _ => break,
        }
    }
    streak
}

fn dashboard_round_blocker(
    round: &IssueRoundSummary,
    groups: &HiveLoopDashboardRoundGroups,
) -> Option<String> {
    groups
        .admissions
        .iter()
        .find(|admission| admission.result == "rejected")
        .map(|admission| admission.reason.clone())
        .or_else(|| {
            groups
                .evidence
                .iter()
                .find(|evidence| evidence.blocked_phase.is_some())
                .and_then(|evidence| evidence.blocked_phase.clone())
        })
        .or_else(|| {
            groups.verdicts.iter().rev().find_map(|verdict| {
                if matches!(
                    verdict.decision.as_str(),
                    "reject" | "blocked" | "needs-review"
                ) {
                    verdict
                        .reason_code
                        .clone()
                        .or_else(|| Some(verdict.summary.clone()))
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            if round.receipt_missing_count > 0 {
                Some(format!("missing_receipts={}", round.receipt_missing_count))
            } else {
                None
            }
        })
}

fn dashboard_retry_lineage(
    round: &IssueRoundSummary,
    current_round: i64,
    failed_rounds: &[i64],
) -> Option<String> {
    if round.round == current_round {
        let prior = failed_rounds
            .iter()
            .copied()
            .filter(|failed_round| *failed_round < current_round)
            .collect::<Vec<_>>();
        if prior.is_empty() {
            None
        } else {
            Some(format!(
                "recovered_from {}",
                prior
                    .iter()
                    .map(|value| format!("r{value}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        }
    } else if failed_rounds.contains(&round.round) {
        Some(format!("retried_after r{}", round.round))
    } else {
        None
    }
}

fn dashboard_round_summary_failed(round: &IssueRoundSummary) -> bool {
    round.status != "kept"
        && (round.rejected_count > 0
            || round.receipt_missing_count > 0
            || round.worker_retry_exhausted_count > 0
            || round.worker_timeout_count > 0
            || matches!(
                round.decision.as_deref(),
                Some("reject" | "blocked" | "needs-review")
            ))
}

fn dashboard_state(
    issue: Option<&HiveIssue>,
    doctor: &HiveLoopDoctorReport,
    preflight: &HiveLoopRuntimePreflightReport,
    lifecycle_state: &str,
) -> &'static str {
    if let Some(issue) = issue {
        match issue.status.as_str() {
            "Blocked" => return "blocked",
            "Needs Review" => return "needs_review",
            "Done" => return "done",
            "Canceled" => return "canceled",
            _ => {}
        }
    }
    if preflight.preflight_state == "blocked" {
        return "blocked";
    }
    match lifecycle_state {
        "running" => "running",
        "pending" => "pending",
        "worker_failed" => "worker_failed",
        "needs_review" => "needs_review",
        "blocked" => "blocked",
        "succeeded" if doctor.health == "ok" => "ok",
        _ if doctor.health == "ok" => "ok",
        _ => "attention",
    }
}

fn dashboard_summary(
    loop_id: i64,
    issue_status: Option<&str>,
    dashboard_state: &str,
    kernel: &HiveLoopDashboardKernel,
    lifecycle: &HiveLoopWorkerLifecycleReport,
    reviewer: &HiveLoopDashboardReviewer,
) -> String {
    format!(
        "Loop #{} dashboard is {}; issue {}; kernel {} via {}; workers {}/{}; reviewer budget {}/{}; decision {}.",
        loop_id,
        dashboard_state,
        issue_status.unwrap_or("none"),
        kernel.preflight_state,
        kernel.gate,
        lifecycle.current.worker_ok_count,
        lifecycle.current.worker_count,
        reviewer.reviewer_invalid_rounds_used,
        reviewer.reviewer_invalid_round_budget,
        reviewer.decision.as_deref().unwrap_or("pending")
    )
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.iter().any(|existing| existing == &item) {
        items.push(item);
    }
}

fn audit_check(
    name: &str,
    passed: bool,
    summary: String,
    details: serde_json::Value,
) -> HiveLoopAuditCheck {
    HiveLoopAuditCheck {
        name: name.to_string(),
        passed,
        summary,
        details,
    }
}

fn retry_run_command(issue_id: i64, runtime: &str) -> String {
    if runtime == "codex" {
        return format!(
            "entrance hive issue retry-run {issue_id} --body <note> --human-confirmed --runtime codex --worker-attempts 2 --compact"
        );
    }
    format!("entrance hive issue retry-run {issue_id} --body <note> --human-confirmed --compact")
}

fn stage_sequence_audit_errors(
    contract: &HiveLoopContract,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let mut groups: HashMap<(i64, &str), Vec<&HiveLoopStage>> = HashMap::new();
    for stage in stages {
        let mut row_errors = Vec::new();
        if stage.round < 1 || stage.round > contract.current_round {
            row_errors.push("stage.round");
        }
        if !known_stage_roles().contains(&stage.role.as_str()) {
            row_errors.push("stage.role");
        }
        if stage.status != "done" {
            row_errors.push("stage.status");
        }
        if !row_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "stage_row",
                "stage_id": stage.id,
                "round": stage.round,
                "role": stage.role,
                "status": stage.status,
                "current_round": contract.current_round,
                "errors": row_errors
            }));
        }
        groups
            .entry((stage.round, stage.role.as_str()))
            .or_default()
            .push(stage);
    }

    for ((round, role), role_stages) in groups {
        if role_stages.len() > 1 {
            errors.push(serde_json::json!({
                "scope": "stage_role",
                "round": round,
                "role": role,
                "stage_ids": role_stages.iter().map(|stage| stage.id).collect::<Vec<_>>(),
                "errors": ["stage.role_duplicate"]
            }));
        }
    }

    let admission_rejection_role =
        current_round_admission_rejection_role(contract, stages, evidence);
    let expected_roles =
        expected_stage_roles_for_contract(contract, admission_rejection_role.as_deref(), stages);
    if !expected_roles.is_empty() {
        let missing_roles = expected_roles
            .iter()
            .copied()
            .filter(|role| {
                !stages.iter().any(|stage| {
                    stage.round == contract.current_round && stage.role.as_str() == *role
                })
            })
            .collect::<Vec<_>>();
        if !missing_roles.is_empty() {
            errors.push(serde_json::json!({
                "scope": "stage_round",
                "round": contract.current_round,
                "status": contract.status,
                "active_phase": contract.active_phase,
                "expected_roles": expected_roles,
                "missing_roles": missing_roles,
                "errors": ["stage.role_missing"]
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            error
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn canonical_stage_roles() -> &'static [&'static str] {
    CURRENT_LOOP_ROLES
}

fn compat_stage_roles() -> &'static [&'static str] {
    COMPAT_LOOP_ROLES
}

fn known_stage_roles() -> &'static [&'static str] {
    &[
        "kernel",
        "explorer",
        "developer",
        "reviewer",
        "doer",
        "evaluator",
    ]
}

fn expected_stage_roles_for_contract(
    contract: &HiveLoopContract,
    admission_rejection_role: Option<&str>,
    stages: &[HiveLoopStage],
) -> Vec<&'static str> {
    let roles = stage_role_family_for_contract(contract, admission_rejection_role, stages);
    match contract.status.as_str() {
        "kept" | "rejected" => roles.to_vec(),
        "needs-review"
            if contract.active_phase == "human-review" && admission_rejection_role.is_some() =>
        {
            expected_stage_roles_through(admission_rejection_role.unwrap_or_default())
        }
        "needs-review" => roles.to_vec(),
        "blocked" => match contract.active_phase.as_str() {
            _ if admission_rejection_role.is_some() => {
                expected_stage_roles_through(admission_rejection_role.unwrap_or_default())
            }
            "kernel" => vec!["kernel"],
            "explorer" => vec!["explorer"],
            "developer" => vec!["explorer", "developer"],
            "reviewer" => canonical_stage_roles().to_vec(),
            "doer" => vec!["explorer", "doer"],
            "evaluator" => compat_stage_roles().to_vec(),
            "complete" | "human-review" => roles.to_vec(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn stage_role_family_for_contract(
    contract: &HiveLoopContract,
    admission_rejection_role: Option<&str>,
    stages: &[HiveLoopStage],
) -> &'static [&'static str] {
    if matches!(
        admission_rejection_role,
        Some("developer") | Some("reviewer")
    ) || matches!(contract.active_phase.as_str(), "developer" | "reviewer")
        || stages
            .iter()
            .any(|stage| matches!(stage.role.as_str(), "developer" | "reviewer"))
    {
        return canonical_stage_roles();
    }
    if matches!(admission_rejection_role, Some("doer") | Some("evaluator"))
        || matches!(contract.active_phase.as_str(), "doer" | "evaluator")
        || stages
            .iter()
            .any(|stage| matches!(stage.role.as_str(), "doer" | "evaluator"))
    {
        return compat_stage_roles();
    }
    canonical_stage_roles()
}

fn expected_stage_roles_through(role: &str) -> Vec<&'static str> {
    match role {
        "kernel" => vec!["kernel"],
        "explorer" => vec!["explorer"],
        "developer" => vec!["explorer", "developer"],
        "reviewer" => canonical_stage_roles().to_vec(),
        "doer" => vec!["explorer", "doer"],
        "evaluator" => compat_stage_roles().to_vec(),
        _ => Vec::new(),
    }
}

fn current_round_admission_rejection_role(
    contract: &HiveLoopContract,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Option<String> {
    let stages_by_id = stages
        .iter()
        .map(|stage| (stage.id, stage))
        .collect::<HashMap<_, _>>();
    evidence.iter().find_map(|row| {
        if row.round != contract.current_round || row.kind != "admission_rejection" {
            return None;
        }
        row.stage_id
            .and_then(|stage_id| stages_by_id.get(&stage_id))
            .map(|stage| stage.role.clone())
            .or_else(|| {
                row.payload
                    .get("phase")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned)
            })
    })
}

fn stage_evidence_audit_errors(
    contract: &HiveLoopContract,
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let stages_by_id = stages
        .iter()
        .map(|stage| (stage.id, stage))
        .collect::<HashMap<_, _>>();
    let mut stage_evidence_groups: HashMap<(i64, &str), Vec<&HiveLoopEvidence>> = HashMap::new();

    let admission_rejection_role =
        current_round_admission_rejection_role(contract, stages, evidence);

    for row in evidence {
        match row.stage_id {
            Some(stage_id) => {
                let mut row_errors = Vec::new();
                if row.round < 1 || row.round > contract.current_round {
                    row_errors.push("evidence.round");
                }
                if let Some(stage) = stages_by_id.get(&stage_id) {
                    if row.round != stage.round {
                        row_errors.push("evidence.stage_round");
                    }
                    let expected_kind = expected_stage_evidence_kind(
                        contract,
                        stage,
                        admission_rejection_role.as_deref(),
                    );
                    if stage.round == contract.current_round
                        && expected_kind.is_some_and(|expected| expected != row.kind)
                    {
                        row_errors.push("evidence.kind");
                    } else if !stage_evidence_kind_allowed_for_role(&stage.role, &row.kind) {
                        row_errors.push("evidence.kind");
                    }
                } else {
                    row_errors.push("evidence.stage_link");
                }
                if !row_errors.is_empty() {
                    errors.push(serde_json::json!({
                        "scope": "evidence_row",
                        "evidence_id": row.id,
                        "stage_id": stage_id,
                        "round": row.round,
                        "kind": row.kind,
                        "errors": row_errors
                    }));
                }
                stage_evidence_groups
                    .entry((stage_id, row.kind.as_str()))
                    .or_default()
                    .push(row);
            }
            None if stage_bound_evidence_kind(&row.kind) => {
                errors.push(serde_json::json!({
                    "scope": "evidence_row",
                    "evidence_id": row.id,
                    "round": row.round,
                    "kind": row.kind,
                    "errors": ["evidence.stage_id"]
                }));
            }
            None => {}
        }
    }

    for ((stage_id, kind), rows) in stage_evidence_groups {
        if rows.len() > 1 {
            let stage = stages_by_id.get(&stage_id);
            errors.push(serde_json::json!({
                "scope": "evidence_stage",
                "stage_id": stage_id,
                "round": stage.map(|stage| stage.round),
                "role": stage.map(|stage| stage.role.as_str()),
                "kind": kind,
                "evidence_ids": rows.iter().map(|row| row.id).collect::<Vec<_>>(),
                "errors": ["evidence.stage_duplicate"]
            }));
        }
    }

    let expected_roles =
        expected_stage_roles_for_contract(contract, admission_rejection_role.as_deref(), stages);
    for stage in stages.iter().filter(|stage| {
        stage.round == contract.current_round
            && expected_roles
                .iter()
                .any(|role| *role == stage.role.as_str())
    }) {
        let Some(expected_kind) =
            expected_stage_evidence_kind(contract, stage, admission_rejection_role.as_deref())
        else {
            continue;
        };
        if !evidence
            .iter()
            .any(|row| row.stage_id == Some(stage.id) && row.kind == expected_kind)
        {
            errors.push(serde_json::json!({
                "scope": "stage",
                "stage_id": stage.id,
                "round": stage.round,
                "role": stage.role,
                "expected_kind": expected_kind,
                "errors": ["evidence.stage_missing"]
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            error
                .get("stage_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("kind")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn evidence_worker_policy_audit_errors(
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let registry = runtime_policy_registry();
    let stages_by_id = stages
        .iter()
        .map(|stage| (stage.id, stage))
        .collect::<HashMap<_, _>>();
    let mut errors = Vec::new();

    for row in evidence.iter().filter(|row| {
        matches!(
            row.kind.as_str(),
            "exploration_packet" | "execution_packet" | "verdict_packet"
        )
    }) {
        let Some(stage) = row
            .stage_id
            .and_then(|stage_id| stages_by_id.get(&stage_id))
        else {
            continue;
        };
        let row_errors = match row.payload.get("worker") {
            Some(worker) => runtime_worker_policy_errors(&registry, worker, &stage.role),
            None => vec!["worker".to_string()],
        };
        if !row_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "evidence_worker",
                "evidence_id": row.id,
                "stage_id": stage.id,
                "round": row.round,
                "role": stage.role,
                "kind": row.kind,
                "errors": row_errors
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("stage_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("evidence_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
        )
    });
    errors
}

fn expected_stage_evidence_kind(
    contract: &HiveLoopContract,
    stage: &HiveLoopStage,
    admission_rejection_role: Option<&str>,
) -> Option<&'static str> {
    if stage.round == contract.current_round
        && admission_rejection_role.is_some_and(|role| role == stage.role.as_str())
    {
        return Some("admission_rejection");
    }
    if contract.status == "blocked"
        && stage.round == contract.current_round
        && contract.active_phase == stage.role
        && matches!(
            contract.active_phase.as_str(),
            "explorer" | "developer" | "reviewer" | "doer" | "evaluator"
        )
    {
        return Some("admission_rejection");
    }
    canonical_stage_evidence_kind(&stage.role)
}

fn canonical_stage_evidence_kind(role: &str) -> Option<&'static str> {
    match role {
        "explorer" => Some("exploration_packet"),
        "developer" => Some("execution_packet"),
        "reviewer" => Some("verdict_packet"),
        "doer" => Some("execution_packet"),
        "evaluator" => Some("verdict_packet"),
        _ => None,
    }
}

fn stage_evidence_kind_allowed_for_role(role: &str, kind: &str) -> bool {
    canonical_stage_evidence_kind(role) == Some(kind) || kind == "admission_rejection"
}

fn stage_bound_evidence_kind(kind: &str) -> bool {
    matches!(
        kind,
        "exploration_packet" | "execution_packet" | "verdict_packet" | "admission_rejection"
    )
}

fn packet_sequence_audit_errors(packets: &[HiveLoopPacket]) -> Vec<serde_json::Value> {
    let mut groups: HashMap<(i64, &str, &str, &str, &str), Vec<&HiveLoopPacket>> = HashMap::new();
    for packet in packets {
        groups
            .entry((
                packet.round,
                packet.object_kind.as_str(),
                packet.writer_role.as_str(),
                packet.route_from.as_str(),
                packet.route_to.as_str(),
            ))
            .or_default()
            .push(packet);
    }

    let mut errors = groups
        .into_iter()
        .filter_map(
            |((round, object_kind, writer_role, route_from, route_to), packets)| {
                (packets.len() > 1).then(|| {
                    serde_json::json!({
                        "scope": "packet_route",
                        "round": round,
                        "object_kind": object_kind,
                        "writer_role": writer_role,
                        "route_from": route_from,
                        "route_to": route_to,
                        "packet_ids": packets.iter().map(|packet| packet.id).collect::<Vec<_>>(),
                        "errors": ["packet.route_duplicate"]
                    })
                })
            },
        )
        .collect::<Vec<_>>();
    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("object_kind")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn packet_row_binding_errors(packet: &HiveLoopPacket) -> Vec<String> {
    let payload = &packet.payload;
    let mut errors = Vec::new();
    if payload
        .get("loop_id")
        .and_then(|value| value.as_i64())
        .is_some_and(|value| value != packet.loop_id)
    {
        errors.push("row.loop_id".to_string());
    }
    if payload
        .get("round")
        .and_then(|value| value.as_i64())
        .is_some_and(|value| value != packet.round)
    {
        errors.push("row.round".to_string());
    }
    if packet_object_kind(payload).is_some_and(|value| value != packet.object_kind.as_str()) {
        errors.push("row.object_kind".to_string());
    }
    if payload
        .pointer("/writer/role")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.writer_role.as_str())
    {
        errors.push("row.writer_role".to_string());
    }
    if payload
        .pointer("/route/from")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.route_from.as_str())
    {
        errors.push("row.route_from".to_string());
    }
    if payload
        .pointer("/route/to")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.route_to.as_str())
    {
        errors.push("row.route_to".to_string());
    }
    if payload
        .get("state_code")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value != packet.state_code.as_str())
    {
        errors.push("row.state_code".to_string());
    }
    errors
}

fn packet_admission_audit_errors(
    packets: &[HiveLoopPacket],
    admissions: &[HiveLoopAdmission],
) -> Vec<serde_json::Value> {
    let mut admission_counts = HashMap::new();
    for admission in admissions {
        *admission_counts
            .entry(admission.packet_id)
            .or_insert(0usize) += 1;
    }

    packets
        .iter()
        .filter_map(|packet| {
            let count = admission_counts
                .get(&packet.id)
                .copied()
                .unwrap_or_default();
            let errors = match count {
                0 => vec!["packet.admission_missing"],
                1 => Vec::new(),
                _ => vec!["packet.admission_duplicate"],
            };
            (!errors.is_empty()).then(|| {
                serde_json::json!({
                    "scope": "packet_admission",
                    "packet_id": packet.id,
                    "round": packet.round,
                    "object_kind": packet.object_kind,
                    "writer_role": packet.writer_role,
                    "admission_count": count,
                    "errors": errors
                })
            })
        })
        .collect()
}

fn admission_audit_errors(
    admission: &HiveLoopAdmission,
    packet_by_id: &HashMap<i64, &HiveLoopPacket>,
    gate_context: GateEvaluationContext<'_>,
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let packet = packet_by_id.get(&admission.packet_id).copied();
    if admission
        .policy
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(ADMISSION_SCHEMA_VERSION)
    {
        errors.push("schema_version".to_string());
    }
    if packet.is_none() {
        errors.push("packet.link".to_string());
    }
    let envelope_valid = admission
        .policy
        .pointer("/packet/envelope/valid")
        .and_then(|value| value.as_bool());
    if envelope_valid != Some(true) {
        errors.push("packet.envelope".to_string());
    }
    if admission
        .policy
        .get("result")
        .and_then(|value| value.as_str())
        != Some(admission.result.as_str())
    {
        errors.push("result.binding".to_string());
    }
    if !matches!(admission.result.as_str(), "admitted" | "rejected") {
        errors.push("result.value".to_string());
    }

    let gate_name = admission
        .policy
        .pointer("/gate/name")
        .and_then(|value| value.as_str());
    if let Some(gate_name) = gate_name {
        if gate_spec(gate_name).is_none() {
            errors.push("gate.unknown".to_string());
        }
    }
    let gate_passed = admission
        .policy
        .pointer("/gate/passed")
        .and_then(|value| value.as_bool());
    let policy_missing = admission
        .policy
        .get("policy")
        .map_or(true, serde_json::Value::is_null);
    if policy_missing && admission.result == "admitted" {
        errors.push("policy.missing".to_string());
    }
    if let Some(packet) = packet {
        if packet.loop_id != admission.loop_id {
            errors.push("packet.loop_id".to_string());
        }
        if admission
            .policy
            .pointer("/packet/id")
            .and_then(|value| value.as_i64())
            != Some(packet.id)
        {
            errors.push("packet.id".to_string());
        }
        if admission_field(&admission.policy, "/packet/object_kind")
            != Some(packet.object_kind.as_str())
        {
            errors.push("packet.object_kind".to_string());
        }
        if admission_field(&admission.policy, "/packet/writer_role")
            != Some(packet.writer_role.as_str())
        {
            errors.push("packet.writer_role".to_string());
        }
        if admission_field(&admission.policy, "/packet/route_from")
            != Some(packet.route_from.as_str())
        {
            errors.push("packet.route_from".to_string());
        }
        if admission_field(&admission.policy, "/packet/route_to") != Some(packet.route_to.as_str())
        {
            errors.push("packet.route_to".to_string());
        }
        if admission_field(&admission.policy, "/packet/state_code")
            != Some(packet.state_code.as_str())
        {
            errors.push("packet.state_code".to_string());
        }
        if envelope_valid != Some(typed_packet_envelope_valid(&packet.payload)) {
            errors.push("packet.envelope_binding".to_string());
        }

        let expected_required = receipt_requirements_for_packet(&packet.object_kind)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let (_packet_required, packet_missing_receipts) =
            receipt_requirement_status(&packet.payload);
        let declared_required = packet_receipt_requirements(&packet.payload);
        let receipt_required = string_array_at(&admission.policy, "/receipt/required");
        if declared_required != expected_required {
            errors.push("packet.receipt_requirements".to_string());
        }
        if receipt_required != expected_required {
            errors.push("receipt.required_binding".to_string());
        }
        let receipt_missing = string_array_at(&admission.policy, "/receipt/missing");
        if receipt_missing != packet_missing_receipts {
            errors.push("receipt.missing_binding".to_string());
        }
    }

    if admission
        .policy
        .pointer("/receipt/required")
        .and_then(|value| value.as_array())
        .is_none()
    {
        errors.push("receipt.required".to_string());
    }
    let receipt_missing_array = admission
        .policy
        .pointer("/receipt/missing")
        .and_then(|value| value.as_array());
    if receipt_missing_array.is_none() {
        errors.push("receipt.missing".to_string());
    }
    let receipt_missing = string_array_at(&admission.policy, "/receipt/missing");
    let receipt_satisfied = admission
        .policy
        .pointer("/receipt/satisfied")
        .and_then(|value| value.as_bool());
    if receipt_satisfied.is_none() {
        errors.push("receipt.satisfied".to_string());
    }
    if receipt_satisfied != Some(receipt_missing.is_empty()) {
        errors.push("receipt.satisfied_binding".to_string());
    }
    if let Some(packet) = packet {
        let (_packet_required, packet_missing_receipts) =
            receipt_requirement_status(&packet.payload);
        if receipt_satisfied != Some(packet_missing_receipts.is_empty()) {
            errors.push("receipt.satisfied_packet_binding".to_string());
        }
    }

    if let Some(policy) = admission
        .policy
        .get("policy")
        .filter(|value| !value.is_null())
    {
        if policy
            .get("schema_version")
            .and_then(|value| value.as_str())
            != Some(POLICY_SCHEMA_VERSION)
        {
            errors.push("policy.schema_version".to_string());
        }
        if policy.get("status").and_then(|value| value.as_str()) != Some("active") {
            errors.push("policy.status".to_string());
        }
        let policy_gate = policy.get("gate").and_then(|value| value.as_str());
        if policy_gate != gate_name {
            errors.push("policy.gate_binding".to_string());
        }
        if let Some(packet) = packet {
            if policy.get("object_kind").and_then(|value| value.as_str())
                != Some(packet.object_kind.as_str())
            {
                errors.push("policy.object_kind".to_string());
            }
            if policy.get("writer_role").and_then(|value| value.as_str())
                != Some(packet.writer_role.as_str())
            {
                errors.push("policy.writer_role".to_string());
            }
            if policy.get("route_from").and_then(|value| value.as_str())
                != Some(packet.route_from.as_str())
            {
                errors.push("policy.route_from".to_string());
            }
            if policy.get("route_to").and_then(|value| value.as_str())
                != Some(packet.route_to.as_str())
            {
                errors.push("policy.route_to".to_string());
            }
        }
        if let Some(policy_gate) = policy_gate {
            admission_gate_spec_errors(
                policy,
                "/gate_spec",
                policy_gate,
                packet,
                "policy.gate_spec",
                &mut errors,
            );
        }
    }

    if let Some(gate_name) = gate_name {
        admission_gate_spec_errors(
            &admission.policy,
            "/gate/spec",
            gate_name,
            packet,
            "gate.spec",
            &mut errors,
        );
    }
    if let Some(packet) = packet {
        admission_target_binding_errors(
            admission,
            packet,
            gate_name,
            gate_passed,
            receipt_satisfied,
            gate_context,
            &mut errors,
        );
        admission_gate_result_binding_errors(
            admission,
            packet,
            gate_name,
            gate_passed,
            policy_missing,
            gate_context,
            &mut errors,
        );
    }
    match (
        admission.result.as_str(),
        gate_passed,
        receipt_satisfied,
        envelope_valid,
        policy_missing,
    ) {
        ("admitted", Some(true), Some(true), Some(true), false) => {}
        ("admitted", _, _, _, _) => errors.push("result.admission_conditions".to_string()),
        ("rejected", Some(true), Some(true), Some(true), false) => {
            errors.push("gate.result_binding".to_string());
        }
        ("rejected", _, _, _, _) => {}
        _ => {}
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "admission_id": admission.id,
            "packet_id": admission.packet_id,
            "errors": errors
        }))
    }
}

fn admission_field<'a>(value: &'a serde_json::Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(|value| value.as_str())
}

fn admission_gate_result_binding_errors(
    admission: &HiveLoopAdmission,
    packet: &HiveLoopPacket,
    gate_name: Option<&str>,
    gate_passed: Option<bool>,
    policy_missing: bool,
    gate_context: GateEvaluationContext<'_>,
    errors: &mut Vec<String>,
) {
    let Some(gate_name) = gate_name else {
        return;
    };
    if gate_spec(gate_name).is_none() {
        return;
    }

    let expected_gate_passed = gate_passes_with_context(gate_name, &packet.payload, gate_context);
    if gate_passed != Some(expected_gate_passed) {
        errors.push("gate.passed_binding".to_string());
    }

    let expected_reason = if expected_gate_passed {
        format!("{gate_name} passed")
    } else {
        gate_failure_reason_with_context(gate_name, &packet.payload, gate_context)
    };
    if admission.reason != expected_reason {
        errors.push("reason.gate_binding".to_string());
    }
    if admission
        .policy
        .get("reason")
        .and_then(|value| value.as_str())
        != Some(admission.reason.as_str())
    {
        errors.push("reason.binding".to_string());
    }

    let expected_result = if expected_gate_passed {
        "admitted"
    } else {
        "rejected"
    };
    if !policy_missing && admission.result != expected_result {
        errors.push("result.gate_binding".to_string());
    }
}

fn admission_target_binding_errors(
    admission: &HiveLoopAdmission,
    packet: &HiveLoopPacket,
    gate_name: Option<&str>,
    gate_passed: Option<bool>,
    receipt_satisfied: Option<bool>,
    gate_context: GateEvaluationContext<'_>,
    errors: &mut Vec<String>,
) {
    let actual = admission.policy.get("target_binding");
    if packet.object_kind != "EXECUTION_PACKET" {
        if actual.is_some_and(|value| !value.is_null()) {
            errors.push("target_binding.unexpected".to_string());
        }
        return;
    }

    let Some(actual) = actual else {
        errors.push("target_binding.missing".to_string());
        return;
    };
    if actual.is_null() {
        errors.push("target_binding.missing".to_string());
        return;
    }

    let expected = target_binding_receipt(packet, &packet.payload, gate_context);
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/schema_version",
        "target_binding.schema_version",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/name",
        "target_binding.name",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/passed",
        "target_binding.passed",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/reason",
        "target_binding.reason",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/developer_packet_id",
        "target_binding.developer_packet_id",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/explorer_packet_id",
        "target_binding.explorer_packet_id",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/explorer_candidate_count",
        "target_binding.explorer_candidate_count",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/expected_candidate",
        "target_binding.expected_candidate",
        errors,
    );
    compare_admission_target_binding_field(
        actual,
        &expected,
        "/accepted_candidate",
        "target_binding.accepted_candidate",
        errors,
    );

    if gate_name == Some(ACCEPTED_CANDIDATE_BOUND_GATE)
        && receipt_satisfied == Some(true)
        && gate_passed
            != expected
                .pointer("/passed")
                .and_then(|value| value.as_bool())
    {
        errors.push("target_binding.gate_binding".to_string());
    }
}

fn compare_admission_target_binding_field(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    pointer: &str,
    field: &str,
    errors: &mut Vec<String>,
) {
    if actual.pointer(pointer) != expected.pointer(pointer) {
        errors.push(field.to_string());
    }
}

fn admission_gate_spec_errors(
    value: &serde_json::Value,
    pointer: &str,
    gate_name: &str,
    packet: Option<&HiveLoopPacket>,
    prefix: &str,
    errors: &mut Vec<String>,
) {
    let Some(spec_value) = value.pointer(pointer) else {
        errors.push(format!("{prefix}.missing"));
        return;
    };
    let Some(spec) = gate_spec(gate_name) else {
        return;
    };
    if spec_value
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(POLICY_SCHEMA_VERSION)
    {
        errors.push(format!("{prefix}.schema_version"));
    }
    if spec_value.get("name").and_then(|value| value.as_str()) != Some(gate_name) {
        errors.push(format!("{prefix}.name"));
    }
    if spec_value
        .get("expected_object_kind")
        .and_then(|value| value.as_str())
        != spec.expected_object_kind
    {
        errors.push(format!("{prefix}.expected_object_kind"));
    }
    if string_array_at(spec_value, "/required_receipts")
        != spec
            .required_receipts
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    {
        errors.push(format!("{prefix}.required_receipts"));
    }
    if let Some(packet) = packet {
        if spec
            .expected_object_kind
            .is_some_and(|expected| expected != packet.object_kind)
        {
            errors.push(format!("{prefix}.packet_object_kind"));
        }
        if spec.required_receipts != receipt_requirements_for_packet(&packet.object_kind).as_slice()
        {
            errors.push(format!("{prefix}.packet_receipts"));
        }
    }
}

fn worker_receipt_audit_errors(packet: &HiveLoopPacket) -> Option<serde_json::Value> {
    let Some(worker) = packet_role_worker(&packet.payload) else {
        return None;
    };
    let mut errors = Vec::new();
    if worker
        .get("kind")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("kind".to_string());
    }
    if worker
        .get("mode")
        .and_then(|value| value.as_str())
        .map_or(true, |value| value.trim().is_empty())
    {
        errors.push("mode".to_string());
    }
    let worker_role = worker.get("role").and_then(|value| value.as_str());
    if worker_role.map_or(true, |value| value.trim().is_empty()) {
        errors.push("role".to_string());
    }
    if worker_role.is_some_and(|role| role != packet.writer_role) {
        errors.push("role_binding".to_string());
    }
    if worker.get("ok").and_then(|value| value.as_bool()).is_none() {
        errors.push("ok".to_string());
    }
    match worker.get("timeout_secs").and_then(|value| value.as_u64()) {
        Some(1..=MAX_WORKER_TIMEOUT_SECS) => {}
        _ => errors.push("timeout_secs".to_string()),
    }
    let attempt_count = worker.get("attempt_count").and_then(|value| value.as_u64());
    let max_attempts = worker.get("max_attempts").and_then(|value| value.as_u64());
    match max_attempts {
        Some(1..=MAX_WORKER_ATTEMPTS) => {}
        _ => errors.push("max_attempts".to_string()),
    }
    match (attempt_count, max_attempts) {
        (Some(count), Some(max)) if count <= max => {}
        _ => errors.push("attempt_count".to_string()),
    }
    if worker.get("ok").and_then(|value| value.as_bool()) == Some(true) {
        match worker_structured_receipt(worker) {
            Some(receipt) => errors.extend(
                worker_receipt_contract_errors(&receipt, Some(&packet.writer_role))
                    .into_iter()
                    .map(|field| format!("receipt.{field}")),
            ),
            None => errors.push("receipt".to_string()),
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "packet_id": packet.id,
            "object_kind": packet.object_kind,
            "writer_role": packet.writer_role,
            "worker_role": worker_role,
            "errors": errors
        }))
    }
}

fn runtime_policy_audit_errors(
    contract: &HiveLoopContract,
    packets: &[HiveLoopPacket],
) -> Vec<serde_json::Value> {
    let registry = runtime_policy_registry();
    let mut errors = Vec::new();
    if runtime_policy_spec(&registry, &contract.runtime).is_none() {
        errors.push(serde_json::json!({
            "scope": "contract",
            "runtime": contract.runtime,
            "errors": ["runtime.unsupported"]
        }));
    }

    for packet in packets
        .iter()
        .filter(|packet| packet.round == contract.current_round)
    {
        for (receipt, worker) in packet_worker_receipts(packet) {
            let worker_errors =
                runtime_worker_policy_errors(&registry, worker, &packet.writer_role);
            if !worker_errors.is_empty() {
                errors.push(serde_json::json!({
                    "scope": "worker_receipt",
                    "packet_id": packet.id,
                    "object_kind": packet.object_kind,
                    "writer_role": packet.writer_role,
                    "receipt": receipt,
                    "kind": worker.get("kind").and_then(|value| value.as_str()),
                    "worker_role": worker.get("role").and_then(|value| value.as_str()),
                    "errors": worker_errors
                }));
            }
        }
    }
    errors
}

fn packet_worker_receipts<'a>(
    packet: &'a HiveLoopPacket,
) -> Vec<(&'static str, &'a serde_json::Value)> {
    let body = packet_body(&packet.payload);
    let mut workers = Vec::new();
    if let Some(worker) = body.get("role_worker") {
        workers.push(("role_worker", worker));
    }
    if let Some(worker) = body.get("runtime_worker") {
        workers.push(("runtime_worker", worker));
    }
    workers
}

fn runtime_worker_policy_errors(
    registry: &RuntimePolicyRegistry,
    worker: &serde_json::Value,
    expected_role: &str,
) -> Vec<String> {
    let mut errors = Vec::new();
    let kind = worker.get("kind").and_then(|value| value.as_str());
    match kind.and_then(|kind| runtime_policy_spec(registry, kind)) {
        Some(spec) => {
            if worker.get("mode").and_then(|value| value.as_str()) != Some(spec.mode.as_str()) {
                errors.push("mode".to_string());
            }
            for field in &spec.required_worker_context {
                if !worker_context_field_present(worker, field) {
                    errors.push(format!("context.{field}"));
                }
            }
        }
        None if kind.is_some() => errors.push("kind.unsupported".to_string()),
        None => errors.push("kind".to_string()),
    }

    let role = worker.get("role").and_then(|value| value.as_str());
    if role.map_or(true, |value| value.trim().is_empty()) {
        errors.push("role".to_string());
    }
    if role.is_some_and(|value| value != expected_role) {
        errors.push("role_binding".to_string());
    }

    match worker.get("timeout_secs").and_then(|value| value.as_u64()) {
        Some(1..=MAX_WORKER_TIMEOUT_SECS) => {}
        _ => errors.push("timeout_secs".to_string()),
    }
    let attempt_count = worker.get("attempt_count").and_then(|value| value.as_u64());
    let max_attempts = worker.get("max_attempts").and_then(|value| value.as_u64());
    match max_attempts {
        Some(1..=MAX_WORKER_ATTEMPTS) => {}
        _ => errors.push("max_attempts".to_string()),
    }
    match (attempt_count, max_attempts) {
        (Some(count), Some(max)) if count <= max => {}
        _ => errors.push("attempt_count".to_string()),
    }
    if worker.get("ok").and_then(|value| value.as_bool()) == Some(true) {
        match worker_structured_receipt(worker) {
            Some(receipt) => errors.extend(
                worker_receipt_contract_errors(&receipt, Some(expected_role))
                    .into_iter()
                    .map(|field| format!("receipt.{field}")),
            ),
            None => errors.push("receipt".to_string()),
        }
    }
    errors
}

fn worker_context_field_present(worker: &serde_json::Value, field: &str) -> bool {
    match field {
        "command" | "cwd" | "output_last_message_path" => worker
            .get(field)
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty()),
        "prompt_chars" => worker
            .get(field)
            .and_then(|value| value.as_u64())
            .is_some_and(|value| value > 0),
        _ => worker.get(field).is_some_and(|value| !value.is_null()),
    }
}

fn runtime_policy_spec<'a>(
    registry: &'a RuntimePolicyRegistry,
    runtime: &str,
) -> Option<&'a RuntimePolicySpec> {
    registry.supported.iter().find(|spec| spec.name == runtime)
}

fn active_policy_audit_errors(active_policies: &[&HiveLoopPolicy]) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let expected_policies = expected_loop_policies_for_active(active_policies);
    if active_policies.len() != expected_policies.len() {
        errors.push(serde_json::json!({
            "scope": "active_policy_set",
            "expected": expected_policies.len(),
            "actual": active_policies.len(),
            "errors": ["active_policy_count"]
        }));
    }

    for expected in expected_policies {
        let matches = active_policies
            .iter()
            .filter(|policy| policy_matches_expected_route(policy, expected))
            .collect::<Vec<_>>();
        match matches.len() {
            0 => errors.push(serde_json::json!({
                "scope": "expected_policy",
                "object_kind": expected.object_kind,
                "writer_role": expected.writer_role,
                "route_from": expected.route_from,
                "route_to": expected.route_to,
                "gate": expected.gate,
                "errors": ["policy.missing"]
            })),
            1 => {
                let policy = matches[0];
                if policy.gate != expected.gate {
                    errors.push(serde_json::json!({
                        "scope": "policy",
                        "policy_id": policy.id,
                        "object_kind": policy.object_kind,
                        "writer_role": policy.writer_role,
                        "route_from": policy.route_from,
                        "route_to": policy.route_to,
                        "gate": policy.gate,
                        "expected_gate": expected.gate,
                        "errors": ["policy.gate"]
                    }));
                }
            }
            _ => errors.push(serde_json::json!({
                "scope": "expected_policy",
                "object_kind": expected.object_kind,
                "writer_role": expected.writer_role,
                "route_from": expected.route_from,
                "route_to": expected.route_to,
                "gate": expected.gate,
                "policy_ids": matches.iter().map(|policy| policy.id).collect::<Vec<_>>(),
                "errors": ["policy.duplicate"]
            })),
        }
    }

    for policy in active_policies {
        let mut policy_errors = Vec::new();
        match gate_spec(&policy.gate) {
            Some(spec) => {
                if spec
                    .expected_object_kind
                    .is_some_and(|expected| expected != policy.object_kind)
                {
                    policy_errors.push("gate.expected_object_kind".to_string());
                }
                if spec.required_receipts
                    != receipt_requirements_for_packet(&policy.object_kind).as_slice()
                {
                    policy_errors.push("gate.required_receipts".to_string());
                }
            }
            None => policy_errors.push("gate.unknown".to_string()),
        }
        if !expected_policies
            .iter()
            .any(|expected| policy_matches_expected_route(policy, expected))
        {
            policy_errors.push("policy.route".to_string());
        }
        if !policy_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "policy",
                "policy_id": policy.id,
                "object_kind": policy.object_kind,
                "writer_role": policy.writer_role,
                "route_from": policy.route_from,
                "route_to": policy.route_to,
                "gate": policy.gate,
                "errors": policy_errors
            }));
        }
    }

    errors
}

fn expected_loop_policies_for_active(
    active_policies: &[&HiveLoopPolicy],
) -> &'static [LoopPolicySpec] {
    let compat_match_count = COMPAT_LOOP_POLICIES
        .iter()
        .filter(|expected| {
            active_policies
                .iter()
                .any(|policy| policy_matches_expected_route(policy, expected))
        })
        .count();
    let current_match_count = CURRENT_LOOP_POLICIES
        .iter()
        .filter(|expected| {
            active_policies
                .iter()
                .any(|policy| policy_matches_expected_route(policy, expected))
        })
        .count();
    if compat_match_count > current_match_count {
        COMPAT_LOOP_POLICIES
    } else if active_policies
        .iter()
        .any(|policy| policy_matches_expected_route(policy, &DEFAULT_LOOP_POLICIES[0]))
    {
        DEFAULT_LOOP_POLICIES
    } else {
        CURRENT_LOOP_POLICIES
    }
}

fn policy_matches_expected_route(policy: &HiveLoopPolicy, expected: &LoopPolicySpec) -> bool {
    policy.object_kind == expected.object_kind
        && policy.writer_role == expected.writer_role
        && policy.route_from == expected.route_from
        && policy.route_to == expected.route_to
}

fn verdict_sequence_audit_errors(
    contract: &HiveLoopContract,
    verdicts: &[HiveLoopVerdict],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let mut errors = Vec::new();
    let mut verdicts_by_round: HashMap<i64, Vec<&HiveLoopVerdict>> = HashMap::new();
    for verdict in verdicts {
        if verdict.round < 1 || verdict.round > contract.current_round {
            errors.push(serde_json::json!({
                "scope": "verdict_row",
                "verdict_id": verdict.id,
                "round": verdict.round,
                "current_round": contract.current_round,
                "errors": ["verdict.round"]
            }));
        }
        verdicts_by_round
            .entry(verdict.round)
            .or_default()
            .push(verdict);
    }

    for (round, round_verdicts) in verdicts_by_round {
        if round_verdicts.len() > 1 {
            errors.push(serde_json::json!({
                "scope": "verdict_round",
                "round": round,
                "verdict_ids": round_verdicts.iter().map(|verdict| verdict.id).collect::<Vec<_>>(),
                "decisions": round_verdicts
                    .iter()
                    .map(|verdict| verdict.decision.as_str())
                    .collect::<Vec<_>>(),
                "errors": ["verdict.round_duplicate"]
            }));
        }
    }

    if terminal_contract_status(&contract.status)
        && !verdicts
            .iter()
            .any(|verdict| verdict.round == contract.current_round)
    {
        errors.push(serde_json::json!({
            "scope": "verdict_round",
            "round": contract.current_round,
            "status": contract.status,
            "errors": ["verdict.current_round_missing"]
        }));
    }
    let current_round_verdicts = verdicts
        .iter()
        .filter(|verdict| verdict.round == contract.current_round)
        .collect::<Vec<_>>();
    let operator_decision_controls_contract = evidence
        .iter()
        .any(|row| row.round == contract.current_round && row.kind == "operator_decision");
    if terminal_contract_status(&contract.status)
        && current_round_verdicts.len() == 1
        && !operator_decision_controls_contract
    {
        let verdict = current_round_verdicts[0];
        if let Some(expected_status) = contract_status_for_verdict_decision(&verdict.decision) {
            if contract.status != expected_status {
                errors.push(serde_json::json!({
                    "scope": "verdict_contract",
                    "round": contract.current_round,
                    "verdict_id": verdict.id,
                    "decision": verdict.decision,
                    "expected_contract_status": expected_status,
                    "actual_contract_status": contract.status,
                    "errors": ["contract.status_binding"]
                }));
            }
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("scope")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    });
    errors
}

fn terminal_contract_status(status: &str) -> bool {
    matches!(status, "kept" | "rejected" | "needs-review" | "blocked")
}

fn contract_status_for_verdict_decision(decision: &str) -> Option<&'static str> {
    match decision {
        "keep" => Some("kept"),
        "reject" => Some("rejected"),
        "needs-review" => Some("needs-review"),
        "blocked" => Some("blocked"),
        _ => None,
    }
}

fn verdict_audit_errors(verdict: &HiveLoopVerdict) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let score_decision = verdict
        .score
        .get("decision")
        .and_then(|value| value.as_str());
    let evidence_decision = verdict
        .evidence
        .get("decision")
        .and_then(|value| value.as_str());
    let score_reason = verdict
        .score
        .get("reason_code")
        .and_then(|value| value.as_str());
    let evidence_reason = verdict
        .evidence
        .get("reason_code")
        .and_then(|value| value.as_str());
    if verdict
        .score
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(VERDICT_SCHEMA_VERSION)
    {
        errors.push("score.schema_version".to_string());
    }
    if !decision_label_allowed(&verdict.decision) {
        errors.push("decision".to_string());
    }
    if score_decision != Some(verdict.decision.as_str()) {
        errors.push("score.decision_binding".to_string());
    }
    if evidence_decision != Some(verdict.decision.as_str()) {
        errors.push("evidence.decision_binding".to_string());
    }
    if score_reason.is_none() {
        errors.push("score.reason_code".to_string());
    }
    if evidence_reason.is_none() {
        errors.push("evidence.reason_code".to_string());
    }
    if score_reason.is_some() && evidence_reason.is_some() && score_reason != evidence_reason {
        errors.push("reason_code.binding".to_string());
    }
    if verdict.score.get("gate_results").map_or(true, |value| {
        !value.is_object() || value.as_object().is_some_and(serde_json::Map::is_empty)
    }) {
        errors.push("score.gate_results".to_string());
    }
    match verdict.score.get("score_vector") {
        Some(score_vector) if score_vector.is_object() => {
            errors.extend(verdict_score_vector_errors(
                score_vector,
                verdict.decision.as_str(),
            ));
        }
        _ => errors.push("score.score_vector".to_string()),
    }
    match verdict
        .score
        .get("gates_passed")
        .and_then(|value| value.as_bool())
    {
        Some(value) if value == (verdict.decision == "keep") => {}
        _ => errors.push("score.gates_passed".to_string()),
    }
    match verdict
        .score
        .get("operator_review_needed")
        .and_then(|value| value.as_bool())
    {
        Some(value) if value == (verdict.decision != "keep") => {}
        _ => errors.push("score.operator_review_needed".to_string()),
    }
    if human_options(&verdict.score) != expected_human_options_for_decision(&verdict.decision) {
        errors.push("score.human_options".to_string());
    }
    if verdict
        .evidence
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(VERDICT_SCHEMA_VERSION)
    {
        errors.push("evidence.schema_version".to_string());
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "verdict_id": verdict.id,
            "round": verdict.round,
            "errors": errors
        }))
    }
}

fn verdict_evidence_binding_audit_errors(
    contract: &HiveLoopContract,
    verdicts: &[HiveLoopVerdict],
    packets: &[HiveLoopPacket],
    admissions: &[HiveLoopAdmission],
    evidence: &[HiveLoopEvidence],
) -> Vec<serde_json::Value> {
    let packets_by_id = packets
        .iter()
        .map(|packet| (packet.id, packet))
        .collect::<HashMap<_, _>>();
    let admissions_by_id = admissions
        .iter()
        .map(|admission| (admission.id, admission))
        .collect::<HashMap<_, _>>();
    let evidence_by_id = evidence
        .iter()
        .map(|row| (row.id, row))
        .collect::<HashMap<_, _>>();
    let mut errors = Vec::new();

    for verdict in verdicts {
        let reason_code = verdict
            .evidence
            .get("reason_code")
            .and_then(|value| value.as_str());
        let verdict_errors = if reason_code == Some("admission_rejected") {
            admission_rejection_verdict_binding_errors(
                contract,
                verdict,
                &packets_by_id,
                &admissions_by_id,
                &evidence_by_id,
            )
        } else {
            standard_verdict_binding_errors(verdict, verdicts, packets, evidence)
        };
        if !verdict_errors.is_empty() {
            errors.push(serde_json::json!({
                "scope": "verdict_evidence",
                "verdict_id": verdict.id,
                "round": verdict.round,
                "decision": verdict.decision,
                "reason_code": reason_code,
                "errors": verdict_errors
            }));
        }
    }

    errors.sort_by_key(|error| {
        (
            error
                .get("round")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
            error
                .get("verdict_id")
                .and_then(|value| value.as_i64())
                .unwrap_or_default(),
        )
    });
    errors
}

fn standard_verdict_binding_errors(
    verdict: &HiveLoopVerdict,
    verdicts: &[HiveLoopVerdict],
    packets: &[HiveLoopPacket],
    evidence: &[HiveLoopEvidence],
) -> Vec<String> {
    let mut errors = Vec::new();
    let round_stage_evidence = evidence
        .iter()
        .filter(|row| row.round == verdict.round && stage_bound_evidence_kind(&row.kind))
        .collect::<Vec<_>>();
    let expected_evidence_count = round_stage_evidence.len() as i64;
    match verdict
        .evidence
        .get("evidence_count")
        .and_then(|value| value.as_i64())
    {
        Some(count) if count == expected_evidence_count => {}
        _ => errors.push("evidence.count".to_string()),
    }

    let evidence_before_verdict = round_stage_evidence
        .iter()
        .filter(|row| row.kind != "verdict_packet")
        .count() as i64;
    match verdict
        .evidence
        .pointer("/source/round_evidence_before_verdict")
        .and_then(|value| value.as_i64())
    {
        Some(count) if count == evidence_before_verdict => {}
        _ => errors.push("evidence.source_round_count".to_string()),
    }

    let score_runtime_ready = verdict
        .score
        .pointer("/gate_results/runtime_ready")
        .and_then(|value| value.as_bool());
    let evidence_runtime_ready = verdict
        .evidence
        .get("runtime_ready")
        .and_then(|value| value.as_bool());
    match (evidence_runtime_ready, score_runtime_ready) {
        (Some(evidence_value), Some(score_value)) if evidence_value == score_value => {}
        _ => errors.push("evidence.runtime_ready".to_string()),
    }
    let score_invalid_rounds_used = verdict
        .score
        .get("reviewer_invalid_rounds_used")
        .and_then(|value| value.as_i64());
    let score_gate_invalid_rounds_used = verdict
        .score
        .pointer("/gate_results/reviewer_invalid_rounds_used")
        .and_then(|value| value.as_i64());
    let evidence_invalid_rounds_used = verdict
        .evidence
        .get("reviewer_invalid_rounds_used")
        .and_then(|value| value.as_i64());
    let (expected_invalid_rounds_used, expected_budget_exhausted) =
        expected_reviewer_budget_for_verdict(verdicts, verdict);
    match (score_invalid_rounds_used, evidence_invalid_rounds_used) {
        (Some(score_value), Some(evidence_value)) if score_value == evidence_value => {}
        _ => errors.push("evidence.reviewer_invalid_rounds_used".to_string()),
    }
    match (score_invalid_rounds_used, score_gate_invalid_rounds_used) {
        (Some(score_value), Some(gate_value)) if score_value == gate_value => {}
        _ => errors.push("score.gate_results.reviewer_invalid_rounds_used".to_string()),
    }
    if score_invalid_rounds_used != Some(expected_invalid_rounds_used)
        || evidence_invalid_rounds_used != Some(expected_invalid_rounds_used)
        || score_gate_invalid_rounds_used != Some(expected_invalid_rounds_used)
    {
        errors.push("reviewer_budget.rounds_used_binding".to_string());
    }
    let score_budget_exhausted = verdict
        .score
        .get("reviewer_invalid_budget_exhausted")
        .and_then(|value| value.as_bool());
    let score_gate_budget_exhausted = verdict
        .score
        .pointer("/gate_results/reviewer_invalid_budget_exhausted")
        .and_then(|value| value.as_bool());
    let evidence_budget_exhausted = verdict
        .evidence
        .get("reviewer_invalid_budget_exhausted")
        .and_then(|value| value.as_bool());
    match (score_budget_exhausted, evidence_budget_exhausted) {
        (Some(score_value), Some(evidence_value)) if score_value == evidence_value => {}
        _ => errors.push("evidence.reviewer_invalid_budget_exhausted".to_string()),
    }
    match (score_budget_exhausted, score_gate_budget_exhausted) {
        (Some(score_value), Some(gate_value)) if score_value == gate_value => {}
        _ => errors.push("score.gate_results.reviewer_invalid_budget_exhausted".to_string()),
    }
    if score_budget_exhausted != Some(expected_budget_exhausted)
        || evidence_budget_exhausted != Some(expected_budget_exhausted)
        || score_gate_budget_exhausted != Some(expected_budget_exhausted)
    {
        errors.push("reviewer_budget.exhausted_binding".to_string());
    }
    let reason_code = verdict_reason_code(verdict);
    if expected_budget_exhausted {
        if verdict.decision != "blocked" {
            errors.push("reviewer_budget.decision_binding".to_string());
        }
        if reason_code.as_deref() != Some("review_budget_exhausted") {
            errors.push("reviewer_budget.reason_binding".to_string());
        }
    } else if reason_code.as_deref() == Some("review_budget_exhausted") {
        errors.push("reviewer_budget.reason_binding".to_string());
    }

    let reviewer_packet = packets.iter().find(|packet| {
        packet.round == verdict.round
            && packet.object_kind == "VERDICT_PACKET"
            && matches!(packet.writer_role.as_str(), "reviewer" | "evaluator")
    });
    let expected_worker = reviewer_packet.and_then(|packet| packet_role_worker(&packet.payload));
    match (verdict.evidence.get("role_worker"), expected_worker) {
        (Some(actual), Some(expected)) if actual == expected => {}
        (Some(_), Some(_)) => errors.push("evidence.role_worker_binding".to_string()),
        _ => errors.push("evidence.role_worker".to_string()),
    }
    let source_reviewer = verdict
        .evidence
        .pointer("/source/reviewer")
        .and_then(|value| value.as_str());
    let source_evaluator = verdict
        .evidence
        .pointer("/source/evaluator")
        .and_then(|value| value.as_str());
    if source_reviewer != Some("hive-loop-control") && source_evaluator != Some("hive-loop-control")
    {
        errors.push("evidence.source_reviewer".to_string());
    }

    errors
}

fn expected_reviewer_budget_for_verdict(
    verdicts: &[HiveLoopVerdict],
    verdict: &HiveLoopVerdict,
) -> (i64, bool) {
    let prior_round = verdict.round.saturating_sub(1);
    let prior_invalid_rounds = reviewer_invalid_streak_from_verdicts(verdicts, prior_round)
        .min(REVIEWER_INVALID_ROUND_BUDGET);
    let reason_code = verdict_reason_code(verdict);
    let current_invalid =
        verdict.decision == "reject" || reason_code.as_deref() == Some("review_budget_exhausted");
    if !current_invalid {
        return (0, false);
    }

    let current_invalid_rounds = (prior_invalid_rounds + 1).min(REVIEWER_INVALID_ROUND_BUDGET);
    let budget_exhausted = prior_invalid_rounds + 1 >= REVIEWER_INVALID_ROUND_BUDGET;
    (current_invalid_rounds, budget_exhausted)
}

fn admission_rejection_verdict_binding_errors(
    contract: &HiveLoopContract,
    verdict: &HiveLoopVerdict,
    packets_by_id: &HashMap<i64, &HiveLoopPacket>,
    admissions_by_id: &HashMap<i64, &HiveLoopAdmission>,
    evidence_by_id: &HashMap<i64, &HiveLoopEvidence>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let evidence_id = verdict
        .evidence
        .get("evidence_id")
        .and_then(|value| value.as_i64());
    let admission_id = verdict
        .evidence
        .get("admission_id")
        .and_then(|value| value.as_i64());
    let packet_id = verdict
        .evidence
        .get("packet_id")
        .and_then(|value| value.as_i64());
    let phase = verdict
        .evidence
        .get("phase")
        .and_then(|value| value.as_str());

    let linked_evidence = evidence_id.and_then(|evidence_id| evidence_by_id.get(&evidence_id));
    match linked_evidence {
        Some(row) if row.kind == "admission_rejection" && row.round == verdict.round => {}
        Some(_) => errors.push("evidence.link".to_string()),
        None => errors.push("evidence.link".to_string()),
    }

    let linked_admission =
        admission_id.and_then(|admission_id| admissions_by_id.get(&admission_id));
    match linked_admission {
        Some(admission) if admission.result == "rejected" => {}
        Some(_) => errors.push("admission.result".to_string()),
        None => errors.push("admission.link".to_string()),
    }

    match (linked_admission, packet_id) {
        (Some(admission), Some(packet_id)) if admission.packet_id == packet_id => {}
        _ => errors.push("admission.packet_binding".to_string()),
    }
    if !packet_id.is_some_and(|packet_id| packets_by_id.contains_key(&packet_id)) {
        errors.push("packet.link".to_string());
    }

    if let Some(row) = linked_evidence {
        if row.payload.get("phase").and_then(|value| value.as_str()) != phase {
            errors.push("evidence.phase_binding".to_string());
        }
        if row
            .payload
            .get("admission_id")
            .and_then(|value| value.as_i64())
            != admission_id
        {
            errors.push("evidence.admission_binding".to_string());
        }
        if row
            .payload
            .get("packet_id")
            .and_then(|value| value.as_i64())
            != packet_id
        {
            errors.push("evidence.packet_binding".to_string());
        }
    }
    if contract.status == "blocked"
        && verdict.round == contract.current_round
        && phase != Some(contract.active_phase.as_str())
    {
        errors.push("evidence.phase".to_string());
    }
    if let Some(admission) = linked_admission {
        if verdict.evidence.pointer("/source/admission_receipt") != Some(&admission.policy) {
            errors.push("evidence.admission_receipt_binding".to_string());
        }
    }
    let source_reviewer = verdict
        .evidence
        .pointer("/source/reviewer")
        .and_then(|value| value.as_str());
    let source_evaluator = verdict
        .evidence
        .pointer("/source/evaluator")
        .and_then(|value| value.as_str());
    if source_reviewer != Some("hive-loop-control") && source_evaluator != Some("hive-loop-control")
    {
        errors.push("evidence.source_reviewer".to_string());
    }

    errors
}

fn verdict_score_vector_errors(score_vector: &serde_json::Value, decision: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for metric in VERDICT_SCORE_METRICS {
        let value = score_vector.get(*metric);
        if *metric == "runtime_readiness"
            && decision == "blocked"
            && value == Some(&serde_json::Value::Null)
        {
            continue;
        }
        match value.and_then(|value| value.as_f64()) {
            Some(value) if (0.0..=1.0).contains(&value) => {}
            _ => errors.push(format!("score.score_vector.{metric}")),
        }
    }
    errors
}

fn expected_human_options_for_decision(decision: &str) -> Vec<String> {
    match decision {
        "keep" => option_list(&["comment"]),
        "reject" => option_list(&["comment", "retry"]),
        "needs-review" => option_list(&["comment", "retry", "cancel"]),
        "blocked" => option_list(&["comment", "retry", "request-review", "cancel"]),
        _ => Vec::new(),
    }
}

#[derive(Default)]
struct IssueSurfaceAudit {
    comment_count: usize,
    action_count: usize,
    operator_evidence_count: usize,
    errors: Vec<serde_json::Value>,
}

fn issue_surface_audit(
    store: &Store,
    contract: &HiveLoopContract,
    issues: &[HiveIssue],
    evidence: &[HiveLoopEvidence],
) -> Result<IssueSurfaceAudit> {
    let mut audit = IssueSurfaceAudit::default();
    let mut comments_by_id = HashMap::new();
    if issues.is_empty() {
        audit.errors.push(serde_json::json!({
            "scope": "loop",
            "loop_id": contract.id,
            "errors": ["issue.missing"]
        }));
    }

    for issue in issues {
        let mut issue_errors = Vec::new();
        if issue.loop_id != Some(contract.id) {
            issue_errors.push("issue.loop_id".to_string());
        }
        if !issue_status_allowed(&issue.status) {
            issue_errors.push("issue.status".to_string());
        }
        let expected_status = issue_status_for_contract_status(&contract.status);
        if expected_status.is_some_and(|expected| issue.status != expected) {
            issue_errors.push("issue.contract_status_binding".to_string());
        }
        if issue.title.trim().is_empty() {
            issue_errors.push("issue.title".to_string());
        }

        let comments = store.list_hive_comments(issue.id)?;
        audit.comment_count += comments.len();
        if comments.is_empty() {
            issue_errors.push("comment.missing".to_string());
        }
        if !issue_errors.is_empty() {
            audit.errors.push(serde_json::json!({
                "scope": "issue",
                "issue_id": issue.id,
                "contract_status": contract.status,
                "expected_status": expected_status,
                "actual_status": issue.status,
                "errors": issue_errors
            }));
        }
        if let Some(loop_id) = issue.loop_id.filter(|loop_id| *loop_id == contract.id) {
            let trace = issue_trace_summary_without_audit(store, loop_id, Some(issue))?;
            let doctor = issue_doctor_summary(store, loop_id, issue, &trace)?;
            let actions = issue_actions(issue, Some(&trace), Some(&doctor));
            audit.action_count += actions.len();
            if let Some(error) = issue_action_audit_error(issue, contract, &trace, &actions) {
                audit.errors.push(error);
            }
        }

        for comment in &comments {
            comments_by_id.insert(comment.id, comment.clone());
            if let Some(error) = issue_comment_audit_error(&comment, issue, evidence) {
                audit.errors.push(error);
            }
        }
    }

    for row in evidence
        .iter()
        .filter(|row| row.kind == "operator_comment" || row.kind == "operator_decision")
    {
        audit.operator_evidence_count += 1;
        if let Some(error) = operator_evidence_audit_error(row, issues, &comments_by_id) {
            audit.errors.push(error);
        }
    }

    Ok(audit)
}

fn issue_transition_policy_audit_errors(
    store: &Store,
    issues: &[HiveIssue],
) -> Result<Vec<serde_json::Value>> {
    let registry = issue_transition_policy_registry();
    let registry_actions = registry
        .actions
        .iter()
        .map(|action| action.action.clone())
        .collect::<BTreeSet<_>>();
    let mut errors = Vec::new();

    for issue in issues {
        let trace = match issue
            .loop_id
            .map(|loop_id| issue_trace_summary_without_audit(store, loop_id, Some(issue)))
            .transpose()
        {
            Ok(trace) => trace,
            Err(error) => {
                errors.push(serde_json::json!({
                    "issue_id": issue.id,
                    "scope": "issue_transition_policy",
                    "errors": ["trace.load"],
                    "error": error.to_string()
                }));
                continue;
            }
        };
        let actions = issue_actions(issue, trace.as_ref(), None);
        let state_class = issue_transition_state_class(issue).to_string();
        let allowed_actions = actions
            .iter()
            .map(|action| issue_transition_policy_action(issue, action))
            .collect::<Vec<_>>();
        let blocked_actions = issue_transition_blocked_actions(issue, &actions);
        let confirmation = issue_transition_confirmation_policy(&actions);
        let reviewer_budget = trace
            .as_ref()
            .map(issue_transition_reviewer_budget_from_trace);

        let mut issue_errors = Vec::new();
        if !registry
            .state_classes
            .iter()
            .any(|state| state.class == state_class && state.statuses.contains(&issue.status))
        {
            issue_errors.push("state_class".to_string());
        }
        let allowed = allowed_actions
            .iter()
            .map(|action| action.action.action.clone())
            .collect::<BTreeSet<_>>();
        let blocked = blocked_actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<BTreeSet<_>>();
        let union = allowed.union(&blocked).cloned().collect::<BTreeSet<_>>();
        if union != registry_actions {
            issue_errors.push("action.coverage".to_string());
        }
        if !allowed
            .intersection(&blocked)
            .collect::<Vec<_>>()
            .is_empty()
        {
            issue_errors.push("action.overlap".to_string());
        }
        for action in &allowed_actions {
            issue_errors.extend(
                issue_transition_allowed_action_policy_errors(issue, action, &registry)
                    .into_iter()
                    .map(|field| format!("allowed.{}.{}", action.action.action, field)),
            );
        }
        for action in &blocked_actions {
            if !registry_actions.contains(&action.action) {
                issue_errors.push(format!("blocked.{}.unknown", action.action));
            }
        }
        if confirmation.confirmation_arg != registry.confirmation.confirmation_arg {
            issue_errors.push("confirmation.arg".to_string());
        }
        if confirmation.receipt_schema != registry.confirmation.receipt_schema {
            issue_errors.push("confirmation.receipt_schema".to_string());
        }
        if confirmation.policy_schema_version != registry.confirmation.policy_schema_version {
            issue_errors.push("confirmation.policy_schema_version".to_string());
        }
        if let Some(budget) = reviewer_budget.as_ref() {
            if budget.reviewer_invalid_round_budget
                != registry.reviewer_fallback.invalid_round_budget
            {
                issue_errors.push("reviewer_budget.invalid_round_budget".to_string());
            }
            if budget.fallback_status != registry.reviewer_fallback.fallback_status {
                issue_errors.push("reviewer_budget.fallback_status".to_string());
            }
        }

        if !issue_errors.is_empty() {
            issue_errors.sort();
            issue_errors.dedup();
            errors.push(serde_json::json!({
                "issue_id": issue.id,
                "status": issue.status,
                "state_class": state_class,
                "allowed_actions": allowed,
                "blocked_actions": blocked,
                "errors": issue_errors
            }));
        }
    }

    Ok(errors)
}

fn issue_transition_allowed_action_policy_errors(
    issue: &HiveIssue,
    action: &IssueTransitionPolicyAction,
    registry: &IssueTransitionPolicyRegistry,
) -> Vec<String> {
    let Some(policy) = registry
        .actions
        .iter()
        .find(|policy| policy.action == action.action.action)
    else {
        return vec!["unknown".to_string()];
    };
    let mut errors = Vec::new();
    if !policy.from_statuses.contains(&issue.status) {
        errors.push("from_status".to_string());
    }
    if action.gate != policy.gate {
        errors.push("gate".to_string());
    }
    if action.requires_human != policy.requires_confirmation {
        errors.push("requires_human".to_string());
    }
    if action.action.label != policy.label {
        errors.push("label".to_string());
    }
    if action.action.input != policy.input {
        errors.push("input".to_string());
    }
    if action.action.destructive != policy.destructive {
        errors.push("destructive".to_string());
    }
    if action.action.confirmation_required != policy.requires_confirmation {
        errors.push("confirmation_required".to_string());
    }
    let expected_to_status = if policy.to_status == "same_status" {
        issue.status.clone()
    } else {
        policy.to_status.clone()
    };
    if action.to_status.as_deref() != Some(expected_to_status.as_str()) {
        errors.push("to_status".to_string());
    }
    errors
}

fn issue_status_for_contract_status(status: &str) -> Option<&'static str> {
    match status {
        "todo" => Some("Todo"),
        "running" => Some("Doing"),
        "blocked" => Some("Blocked"),
        "needs-review" => Some("Needs Review"),
        "rejected" => Some("Canceled"),
        "kept" => Some("Done"),
        _ => None,
    }
}

fn issue_action_audit_error(
    issue: &HiveIssue,
    contract: &HiveLoopContract,
    trace: &IssueTraceSummary,
    actions: &[IssueAction],
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let mut expected_actions = Vec::new();
    if issue.loop_id == Some(contract.id) && issue.status == "Todo" {
        expected_actions.push("run".to_string());
    }
    expected_actions.extend(trace.human_options.clone());
    let action_names = actions
        .iter()
        .map(|action| action.action.clone())
        .collect::<Vec<_>>();
    if action_names != expected_actions {
        errors.push("action.sequence".to_string());
    }
    let mut seen = HashMap::new();
    for action in actions {
        *seen.entry(action.action.as_str()).or_insert(0usize) += 1;
        issue_action_field_errors(issue, contract, action, &expected_actions, &mut errors);
    }
    if actions.is_empty() {
        errors.push("action.missing".to_string());
    }
    if seen.values().any(|count| *count > 1) {
        errors.push("action.duplicate".to_string());
    }

    if errors.is_empty() {
        return None;
    }
    errors.sort();
    errors.dedup();
    Some(serde_json::json!({
        "scope": "issue_action",
        "issue_id": issue.id,
        "status": issue.status,
        "expected_actions": expected_actions,
        "actual_actions": action_names,
        "errors": errors
    }))
}

fn issue_action_field_errors(
    issue: &HiveIssue,
    contract: &HiveLoopContract,
    action: &IssueAction,
    expected_actions: &[String],
    errors: &mut Vec<String>,
) {
    let policy = issue_transition_action_policy(&action.action);
    if action.schema_version != ISSUE_ACTION_SCHEMA_VERSION {
        errors.push("action.schema_version".to_string());
    }
    if policy.is_none() {
        errors.push("action.name".to_string());
    }
    if !expected_actions
        .iter()
        .any(|expected| expected == &action.action)
    {
        errors.push("action.unexpected".to_string());
    }
    let expected_label = policy.as_ref().map(|policy| policy.label.as_str());
    if expected_label.is_some_and(|label| action.label != label) {
        errors.push("action.label".to_string());
    }
    let expected_source = if action.action == "run" {
        "runtime"
    } else {
        "human_options"
    };
    if action.source != expected_source {
        errors.push("action.source".to_string());
    }
    let expected_input = policy.as_ref().map(|policy| policy.input.as_str());
    if expected_input.is_some_and(|input| action.input != input) {
        errors.push("action.input".to_string());
    }
    if policy
        .as_ref()
        .is_some_and(|policy| action.destructive != policy.destructive)
    {
        errors.push("action.destructive".to_string());
    }
    let confirmation_required = policy
        .as_ref()
        .map(|policy| policy.requires_confirmation)
        .unwrap_or_else(|| issue_action_requires_confirmation(&action.action));
    if action.confirmation_required != confirmation_required {
        errors.push("action.confirmation_required".to_string());
    }
    if confirmation_required {
        if action.confirmation_arg.as_deref() != Some(OPERATOR_ACTION_CONFIRMATION_ARG) {
            errors.push("action.confirmation_arg".to_string());
        }
        if action.receipt_schema.as_deref() != Some(OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION) {
            errors.push("action.receipt_schema".to_string());
        }
        if action.policy_schema_version.as_deref() != Some(OPERATOR_ACTION_POLICY_SCHEMA_VERSION) {
            errors.push("action.policy_schema_version".to_string());
        }
    } else {
        if action.confirmation_arg.is_some() {
            errors.push("action.confirmation_arg".to_string());
        }
        if action.receipt_schema.is_some() {
            errors.push("action.receipt_schema".to_string());
        }
        if action.policy_schema_version.is_some() {
            errors.push("action.policy_schema_version".to_string());
        }
    }
    match action.action.as_str() {
        "run" => {
            if action.runtime.as_deref() != Some(contract.runtime.as_str()) {
                errors.push("action.runtime".to_string());
            }
            if !action
                .command
                .starts_with(&format!("entrance hive issue run {}", issue.id))
                || !action.command.contains("--compact")
                || !action
                    .command
                    .contains(&format!("--runtime {}", contract.runtime))
            {
                errors.push("action.command".to_string());
            }
        }
        "comment" => {
            if action.runtime.is_some() {
                errors.push("action.runtime".to_string());
            }
            if action.command
                != format!(
                    "entrance hive issue comment {} --body <text> --compact",
                    issue.id
                )
            {
                errors.push("action.command".to_string());
            }
        }
        "retry" => {
            if action.runtime.as_deref() != Some(contract.runtime.as_str()) {
                errors.push("action.runtime".to_string());
            }
            if !action
                .command
                .starts_with(&format!("entrance hive issue retry-run {}", issue.id))
                || !action.command.contains("--body <note>")
                || !action.command.contains("--human-confirmed")
                || !action.command.contains("--compact")
            {
                errors.push("action.command".to_string());
            }
            if contract.runtime == "codex"
                && (!action.command.contains("--runtime codex")
                    || !action.command.contains("--worker-attempts 2"))
            {
                errors.push("action.command".to_string());
            }
        }
        "request-review" => {
            if action.runtime.is_some() {
                errors.push("action.runtime".to_string());
            }
            if action.command
                != format!(
                    "entrance hive issue decide {} request-review --body <note> --human-confirmed --compact",
                    issue.id
                )
            {
                errors.push("action.command".to_string());
            }
        }
        "cancel" => {
            if action.runtime.is_some() {
                errors.push("action.runtime".to_string());
            }
            if action.command
                != format!(
                    "entrance hive issue decide {} cancel --body <note> --human-confirmed --compact",
                    issue.id
                )
            {
                errors.push("action.command".to_string());
            }
        }
        _ => {}
    }
}

fn issue_comment_audit_error(
    comment: &HiveComment,
    issue: &HiveIssue,
    evidence: &[HiveLoopEvidence],
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    if comment.issue_id != issue.id {
        errors.push("comment.issue_id".to_string());
    }
    if comment.author.trim().is_empty() {
        errors.push("comment.author".to_string());
    }
    if comment.body.trim().is_empty() {
        errors.push("comment.body".to_string());
    }
    let schema = schema_version(&comment.payload);
    let source = comment
        .payload
        .get("source")
        .and_then(|value| value.as_str());
    let expected_schema = expected_comment_schema(comment);
    if source.is_none() {
        errors.push("comment.payload.source".to_string());
    }
    if !comment_schema_allowed(schema.as_deref()) {
        errors.push("comment.payload.schema_version".to_string());
    }
    if expected_schema.is_some() && schema.as_deref() != expected_schema {
        errors.push("comment.payload.schema_binding".to_string());
    }
    match expected_schema {
        Some(SYSTEM_COMMENT_SCHEMA_VERSION) => {
            errors.extend(system_comment_audit_errors(comment, issue, evidence));
        }
        Some(OPERATOR_COMMENT_SCHEMA_VERSION) => {
            match comment.payload.get("transition_admission") {
                Some(admission) => errors.extend(issue_transition_admission_audit_errors(
                    admission,
                    Some("comment"),
                    Some(false),
                )),
                None => errors.push("comment.transition_admission".to_string()),
            }
            if !evidence.iter().any(|row| {
                row.kind == "operator_comment"
                    && row
                        .payload
                        .pointer("/issue/comment_id")
                        .and_then(|value| value.as_i64())
                        == Some(comment.id)
            }) {
                errors.push("comment.operator_evidence".to_string());
            }
        }
        Some(OPERATOR_DECISION_SCHEMA_VERSION) => {
            let action = comment
                .payload
                .get("action")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            if action.is_none() {
                errors.push("comment.payload.action".to_string());
            }
            match comment.payload.get("transition_admission") {
                Some(admission) => errors.extend(issue_transition_admission_audit_errors(
                    admission,
                    action.as_deref(),
                    Some(true),
                )),
                None => errors.push("comment.transition_admission".to_string()),
            }
            if let Some(receipt) = comment.payload.get("confirmation_receipt") {
                errors.extend(operator_confirmation_receipt_audit_errors(
                    receipt,
                    action.as_deref(),
                    &comment.author,
                ));
            } else {
                errors.push("comment.confirmation_receipt.missing".to_string());
            }
            if !evidence.iter().any(|row| {
                row.kind == "operator_decision"
                    && row
                        .payload
                        .pointer("/issue/comment_id")
                        .and_then(|value| value.as_i64())
                        == Some(comment.id)
            }) {
                errors.push("comment.operator_evidence".to_string());
            }
        }
        _ => {}
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "scope": "comment",
            "issue_id": issue.id,
            "comment_id": comment.id,
            "author": comment.author,
            "source": source,
            "schema_version": schema,
            "errors": errors
        }))
    }
}

fn system_comment_audit_errors(
    comment: &HiveComment,
    issue: &HiveIssue,
    evidence: &[HiveLoopEvidence],
) -> Vec<String> {
    let mut errors = Vec::new();
    let payload = &comment.payload;

    if let Some(loop_id) = issue.loop_id {
        let comment_loop_id = payload.get("loop_id").and_then(|value| value.as_i64());
        if comment_loop_id != Some(loop_id) {
            errors.push("comment.payload.loop_id".to_string());
        }
    }

    let has_stage_fields = ["stage_role", "evidence_kind", "worker"]
        .iter()
        .any(|field| payload.get(*field).is_some());
    if !has_stage_fields {
        return errors;
    }

    let round = payload.get("round").and_then(|value| value.as_i64());
    if !round.is_some_and(|round| round >= 1) {
        errors.push("comment.stage.round".to_string());
    }

    let stage_role = payload.get("stage_role").and_then(|value| value.as_str());
    let valid_stage_role = stage_role.filter(|role| canonical_stage_roles().contains(role));
    if valid_stage_role.is_none() {
        errors.push("comment.stage.role".to_string());
    }

    let evidence_kind = payload
        .get("evidence_kind")
        .and_then(|value| value.as_str());
    if let Some(role) = valid_stage_role {
        if canonical_stage_evidence_kind(role) != evidence_kind {
            errors.push("comment.stage.evidence_kind".to_string());
        }
    } else if evidence_kind.is_none() {
        errors.push("comment.stage.evidence_kind".to_string());
    }
    let evidence_id = payload.get("evidence_id").and_then(|value| value.as_i64());
    if !evidence_id.is_some_and(|id| id > 0) {
        errors.push("comment.stage.evidence_id".to_string());
    }

    let admission = payload.get("admission").and_then(|value| value.as_str());
    if admission != Some("admitted") {
        errors.push("comment.stage.admission".to_string());
    }

    let worker = payload.get("worker");
    if !worker.is_some_and(|worker| worker.is_object()) {
        errors.push("comment.stage.worker".to_string());
    }
    if let (Some(worker), Some(role)) = (worker, valid_stage_role) {
        if worker.get("role").and_then(|value| value.as_str()) != Some(role) {
            errors.push("comment.stage.worker_role".to_string());
        }
    }

    if let (Some(round), Some(evidence_id), Some(evidence_kind), Some(admission), Some(worker)) =
        (round, evidence_id, evidence_kind, admission, worker)
    {
        let has_evidence_binding = evidence.iter().any(|row| {
            row.id == evidence_id
                && row.round == round
                && row.kind == evidence_kind
                && row
                    .payload
                    .get("admission")
                    .and_then(|value| value.as_str())
                    == Some(admission)
                && row.payload.get("worker") == Some(worker)
        });
        if !has_evidence_binding {
            errors.push("comment.stage.evidence_binding".to_string());
        }
    }

    errors
}

fn operator_confirmation_receipt_audit_errors(
    receipt: &serde_json::Value,
    action: Option<&str>,
    author: &str,
) -> Vec<String> {
    let mut errors = Vec::new();
    if receipt
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION)
    {
        errors.push("comment.confirmation_receipt.schema_version".to_string());
    }
    if receipt
        .get("source")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("comment.confirmation_receipt.source".to_string());
    }
    if receipt
        .get("policy_schema_version")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("comment.confirmation_receipt.policy_schema_version".to_string());
    }
    if receipt
        .get("confirmation_arg")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("comment.confirmation_receipt.confirmation_arg".to_string());
    }
    if receipt
        .get("human_confirmed")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        errors.push("comment.confirmation_receipt.human_confirmed".to_string());
    }
    if receipt.get("action").and_then(|value| value.as_str()) != action {
        errors.push("comment.confirmation_receipt.action".to_string());
    }
    if receipt.get("author").and_then(|value| value.as_str()) != Some(author) {
        errors.push("comment.confirmation_receipt.author".to_string());
    }
    if receipt
        .get("marker")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("comment.confirmation_receipt.marker".to_string());
    }
    if let Some(client) = receipt.get("client") {
        if !client.is_object() {
            errors.push("comment.confirmation_receipt.client".to_string());
        } else {
            if client
                .get("name")
                .and_then(|value| value.as_str())
                .is_none_or(|value| value.trim().is_empty())
            {
                errors.push("comment.confirmation_receipt.client.name".to_string());
            }
            if client
                .get("source")
                .and_then(|value| value.as_str())
                .is_none_or(|value| value.trim().is_empty())
            {
                errors.push("comment.confirmation_receipt.client.source".to_string());
            }
            if client
                .get("version")
                .and_then(|value| value.as_str())
                .is_some_and(|value| value.trim().is_empty())
            {
                errors.push("comment.confirmation_receipt.client.version".to_string());
            }
        }
    }
    if let Some(actor) = receipt.get("actor") {
        if !actor.is_object() {
            errors.push("comment.confirmation_receipt.actor".to_string());
        } else {
            for field in ["id", "label", "source", "trust"] {
                if actor
                    .get(field)
                    .and_then(|value| value.as_str())
                    .is_none_or(|value| value.trim().is_empty())
                {
                    errors.push(format!("comment.confirmation_receipt.actor.{field}"));
                }
            }
            if actor
                .get("verified")
                .and_then(|value| value.as_bool())
                .is_none()
            {
                errors.push("comment.confirmation_receipt.actor.verified".to_string());
            }
        }
    }
    errors
}

fn issue_transition_admission_audit_errors(
    admission: &serde_json::Value,
    action: Option<&str>,
    requires_confirmation: Option<bool>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if admission
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some(ISSUE_TRANSITION_ADMISSION_SCHEMA_VERSION)
    {
        errors.push("transition_admission.schema_version".to_string());
    }
    if admission
        .get("policy_schema_version")
        .and_then(|value| value.as_str())
        != Some(POLICY_SCHEMA_VERSION)
    {
        errors.push("transition_admission.policy_schema_version".to_string());
    }
    if admission
        .get("policy_owner")
        .and_then(|value| value.as_str())
        != Some("hive-kernel")
    {
        errors.push("transition_admission.policy_owner".to_string());
    }
    if admission
        .get("policy_scope")
        .and_then(|value| value.as_str())
        != Some("issue.status.transition")
    {
        errors.push("transition_admission.policy_scope".to_string());
    }
    if admission.get("action").and_then(|value| value.as_str()) != action {
        errors.push("transition_admission.action".to_string());
    }
    if admission
        .get("gate")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("transition_admission.gate".to_string());
    }
    if admission.get("result").and_then(|value| value.as_str()) != Some("admitted") {
        errors.push("transition_admission.result".to_string());
    }
    if admission
        .get("from_status")
        .and_then(|value| value.as_str())
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("transition_admission.from_status".to_string());
    }
    if let Some(expected) = requires_confirmation {
        if admission
            .get("requires_confirmation")
            .and_then(|value| value.as_bool())
            != Some(expected)
        {
            errors.push("transition_admission.requires_confirmation".to_string());
        }
    }
    if !admission
        .get("allowed_actions")
        .and_then(|value| value.as_array())
        .is_some_and(|values| {
            !values.is_empty()
                && values
                    .iter()
                    .all(|value| value.as_str().is_some_and(|item| !item.trim().is_empty()))
        })
    {
        errors.push("transition_admission.allowed_actions".to_string());
    }
    errors
}

fn operator_evidence_audit_error(
    row: &HiveLoopEvidence,
    issues: &[HiveIssue],
    comments_by_id: &HashMap<i64, HiveComment>,
) -> Option<serde_json::Value> {
    let mut errors = Vec::new();
    let expected_schema = match row.kind.as_str() {
        "operator_comment" => OPERATOR_COMMENT_SCHEMA_VERSION,
        "operator_decision" => OPERATOR_DECISION_SCHEMA_VERSION,
        _ => return None,
    };
    if row.payload.get("source").and_then(|value| value.as_str()) != Some("issue/status/comment") {
        errors.push("evidence.source".to_string());
    }
    if schema_version(&row.payload).as_deref() != Some(expected_schema) {
        errors.push("evidence.schema_version".to_string());
    }
    let issue_id = row
        .payload
        .pointer("/issue/id")
        .and_then(|value| value.as_i64());
    if !issue_id.is_some_and(|issue_id| issues.iter().any(|issue| issue.id == issue_id)) {
        errors.push("evidence.issue_id".to_string());
    }
    let comment_id = row
        .payload
        .pointer("/issue/comment_id")
        .and_then(|value| value.as_i64());
    if comment_id.is_none() {
        errors.push("evidence.comment_id".to_string());
    }
    let linked_comment = comment_id.and_then(|comment_id| comments_by_id.get(&comment_id));
    if comment_id.is_some() && linked_comment.is_none() {
        errors.push("evidence.comment_link".to_string());
    }
    if row
        .payload
        .pointer("/loop/id")
        .and_then(|value| value.as_i64())
        != Some(row.loop_id)
    {
        errors.push("evidence.loop_id_binding".to_string());
    }
    let evidence_round = row
        .payload
        .pointer("/loop/round")
        .and_then(|value| value.as_i64());
    if evidence_round != Some(row.round) {
        errors.push("evidence.loop_round_binding".to_string());
    }

    if let Some(comment) = linked_comment {
        if issue_id != Some(comment.issue_id) {
            errors.push("evidence.comment_issue_id".to_string());
        }
        if expected_comment_schema(comment) != Some(expected_schema) {
            errors.push("evidence.comment_schema_binding".to_string());
        }
        if row
            .payload
            .pointer("/operator/author")
            .and_then(|value| value.as_str())
            != Some(comment.author.as_str())
        {
            errors.push("evidence.author_binding".to_string());
        }
        if row
            .payload
            .pointer("/operator/comment_body")
            .and_then(|value| value.as_str())
            != Some(comment.body.as_str())
        {
            errors.push("evidence.comment_body_binding".to_string());
        }
        if comment
            .payload
            .get("loop_id")
            .and_then(|value| value.as_i64())
            != Some(row.loop_id)
        {
            errors.push("evidence.comment_loop_binding".to_string());
        }
        if row.kind == "operator_comment" {
            if comment.payload.get("transition_admission")
                != row.payload.get("transition_admission")
            {
                errors.push("evidence.transition_admission_binding".to_string());
            }
            if let Some(admission) = row.payload.get("transition_admission") {
                errors.extend(issue_transition_admission_audit_errors(
                    admission,
                    Some("comment"),
                    Some(false),
                ));
                errors.extend(operator_transition_admission_binding_errors(
                    row,
                    admission,
                    Some("comment"),
                ));
            } else {
                errors.push("evidence.transition_admission".to_string());
            }
            if comment
                .payload
                .get("round")
                .and_then(|value| value.as_i64())
                != evidence_round
            {
                errors.push("evidence.comment_round_binding".to_string());
            }
            if comment
                .payload
                .get("status")
                .and_then(|value| value.as_str())
                != row
                    .payload
                    .pointer("/issue/status")
                    .and_then(|value| value.as_str())
            {
                errors.push("evidence.comment_status_binding".to_string());
            }
            if comment
                .payload
                .get("phase")
                .and_then(|value| value.as_str())
                != row
                    .payload
                    .pointer("/loop/phase")
                    .and_then(|value| value.as_str())
            {
                errors.push("evidence.comment_phase_binding".to_string());
            }
        }
        if row.kind == "operator_decision" {
            let evidence_action = row
                .payload
                .pointer("/operator/action")
                .and_then(|value| value.as_str());
            let comment_action = comment
                .payload
                .get("action")
                .and_then(|value| value.as_str());
            if evidence_action != comment_action {
                errors.push("evidence.action_binding".to_string());
            }
            if comment.payload.get("confirmation_receipt")
                != row.payload.pointer("/operator/confirmation_receipt")
            {
                errors.push("evidence.confirmation_receipt_binding".to_string());
            }
            if comment.payload.get("transition_admission")
                != row.payload.get("transition_admission")
            {
                errors.push("evidence.transition_admission_binding".to_string());
            }
            if comment
                .payload
                .get("next_round")
                .and_then(|value| value.as_i64())
                != evidence_round
            {
                errors.push("evidence.comment_next_round_binding".to_string());
            }
            match evidence_action.map(parse_issue_decision_action) {
                Some(Ok(action)) => {
                    if let Some(admission) = row.payload.get("transition_admission") {
                        errors.extend(issue_transition_admission_audit_errors(
                            admission,
                            Some(action.as_str()),
                            Some(true),
                        ));
                        errors.extend(operator_transition_admission_binding_errors(
                            row,
                            admission,
                            Some(action.as_str()),
                        ));
                    } else {
                        errors.push("evidence.transition_admission".to_string());
                    }
                    if row
                        .payload
                        .pointer("/issue/to_status")
                        .and_then(|value| value.as_str())
                        != Some(action.issue_status())
                    {
                        errors.push("evidence.issue_status_binding".to_string());
                    }
                    if row
                        .payload
                        .pointer("/loop/next_status")
                        .and_then(|value| value.as_str())
                        != Some(action.contract_status())
                    {
                        errors.push("evidence.loop_status_binding".to_string());
                    }
                    if row
                        .payload
                        .pointer("/loop/next_phase")
                        .and_then(|value| value.as_str())
                        != Some(action.contract_phase())
                    {
                        errors.push("evidence.loop_phase_binding".to_string());
                    }
                    if action == IssueDecisionAction::Retry
                        && !evidence_round.is_some_and(|round| round > 1)
                    {
                        errors.push("evidence.retry_round".to_string());
                    }
                }
                _ => errors.push("evidence.action".to_string()),
            }
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "scope": "operator_evidence",
            "evidence_id": row.id,
            "kind": row.kind,
            "issue_id": issue_id,
            "errors": errors
        }))
    }
}

fn operator_transition_admission_binding_errors(
    row: &HiveLoopEvidence,
    admission: &serde_json::Value,
    action: Option<&str>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let issue_id = row
        .payload
        .pointer("/issue/id")
        .and_then(|value| value.as_i64());
    let expected_from_status = if row.kind == "operator_comment" {
        row.payload
            .pointer("/issue/status")
            .and_then(|value| value.as_str())
    } else {
        row.payload
            .pointer("/issue/from_status")
            .and_then(|value| value.as_str())
    };
    let expected_to_status = if row.kind == "operator_comment" {
        row.payload
            .pointer("/issue/status")
            .and_then(|value| value.as_str())
    } else {
        row.payload
            .pointer("/issue/to_status")
            .and_then(|value| value.as_str())
    };

    if admission
        .get("from_status")
        .and_then(|value| value.as_str())
        != expected_from_status
    {
        errors.push("transition_admission.from_status_binding".to_string());
    }
    if !admission_to_status_matches(
        admission.get("to_status").and_then(|value| value.as_str()),
        expected_to_status,
    ) {
        errors.push("transition_admission.to_status_binding".to_string());
    }
    if admission
        .get("policy_resource")
        .and_then(|value| value.as_str())
        != Some("entrance://policy/registry")
    {
        errors.push("transition_admission.policy_resource".to_string());
    }
    if let Some(issue_id) = issue_id {
        let expected_resource = format!("entrance://issues/{issue_id}/transition-policy");
        if admission
            .get("transition_policy_resource")
            .and_then(|value| value.as_str())
            != Some(expected_resource.as_str())
        {
            errors.push("transition_admission.transition_policy_resource".to_string());
        }
    }
    if let Some(action) = action {
        if !admission
            .get("allowed_actions")
            .and_then(|value| value.as_array())
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(action)))
        {
            errors.push("transition_admission.allowed_action_binding".to_string());
        }
    }

    errors
}

fn admission_to_status_matches(actual: Option<&str>, expected: Option<&str>) -> bool {
    match (actual, expected) {
        (Some(actual), Some(expected)) => {
            actual == expected || actual.split([',', '\n']).next().map(str::trim) == Some(expected)
        }
        (None, None) => true,
        _ => false,
    }
}

fn expected_comment_schema(comment: &HiveComment) -> Option<&'static str> {
    let source = comment
        .payload
        .get("source")
        .and_then(|value| value.as_str());
    let action = comment
        .payload
        .get("action")
        .and_then(|value| value.as_str());
    match source {
        Some("operator") if action.is_some() => Some(OPERATOR_DECISION_SCHEMA_VERSION),
        Some("operator") => Some(OPERATOR_COMMENT_SCHEMA_VERSION),
        Some("hive" | "compiler") => Some(SYSTEM_COMMENT_SCHEMA_VERSION),
        _ => None,
    }
}

fn comment_schema_allowed(schema: Option<&str>) -> bool {
    matches!(
        schema,
        Some(OPERATOR_COMMENT_SCHEMA_VERSION)
            | Some(OPERATOR_DECISION_SCHEMA_VERSION)
            | Some(SYSTEM_COMMENT_SCHEMA_VERSION)
    )
}

fn issue_status_allowed(value: &str) -> bool {
    matches!(
        value,
        "Todo" | "Doing" | "Blocked" | "Needs Review" | "Done" | "Canceled"
    )
}

fn decision_label_allowed(value: &str) -> bool {
    matches!(value, "keep" | "reject" | "needs-review" | "blocked")
}

