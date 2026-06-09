pub fn add_comment(store: &Store, request: IssueCommentRequest) -> Result<IssueCard> {
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let author = default_text(request.author, "human");
    let body = request.body.trim().to_string();
    if body.is_empty() {
        anyhow::bail!("hive issue comment requires a non-empty body");
    }
    let contract = issue
        .loop_id
        .map(|loop_id| store.get_hive_loop_contract(loop_id))
        .transpose()?
        .flatten();
    let transition_admission = issue_transition_admission(store, &issue, "comment")?;
    let transition_admission_value = serde_json::to_value(&transition_admission)?;
    let comment_id = store.insert_hive_comment(HiveCommentCreate {
        issue_id: request.issue_id,
        author: author.clone(),
        body: body.clone(),
        payload: serde_json::json!({
            "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
            "source": "operator",
            "loop_id": issue.loop_id,
            "round": contract.as_ref().map(|contract| contract.current_round),
            "status": issue.status,
            "phase": contract.as_ref().map(|contract| contract.active_phase.as_str()),
            "transition_admission": transition_admission_value
        }),
    })?;
    record_operator_comment_evidence(
        store,
        &issue,
        comment_id,
        &author,
        &body,
        &transition_admission,
    )?;

    issue_card_from_issue(store, issue)
}

pub fn decide_issue(store: &Store, request: IssueDecisionRequest) -> Result<IssueCard> {
    let action = parse_issue_decision_action(&request.action)?;
    let issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let transition_admission = issue_transition_admission(store, &issue, action.as_str())?;
    let author = default_text(request.author, "human");
    let note = request.body.as_deref().unwrap_or_default().trim();
    let receipt = request.confirmation_receipt.as_ref();
    if transition_admission.requires_confirmation && receipt.is_none() {
        anyhow::bail!(
            "issue transition `{}` requires an operator confirmation receipt; use MCP/Panel human_confirmed=true or pass --human-confirmed from the CLI",
            action.as_str()
        );
    }
    if let Some(receipt) = receipt {
        ensure_operator_confirmation_receipt(receipt, action, &author)?;
    }
    let confirmation_receipt = request
        .confirmation_receipt
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    let transition_admission_value = serde_json::to_value(&transition_admission)?;
    let mut next_round = None;

    if let Some(loop_id) = issue.loop_id {
        let contract = store
            .get_hive_loop_contract(loop_id)?
            .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
        let round = match action {
            IssueDecisionAction::Retry => contract.current_round + 1,
            IssueDecisionAction::RequestReview | IssueDecisionAction::Cancel => {
                contract.current_round
            }
        };
        store.update_hive_loop_contract_state(
            loop_id,
            action.contract_status(),
            action.contract_phase(),
            round,
        )?;
        next_round = Some(round);
    }

    let issue_summary = action.issue_summary(next_round);
    let comment_body = action.comment_body(next_round, note);

    store.update_hive_issue_status(issue.id, action.issue_status(), Some(&issue_summary))?;
    let mut comment_payload = serde_json::json!({
        "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
        "source": "operator",
        "action": action.as_str(),
        "loop_id": issue.loop_id,
        "next_round": next_round,
        "note": note,
        "transition_admission": transition_admission_value
    });
    if let Some(receipt) = confirmation_receipt.as_ref() {
        comment_payload["confirmation_receipt"] = receipt.clone();
    }
    let comment_id = store.insert_hive_comment(HiveCommentCreate {
        issue_id: issue.id,
        author: author.clone(),
        body: comment_body.clone(),
        payload: comment_payload,
    })?;
    record_operator_decision_evidence(
        store,
        &issue,
        action,
        comment_id,
        &author,
        note,
        next_round,
        &issue_summary,
        &comment_body,
        confirmation_receipt.as_ref(),
        &transition_admission,
    )?;

    issue_card(store, issue.id)
}

pub fn run_issue(store: &Store, request: IssueRunRequest) -> Result<HiveLoopReport> {
    let mut issue = store
        .get_hive_issue(request.issue_id)?
        .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    let loop_id = issue
        .loop_id
        .with_context(|| format!("hive issue #{} is not linked to a loop", issue.id))?;

    if request.retry {
        decide_issue(
            store,
            IssueDecisionRequest {
                issue_id: issue.id,
                action: "retry".to_string(),
                author: default_text(request.author, "human"),
                body: request.body.clone(),
                confirmation_receipt: request.confirmation_receipt.clone(),
            },
        )?;
        issue = store
            .get_hive_issue(request.issue_id)?
            .with_context(|| format!("unknown hive issue `{}`", request.issue_id))?;
    }
    if let Err(error) = issue_transition_admission(store, &issue, "run") {
        if !request.retry {
            anyhow::bail!(
                "hive issue run requires issue #{} to be admitted for `run`; current status is `{}`. Use `hive issue retry-run {}` to record a retry decision first. Policy admission: {error}",
                issue.id,
                issue.status,
                issue.id
            );
        }
        return Err(error);
    }

    run(
        store,
        HiveLoopRunRequest {
            loop_id,
            runtime: request.runtime,
            decision: request.decision,
            worker_timeout_secs: request.worker_timeout_secs,
            worker_attempts: request.worker_attempts,
        },
    )
}

