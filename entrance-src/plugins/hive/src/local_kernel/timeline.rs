pub fn issue_timeline(store: &Store, issue_id: i64) -> Result<IssueTimelineReport> {
    let card = issue_card(store, issue_id)?;
    let mut items = Vec::new();
    let mut sequence = 0usize;
    items.push(issue_timeline_issue_created_item(
        &card.issue,
        &mut sequence,
    ));

    for comment in &card.comments {
        items.push(issue_timeline_comment_item(
            &card.issue,
            comment,
            &mut sequence,
        ));
    }

    let mut loop_evidence = Vec::new();
    if let Some(loop_id) = card.issue.loop_id {
        let stages = store.list_hive_loop_stages(loop_id)?;
        let stage_roles = stage_role_map(&stages);
        loop_evidence = store.list_hive_loop_evidence(loop_id)?;
        for evidence in &loop_evidence {
            let summary = issue_evidence_summary(&evidence, &stage_roles);
            items.push(issue_timeline_evidence_item(
                &card.issue,
                evidence,
                &summary,
                &mut sequence,
            ));
        }
        for verdict in store.list_hive_loop_verdicts(loop_id)? {
            items.push(issue_timeline_verdict_item(
                &card.issue,
                &verdict,
                &mut sequence,
            ));
        }
    }

    items.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.sequence.cmp(&right.sequence))
    });
    for (index, item) in items.iter_mut().enumerate() {
        item.sequence = index + 1;
    }
    let counts = issue_timeline_counts(&items);
    let rounds = issue_timeline_round_groups(&items);
    let decision_receipts =
        issue_timeline_decision_receipts(&card.issue, &card.comments, &loop_evidence);
    let timeline_state = issue_timeline_state(&card.issue);
    let summary = issue_timeline_summary(&card.issue, &timeline_state, &counts, &rounds);
    let human_decision = issue_timeline_human_decision(&card, &items, &decision_receipts);
    let next_actions = issue_timeline_next_actions(&card);
    Ok(IssueTimelineReport {
        schema_version: ISSUE_TIMELINE_SCHEMA_VERSION.to_string(),
        issue: card.issue.clone(),
        loop_id: card.issue.loop_id,
        timeline_state,
        summary,
        counts,
        rounds,
        human_decision,
        decision_receipts,
        items,
        resources: issue_timeline_resources(&card.issue),
        next_actions,
    })
}

pub fn issue_timeline_item(
    store: &Store,
    issue_id: i64,
    item_id: &str,
) -> Result<IssueTimelineItemReport> {
    let timeline = issue_timeline(store, issue_id)?;
    let item_index = timeline
        .items
        .iter()
        .position(|item| item.id == item_id)
        .with_context(|| format!("unknown issue #{issue_id} timeline item `{item_id}`"))?;
    let item = timeline.items[item_index].clone();
    let previous_item_id = item_index
        .checked_sub(1)
        .and_then(|index| timeline.items.get(index))
        .map(|item| item.id.clone());
    let next_item_id = timeline
        .items
        .get(item_index + 1)
        .map(|item| item.id.clone());
    let round = timeline
        .rounds
        .iter()
        .find(|round| round.round == item.round)
        .cloned();
    let decision_receipt = timeline
        .decision_receipts
        .iter()
        .find(|receipt| {
            item.comment_id.is_some() && receipt.comment_id == item.comment_id
                || item.evidence_id.is_some() && receipt.evidence_id == item.evidence_id
        })
        .cloned();
    let resources = issue_timeline_item_resources(&timeline.issue, &item);
    let mut next_actions = vec![
        format!("entrance hive issue timeline-item {issue_id} {}", item.id),
        format!("entrance hive issue timeline {issue_id}"),
    ];
    if let Some(linked) = item.linked_resource.as_ref() {
        next_actions.push(format!("open resource {linked}"));
    }
    Ok(IssueTimelineItemReport {
        schema_version: ISSUE_TIMELINE_ITEM_SCHEMA_VERSION.to_string(),
        issue: timeline.issue.clone(),
        loop_id: timeline.loop_id,
        item,
        item_index: item_index + 1,
        previous_item_id,
        next_item_id,
        round,
        decision_receipt,
        resources,
        next_actions,
    })
}

fn issue_timeline_issue_created_item(issue: &HiveIssue, sequence: &mut usize) -> IssueTimelineItem {
    *sequence += 1;
    let id = format!("issue-{}-created", issue.id);
    IssueTimelineItem {
        permalink: issue_timeline_item_permalink(issue.id, &id),
        id,
        sequence: *sequence,
        timestamp: issue.created_at.clone(),
        source: "issue".to_string(),
        event_kind: "issue_created".to_string(),
        actor: "kernel".to_string(),
        round: None,
        status: Some(issue.status.clone()),
        phase: None,
        title: format!("Issue #{} created", issue.id),
        summary: issue
            .summary
            .clone()
            .unwrap_or_else(|| "Issue created.".to_string()),
        body_excerpt: None,
        schema_version: None,
        comment_id: None,
        evidence_id: None,
        verdict_id: None,
        action: None,
        decision: None,
        blocker: None,
        linked_resource: Some(format!("entrance://issues/{}", issue.id)),
        details: serde_json::json!({
            "title": issue.title.clone(),
            "created_at": issue.created_at.clone(),
            "updated_at": issue.updated_at.clone()
        }),
    }
}

