pub fn policies(store: &Store, loop_id: i64) -> Result<HiveLoopPolicyReport> {
    store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let policies = store
        .list_hive_loop_policies(loop_id)?
        .into_iter()
        .map(|policy| HiveLoopPolicyCard {
            gate_spec: gate_spec(&policy.gate).map(PolicyGateSpec::from),
            policy,
        })
        .collect();
    Ok(HiveLoopPolicyReport { loop_id, policies })
}

pub fn trace(store: &Store, loop_id: i64) -> Result<HiveLoopTraceReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issue = store.list_hive_issues_for_loop(loop_id)?.into_iter().next();
    Ok(HiveLoopTraceReport {
        trace: issue_trace_summary(store, loop_id, issue.as_ref())?,
        contract,
        issue,
    })
}

pub fn evidence_report(store: &Store, loop_id: i64) -> Result<HiveLoopEvidenceReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let stages = store.list_hive_loop_stages(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let evidence = store
        .list_hive_loop_evidence(loop_id)?
        .iter()
        .map(|row| issue_evidence_summary(row, &stage_roles))
        .collect();
    Ok(HiveLoopEvidenceReport {
        current_round: contract.current_round,
        contract,
        evidence,
    })
}

pub fn evidence_drilldown(store: &Store, loop_id: i64) -> Result<HiveLoopEvidenceDrilldownReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issue = store.list_hive_issues_for_loop(loop_id)?.into_iter().next();
    let actions = issue
        .as_ref()
        .map(|issue| issue_card_from_issue(store, issue.clone()).map(|card| card.actions))
        .transpose()?
        .unwrap_or_default();
    let stages = store.list_hive_loop_stages(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let evidence_rows = store.list_hive_loop_evidence(loop_id)?;
    let mut previous_payload: Option<&HiveLoopEvidence> = None;
    let mut items = Vec::new();
    for row in &evidence_rows {
        let summary = issue_evidence_summary(row, &stage_roles);
        items.push(evidence_drilldown_item(
            row,
            &summary,
            previous_payload.map(|previous| (previous.id, &previous.payload)),
        ));
        previous_payload = Some(row);
    }
    let blockers = evidence_drilldown_blockers(&contract, issue.as_ref(), &items, &actions);
    let human_decision = evidence_drilldown_human_decision(issue.as_ref(), &items, &actions);
    let next_actions = evidence_drilldown_next_actions(loop_id, issue.as_ref(), &actions);
    let summary =
        evidence_drilldown_summary(&contract, issue.as_ref(), items.len(), blockers.len());
    let drilldown_state = evidence_drilldown_state(&contract, blockers.len(), &human_decision);
    Ok(HiveLoopEvidenceDrilldownReport {
        schema_version: EVIDENCE_DRILLDOWN_SCHEMA_VERSION.to_string(),
        loop_id,
        issue_id: issue.as_ref().map(|issue| issue.id),
        issue_status: issue.as_ref().map(|issue| issue.status.clone()),
        status: contract.status.clone(),
        active_phase: contract.active_phase.clone(),
        current_round: contract.current_round,
        runtime: contract.runtime.clone(),
        drilldown_state,
        summary,
        evidence_count: items.len(),
        items,
        blockers,
        human_decision,
        resources: HiveLoopEvidenceDrilldownResources {
            evidence_drilldown: format!("entrance://loops/{loop_id}/evidence-drilldown"),
            evidence_manifest: format!("entrance://loops/{loop_id}/evidence-manifest"),
            loop_dashboard: format!("entrance://loops/{loop_id}/dashboard"),
            worker_lifecycle: format!("entrance://loops/{loop_id}/worker-lifecycle"),
            runtime_preflight: format!("entrance://loops/{loop_id}/runtime-preflight"),
            issue: issue
                .as_ref()
                .map(|issue| format!("entrance://issues/{}", issue.id)),
            issue_control: issue
                .as_ref()
                .map(|issue| format!("entrance://issues/{}/control", issue.id)),
            review_queue: "entrance://review-queue".to_string(),
        },
        next_actions,
    })
}