fn issue_transition_admission(
    store: &Store,
    issue: &HiveIssue,
    action: &str,
) -> Result<IssueTransitionAdmissionReceipt> {
    let trace = issue
        .loop_id
        .map(|loop_id| issue_trace_summary_without_audit(store, loop_id, Some(issue)))
        .transpose()?;
    let actions = issue_actions(issue, trace.as_ref(), None);
    let allowed_actions = actions
        .iter()
        .map(|action| action.action.clone())
        .collect::<Vec<_>>();
    let Some(issue_action) = actions.iter().find(|candidate| candidate.action == action) else {
        let blocked_actions = issue_transition_blocked_actions(issue, &actions)
            .into_iter()
            .map(|blocked| blocked.action)
            .collect::<Vec<_>>();
        anyhow::bail!(
            "issue transition `{action}` is not admitted when issue #{} is `{}`; allowed actions: {}; blocked actions: {}",
            issue.id,
            issue.status,
            allowed_actions.join(", "),
            blocked_actions.join(", ")
        );
    };
    let registry = issue_transition_policy_registry();
    let policy_action = issue_transition_policy_action(issue, issue_action);
    let policy_errors =
        issue_transition_allowed_action_policy_errors(issue, &policy_action, &registry);
    if !policy_errors.is_empty() {
        anyhow::bail!(
            "issue transition `{action}` failed policy admission for issue #{}: {}",
            issue.id,
            policy_errors.join(", ")
        );
    }

    Ok(IssueTransitionAdmissionReceipt {
        schema_version: ISSUE_TRANSITION_ADMISSION_SCHEMA_VERSION.to_string(),
        policy_schema_version: registry.schema_version,
        policy_owner: registry.owner,
        policy_scope: registry.scope,
        policy_resource: "entrance://policy/registry".to_string(),
        transition_policy_resource: format!("entrance://issues/{}/transition-policy", issue.id),
        action: action.to_string(),
        gate: policy_action.gate,
        result: "admitted".to_string(),
        from_status: policy_action.from_status,
        to_status: policy_action.to_status,
        requires_confirmation: policy_action.requires_human,
        allowed_actions,
    })
}

fn record_operator_decision_evidence(
    store: &Store,
    issue: &HiveIssue,
    action: IssueDecisionAction,
    comment_id: i64,
    author: &str,
    note: &str,
    next_round: Option<i64>,
    summary: &str,
    comment_body: &str,
    confirmation_receipt: Option<&serde_json::Value>,
    transition_admission: &IssueTransitionAdmissionReceipt,
) -> Result<()> {
    let Some(loop_id) = issue.loop_id else {
        return Ok(());
    };
    let round = match next_round {
        Some(round) => round,
        None => store
            .get_hive_loop_contract(loop_id)?
            .map(|contract| contract.current_round)
            .unwrap_or(1),
    };

    let mut payload = serde_json::json!({
        "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
        "source": "issue/status/comment",
        "issue": {
            "id": issue.id,
            "comment_id": comment_id,
            "from_status": issue.status,
            "to_status": action.issue_status()
        },
        "loop": {
            "id": loop_id,
            "next_status": action.contract_status(),
            "next_phase": action.contract_phase(),
            "round": round
        },
        "operator": {
            "author": author,
            "action": action.as_str(),
            "note": note,
            "comment_body": comment_body
        },
        "transition_admission": serde_json::to_value(transition_admission)?
    });
    if let Some(receipt) = confirmation_receipt {
        payload["operator"]["confirmation_receipt"] = receipt.clone();
    }

    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id,
        stage_id: None,
        round,
        kind: "operator_decision".to_string(),
        summary: summary.to_string(),
        path: None,
        payload,
    })?;
    Ok(())
}

fn ensure_operator_confirmation_receipt(
    receipt: &OperatorConfirmationReceipt,
    action: IssueDecisionAction,
    author: &str,
) -> Result<()> {
    if receipt.schema_version != OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION {
        anyhow::bail!(
            "operator confirmation receipt schema `{}` is not supported",
            receipt.schema_version
        );
    }
    if receipt.source.trim().is_empty() {
        anyhow::bail!("operator confirmation receipt source is required");
    }
    if receipt.policy_schema_version.trim().is_empty() {
        anyhow::bail!("operator confirmation receipt policy schema is required");
    }
    if receipt.confirmation_arg.trim().is_empty() {
        anyhow::bail!("operator confirmation receipt confirmation_arg is required");
    }
    if !receipt.human_confirmed {
        anyhow::bail!("operator confirmation receipt requires human_confirmed=true");
    }
    if receipt.action != action.as_str() {
        anyhow::bail!(
            "operator confirmation receipt action `{}` does not match decision `{}`",
            receipt.action,
            action.as_str()
        );
    }
    if receipt.author != author {
        anyhow::bail!(
            "operator confirmation receipt author `{}` does not match decision author `{author}`",
            receipt.author
        );
    }
    if receipt.marker.trim().is_empty() {
        anyhow::bail!("operator confirmation receipt marker is required");
    }
    if let Some(client) = receipt.client.as_ref() {
        if client.name.trim().is_empty() {
            anyhow::bail!("operator confirmation receipt client name is required");
        }
        if client.source.trim().is_empty() {
            anyhow::bail!("operator confirmation receipt client source is required");
        }
        if client
            .version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            anyhow::bail!("operator confirmation receipt client version cannot be empty");
        }
    }
    if let Some(actor) = receipt.actor.as_ref() {
        if actor.id.trim().is_empty() {
            anyhow::bail!("operator confirmation receipt actor id is required");
        }
        if actor.label.trim().is_empty() {
            anyhow::bail!("operator confirmation receipt actor label is required");
        }
        if actor.source.trim().is_empty() {
            anyhow::bail!("operator confirmation receipt actor source is required");
        }
        if actor.trust.trim().is_empty() {
            anyhow::bail!("operator confirmation receipt actor trust is required");
        }
    }
    Ok(())
}

fn record_operator_comment_evidence(
    store: &Store,
    issue: &HiveIssue,
    comment_id: i64,
    author: &str,
    body: &str,
    transition_admission: &IssueTransitionAdmissionReceipt,
) -> Result<()> {
    let Some(loop_id) = issue.loop_id else {
        return Ok(());
    };
    let Some(contract) = store.get_hive_loop_contract(loop_id)? else {
        return Ok(());
    };

    store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id,
        stage_id: None,
        round: contract.current_round,
        kind: "operator_comment".to_string(),
        summary: body.to_string(),
        path: None,
        payload: serde_json::json!({
            "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
            "source": "issue/status/comment",
            "issue": {
                "id": issue.id,
                "status": issue.status,
                "comment_id": comment_id
            },
            "loop": {
                "id": loop_id,
                "status": contract.status,
                "phase": contract.active_phase,
                "round": contract.current_round
            },
            "operator": {
                "author": author,
                "comment_body": body
            },
            "transition_admission": serde_json::to_value(transition_admission)?
        }),
    })?;
    Ok(())
}