fn issue_timeline_comment_item(
    issue: &HiveIssue,
    comment: &HiveComment,
    sequence: &mut usize,
) -> IssueTimelineItem {
    *sequence += 1;
    let schema = schema_version(&comment.payload);
    let event_kind = issue_timeline_comment_kind(schema.as_deref(), &comment.payload);
    let action = string_at(&comment.payload, "/action");
    let round = comment
        .payload
        .pointer("/round")
        .or_else(|| comment.payload.pointer("/next_round"))
        .and_then(|value| value.as_i64());
    let evidence_id = comment
        .payload
        .pointer("/evidence_id")
        .and_then(|value| value.as_i64());
    IssueTimelineItem {
        id: format!("comment-{}", comment.id),
        permalink: issue_timeline_item_permalink(issue.id, &format!("comment-{}", comment.id)),
        sequence: *sequence,
        timestamp: comment.created_at.clone(),
        source: "comment".to_string(),
        event_kind: event_kind.clone(),
        actor: comment.author.clone(),
        round,
        status: string_at(&comment.payload, "/status").or_else(|| Some(issue.status.clone())),
        phase: string_at(&comment.payload, "/phase")
            .or_else(|| string_at(&comment.payload, "/stage_role")),
        title: issue_timeline_comment_title(comment, &event_kind, action.as_deref()),
        summary: truncate_text(&comment.body, 180),
        body_excerpt: Some(truncate_text(&comment.body, 520)),
        schema_version: schema,
        comment_id: Some(comment.id),
        evidence_id,
        verdict_id: None,
        action,
        decision: None,
        blocker: string_at(&comment.payload, "/blocker"),
        linked_resource: Some(format!("entrance://issues/{}/timeline", issue.id)),
        details: serde_json::json!({
            "payload_top_level_keys": top_level_keys(&comment.payload),
            "payload_excerpt": json_excerpt(&comment.payload, 520),
            "confirmation_receipt_schema": comment.payload.pointer("/confirmation_receipt/schema_version").and_then(|value| value.as_str())
        }),
    }
}

fn issue_timeline_evidence_item(
    issue: &HiveIssue,
    evidence: &HiveLoopEvidence,
    summary: &IssueEvidenceSummary,
    sequence: &mut usize,
) -> IssueTimelineItem {
    *sequence += 1;
    let blocker = evidence_item_blocker(evidence, summary);
    let actor = summary
        .operator_author
        .clone()
        .or_else(|| summary.stage_role.clone())
        .unwrap_or_else(|| "hive".to_string());
    IssueTimelineItem {
        id: format!("evidence-{}", evidence.id),
        permalink: issue_timeline_item_permalink(issue.id, &format!("evidence-{}", evidence.id)),
        sequence: *sequence,
        timestamp: evidence.created_at.clone(),
        source: "evidence".to_string(),
        event_kind: evidence.kind.clone(),
        actor,
        round: Some(evidence.round),
        status: summary.admission_result.clone().or_else(|| {
            summary
                .worker_ok
                .map(|ok| if ok { "ok" } else { "failed" }.to_string())
        }),
        phase: summary
            .blocked_phase
            .clone()
            .or_else(|| summary.stage_role.clone()),
        title: format!(
            "Evidence #{} {} {}",
            evidence.id,
            summary.stage_role.as_deref().unwrap_or("kernel"),
            evidence.kind
        ),
        summary: evidence.summary.clone(),
        body_excerpt: summary
            .transcript_excerpt
            .clone()
            .or_else(|| Some(json_excerpt(&evidence.payload, 520))),
        schema_version: summary.schema_version.clone(),
        comment_id: evidence
            .payload
            .pointer("/issue/comment_id")
            .and_then(|value| value.as_i64()),
        evidence_id: Some(evidence.id),
        verdict_id: None,
        action: summary
            .operator_action
            .clone()
            .or_else(|| summary.worker_action.clone()),
        decision: None,
        blocker,
        linked_resource: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/evidence-drilldown")),
        details: serde_json::json!({
            "kind": evidence.kind.clone(),
            "worker_kind": summary.worker_kind.clone(),
            "worker_mode": summary.worker_mode.clone(),
            "worker_ok": summary.worker_ok,
            "receipt_ok": summary.worker_receipt_ok,
            "receipt_errors": summary.worker_receipt_errors.clone(),
            "missing_receipts": summary.missing_receipts.clone(),
            "top_level_keys": top_level_keys(&evidence.payload)
        }),
    }
}