pub fn evidence_manifest(store: &Store, loop_id: i64) -> Result<HiveLoopEvidenceManifestReport> {
    let contract = store
        .get_hive_loop_contract(loop_id)?
        .with_context(|| format!("unknown hive loop `{loop_id}`"))?;
    let issue = store.list_hive_issues_for_loop(loop_id)?.into_iter().next();
    let stages = store.list_hive_loop_stages(loop_id)?;
    let stage_roles = stage_role_map(&stages);
    let evidence_rows = store.list_hive_loop_evidence(loop_id)?;
    let mut entries = Vec::new();
    for row in &evidence_rows {
        let summary = issue_evidence_summary(row, &stage_roles);
        entries.push(evidence_manifest_payload_entry(row, &summary));
        if let Some(receipt) = row
            .payload
            .get("worker")
            .and_then(worker_structured_receipt)
        {
            entries.push(evidence_manifest_receipt_entry(row, &summary, receipt));
        }
        if let Some(transcript) = summary.transcript_excerpt.as_ref() {
            entries.push(evidence_manifest_transcript_entry(
                row, &summary, transcript,
            ));
        }
        for (index, artifact) in evidence_artifacts(row).into_iter().enumerate() {
            entries.push(evidence_manifest_artifact_entry(
                row, &summary, artifact, index,
            ));
        }
    }
    let coverage = evidence_manifest_coverage(evidence_rows.len(), &entries);
    let manifest_state = evidence_manifest_state(&coverage);
    let summary = evidence_manifest_summary(&manifest_state, &coverage);
    let next_actions = evidence_manifest_next_actions(loop_id, issue.as_ref(), &coverage);
    Ok(HiveLoopEvidenceManifestReport {
        schema_version: EVIDENCE_MANIFEST_SCHEMA_VERSION.to_string(),
        loop_id,
        issue_id: issue.as_ref().map(|issue| issue.id),
        issue_status: issue.as_ref().map(|issue| issue.status.clone()),
        status: contract.status.clone(),
        active_phase: contract.active_phase.clone(),
        current_round: contract.current_round,
        runtime: contract.runtime.clone(),
        manifest_state,
        summary,
        coverage,
        entries,
        resources: HiveLoopEvidenceManifestResources {
            evidence_manifest: format!("entrance://loops/{loop_id}/evidence-manifest"),
            evidence_drilldown: format!("entrance://loops/{loop_id}/evidence-drilldown"),
            loop_dashboard: format!("entrance://loops/{loop_id}/dashboard"),
            worker_lifecycle: format!("entrance://loops/{loop_id}/worker-lifecycle"),
            runtime_preflight: format!("entrance://loops/{loop_id}/runtime-preflight"),
            issue: issue
                .as_ref()
                .map(|issue| format!("entrance://issues/{}", issue.id)),
            issue_control: issue
                .as_ref()
                .map(|issue| format!("entrance://issues/{}/control", issue.id)),
            review_queue: "entrance://review-queue".to_string(),
        },
        next_actions,
    })
}

fn evidence_manifest_payload_entry(
    row: &HiveLoopEvidence,
    summary: &IssueEvidenceSummary,
) -> HiveLoopEvidenceManifestEntry {
    let top_level_keys = top_level_keys(&row.payload);
    HiveLoopEvidenceManifestEntry {
        id: format!("evidence-{}-payload", row.id),
        evidence_id: row.id,
        round: row.round,
        stage_role: summary.stage_role.clone(),
        kind: row.kind.clone(),
        source: "evidence.payload".to_string(),
        entry_kind: "payload".to_string(),
        label: format!(
            "payload #{} {} {}",
            row.id,
            summary.stage_role.as_deref().unwrap_or("kernel"),
            row.kind
        ),
        summary: row.summary.clone(),
        path: None,
        path_status: "none".to_string(),
        schema_version: summary.schema_version.clone(),
        sha256: Some(sha256_json(&row.payload)),
        size_bytes: json_size_bytes(&row.payload),
        required: true,
        verified: true,
        details: serde_json::json!({
            "top_level_keys": top_level_keys,
            "excerpt": json_excerpt(&row.payload, 520)
        }),
    }
}