fn insert_stage(
    store: &Store,
    contract: &HiveLoopContract,
    role: &str,
    summary: &str,
    input: serde_json::Value,
    output: serde_json::Value,
) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    store.insert_hive_loop_stage(HiveLoopStageCreate {
        loop_id: contract.id,
        round: contract.current_round,
        role: role.to_string(),
        status: "done".to_string(),
        summary: Some(summary.to_string()),
        input,
        output,
        started_at: Some(now.clone()),
        completed_at: Some(now),
    })
}

fn issue_card(store: &Store, issue_id: i64) -> Result<IssueCard> {
    let issue = store
        .get_hive_issue(issue_id)?
        .with_context(|| format!("unknown hive issue `{issue_id}`"))?;
    issue_card_from_issue(store, issue)
}

fn issue_card_from_issue(store: &Store, issue: HiveIssue) -> Result<IssueCard> {
    let comments = store.list_hive_comments(issue.id)?;
    let trace = issue
        .loop_id
        .map(|loop_id| issue_trace_summary(store, loop_id, Some(&issue)))
        .transpose()?;
    let doctor = issue
        .loop_id
        .zip(trace.as_ref())
        .map(|(loop_id, trace)| issue_doctor_summary(store, loop_id, &issue, trace))
        .transpose()?;
    let actions = issue_actions(&issue, trace.as_ref(), doctor.as_ref());
    Ok(IssueCard {
        issue,
        comments,
        actions,
        trace,
        doctor,
    })
}

fn issue_actions(
    issue: &HiveIssue,
    trace: Option<&IssueTraceSummary>,
    doctor: Option<&IssueDoctorSummary>,
) -> Vec<IssueAction> {
    let mut actions = Vec::new();
    let runtime = doctor
        .map(|doctor| doctor.runtime.as_str())
        .filter(|runtime| !runtime.is_empty());

    if issue.loop_id.is_some() && issue.status == "Todo" {
        actions.push(issue_action(
            "run",
            "Run",
            issue_run_action_command(issue, doctor, runtime),
            "runtime",
            "none",
            false,
            runtime,
        ));
    }

    let source = if trace.is_some() {
        "human_options"
    } else {
        "status_fallback"
    };
    let options = trace
        .map(|trace| trace.human_options.clone())
        .unwrap_or_else(|| issue_human_options(Some(issue), &[], &[]));
    for option in options {
        match option.as_str() {
            "comment" => actions.push(issue_action(
                "comment",
                "Comment",
                format!(
                    "entrance hive issue comment {} --body <text> --compact",
                    issue.id
                ),
                source,
                "body",
                false,
                None,
            )),
            "retry" => actions.push(issue_action(
                "retry",
                "Retry",
                issue_retry_action_command(issue.id, doctor, runtime),
                source,
                "note",
                false,
                runtime,
            )),
            "request-review" => actions.push(issue_action(
                "request-review",
                "Review",
                format!(
                    "entrance hive issue decide {} request-review --body <note> --human-confirmed --compact",
                    issue.id
                ),
                source,
                "note",
                false,
                None,
            )),
            "cancel" => actions.push(issue_action(
                "cancel",
                "Cancel",
                format!(
                    "entrance hive issue decide {} cancel --body <note> --human-confirmed --compact",
                    issue.id
                ),
                source,
                "note",
                true,
                None,
            )),
            _ => {}
        }
    }

    actions
}

fn issue_action(
    action: &str,
    label: &str,
    command: String,
    source: &str,
    input: &str,
    destructive: bool,
    runtime: Option<&str>,
) -> IssueAction {
    let confirmation_required = issue_action_requires_confirmation(action);
    IssueAction {
        schema_version: ISSUE_ACTION_SCHEMA_VERSION.to_string(),
        action: action.to_string(),
        label: label.to_string(),
        command,
        source: source.to_string(),
        input: input.to_string(),
        destructive,
        runtime: runtime.map(ToOwned::to_owned),
        confirmation_required,
        confirmation_arg: confirmation_required
            .then(|| OPERATOR_ACTION_CONFIRMATION_ARG.to_string()),
        receipt_schema: confirmation_required
            .then(|| OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string()),
        policy_schema_version: confirmation_required
            .then(|| OPERATOR_ACTION_POLICY_SCHEMA_VERSION.to_string()),
    }
}

fn issue_action_requires_confirmation(action: &str) -> bool {
    matches!(action, "retry" | "request-review" | "cancel")
}

fn issue_run_action_command(
    issue: &HiveIssue,
    doctor: Option<&IssueDoctorSummary>,
    runtime: Option<&str>,
) -> String {
    doctor
        .and_then(|doctor| {
            doctor
                .next_actions
                .iter()
                .find(|action| action.contains("entrance hive issue run"))
                .cloned()
        })
        .unwrap_or_else(|| match runtime {
            Some(runtime) => format!(
                "entrance hive issue run {} --runtime {} --compact",
                issue.id, runtime
            ),
            None => format!("entrance hive issue run {} --compact", issue.id),
        })
}

fn issue_retry_action_command(
    issue_id: i64,
    doctor: Option<&IssueDoctorSummary>,
    runtime: Option<&str>,
) -> String {
    doctor
        .and_then(|doctor| {
            doctor
                .next_actions
                .iter()
                .find(|action| action.contains("entrance hive issue retry-run"))
                .cloned()
        })
        .unwrap_or_else(|| match runtime {
            Some(runtime) => retry_run_command(issue_id, runtime),
            None => format!(
                "entrance hive issue retry-run {issue_id} --body <note> --human-confirmed --compact"
            ),
        })
}