fn issue_timeline_verdict_item(
    issue: &HiveIssue,
    verdict: &HiveLoopVerdict,
    sequence: &mut usize,
) -> IssueTimelineItem {
    *sequence += 1;
    IssueTimelineItem {
        id: format!("verdict-{}", verdict.id),
        permalink: issue_timeline_item_permalink(issue.id, &format!("verdict-{}", verdict.id)),
        sequence: *sequence,
        timestamp: verdict.created_at.clone(),
        source: "verdict".to_string(),
        event_kind: "verdict".to_string(),
        actor: "reviewer".to_string(),
        round: Some(verdict.round),
        status: Some(verdict.decision.clone()),
        phase: Some("reviewer".to_string()),
        title: format!("Verdict #{} {}", verdict.id, verdict.decision),
        summary: verdict.summary.clone(),
        body_excerpt: Some(json_excerpt(&verdict.evidence, 520)),
        schema_version: schema_version(&verdict.evidence),
        comment_id: None,
        evidence_id: verdict
            .evidence
            .pointer("/reviewer_evidence_id")
            .and_then(|value| value.as_i64()),
        verdict_id: Some(verdict.id),
        action: None,
        decision: Some(verdict.decision.clone()),
        blocker: verdict
            .evidence
            .pointer("/blocker")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        linked_resource: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/dashboard")),
        details: serde_json::json!({
            "score_vector": score_vector(&verdict.score),
            "reason_code": verdict.evidence.pointer("/reason_code").and_then(|value| value.as_str()),
            "human_options": verdict.evidence.pointer("/human_options").cloned().unwrap_or_else(|| serde_json::json!([]))
        }),
    }
}

fn issue_timeline_comment_kind(schema: Option<&str>, payload: &serde_json::Value) -> String {
    match schema {
        Some(OPERATOR_DECISION_SCHEMA_VERSION) => "operator_decision".to_string(),
        Some(OPERATOR_COMMENT_SCHEMA_VERSION) => "operator_comment".to_string(),
        Some(SYSTEM_COMMENT_SCHEMA_VERSION) => {
            if payload.get("stage_role").is_some() || payload.get("evidence_id").is_some() {
                "stage_comment".to_string()
            } else {
                "system_comment".to_string()
            }
        }
        Some(value) => value.rsplit('.').next().unwrap_or("comment").to_string(),
        None => "comment".to_string(),
    }
}

fn issue_timeline_comment_title(
    comment: &HiveComment,
    event_kind: &str,
    action: Option<&str>,
) -> String {
    match action {
        Some(action) => format!("{} {}", comment.author, action),
        None => format!("{} {}", comment.author, event_kind.replace('_', " ")),
    }
}

fn issue_timeline_counts(items: &[IssueTimelineItem]) -> IssueTimelineCounts {
    let comment_count = items.iter().filter(|item| item.source == "comment").count();
    let evidence_count = items
        .iter()
        .filter(|item| item.source == "evidence")
        .count();
    let verdict_count = items.iter().filter(|item| item.source == "verdict").count();
    let operator_event_count = items
        .iter()
        .filter(|item| {
            matches!(
                item.event_kind.as_str(),
                "operator_comment" | "operator_decision"
            ) || item.actor == "human"
                || item.actor == "operator"
        })
        .count();
    let blocker_count = items.iter().filter(|item| item.blocker.is_some()).count();
    let receipt_issue_count = items
        .iter()
        .filter(|item| {
            item.details
                .pointer("/receipt_errors")
                .and_then(|value| value.as_array())
                .is_some_and(|values| !values.is_empty())
                || item
                    .details
                    .pointer("/missing_receipts")
                    .and_then(|value| value.as_array())
                    .is_some_and(|values| !values.is_empty())
        })
        .count();
    let decision_receipt_count = items
        .iter()
        .filter(|item| {
            item.details
                .pointer("/confirmation_receipt_schema")
                .and_then(|value| value.as_str())
                .is_some()
        })
        .count();
    IssueTimelineCounts {
        item_count: items.len(),
        comment_count,
        evidence_count,
        verdict_count,
        operator_event_count,
        blocker_count,
        receipt_issue_count,
        decision_receipt_count,
    }
}