fn evidence_manifest_receipt_entry(
    row: &HiveLoopEvidence,
    summary: &IssueEvidenceSummary,
    receipt: serde_json::Value,
) -> HiveLoopEvidenceManifestEntry {
    let receipt_errors = worker_receipt_contract_errors(&receipt, summary.stage_role.as_deref());
    HiveLoopEvidenceManifestEntry {
        id: format!("evidence-{}-receipt", row.id),
        evidence_id: row.id,
        round: row.round,
        stage_role: summary.stage_role.clone(),
        kind: row.kind.clone(),
        source: "worker.receipt".to_string(),
        entry_kind: "receipt".to_string(),
        label: format!(
            "receipt #{} {} {}",
            row.id,
            receipt
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown"),
            receipt
                .get("action")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
        ),
        summary: receipt
            .get("evidence_summary")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| row.summary.clone()),
        path: None,
        path_status: "none".to_string(),
        schema_version: schema_version(&receipt),
        sha256: Some(sha256_json(&receipt)),
        size_bytes: json_size_bytes(&receipt),
        required: true,
        verified: receipt_errors.is_empty(),
        details: serde_json::json!({
            "ok": receipt.get("ok").and_then(|value| value.as_bool()),
            "role": receipt.get("role").and_then(|value| value.as_str()),
            "action": receipt.get("action").and_then(|value| value.as_str()),
            "gates": receipt.get("gates").cloned().unwrap_or_else(|| serde_json::json!({})),
            "receipt_errors": receipt_errors,
            "excerpt": json_excerpt(&receipt, 520)
        }),
    }
}

fn evidence_manifest_transcript_entry(
    row: &HiveLoopEvidence,
    summary: &IssueEvidenceSummary,
    transcript: &str,
) -> HiveLoopEvidenceManifestEntry {
    HiveLoopEvidenceManifestEntry {
        id: format!("evidence-{}-transcript", row.id),
        evidence_id: row.id,
        round: row.round,
        stage_role: summary.stage_role.clone(),
        kind: row.kind.clone(),
        source: "worker.transcript".to_string(),
        entry_kind: "transcript".to_string(),
        label: format!(
            "transcript #{} {}",
            row.id,
            summary.stage_role.as_deref().unwrap_or("kernel")
        ),
        summary: row.summary.clone(),
        path: None,
        path_status: "none".to_string(),
        schema_version: None,
        sha256: Some(sha256_text(transcript)),
        size_bytes: Some(transcript.len() as u64),
        required: false,
        verified: true,
        details: serde_json::json!({
            "excerpt": truncate_text(transcript, 520)
        }),
    }
}

fn evidence_manifest_artifact_entry(
    row: &HiveLoopEvidence,
    summary: &IssueEvidenceSummary,
    artifact: HiveLoopEvidenceArtifact,
    index: usize,
) -> HiveLoopEvidenceManifestEntry {
    let path_status = evidence_manifest_path_status(artifact.path.as_deref());
    let path_size = evidence_manifest_path_size_bytes(artifact.path.as_deref(), &path_status);
    let manifest = artifact_manifest_value(row, &artifact);
    let verified = match path_status.as_str() {
        "present" => true,
        "none" => artifact.manifest.is_some(),
        _ => false,
    };
    HiveLoopEvidenceManifestEntry {
        id: format!("evidence-{}-artifact-{}", row.id, index + 1),
        evidence_id: row.id,
        round: row.round,
        stage_role: summary.stage_role.clone(),
        kind: row.kind.clone(),
        source: "evidence.artifact".to_string(),
        entry_kind: artifact.kind.clone(),
        label: format!(
            "artifact #{} {} {}",
            row.id,
            artifact.kind,
            artifact.path.as_deref().unwrap_or("inline")
        ),
        summary: artifact
            .summary
            .clone()
            .unwrap_or_else(|| row.summary.clone()),
        path: artifact.path.clone(),
        path_status,
        schema_version: artifact
            .manifest
            .as_ref()
            .and_then(schema_version)
            .or_else(|| summary.schema_version.clone()),
        sha256: Some(sha256_json(&manifest)),
        size_bytes: path_size.or_else(|| json_size_bytes(&manifest)),
        required: false,
        verified,
        details: serde_json::json!({
            "artifact_kind": artifact.kind,
            "manifest": artifact.manifest,
            "summary": artifact.summary
        }),
    }
}

fn artifact_manifest_value(
    row: &HiveLoopEvidence,
    artifact: &HiveLoopEvidenceArtifact,
) -> serde_json::Value {
    artifact.manifest.clone().unwrap_or_else(|| {
        serde_json::json!({
            "kind": artifact.kind.clone(),
            "path": artifact.path.clone(),
            "summary": artifact.summary.as_deref().unwrap_or(&row.summary)
        })
    })
}

