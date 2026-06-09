pub fn panel(store: &Store) -> Result<Vec<IssueCard>> {
    store
        .list_hive_issues()?
        .into_iter()
        .map(|issue| issue_card_from_issue(store, issue))
        .collect()
}

pub fn issue(store: &Store, issue_id: i64) -> Result<IssueCard> {
    issue_card(store, issue_id)
}

pub fn issue_transition_policy(
    store: &Store,
    issue_id: i64,
) -> Result<IssueTransitionPolicyReport> {
    let card = issue_card(store, issue_id)?;
    let registry = issue_transition_policy_registry();
    let lifecycle = card
        .issue
        .loop_id
        .map(|loop_id| worker_lifecycle(store, loop_id))
        .transpose()?;
    let state_class = issue_transition_state_class(&card.issue).to_string();
    let allowed_actions = card
        .actions
        .iter()
        .map(|action| issue_transition_policy_action(&card.issue, action))
        .collect::<Vec<_>>();
    let blocked_actions = issue_transition_blocked_actions(&card.issue, &card.actions);
    let confirmation = issue_transition_confirmation_policy(&card.actions);
    let reviewer_budget = lifecycle
        .as_ref()
        .map(|lifecycle| issue_transition_reviewer_budget(lifecycle, card.trace.as_ref()));
    let human_decision_required =
        matches!(card.issue.status.as_str(), "Blocked" | "Needs Review") || confirmation.required;
    let resources = issue_transition_policy_resources(&card.issue);
    let summary = issue_transition_policy_summary(
        &card.issue,
        &state_class,
        allowed_actions.len(),
        blocked_actions.len(),
        human_decision_required,
        reviewer_budget.as_ref(),
    );
    let mut next_actions = vec![
        format!("entrance hive issue transition-policy {issue_id}"),
        format!("entrance hive issue show {issue_id} --compact"),
        format!("entrance hive issue timeline {issue_id}"),
    ];
    next_actions.extend(card.actions.iter().map(|action| action.command.clone()));

    Ok(IssueTransitionPolicyReport {
        schema_version: ISSUE_TRANSITION_POLICY_SCHEMA_VERSION.to_string(),
        issue: card.issue.clone(),
        loop_id: card.issue.loop_id,
        policy_owner: registry.owner.clone(),
        policy_scope: registry.scope.clone(),
        registry,
        state_class,
        human_decision_required,
        summary,
        allowed_actions,
        blocked_actions,
        confirmation,
        reviewer_budget,
        resources,
        next_actions,
    })
}