fn issue_timeline_round_groups(items: &[IssueTimelineItem]) -> Vec<IssueTimelineRoundGroup> {
    let mut grouped: BTreeMap<Option<i64>, Vec<&IssueTimelineItem>> = BTreeMap::new();
    for item in items {
        grouped.entry(item.round).or_default().push(item);
    }
    grouped
        .into_iter()
        .map(|(round, group_items)| {
            let comment_count = group_items
                .iter()
                .filter(|item| item.source == "comment")
                .count();
            let evidence_count = group_items
                .iter()
                .filter(|item| item.source == "evidence")
                .count();
            let verdict_count = group_items
                .iter()
                .filter(|item| item.source == "verdict")
                .count();
            let operator_event_count = group_items
                .iter()
                .filter(|item| {
                    matches!(
                        item.event_kind.as_str(),
                        "operator_comment" | "operator_decision"
                    ) || item.actor == "human"
                        || item.actor == "operator"
                })
                .count();
            let blocker_count = group_items
                .iter()
                .filter(|item| item.blocker.is_some())
                .count();
            let phases = group_items
                .iter()
                .filter_map(|item| item.phase.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let decisions = group_items
                .iter()
                .filter_map(|item| item.decision.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            IssueTimelineRoundGroup {
                round,
                label: round
                    .map(|round| format!("round {round}"))
                    .unwrap_or_else(|| "issue".to_string()),
                state: issue_timeline_round_state(blocker_count, &decisions, verdict_count),
                item_ids: group_items.iter().map(|item| item.id.clone()).collect(),
                item_count: group_items.len(),
                comment_count,
                evidence_count,
                verdict_count,
                operator_event_count,
                blocker_count,
                first_timestamp: group_items.first().map(|item| item.timestamp.clone()),
                last_timestamp: group_items.last().map(|item| item.timestamp.clone()),
                phases,
                decisions,
            }
        })
        .collect()
}

fn issue_timeline_round_state(
    blocker_count: usize,
    decisions: &[String],
    verdict_count: usize,
) -> String {
    if blocker_count > 0 || decisions.iter().any(|decision| decision == "blocked") {
        "blocked".to_string()
    } else if decisions.iter().any(|decision| decision == "needs-review") {
        "needs_human".to_string()
    } else if decisions.iter().any(|decision| decision == "keep") {
        "kept".to_string()
    } else if decisions.iter().any(|decision| decision == "reject") {
        "rejected".to_string()
    } else if verdict_count > 0 {
        "reviewed".to_string()
    } else {
        "observing".to_string()
    }
}

fn issue_timeline_state(issue: &HiveIssue) -> String {
    match issue.status.as_str() {
        "Blocked" | "Needs Review" => "needs_human".to_string(),
        "Running" => "running".to_string(),
        "Done" | "Canceled" => "closed".to_string(),
        "Todo" => "open".to_string(),
        _ => "observing".to_string(),
    }
}

fn issue_timeline_summary(
    issue: &HiveIssue,
    state: &str,
    counts: &IssueTimelineCounts,
    rounds: &[IssueTimelineRoundGroup],
) -> String {
    format!(
        "Issue #{} timeline is {state}: {} items, {} round groups, {} comments, {} evidence rows, {} verdicts, {} operator events.",
        issue.id,
        counts.item_count,
        rounds.len(),
        counts.comment_count,
        counts.evidence_count,
        counts.verdict_count,
        counts.operator_event_count
    )
}

fn issue_timeline_decision_receipts(
    issue: &HiveIssue,
    comments: &[HiveComment],
    evidence: &[HiveLoopEvidence],
) -> Vec<IssueTimelineDecisionReceipt> {
    let mut evidence_by_comment_id = HashMap::new();
    for row in evidence
        .iter()
        .filter(|row| row.kind == "operator_decision")
    {
        if let Some(comment_id) = row
            .payload
            .pointer("/issue/comment_id")
            .and_then(|value| value.as_i64())
        {
            evidence_by_comment_id.insert(comment_id, row);
        }
    }

    let mut seen_evidence_ids = BTreeSet::new();
    let mut receipts = Vec::new();
    for comment in comments {
        let Some(receipt) = comment.payload.get("confirmation_receipt") else {
            continue;
        };
        let evidence_row = evidence_by_comment_id.get(&comment.id).copied();
        if let Some(row) = evidence_row {
            seen_evidence_ids.insert(row.id);
        }
        receipts.push(issue_timeline_comment_decision_receipt(
            issue,
            comment,
            receipt,
            evidence_row,
        ));
    }

    for row in evidence
        .iter()
        .filter(|row| row.kind == "operator_decision")
    {
        if seen_evidence_ids.contains(&row.id) {
            continue;
        }
        let Some(receipt) = row.payload.pointer("/operator/confirmation_receipt") else {
            continue;
        };
        receipts.push(issue_timeline_evidence_decision_receipt(
            issue, row, receipt,
        ));
    }

    receipts.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.id.cmp(&right.id))
    });
    receipts
}