fn evidence_manifest_coverage(
    evidence_count: usize,
    entries: &[HiveLoopEvidenceManifestEntry],
) -> HiveLoopEvidenceManifestCoverage {
    let payload_count = entries
        .iter()
        .filter(|entry| entry.entry_kind == "payload")
        .count();
    let receipt_count = entries
        .iter()
        .filter(|entry| entry.entry_kind == "receipt")
        .count();
    let transcript_count = entries
        .iter()
        .filter(|entry| entry.entry_kind == "transcript")
        .count();
    let artifact_count = entries
        .iter()
        .filter(|entry| entry.source == "evidence.artifact")
        .count();
    let path_count = entries
        .iter()
        .filter(|entry| entry.path_status != "none")
        .count();
    let path_present_count = entries
        .iter()
        .filter(|entry| entry.path_status == "present")
        .count();
    let path_missing_count = entries
        .iter()
        .filter(|entry| entry.path_status == "missing")
        .count();
    let path_unverified_count = entries
        .iter()
        .filter(|entry| entry.path_status == "unverified-relative")
        .count();
    let path_none_count = entries
        .iter()
        .filter(|entry| entry.path_status == "none")
        .count();
    let digest_count = entries
        .iter()
        .filter(|entry| {
            entry
                .sha256
                .as_deref()
                .is_some_and(|digest| !digest.is_empty())
        })
        .count();
    HiveLoopEvidenceManifestCoverage {
        evidence_count,
        entry_count: entries.len(),
        payload_count,
        receipt_count,
        transcript_count,
        artifact_count,
        path_count,
        path_present_count,
        path_missing_count,
        path_unverified_count,
        path_none_count,
        digest_count,
    }
}

fn evidence_manifest_state(coverage: &HiveLoopEvidenceManifestCoverage) -> String {
    if coverage.entry_count == 0 {
        "observing".to_string()
    } else if coverage.path_missing_count > 0 {
        "blocked".to_string()
    } else if coverage.path_unverified_count > 0 {
        "reviewing".to_string()
    } else {
        "ok".to_string()
    }
}

fn evidence_manifest_summary(state: &str, coverage: &HiveLoopEvidenceManifestCoverage) -> String {
    format!(
        "Evidence manifest is {state}: indexed {} entries from {} evidence rows (payloads {}, receipts {}, transcripts {}, artifacts {}, paths present/missing/unverified {}/{}/{}).",
        coverage.entry_count,
        coverage.evidence_count,
        coverage.payload_count,
        coverage.receipt_count,
        coverage.transcript_count,
        coverage.artifact_count,
        coverage.path_present_count,
        coverage.path_missing_count,
        coverage.path_unverified_count
    )
}

fn evidence_manifest_next_actions(
    loop_id: i64,
    issue: Option<&HiveIssue>,
    coverage: &HiveLoopEvidenceManifestCoverage,
) -> Vec<String> {
    let mut actions = vec![
        format!("entrance hive loop evidence-manifest {loop_id}"),
        format!("entrance hive loop evidence-drilldown {loop_id}"),
    ];
    if coverage.path_missing_count > 0 || coverage.path_unverified_count > 0 {
        if let Some(issue) = issue {
            actions.push(format!(
                "entrance hive issue decide {} request-review --body <artifact-manifest-note> --compact",
                issue.id
            ));
        }
    }
    actions
}

fn evidence_manifest_path_status(path: Option<&str>) -> String {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return "none".to_string();
    };
    let path = Path::new(path);
    if !path.is_absolute() {
        return "unverified-relative".to_string();
    }
    if std::fs::metadata(path).is_ok() {
        "present".to_string()
    } else {
        "missing".to_string()
    }
}

fn evidence_manifest_path_size_bytes(path: Option<&str>, path_status: &str) -> Option<u64> {
    if path_status != "present" {
        return None;
    }
    path.and_then(|path| std::fs::metadata(Path::new(path)).ok())
        .map(|metadata| metadata.len())
}