fn issue_doctor_summary(
    store: &Store,
    loop_id: i64,
    issue: &HiveIssue,
    trace: &IssueTraceSummary,
) -> Result<IssueDoctorSummary> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let audit_passed = trace.audit_passed.unwrap_or(false);
    let worker_failures = doctor_worker_failures(trace);
    let health = doctor_health(
        &contract.status,
        Some(issue.status.as_str()),
        trace.last_decision.as_deref(),
        audit_passed,
        !worker_failures.is_empty(),
    )
    .to_string();
    Ok(IssueDoctorSummary {
        schema_version: DOCTOR_SCHEMA_VERSION.to_string(),
        health: health.clone(),
        summary: doctor_summary(
            &contract,
            Some(issue.status.as_str()),
            trace,
            audit_passed,
            trace.audit_failed_count,
            &health,
        ),
        current_round: contract.current_round,
        next_actions: doctor_next_actions(
            &health,
            loop_id,
            Some(issue.id),
            &contract.runtime,
            audit_passed,
        ),
        runtime: contract.runtime.clone(),
        counts: doctor_counts(trace),
        failed_checks: trace.audit_failed_checks.clone(),
        audit_failure_details: trace.audit_failure_details.clone(),
        missing_receipts: doctor_missing_receipts(trace),
        worker_failures,
    })
}

fn issue_trace_summary(
    store: &Store,
    loop_id: i64,
    issue: Option<&HiveIssue>,
) -> Result<IssueTraceSummary> {
    issue_trace_summary_inner(store, loop_id, issue, true)
}

fn issue_trace_summary_without_audit(
    store: &Store,
    loop_id: i64,
    issue: Option<&HiveIssue>,
) -> Result<IssueTraceSummary> {
    issue_trace_summary_inner(store, loop_id, issue, false)
}