fn issue_timeline_comment_decision_receipt(
    issue: &HiveIssue,
    comment: &HiveComment,
    receipt: &serde_json::Value,
    evidence: Option<&HiveLoopEvidence>,
) -> IssueTimelineDecisionReceipt {
    let evidence_id = evidence.map(|row| row.id);
    IssueTimelineDecisionReceipt {
        id: format!("comment-{}-receipt", comment.id),
        source: if evidence_id.is_some() {
            "comment+evidence".to_string()
        } else {
            "comment".to_string()
        },
        timestamp: comment.created_at.clone(),
        round: comment
            .payload
            .pointer("/next_round")
            .or_else(|| comment.payload.pointer("/round"))
            .and_then(|value| value.as_i64()),
        action: string_at(&comment.payload, "/action").or_else(|| string_at(receipt, "/action")),
        author: Some(comment.author.clone()).or_else(|| string_at(receipt, "/author")),
        comment_id: Some(comment.id),
        evidence_id,
        receipt_schema_version: string_at(receipt, "/schema_version"),
        receipt_source: string_at(receipt, "/source"),
        policy_schema_version: string_at(receipt, "/policy_schema_version"),
        confirmation_arg: string_at(receipt, "/confirmation_arg"),
        human_confirmed: receipt
            .pointer("/human_confirmed")
            .and_then(|value| value.as_bool()),
        client_name: string_at(receipt, "/client/name"),
        actor_label: string_at(receipt, "/actor/label"),
        actor_trust: string_at(receipt, "/actor/trust"),
        note_excerpt: string_at(&comment.payload, "/note")
            .filter(|note| !note.trim().is_empty())
            .map(|note| truncate_text(&note, 180)),
        linked_resource: issue
            .loop_id
            .filter(|_| evidence_id.is_some())
            .map(|loop_id| format!("entrance://loops/{loop_id}/evidence-drilldown"))
            .unwrap_or_else(|| format!("entrance://issues/{}/timeline", issue.id)),
        details: serde_json::json!({
            "comment_payload_schema": schema_version(&comment.payload),
            "evidence_id": evidence_id,
            "receipt_marker": receipt.pointer("/marker").and_then(|value| value.as_str()),
            "receipt_actor_verified": receipt.pointer("/actor/verified").and_then(|value| value.as_bool())
        }),
    }
}

fn issue_timeline_evidence_decision_receipt(
    issue: &HiveIssue,
    evidence: &HiveLoopEvidence,
    receipt: &serde_json::Value,
) -> IssueTimelineDecisionReceipt {
    IssueTimelineDecisionReceipt {
        id: format!("evidence-{}-receipt", evidence.id),
        source: "evidence".to_string(),
        timestamp: evidence.created_at.clone(),
        round: Some(evidence.round),
        action: string_at(&evidence.payload, "/operator/action")
            .or_else(|| string_at(receipt, "/action")),
        author: string_at(&evidence.payload, "/operator/author")
            .or_else(|| string_at(receipt, "/author")),
        comment_id: evidence
            .payload
            .pointer("/issue/comment_id")
            .and_then(|value| value.as_i64()),
        evidence_id: Some(evidence.id),
        receipt_schema_version: string_at(receipt, "/schema_version"),
        receipt_source: string_at(receipt, "/source"),
        policy_schema_version: string_at(receipt, "/policy_schema_version"),
        confirmation_arg: string_at(receipt, "/confirmation_arg"),
        human_confirmed: receipt
            .pointer("/human_confirmed")
            .and_then(|value| value.as_bool()),
        client_name: string_at(receipt, "/client/name"),
        actor_label: string_at(receipt, "/actor/label"),
        actor_trust: string_at(receipt, "/actor/trust"),
        note_excerpt: string_at(&evidence.payload, "/operator/note")
            .filter(|note| !note.trim().is_empty())
            .map(|note| truncate_text(&note, 180)),
        linked_resource: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/evidence-drilldown"))
            .unwrap_or_else(|| format!("entrance://issues/{}/timeline", issue.id)),
        details: serde_json::json!({
            "evidence_kind": evidence.kind.clone(),
            "receipt_marker": receipt.pointer("/marker").and_then(|value| value.as_str()),
            "receipt_actor_verified": receipt.pointer("/actor/verified").and_then(|value| value.as_bool())
        }),
    }
}