fn evidence_drilldown_item(
    row: &HiveLoopEvidence,
    summary: &IssueEvidenceSummary,
    previous_payload: Option<(i64, &serde_json::Value)>,
) -> HiveLoopEvidenceDrilldownItem {
    let blocker = evidence_item_blocker(row, summary);
    HiveLoopEvidenceDrilldownItem {
        id: row.id,
        round: row.round,
        stage_role: summary.stage_role.clone(),
        kind: row.kind.clone(),
        summary: row.summary.clone(),
        created_at: row.created_at.clone(),
        path: row.path.clone(),
        schema_version: summary.schema_version.clone(),
        admission_result: summary.admission_result.clone(),
        blocked_phase: summary.blocked_phase.clone(),
        blocker,
        operator_options: summary.operator_options.clone(),
        worker: evidence_worker_drilldown(summary),
        receipt: evidence_receipt_drilldown(&row.payload),
        artifacts: evidence_artifacts(row),
        payload: evidence_payload_inspection(&row.payload, previous_payload),
    }
}

fn evidence_worker_drilldown(
    summary: &IssueEvidenceSummary,
) -> Option<HiveLoopEvidenceWorkerDrilldown> {
    let has_worker = summary.worker_kind.is_some()
        || summary.worker_mode.is_some()
        || summary.worker_ok.is_some()
        || summary.worker_receipt_ok.is_some()
        || summary.worker_command.is_some()
        || summary.worker_evidence_summary.is_some()
        || !summary.worker_receipt_errors.is_empty()
        || summary.transcript_excerpt.is_some();
    if !has_worker {
        return None;
    }
    Some(HiveLoopEvidenceWorkerDrilldown {
        kind: summary.worker_kind.clone(),
        mode: summary.worker_mode.clone(),
        ok: summary.worker_ok,
        receipt_ok: summary.worker_receipt_ok,
        timed_out: summary.worker_timed_out,
        status: summary.worker_status,
        duration_ms: summary.worker_duration_ms,
        timeout_secs: summary.worker_timeout_secs,
        attempt_count: summary.worker_attempt_count,
        max_attempts: summary.worker_max_attempts,
        retry_exhausted: summary.worker_retry_exhausted,
        command: summary.worker_command.clone(),
        cwd: summary.worker_cwd.clone(),
        action: summary.worker_action.clone(),
        evidence_summary: summary.worker_evidence_summary.clone(),
        gate_count: summary.worker_gate_count,
        receipt_errors: summary.worker_receipt_errors.clone(),
        transcript_excerpt: summary.transcript_excerpt.clone(),
    })
}