fn issue_trace_summary_inner(
    store: &Store,
    loop_id: i64,
    issue: Option<&HiveIssue>,
    include_audit: bool,
) -> Result<IssueTraceSummary> {
    let current_round = store
        .get_hive_loop_contract(loop_id)?
        .map(|contract| contract.current_round)
        .unwrap_or(1);
    let packets = store.list_hive_loop_packets(loop_id)?;
    let admissions = store.list_hive_loop_admissions(loop_id)?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let evidence = store.list_hive_loop_evidence(loop_id)?;
    let verdicts = store.list_hive_loop_verdicts(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let packet_rounds = packets
        .iter()
        .map(|packet| (packet.id, packet.round))
        .collect::<HashMap<_, _>>();
    let admission_in_current_round = |admission: &HiveLoopAdmission| {
        packet_rounds
            .get(&admission.packet_id)
            .is_some_and(|round| *round == current_round)
    };
    let last_admission = admissions
        .iter()
        .rev()
        .find(|admission| admission_in_current_round(admission));
    let last_verdict = verdicts
        .iter()
        .rev()
        .find(|verdict| verdict.round == current_round);
    let execution_evidence = evidence
        .iter()
        .rev()
        .find(|row| row.round == current_round && row.kind == "execution_packet");
    let worker = execution_evidence.and_then(|row| row.payload.get("worker"));
    let round_admissions = admissions
        .iter()
        .filter(|admission| admission_in_current_round(admission))
        .collect::<Vec<_>>();
    let role_worker_count = packets
        .iter()
        .filter(|packet| packet_role_worker(&packet.payload).is_some())
        .count();
    let role_worker_ok_count = packets
        .iter()
        .filter_map(|packet| packet_role_worker(&packet.payload))
        .filter(|worker| worker_ok(worker))
        .count();
    let round_role_worker_count = packets
        .iter()
        .filter(|packet| {
            packet.round == current_round && packet_role_worker(&packet.payload).is_some()
        })
        .count();
    let round_role_worker_ok_count = packets
        .iter()
        .filter(|packet| packet.round == current_round)
        .filter_map(|packet| packet_role_worker(&packet.payload))
        .filter(|worker| worker_ok(worker))
        .count();
    let audit_report = if include_audit {
        audit(store, loop_id).ok()
    } else {
        None
    };
    let audit_failed_checks = audit_report
        .as_ref()
        .map(|report| {
            report
                .checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| check.name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let audit_failure_details = audit_report
        .as_ref()
        .map(audit_failure_details)
        .unwrap_or_default();
    let round_evidence = evidence
        .iter()
        .filter(|row| row.round == current_round)
        .map(|row| issue_evidence_summary(row, &stage_roles))
        .collect::<Vec<_>>();
    let round_worker_duration_ms = round_evidence
        .iter()
        .filter_map(|row| row.worker_duration_ms)
        .sum();
    let round_worker_timeout_count = round_evidence
        .iter()
        .filter(|row| row.worker_timed_out == Some(true))
        .count();
    let round_worker_retry_exhausted_count = round_evidence
        .iter()
        .filter(|row| row.worker_retry_exhausted == Some(true))
        .count();
    let verdict_human_options = last_verdict
        .map(|verdict| human_options(&verdict.score))
        .unwrap_or_default();
    let operator_events = evidence
        .iter()
        .filter(|row| row.kind == "operator_comment" || row.kind == "operator_decision")
        .map(issue_operator_summary)
        .collect::<Vec<_>>();
    let round_operator_events = operator_events
        .iter()
        .filter(|event| event.round == current_round)
        .cloned()
        .collect::<Vec<_>>();
    let last_operator_event = operator_events.last().cloned();
    let rounds = issue_round_summaries(
        current_round,
        &evidence,
        &admissions,
        &packet_rounds,
        &verdicts,
    );

    Ok(IssueTraceSummary {
        current_round,
        rounds,
        packet_count: packets.len(),
        admission_count: admissions.len(),
        evidence_count: evidence.len(),
        verdict_count: verdicts.len(),
        round_packet_count: packets
            .iter()
            .filter(|packet| packet.round == current_round)
            .count(),
        round_admission_count: round_admissions.len(),
        round_evidence_count: evidence
            .iter()
            .filter(|row| row.round == current_round)
            .count(),
        round_verdict_count: verdicts
            .iter()
            .filter(|verdict| verdict.round == current_round)
            .count(),
        receipt_required_count: admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/required"))
            .sum(),
        receipt_missing_count: admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
            .sum(),
        round_receipt_required_count: round_admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/required"))
            .sum(),
        round_receipt_missing_count: round_admissions
            .iter()
            .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
            .sum(),
        role_worker_count,
        role_worker_ok_count,
        round_role_worker_count,
        round_role_worker_ok_count,
        round_worker_duration_ms,
        round_worker_timeout_count,
        round_worker_retry_exhausted_count,
        packet_schema: packets
            .iter()
            .rev()
            .find(|packet| packet.round == current_round)
            .and_then(|packet| schema_version(&packet.payload)),
        policy_schema: last_admission
            .and_then(|admission| admission.policy.pointer("/policy/schema_version"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_schema: last_admission.and_then(|admission| schema_version(&admission.policy)),
        verdict_schema: last_verdict.and_then(|verdict| schema_version(&verdict.score)),
        last_admission_gate: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/name"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        last_gate_description: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/spec/description"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        last_gate_expected_object_kind: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/spec/expected_object_kind"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        last_admission_passed: last_admission
            .and_then(|admission| admission.policy.pointer("/gate/passed"))
            .and_then(|value| value.as_bool()),
        last_decision: last_verdict.map(|verdict| verdict.decision.clone()),
        reason_code: last_verdict
            .and_then(|verdict| {
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
            })
            .map(ToOwned::to_owned),
        score_vector: last_verdict
            .map(|verdict| score_vector(&verdict.score))
            .unwrap_or_default(),
        human_options: issue_human_options(issue, &verdict_human_options, &round_evidence),
        operator_event_count: operator_events.len(),
        round_operator_event_count: round_operator_events.len(),
        last_operator_event,
        operator_events: round_operator_events,
        worker_kind: worker
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_mode: worker
            .and_then(|value| value.get("mode"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_ok: worker
            .and_then(|value| value.get("ok"))
            .and_then(|value| value.as_bool()),
        audit_schema: audit_report
            .as_ref()
            .map(|report| report.schema_version.clone()),
        audit_passed: audit_report.as_ref().map(|report| report.passed),
        audit_failed_count: audit_report
            .as_ref()
            .map(|report| report.failed_count)
            .unwrap_or_default(),
        audit_failed_checks,
        audit_failure_details,
        evidence: round_evidence,
        stages: issue_stage_summaries(&stages, &evidence, current_round),
    })
}

fn stage_role_map(stages: &[HiveLoopStage]) -> HashMap<i64, String> {
    stages
        .iter()
        .map(|stage| (stage.id, stage.role.clone()))
        .collect()
}

fn issue_round_summaries(
    current_round: i64,
    evidence: &[HiveLoopEvidence],
    admissions: &[HiveLoopAdmission],
    packet_rounds: &HashMap<i64, i64>,
    verdicts: &[HiveLoopVerdict],
) -> Vec<IssueRoundSummary> {
    let mut rounds = Vec::new();
    rounds.push(current_round);
    rounds.extend(evidence.iter().map(|row| row.round));
    rounds.extend(verdicts.iter().map(|verdict| verdict.round));
    rounds.extend(
        admissions
            .iter()
            .filter_map(|admission| packet_rounds.get(&admission.packet_id).copied()),
    );
    rounds.sort_unstable();
    rounds.dedup();
    rounds
        .into_iter()
        .map(|round| {
            let round_evidence = evidence.iter().filter(|row| row.round == round);
            let evidence_count = evidence.iter().filter(|row| row.round == round).count();
            let rejected_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter(|row| evidence_row_rejected(row))
                .count();
            let worker_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter(|row| row.payload.get("worker").is_some())
                .count();
            let worker_ok_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter_map(|row| row.payload.get("worker"))
                .filter(|worker| worker_ok(worker))
                .count();
            let worker_timeout_count = round_evidence
                .clone()
                .filter_map(|row| row.payload.get("worker"))
                .filter(|worker| {
                    worker.get("timed_out").and_then(|value| value.as_bool()) == Some(true)
                })
                .count();
            let worker_retry_exhausted_count = evidence
                .iter()
                .filter(|row| row.round == round)
                .filter_map(|row| row.payload.get("worker"))
                .filter(|worker| {
                    worker
                        .get("retry_exhausted")
                        .and_then(|value| value.as_bool())
                        == Some(true)
                })
                .count();
            let round_admissions = admissions.iter().filter(|admission| {
                packet_rounds
                    .get(&admission.packet_id)
                    .is_some_and(|packet_round| *packet_round == round)
            });
            let receipt_required_count = round_admissions
                .clone()
                .map(|admission| receipt_array_len(&admission.policy, "/receipt/required"))
                .sum();
            let receipt_missing_count = admissions
                .iter()
                .filter(|admission| {
                    packet_rounds
                        .get(&admission.packet_id)
                        .is_some_and(|packet_round| *packet_round == round)
                })
                .map(|admission| receipt_array_len(&admission.policy, "/receipt/missing"))
                .sum();
            let round_verdict = verdicts.iter().rev().find(|verdict| verdict.round == round);
            let decision = round_verdict.map(|verdict| verdict.decision.clone());
            let reason_code = round_verdict.and_then(verdict_reason_code);
            let status = issue_round_status(
                decision.as_deref(),
                rejected_count,
                receipt_missing_count,
                worker_count,
                worker_ok_count,
            )
            .to_string();
            IssueRoundSummary {
                round,
                status,
                decision,
                reason_code,
                evidence_count,
                rejected_count,
                receipt_required_count,
                receipt_missing_count,
                worker_count,
                worker_ok_count,
                worker_timeout_count,
                worker_retry_exhausted_count,
            }
        })
        .collect()
}

fn evidence_row_rejected(row: &HiveLoopEvidence) -> bool {
    row.kind == "admission_rejection"
        || row
            .payload
            .get("admission")
            .or_else(|| row.payload.get("result"))
            .and_then(|value| value.as_str())
            == Some("rejected")
}

fn issue_round_status(
    decision: Option<&str>,
    rejected_count: usize,
    receipt_missing_count: usize,
    worker_count: usize,
    worker_ok_count: usize,
) -> &'static str {
    match decision {
        Some("keep") => "kept",
        Some("reject") => "rejected",
        Some("needs-review") => "needs_review",
        Some("blocked") => "blocked",
        _ if rejected_count > 0 || receipt_missing_count > 0 || worker_ok_count < worker_count => {
            "blocked"
        }
        _ if worker_count > 0 => "ran",
        _ => "pending",
    }
}

fn issue_evidence_summary(
    row: &HiveLoopEvidence,
    stage_roles: &HashMap<i64, String>,
) -> IssueEvidenceSummary {
    let worker = row.payload.get("worker");
    let worker_receipt = worker.and_then(worker_structured_receipt);
    let stage_role = row
        .stage_id
        .and_then(|stage_id| stage_roles.get(&stage_id).cloned());
    let worker_receipt_errors = worker
        .map(|worker| worker_receipt_errors_for_summary(worker, stage_role.as_deref()))
        .unwrap_or_default();
    IssueEvidenceSummary {
        id: row.id,
        round: row.round,
        stage_role,
        kind: row.kind.clone(),
        summary: row.summary.clone(),
        schema_version: schema_version(&row.payload),
        admission_result: row
            .payload
            .get("admission")
            .or_else(|| row.payload.get("result"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        blocked_phase: row
            .payload
            .get("phase")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        missing_receipts: string_array_at(&row.payload, "/admission_receipt/receipt/missing"),
        packet_envelope_errors: string_array_at(
            &row.payload,
            "/admission_receipt/packet/envelope/errors",
        ),
        operator_options: string_array_at(&row.payload, "/operator_options"),
        operator_author: string_at(&row.payload, "/operator/author"),
        operator_action: string_at(&row.payload, "/operator/action"),
        worker_kind: worker
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_mode: worker
            .and_then(|value| value.get("mode"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_ok: worker
            .and_then(|value| value.get("ok"))
            .and_then(|value| value.as_bool()),
        worker_receipt_ok: worker
            .and_then(|value| value.get("receipt_ok"))
            .and_then(|value| value.as_bool()),
        worker_timed_out: worker
            .and_then(|value| value.get("timed_out"))
            .and_then(|value| value.as_bool()),
        worker_status: worker
            .and_then(|value| value.get("status"))
            .and_then(|value| value.as_i64()),
        worker_duration_ms: worker
            .and_then(|value| value.get("duration_ms"))
            .and_then(|value| value.as_u64()),
        worker_timeout_secs: worker
            .and_then(|value| value.get("timeout_secs"))
            .and_then(|value| value.as_u64()),
        worker_attempt_count: worker
            .and_then(|value| value.get("attempt_count"))
            .and_then(|value| value.as_u64()),
        worker_max_attempts: worker
            .and_then(|value| value.get("max_attempts"))
            .and_then(|value| value.as_u64()),
        worker_retry_exhausted: worker
            .and_then(|value| value.get("retry_exhausted"))
            .and_then(|value| value.as_bool()),
        worker_command: worker
            .and_then(|value| value.get("command"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_cwd: worker
            .and_then(|value| value.get("cwd"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_action: worker_receipt
            .as_ref()
            .and_then(|receipt| receipt.get("action"))
            .and_then(|value| value.as_str())
            .or_else(|| {
                worker
                    .and_then(|value| value.pointer("/packet/action"))
                    .and_then(|value| value.as_str())
            })
            .map(ToOwned::to_owned),
        worker_evidence_summary: worker_receipt
            .as_ref()
            .and_then(|receipt| receipt.get("evidence_summary"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        worker_gate_count: worker_receipt
            .as_ref()
            .and_then(|receipt| receipt.get("gates"))
            .and_then(|value| value.as_object())
            .map(serde_json::Map::len),
        worker_receipt_errors,
        transcript_excerpt: worker
            .and_then(worker_transcript_excerpt)
            .map(|value| truncate_text(&value, 240)),
    }
}

fn worker_receipt_errors_for_summary(
    worker: &serde_json::Value,
    expected_role: Option<&str>,
) -> Vec<String> {
    let stored_errors = string_array_at(worker, "/receipt_errors");
    if !stored_errors.is_empty() {
        return stored_errors;
    }
    match worker_structured_receipt(worker) {
        Some(receipt) => worker_receipt_contract_errors(&receipt, expected_role),
        None if worker.get("ok").and_then(|value| value.as_bool()) == Some(true) => {
            vec!["receipt".to_string()]
        }
        None => Vec::new(),
    }
}

fn issue_operator_summary(row: &HiveLoopEvidence) -> IssueOperatorSummary {
    let transition_admission = row.payload.get("transition_admission");
    IssueOperatorSummary {
        id: row.id,
        round: row.round,
        kind: row.kind.clone(),
        author: string_at(&row.payload, "/operator/author"),
        action: string_at(&row.payload, "/operator/action"),
        issue_status: string_at(&row.payload, "/issue/to_status")
            .or_else(|| string_at(&row.payload, "/issue/status")),
        loop_status: string_at(&row.payload, "/loop/next_status")
            .or_else(|| string_at(&row.payload, "/loop/status")),
        admission_action: transition_admission
            .and_then(|value| value.get("action"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_gate: transition_admission
            .and_then(|value| value.get("gate"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_from_status: transition_admission
            .and_then(|value| value.get("from_status"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_to_status: transition_admission
            .and_then(|value| value.get("to_status"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_policy_resource: transition_admission
            .and_then(|value| value.get("policy_resource"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_transition_policy_resource: transition_admission
            .and_then(|value| value.get("transition_policy_resource"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        admission_requires_confirmation: transition_admission
            .and_then(|value| value.get("requires_confirmation"))
            .and_then(|value| value.as_bool()),
        note: string_at(&row.payload, "/operator/note")
            .or_else(|| string_at(&row.payload, "/operator/comment_body"))
            .map(|value| truncate_text(&value, 180)),
        summary: truncate_text(&row.summary, 180),
    }
}

fn worker_transcript_excerpt(worker: &serde_json::Value) -> Option<String> {
    ["last_message", "error", "stderr", "stdout"]
        .into_iter()
        .filter_map(|key| worker.get(key).and_then(|value| value.as_str()))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn issue_stage_summaries(
    stages: &[HiveLoopStage],
    evidence: &[HiveLoopEvidence],
    current_round: i64,
) -> Vec<IssueStageSummary> {
    stages
        .iter()
        .filter(|stage| stage.round == current_round)
        .map(|stage| {
            let stage_evidence = evidence
                .iter()
                .rev()
                .find(|row| row.stage_id == Some(stage.id));
            let worker = stage_evidence
                .and_then(|row| row.payload.get("worker"))
                .or_else(|| stage.output.get("role_worker"))
                .or_else(|| stage.output.get("runtime_worker"));
            IssueStageSummary {
                role: stage.role.clone(),
                status: stage.status.clone(),
                summary: stage.summary.clone(),
                evidence_kind: stage_evidence.map(|row| row.kind.clone()),
                evidence_summary: stage_evidence.map(|row| row.summary.clone()),
                admission_result: stage_evidence
                    .and_then(|row| {
                        row.payload
                            .get("admission")
                            .or_else(|| row.payload.get("result"))
                    })
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                worker_kind: worker
                    .and_then(|value| value.get("kind"))
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                worker_mode: worker
                    .and_then(|value| value.get("mode"))
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                worker_ok: worker
                    .and_then(|value| value.get("ok"))
                    .and_then(|value| value.as_bool()),
            }
        })
        .collect()
}

fn schema_version(value: &serde_json::Value) -> Option<String> {
    value
        .get("schema_version")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn receipt_array_len(value: &serde_json::Value, pointer: &str) -> usize {
    value
        .pointer(pointer)
        .and_then(|value| value.as_array())
        .map(Vec::len)
        .unwrap_or_default()
}

fn string_array_at(value: &serde_json::Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn string_at(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn human_options(value: &serde_json::Value) -> Vec<String> {
    value
        .get("human_options")
        .and_then(|value| value.as_array())
        .map(|options| {
            options
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn issue_human_options(
    issue: Option<&HiveIssue>,
    verdict_options: &[String],
    evidence: &[IssueEvidenceSummary],
) -> Vec<String> {
    let Some(issue) = issue else {
        return verdict_options.to_vec();
    };
    match issue.status.as_str() {
        "Todo" => option_list(&["comment", "cancel"]),
        "Doing" | "Done" => option_list(&["comment"]),
        "Blocked" => option_list(&["comment", "retry", "request-review", "cancel"]),
        "Needs Review" => option_list(&["comment", "retry", "cancel"]),
        "Canceled" if latest_operator_action(evidence) == Some("cancel") => {
            option_list(&["comment"])
        }
        "Canceled" if verdict_options.iter().any(|option| option == "retry") => {
            option_list(&["comment", "retry"])
        }
        "Canceled" => option_list(&["comment"]),
        _ if verdict_options.is_empty() => option_list(&["comment"]),
        _ => verdict_options.to_vec(),
    }
}

fn latest_operator_action(evidence: &[IssueEvidenceSummary]) -> Option<&str> {
    evidence
        .iter()
        .rev()
        .find(|row| row.kind == "operator_decision")
        .and_then(|row| row.operator_action.as_deref())
}

fn option_list(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn score_vector(value: &serde_json::Value) -> Vec<ScoreVectorMetric> {
    let Some(metrics) = value
        .get("score_vector")
        .and_then(|value| value.as_object())
    else {
        return Vec::new();
    };
    let preferred_order = [
        "stage_completeness",
        "runtime_readiness",
        "evidence_presence",
        "admission_integrity",
    ];
    let mut output = preferred_order
        .into_iter()
        .filter_map(|name| {
            metrics.get(name).map(|value| ScoreVectorMetric {
                name: name.to_string(),
                value: value.as_f64(),
            })
        })
        .collect::<Vec<_>>();
    let mut extra_names = metrics
        .keys()
        .filter(|name| !preferred_order.contains(&name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    extra_names.sort();
    output.extend(extra_names.into_iter().map(|name| ScoreVectorMetric {
        value: metrics.get(&name).and_then(|value| value.as_f64()),
        name,
    }));
    output
}

fn add_system_comment(
    store: &Store,
    issue_id: i64,
    body: &str,
    payload: serde_json::Value,
) -> Result<()> {
    store.insert_hive_comment(HiveCommentCreate {
        issue_id,
        author: "hive".to_string(),
        body: body.to_string(),
        payload: system_comment_payload("hive", payload),
    })?;
    Ok(())
}

fn add_stage_system_comment(
    store: &Store,
    issue_id: i64,
    loop_id: i64,
    round: i64,
    role: &str,
    evidence_kind: &str,
    evidence_id: i64,
    body: &str,
    admission: &str,
    worker: &serde_json::Value,
) -> Result<()> {
    add_system_comment(
        store,
        issue_id,
        body,
        serde_json::json!({
            "loop_id": loop_id,
            "round": round,
            "phase": role,
            "stage_role": role,
            "evidence_kind": evidence_kind,
            "evidence_id": evidence_id,
            "admission": admission,
            "worker": worker
        }),
    )
}

fn system_comment_payload(source: &str, payload: serde_json::Value) -> serde_json::Value {
    let mut typed = serde_json::Map::new();
    typed.insert(
        "schema_version".to_string(),
        serde_json::Value::String(SYSTEM_COMMENT_SCHEMA_VERSION.to_string()),
    );
    typed.insert(
        "source".to_string(),
        serde_json::Value::String(source.to_string()),
    );
    match payload {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                if key != "schema_version" && key != "source" {
                    typed.insert(key, value);
                }
            }
        }
        other => {
            typed.insert("details".to_string(), other);
        }
    }
    serde_json::Value::Object(typed)
}

fn block_on_admission_rejection(
    store: &Store,
    contract: &HiveLoopContract,
    issue_id: Option<i64>,
    phase: &str,
    stage_id: Option<i64>,
    admission: &HiveLoopAdmission,
) -> Result<HiveLoopReport> {
    let summary = format!(
        "Compiler admission blocked at {phase}: {}.",
        admission.reason
    );
    let rejected_packet = store.get_hive_loop_packet(admission.packet_id)?;
    let rejected_worker = rejected_packet
        .as_ref()
        .and_then(|packet| packet_role_worker(&packet.payload))
        .cloned();
    let mut evidence_payload = serde_json::json!({
        "phase": phase,
        "admission_id": admission.id,
        "packet_id": admission.packet_id,
        "result": admission.result,
        "reason": admission.reason,
        "admission_receipt": admission.policy.clone(),
        "operator_options": ["fix-policy", "retry", "request-human-review"]
    });
    if let Some(worker) = rejected_worker {
        evidence_payload["worker"] = worker;
    }
    let evidence_id = store.insert_hive_loop_evidence(HiveLoopEvidenceCreate {
        loop_id: contract.id,
        stage_id,
        round: contract.current_round,
        kind: "admission_rejection".to_string(),
        summary: summary.clone(),
        path: None,
        payload: evidence_payload,
    })?;

    store.insert_hive_loop_verdict(HiveLoopVerdictCreate {
        loop_id: contract.id,
        round: contract.current_round,
        decision: "blocked".to_string(),
        summary: summary.clone(),
        score: admission_rejection_score_payload(phase),
        evidence: admission_rejection_verdict_evidence_payload(evidence_id, phase, admission),
    })?;

    store.update_hive_loop_contract_state(contract.id, "blocked", phase, contract.current_round)?;

    if let Some(issue_id) = issue_id {
        store.update_hive_issue_status(issue_id, "Blocked", Some(&summary))?;
        add_system_comment(
            store,
            issue_id,
            &summary,
            serde_json::json!({
                "loop_id": contract.id,
                "phase": phase,
                "decision": "blocked",
                "reason_code": "admission_rejected",
                "admission": {
                    "id": admission.id,
                    "packet_id": admission.packet_id,
                    "result": admission.result,
                    "reason": admission.reason
                },
                "operator_options": ["fix-policy", "retry", "request-human-review"]
            }),
        )?;
    }

    report(store, contract.id)
}

fn stage_completeness_for_phase(phase: &str) -> f64 {
    match phase {
        "kernel" => 0.0,
        "explorer" => 0.33,
        "developer" => 0.66,
        "reviewer" => 1.0,
        "doer" => 0.66,
        "evaluator" => 1.0,
        _ => 0.0,
    }
}

fn admission_rejection_score_payload(phase: &str) -> serde_json::Value {
    let stage_completeness = stage_completeness_for_phase(phase);
    serde_json::json!({
        "schema_version": VERDICT_SCHEMA_VERSION,
        "decision": "blocked",
        "reason_code": "admission_rejected",
        "gates_passed": false,
        "operator_review_needed": true,
        "score_vector": {
            "stage_completeness": stage_completeness,
            "runtime_readiness": serde_json::Value::Null,
            "evidence_presence": 1.0,
            "admission_integrity": 0.0,
            "target_alignment": 0.0,
            "goal_alignment": 0.0,
            "acceptance_evidence": 0.0,
            "implementation_specificity": 0.0,
            "regression_risk": 0.0
        },
        "gate_results": {
            "admission_passed": false,
            "blocked_phase": phase
        },
        "human_options": ["comment", "retry", "request-review", "cancel"]
    })
}

fn admission_rejection_verdict_evidence_payload(
    evidence_id: i64,
    phase: &str,
    admission: &HiveLoopAdmission,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": VERDICT_SCHEMA_VERSION,
        "decision": "blocked",
        "reason_code": "admission_rejected",
        "evidence_id": evidence_id,
        "admission_id": admission.id,
        "packet_id": admission.packet_id,
        "phase": phase,
        "source": {
            "reviewer": "hive-loop-control",
            "admission_receipt": admission.policy.clone()
        }
    })
}

impl IssueDecisionAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::RequestReview => "request-review",
            Self::Cancel => "cancel",
        }
    }

    fn issue_status(self) -> &'static str {
        match self {
            Self::Retry => "Todo",
            Self::RequestReview => "Needs Review",
            Self::Cancel => "Canceled",
        }
    }

    fn contract_status(self) -> &'static str {
        match self {
            Self::Retry => "todo",
            Self::RequestReview => "needs-review",
            Self::Cancel => "rejected",
        }
    }

    fn contract_phase(self) -> &'static str {
        match self {
            Self::Retry => "explorer",
            Self::RequestReview => "human-review",
            Self::Cancel => "complete",
        }
    }

    fn issue_summary(self, next_round: Option<i64>) -> String {
        match self {
            Self::Retry => format!(
                "Human chose retry; loop returned to Explorer for round {}.",
                next_round.unwrap_or(1)
            ),
            Self::RequestReview => "Human requested review before the loop continues.".to_string(),
            Self::Cancel => "Human canceled this loop issue.".to_string(),
        }
    }

    fn comment_body(self, next_round: Option<i64>, note: &str) -> String {
        let base = self.issue_summary(next_round);
        if note.is_empty() {
            base
        } else {
            format!("{base} Note: {note}")
        }
    }
}

fn parse_issue_decision_action(value: &str) -> Result<IssueDecisionAction> {
    match value {
        "retry" => Ok(IssueDecisionAction::Retry),
        "request-review" => Ok(IssueDecisionAction::RequestReview),
        "cancel" => Ok(IssueDecisionAction::Cancel),
        other => anyhow::bail!(
            "unsupported issue decision `{other}`; expected retry, request-review, or cancel"
        ),
    }
}