fn issue_timeline_human_decision(
    card: &IssueCard,
    items: &[IssueTimelineItem],
    decision_receipts: &[IssueTimelineDecisionReceipt],
) -> IssueTimelineHumanDecision {
    let issue_status = Some(card.issue.status.clone());
    let required = matches!(card.issue.status.as_str(), "Blocked" | "Needs Review");
    let operator_options = card
        .trace
        .as_ref()
        .map(|trace| trace.human_options.clone())
        .unwrap_or_else(|| {
            card.actions
                .iter()
                .map(|action| action.action.clone())
                .collect::<Vec<_>>()
        });
    let reason = issue_timeline_decision_reason(card, items);
    let normalized_options = normalized_blocker_options(&operator_options);
    let allowed_actions = if normalized_options.is_empty() {
        card.actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<BTreeSet<_>>()
    } else {
        normalized_options
            .iter()
            .filter(|option| card.actions.iter().any(|action| action.action == **option))
            .cloned()
            .collect::<BTreeSet<_>>()
    };
    let selected_actions = if allowed_actions.is_empty() {
        card.actions.clone()
    } else {
        card.actions
            .iter()
            .filter(|action| allowed_actions.contains(&action.action))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut actions = selected_actions
        .into_iter()
        .map(|action| {
            let operator_option = operator_option_for_action(&operator_options, &action.action);
            IssueTimelineDecisionAction {
                recommended: action.action == "retry"
                    || (required && action.action == "request-review")
                    || operator_option.is_some(),
                reason: issue_timeline_decision_action_reason(&card.issue, &reason, &action),
                issue_action: action,
                operator_option,
            }
        })
        .collect::<Vec<_>>();
    actions.sort_by_key(|action| evidence_decision_action_priority(&action.issue_action));
    let primary_action = actions
        .iter()
        .find(|action| action.recommended && action.issue_action.action != "comment")
        .or_else(|| actions.iter().find(|action| action.recommended))
        .or_else(|| actions.first())
        .map(|action| action.issue_action.action.clone());
    IssueTimelineHumanDecision {
        required,
        issue_status,
        primary_action: primary_action.clone(),
        actions,
        receipt_count: decision_receipts.len(),
        last_receipt: decision_receipts.last().cloned(),
        policy_resource: "entrance://policy/mcp-permissions".to_string(),
        review_queue_resource: "entrance://review-queue".to_string(),
        issue_control_resource: format!("entrance://issues/{}/control", card.issue.id),
        confirmation_arg: OPERATOR_ACTION_CONFIRMATION_ARG.to_string(),
        summary: issue_timeline_decision_summary(
            &card.issue,
            required,
            primary_action.as_deref(),
            &reason,
        ),
    }
}

fn issue_timeline_decision_reason(card: &IssueCard, items: &[IssueTimelineItem]) -> String {
    items
        .iter()
        .rev()
        .find_map(|item| item.blocker.clone())
        .or_else(|| {
            items
                .iter()
                .rev()
                .find(|item| item.decision.as_deref() == Some("blocked"))
                .map(|item| item.summary.clone())
        })
        .or_else(|| card.issue.summary.clone())
        .unwrap_or_else(|| format!("Issue #{} status is {}.", card.issue.id, card.issue.status))
}

fn issue_timeline_decision_action_reason(
    issue: &HiveIssue,
    reason: &str,
    action: &IssueAction,
) -> String {
    match action.action.as_str() {
        "retry" => format!("Retry issue #{} after addressing: {reason}", issue.id),
        "request-review" => format!(
            "Ask a human reviewer to resolve issue #{}: {reason}",
            issue.id
        ),
        "cancel" => format!(
            "Cancel issue #{} if this blocker makes the goal invalid: {reason}",
            issue.id
        ),
        "comment" => format!("Add context to issue #{}: {reason}", issue.id),
        _ => format!("Apply `{}` to issue #{}: {reason}", action.action, issue.id),
    }
}

fn issue_timeline_decision_summary(
    issue: &HiveIssue,
    required: bool,
    primary_action: Option<&str>,
    reason: &str,
) -> String {
    if required {
        format!(
            "Issue #{} is {} and requires a human decision; primary action is {}. Reason: {reason}",
            issue.id,
            issue.status,
            primary_action.unwrap_or("comment")
        )
    } else {
        format!(
            "Issue #{} is {} and does not require a blocking human decision; primary action is {}.",
            issue.id,
            issue.status,
            primary_action.unwrap_or("comment")
        )
    }
}

fn issue_transition_state_class(issue: &HiveIssue) -> &'static str {
    match issue.status.as_str() {
        "Todo" => "runnable",
        "Doing" => "running",
        "Blocked" | "Needs Review" => "needs_human",
        "Done" | "Canceled" => "terminal",
        _ => "unknown",
    }
}