fn evidence_receipt_drilldown(
    payload: &serde_json::Value,
) -> Option<HiveLoopEvidenceReceiptDrilldown> {
    let worker = payload.get("worker")?;
    let receipt = worker_structured_receipt(worker)?;
    let gates = receipt
        .get("gates")
        .and_then(|value| value.as_object())
        .map(|gates| {
            gates
                .iter()
                .map(|(name, value)| HiveLoopEvidenceReceiptGate {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(HiveLoopEvidenceReceiptDrilldown {
        schema_version: schema_version(&receipt),
        role: string_at(&receipt, "/role"),
        action: string_at(&receipt, "/action"),
        ok: receipt.get("ok").and_then(|value| value.as_bool()),
        evidence_summary: string_at(&receipt, "/evidence_summary"),
        gates,
        raw_excerpt: Some(json_excerpt(&receipt, 520)),
    })
}

fn evidence_artifacts(row: &HiveLoopEvidence) -> Vec<HiveLoopEvidenceArtifact> {
    let mut artifacts = Vec::new();
    if let Some(path) = row.path.as_ref() {
        artifacts.push(HiveLoopEvidenceArtifact {
            kind: "path".to_string(),
            path: Some(path.clone()),
            summary: Some(row.summary.clone()),
            manifest: None,
        });
    }
    for pointer in ["/artifact", "/artifact_manifest", "/manifest"] {
        if let Some(value) = row.payload.pointer(pointer) {
            artifacts.push(HiveLoopEvidenceArtifact {
                kind: pointer.trim_start_matches('/').to_string(),
                path: value
                    .get("path")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                summary: value
                    .get("summary")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                manifest: Some(value.clone()),
            });
        }
    }
    if let Some(values) = row
        .payload
        .pointer("/artifacts")
        .and_then(|value| value.as_array())
    {
        for value in values {
            artifacts.push(HiveLoopEvidenceArtifact {
                kind: "artifact".to_string(),
                path: value
                    .get("path")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                summary: value
                    .get("summary")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                manifest: Some(value.clone()),
            });
        }
    }
    artifacts
}

fn evidence_payload_inspection(
    payload: &serde_json::Value,
    previous_payload: Option<(i64, &serde_json::Value)>,
) -> HiveLoopEvidencePayloadInspection {
    let keys = top_level_keys(payload);
    HiveLoopEvidencePayloadInspection {
        top_level_keys: keys,
        excerpt: json_excerpt(payload, 720),
        diff_from_previous: evidence_payload_diff(payload, previous_payload),
    }
}

fn evidence_payload_diff(
    payload: &serde_json::Value,
    previous_payload: Option<(i64, &serde_json::Value)>,
) -> HiveLoopEvidencePayloadDiff {
    let Some((previous_id, previous)) = previous_payload else {
        return HiveLoopEvidencePayloadDiff {
            relative_to_evidence_id: None,
            added_keys: Vec::new(),
            removed_keys: Vec::new(),
            changed_keys: top_level_keys(payload),
        };
    };
    let current_keys = top_level_keys(payload).into_iter().collect::<BTreeSet<_>>();
    let previous_keys = top_level_keys(previous)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let added_keys = current_keys
        .difference(&previous_keys)
        .cloned()
        .collect::<Vec<_>>();
    let removed_keys = previous_keys
        .difference(&current_keys)
        .cloned()
        .collect::<Vec<_>>();
    let changed_keys = current_keys
        .intersection(&previous_keys)
        .filter(|key| payload.get(key.as_str()) != previous.get(key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    HiveLoopEvidencePayloadDiff {
        relative_to_evidence_id: Some(previous_id),
        added_keys,
        removed_keys,
        changed_keys,
    }
}

fn top_level_keys(value: &serde_json::Value) -> Vec<String> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn evidence_item_blocker(row: &HiveLoopEvidence, summary: &IssueEvidenceSummary) -> Option<String> {
    string_at(&row.payload, "/reason")
        .or_else(|| string_at(&row.payload, "/blocker"))
        .or_else(|| {
            (!summary.missing_receipts.is_empty())
                .then(|| format!("missing receipts: {}", summary.missing_receipts.join(", ")))
        })
        .or_else(|| {
            (!summary.packet_envelope_errors.is_empty()).then(|| {
                format!(
                    "packet envelope errors: {}",
                    summary.packet_envelope_errors.join(", ")
                )
            })
        })
        .or_else(|| {
            (!summary.worker_receipt_errors.is_empty()).then(|| {
                format!(
                    "worker receipt errors: {}",
                    summary.worker_receipt_errors.join(", ")
                )
            })
        })
}

fn evidence_drilldown_blockers(
    contract: &HiveLoopContract,
    issue: Option<&HiveIssue>,
    items: &[HiveLoopEvidenceDrilldownItem],
    actions: &[IssueAction],
) -> Vec<HiveLoopEvidenceBlocker> {
    let mut blockers = items
        .iter()
        .filter_map(|item| {
            item.blocker.as_ref().map(|reason| {
                let operator_options = item.operator_options.clone();
                HiveLoopEvidenceBlocker {
                    evidence_id: Some(item.id),
                    scope: "evidence".to_string(),
                    round: item.round,
                    kind: item.kind.clone(),
                    phase: item
                        .blocked_phase
                        .clone()
                        .or_else(|| item.stage_role.clone()),
                    reason: reason.clone(),
                    decision_surface: evidence_blocker_decision_surface(
                        issue,
                        actions,
                        &operator_options,
                        Some(item.id),
                        reason,
                    ),
                    operator_options,
                }
            })
        })
        .collect::<Vec<_>>();

    if blockers.is_empty()
        && issue
            .map(|issue| matches!(issue.status.as_str(), "Blocked" | "Needs Review"))
            .unwrap_or_default()
    {
        let reason = evidence_loop_blocker_reason(contract, issue);
        let operator_options = actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<Vec<_>>();
        blockers.push(HiveLoopEvidenceBlocker {
            evidence_id: None,
            scope: "loop".to_string(),
            round: contract.current_round,
            kind: "loop_state".to_string(),
            phase: Some(evidence_loop_blocker_phase(contract)),
            reason: reason.clone(),
            decision_surface: evidence_blocker_decision_surface(
                issue,
                actions,
                &operator_options,
                None,
                &reason,
            ),
            operator_options,
        });
    }
    blockers
}

fn evidence_loop_blocker_reason(contract: &HiveLoopContract, issue: Option<&HiveIssue>) -> String {
    if contract.status == "blocked"
        && contract.current_round >= REVIEWER_INVALID_ROUND_BUDGET
        && matches!(contract.active_phase.as_str(), "reviewer" | "complete")
    {
        return format!(
            "Reviewer invalid budget exhausted after {} round(s).",
            REVIEWER_INVALID_ROUND_BUDGET
        );
    }
    issue
        .and_then(|issue| issue.summary.clone())
        .unwrap_or_else(|| format!("Loop status is {}.", contract.status))
}

fn evidence_loop_blocker_phase(contract: &HiveLoopContract) -> String {
    if contract.status == "blocked"
        && contract.current_round >= REVIEWER_INVALID_ROUND_BUDGET
        && matches!(contract.active_phase.as_str(), "reviewer" | "complete")
    {
        "reviewer".to_string()
    } else {
        contract.active_phase.clone()
    }
}

fn evidence_blocker_decision_surface(
    issue: Option<&HiveIssue>,
    actions: &[IssueAction],
    operator_options: &[String],
    evidence_id: Option<i64>,
    reason: &str,
) -> HiveLoopEvidenceDecisionSurface {
    let issue_status = issue.map(|issue| issue.status.clone());
    let normalized_options = normalized_blocker_options(operator_options);
    let allowed_actions = if normalized_options.is_empty() {
        actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<BTreeSet<_>>()
    } else {
        normalized_options
            .iter()
            .filter(|option| actions.iter().any(|action| action.action == **option))
            .cloned()
            .collect::<BTreeSet<_>>()
    };
    let selected_actions = if allowed_actions.is_empty() {
        actions.to_vec()
    } else {
        actions
            .iter()
            .filter(|action| allowed_actions.contains(&action.action))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut decision_actions = selected_actions
        .into_iter()
        .map(|action| {
            let operator_option = operator_option_for_action(operator_options, &action.action);
            HiveLoopEvidenceDecisionAction {
                recommended: action.action == "retry" || operator_option.is_some(),
                reason: evidence_decision_action_reason(evidence_id, reason, &action),
                issue_action: action,
                operator_option,
            }
        })
        .collect::<Vec<_>>();
    decision_actions.sort_by_key(|action| evidence_decision_action_priority(&action.issue_action));
    let primary_action = decision_actions
        .iter()
        .find(|action| action.recommended)
        .or_else(|| decision_actions.first())
        .map(|action| action.issue_action.action.clone());
    let required = issue_status
        .as_deref()
        .is_some_and(|status| matches!(status, "Blocked" | "Needs Review"));
    HiveLoopEvidenceDecisionSurface {
        required,
        issue_status,
        primary_action,
        actions: decision_actions,
        policy_resource: "entrance://policy/mcp-permissions".to_string(),
        review_queue_resource: "entrance://review-queue".to_string(),
        confirmation_arg: OPERATOR_ACTION_CONFIRMATION_ARG.to_string(),
        summary: evidence_decision_surface_summary(evidence_id, reason),
    }
}

fn normalized_blocker_options(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .filter_map(|value| match value.as_str() {
            "retry" => Some("retry"),
            "request-review" | "request-human-review" | "human-review" => Some("request-review"),
            "cancel" => Some("cancel"),
            "comment" | "fix-policy" | "fix-policy-then-retry" => Some("comment"),
            _ => None,
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn operator_option_for_action(values: &[String], action: &str) -> Option<String> {
    values
        .iter()
        .find(|value| match (value.as_str(), action) {
            ("retry", "retry") => true,
            ("request-review" | "request-human-review" | "human-review", "request-review") => true,
            ("cancel", "cancel") => true,
            ("comment" | "fix-policy" | "fix-policy-then-retry", "comment") => true,
            _ => false,
        })
        .cloned()
}

fn evidence_decision_action_reason(
    evidence_id: Option<i64>,
    reason: &str,
    action: &IssueAction,
) -> String {
    let scope = evidence_id
        .map(|id| format!("evidence #{id}"))
        .unwrap_or_else(|| "loop blocker".to_string());
    match action.action.as_str() {
        "retry" => format!("Retry after changing the assumption or fix for {scope}: {reason}"),
        "request-review" => {
            format!(
                "Ask a human reviewer to decide preference, scope, or policy for {scope}: {reason}"
            )
        }
        "cancel" => format!("Cancel if {scope} makes the candidate no longer valuable: {reason}"),
        "comment" => format!("Add context or policy fix notes for {scope}: {reason}"),
        _ => format!("Apply `{}` to {scope}: {reason}", action.action),
    }
}

fn evidence_decision_action_priority(action: &IssueAction) -> usize {
    match action.action.as_str() {
        "retry" => 0,
        "request-review" => 1,
        "comment" => 2,
        "cancel" => 3,
        _ => 4,
    }
}

fn evidence_decision_surface_summary(evidence_id: Option<i64>, reason: &str) -> String {
    match evidence_id {
        Some(id) => format!("Decision surface for blocker on evidence #{id}: {reason}"),
        None => format!("Decision surface for loop-level blocker: {reason}"),
    }
}

fn evidence_drilldown_human_decision(
    issue: Option<&HiveIssue>,
    items: &[HiveLoopEvidenceDrilldownItem],
    actions: &[IssueAction],
) -> HiveLoopEvidenceHumanDecision {
    let issue_status = issue.map(|issue| issue.status.clone());
    let evidence_options = items
        .iter()
        .rev()
        .find(|item| !item.operator_options.is_empty())
        .map(|item| item.operator_options.clone())
        .unwrap_or_default();
    let options = if evidence_options.is_empty() {
        actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<Vec<_>>()
    } else {
        evidence_options
    };
    let required = issue_status
        .as_deref()
        .is_some_and(|status| matches!(status, "Blocked" | "Needs Review"))
        || actions.iter().any(|action| action.confirmation_required);
    HiveLoopEvidenceHumanDecision {
        required,
        issue_status,
        options,
        actions: actions.to_vec(),
    }
}

fn evidence_drilldown_next_actions(
    loop_id: i64,
    issue: Option<&HiveIssue>,
    actions: &[IssueAction],
) -> Vec<String> {
    let mut next = vec![
        format!("entrance hive loop evidence-drilldown {loop_id}"),
        format!("entrance hive loop evidence-manifest {loop_id}"),
        format!("entrance hive loop dashboard {loop_id}"),
        format!("entrance hive loop evidence {loop_id}"),
        format!("entrance hive loop worker-lifecycle {loop_id}"),
    ];
    if issue
        .map(|issue| matches!(issue.status.as_str(), "Blocked" | "Needs Review"))
        .unwrap_or_default()
    {
        next.extend(actions.iter().map(|action| action.command.clone()));
    }
    next
}

fn evidence_drilldown_summary(
    contract: &HiveLoopContract,
    issue: Option<&HiveIssue>,
    evidence_count: usize,
    blocker_count: usize,
) -> String {
    let issue_label = issue
        .map(|issue| format!("issue {} {}", issue.id, issue.status))
        .unwrap_or_else(|| "no issue".to_string());
    if blocker_count > 0 {
        format!(
            "Loop #{} evidence drilldown has {evidence_count} evidence item(s), {blocker_count} blocker(s), {issue_label}.",
            contract.id
        )
    } else {
        format!(
            "Loop #{} evidence drilldown has {evidence_count} evidence item(s), no blockers, {issue_label}.",
            contract.id
        )
    }
}

fn evidence_drilldown_state(
    contract: &HiveLoopContract,
    blocker_count: usize,
    human_decision: &HiveLoopEvidenceHumanDecision,
) -> String {
    if human_decision.required {
        "needs_human".to_string()
    } else if blocker_count > 0 || matches!(contract.status.as_str(), "blocked" | "needs-review") {
        "blocked".to_string()
    } else if contract.status == "kept" {
        "complete".to_string()
    } else {
        "observing".to_string()
    }
}

fn json_excerpt(value: &serde_json::Value, max_chars: usize) -> String {
    serde_json::to_string(value)
        .map(|text| truncate_text(&text, max_chars))
        .unwrap_or_else(|_| "<json unavailable>".to_string())
}

fn json_size_bytes(value: &serde_json::Value) -> Option<u64> {
    serde_json::to_vec(value)
        .ok()
        .map(|bytes| bytes.len() as u64)
}

fn sha256_json(value: &serde_json::Value) -> String {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .unwrap_or_else(|_| sha256_text("<json unavailable>"))
}

fn sha256_text(value: &str) -> String {
    sha256_bytes(value.as_bytes())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