fn issue_transition_policy_action(
    issue: &HiveIssue,
    action: &IssueAction,
) -> IssueTransitionPolicyAction {
    let policy = issue_transition_action_policy(&action.action);
    IssueTransitionPolicyAction {
        action: action.clone(),
        from_status: issue.status.clone(),
        to_status: issue_transition_action_to_status(issue, action, policy.as_ref()),
        gate: policy
            .as_ref()
            .map(|policy| policy.gate.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        requires_human: action.confirmation_required,
        rationale: issue_transition_action_rationale(issue, action),
    }
}

fn issue_transition_action_to_status(
    issue: &HiveIssue,
    action: &IssueAction,
    policy: Option<&IssueTransitionActionPolicySpec>,
) -> Option<String> {
    let Some(policy) = policy else {
        return None;
    };
    if action.action == "comment" && policy.to_status == "same_status" {
        Some(issue.status.clone())
    } else {
        Some(policy.to_status.clone())
    }
}

fn issue_transition_action_rationale(issue: &HiveIssue, action: &IssueAction) -> String {
    match action.action.as_str() {
        "run" => format!(
            "Issue #{} is Todo and loop-bound, so the kernel can run Explorer -> Developer -> Reviewer.",
            issue.id
        ),
        "comment" => format!(
            "Comments are ledger entries that preserve context without changing issue #{} status.",
            issue.id
        ),
        "retry" => format!(
            "Retry is a human-confirmed boundary that opens the next loop round for issue #{}.",
            issue.id
        ),
        "request-review" => format!(
            "Review moves blocked issue #{} to Needs Review for an explicit human decision.",
            issue.id
        ),
        "cancel" => format!(
            "Cancel is a human-confirmed terminal transition for issue #{}.",
            issue.id
        ),
        _ => format!("Action `{}` is exposed by the issue action contract.", action.action),
    }
}

fn issue_transition_blocked_actions(
    issue: &HiveIssue,
    actions: &[IssueAction],
) -> Vec<IssueTransitionPolicyBlockedAction> {
    let allowed = actions
        .iter()
        .map(|action| action.action.as_str())
        .collect::<BTreeSet<_>>();
    ["run", "comment", "retry", "request-review", "cancel"]
        .into_iter()
        .filter(|action| !allowed.contains(action))
        .map(|action| issue_transition_blocked_action(issue, action))
        .collect()
}

fn issue_transition_blocked_action(
    issue: &HiveIssue,
    action: &str,
) -> IssueTransitionPolicyBlockedAction {
    match action {
        "run" => IssueTransitionPolicyBlockedAction {
            action: action.to_string(),
            required_statuses: vec!["Todo".to_string()],
            reason: if issue.loop_id.is_some() {
                format!(
                    "`run` is only allowed from Todo; issue #{} is {}.",
                    issue.id, issue.status
                )
            } else {
                format!("`run` requires a loop-bound issue; issue #{} has no loop.", issue.id)
            },
            hint: matches!(issue.status.as_str(), "Blocked" | "Needs Review").then(|| {
                format!(
                    "Use `entrance hive issue retry-run {} --body <note> --human-confirmed --compact`.",
                    issue.id
                )
            }),
        },
        "comment" => IssueTransitionPolicyBlockedAction {
            action: action.to_string(),
            required_statuses: vec![
                "Todo".to_string(),
                "Doing".to_string(),
                "Blocked".to_string(),
                "Needs Review".to_string(),
                "Done".to_string(),
                "Canceled".to_string(),
            ],
            reason: format!("No comment action is exposed for issue #{}.", issue.id),
            hint: Some(format!(
                "Use `entrance hive issue comment {} --body <text> --compact` if this is unexpected.",
                issue.id
            )),
        },
        "retry" => IssueTransitionPolicyBlockedAction {
            action: action.to_string(),
            required_statuses: vec![
                "Blocked".to_string(),
                "Needs Review".to_string(),
                "Canceled(retryable)".to_string(),
            ],
            reason: format!(
                "`retry` is only exposed when issue #{} is waiting on a human recovery decision.",
                issue.id
            ),
            hint: Some("Wait for a Blocked/Needs Review state or add a comment with context.".to_string()),
        },
        "request-review" => IssueTransitionPolicyBlockedAction {
            action: action.to_string(),
            required_statuses: vec!["Blocked".to_string()],
            reason: format!(
                "`request-review` is only exposed for Blocked issues; issue #{} is {}.",
                issue.id, issue.status
            ),
            hint: Some("Blocked issues can be escalated to Needs Review.".to_string()),
        },
        "cancel" => IssueTransitionPolicyBlockedAction {
            action: action.to_string(),
            required_statuses: vec![
                "Todo".to_string(),
                "Blocked".to_string(),
                "Needs Review".to_string(),
            ],
            reason: format!(
                "`cancel` is not exposed for issue #{} in {}.",
                issue.id, issue.status
            ),
            hint: matches!(issue.status.as_str(), "Done" | "Canceled")
                .then(|| "Terminal issues are comment-only.".to_string()),
        },
        _ => IssueTransitionPolicyBlockedAction {
            action: action.to_string(),
            required_statuses: Vec::new(),
            reason: format!("Unknown issue action `{action}`."),
            hint: None,
        },
    }
}

fn issue_transition_confirmation_policy(
    actions: &[IssueAction],
) -> IssueTransitionConfirmationPolicy {
    let registry = issue_transition_policy_registry();
    let mut required_actions = actions
        .iter()
        .filter(|action| action.confirmation_required)
        .map(|action| action.action.clone())
        .collect::<Vec<_>>();
    required_actions.sort();
    required_actions.dedup();
    IssueTransitionConfirmationPolicy {
        required: !required_actions.is_empty(),
        required_actions,
        confirmation_arg: registry.confirmation.confirmation_arg,
        receipt_schema: registry.confirmation.receipt_schema,
        policy_schema_version: registry.confirmation.policy_schema_version,
        policy_resource: registry.confirmation.policy_resource,
        review_queue_resource: "entrance://review-queue".to_string(),
        actor_identity_resource: registry.confirmation.actor_identity_resource,
    }
}

fn issue_transition_reviewer_budget(
    lifecycle: &HiveLoopWorkerLifecycleReport,
    trace: Option<&IssueTraceSummary>,
) -> IssueTransitionReviewerBudget {
    let registry = issue_transition_policy_registry();
    IssueTransitionReviewerBudget {
        current_round: lifecycle.current_round,
        reviewer_invalid_rounds_used: lifecycle.current.reviewer_invalid_rounds_used,
        reviewer_invalid_round_budget: registry.reviewer_fallback.invalid_round_budget,
        reviewer_invalid_budget_exhausted: lifecycle.current.reviewer_invalid_budget_exhausted,
        fallback_status: registry.reviewer_fallback.fallback_status,
        current_decision: lifecycle
            .current
            .decision
            .clone()
            .or_else(|| trace.and_then(|trace| trace.last_decision.clone())),
        reason_code: trace.and_then(|trace| trace.reason_code.clone()),
    }
}

fn issue_transition_reviewer_budget_from_trace(
    trace: &IssueTraceSummary,
) -> IssueTransitionReviewerBudget {
    let registry = issue_transition_policy_registry();
    let current_decision = trace
        .rounds
        .iter()
        .find(|round| round.round == trace.current_round)
        .and_then(|round| round.decision.clone())
        .or_else(|| trace.last_decision.clone());
    let reviewer_invalid_rounds_used =
        reviewer_invalid_streak_from_rounds(&trace.rounds, trace.current_round)
            .min(registry.reviewer_fallback.invalid_round_budget);
    let reviewer_invalid_budget_exhausted = reviewer_invalid_rounds_used
        >= registry.reviewer_fallback.invalid_round_budget
        && trace.reason_code.as_deref() == Some("review_budget_exhausted");
    IssueTransitionReviewerBudget {
        current_round: trace.current_round,
        reviewer_invalid_rounds_used,
        reviewer_invalid_round_budget: registry.reviewer_fallback.invalid_round_budget,
        reviewer_invalid_budget_exhausted,
        fallback_status: registry.reviewer_fallback.fallback_status,
        current_decision,
        reason_code: trace.reason_code.clone(),
    }
}

fn issue_transition_policy_summary(
    issue: &HiveIssue,
    state_class: &str,
    allowed_count: usize,
    blocked_count: usize,
    human_decision_required: bool,
    reviewer_budget: Option<&IssueTransitionReviewerBudget>,
) -> String {
    let human = if human_decision_required {
        "requires a human decision"
    } else {
        "does not require a human decision"
    };
    let budget = reviewer_budget
        .map(|budget| {
            format!(
                " Reviewer invalid budget: {}/{} used; fallback {}{}.",
                budget.reviewer_invalid_rounds_used,
                budget.reviewer_invalid_round_budget,
                budget.fallback_status,
                if budget.reviewer_invalid_budget_exhausted {
                    " exhausted"
                } else {
                    ""
                }
            )
        })
        .unwrap_or_default();
    format!(
        "Issue #{} is {} ({state_class}) and {human}; {} actions allowed, {} blocked.{budget}",
        issue.id, issue.status, allowed_count, blocked_count
    )
}

fn issue_transition_policy_resources(issue: &HiveIssue) -> IssueTransitionPolicyResources {
    IssueTransitionPolicyResources {
        issue: format!("entrance://issues/{}", issue.id),
        issue_control: format!("entrance://issues/{}/control", issue.id),
        transition_policy: format!("entrance://issues/{}/transition-policy", issue.id),
        issue_timeline: format!("entrance://issues/{}/timeline", issue.id),
        loop_dashboard: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/dashboard")),
        worker_lifecycle: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/worker-lifecycle")),
        runtime_preflight: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/runtime-preflight")),
        review_queue: "entrance://review-queue".to_string(),
        policy_registry: "entrance://policy/registry".to_string(),
    }
}

fn issue_timeline_resources(issue: &HiveIssue) -> IssueTimelineResources {
    IssueTimelineResources {
        issue: format!("entrance://issues/{}", issue.id),
        issue_control: format!("entrance://issues/{}/control", issue.id),
        issue_timeline: format!("entrance://issues/{}/timeline", issue.id),
        loop_dashboard: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/dashboard")),
        evidence_drilldown: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/evidence-drilldown")),
        evidence_manifest: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/evidence-manifest")),
        runtime_preflight: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/runtime-preflight")),
        worker_lifecycle: issue
            .loop_id
            .map(|loop_id| format!("entrance://loops/{loop_id}/worker-lifecycle")),
        review_queue: "entrance://review-queue".to_string(),
    }
}

fn issue_timeline_item_resources(
    issue: &HiveIssue,
    item: &IssueTimelineItem,
) -> IssueTimelineItemResources {
    IssueTimelineItemResources {
        issue: format!("entrance://issues/{}", issue.id),
        issue_control: format!("entrance://issues/{}/control", issue.id),
        issue_timeline: format!("entrance://issues/{}/timeline", issue.id),
        item_permalink: item.permalink.clone(),
        linked_resource: item.linked_resource.clone(),
        review_queue: "entrance://review-queue".to_string(),
    }
}

fn issue_timeline_item_permalink(issue_id: i64, item_id: &str) -> String {
    format!("entrance://issues/{issue_id}/timeline/items/{item_id}")
}

fn issue_timeline_next_actions(card: &IssueCard) -> Vec<String> {
    let mut actions = vec![
        format!("entrance hive issue timeline {}", card.issue.id),
        format!("entrance hive issue show {} --compact", card.issue.id),
    ];
    if let Some(loop_id) = card.issue.loop_id {
        actions.push(format!("entrance hive loop dashboard {loop_id}"));
    }
    actions.extend(card.actions.iter().map(|action| action.command.clone()));
    actions
}
