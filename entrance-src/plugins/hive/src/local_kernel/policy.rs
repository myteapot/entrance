pub fn policy_registry() -> PolicyRegistryReport {
    PolicyRegistryReport {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        gates: all_gate_specs()
            .into_iter()
            .map(PolicyGateSpec::from)
            .collect(),
        runtime: runtime_policy_registry(),
        issue_transitions: issue_transition_policy_registry(),
    }
}

fn issue_transition_policy_registry() -> IssueTransitionPolicyRegistry {
    let state_classes = vec![
        issue_transition_state_class_spec("runnable", &["Todo"], false, false),
        issue_transition_state_class_spec("running", &["Doing"], false, false),
        issue_transition_state_class_spec("needs_human", &["Blocked", "Needs Review"], false, true),
        issue_transition_state_class_spec("terminal", &["Done", "Canceled"], true, false),
    ];
    let actions = vec![
        issue_transition_action_policy_spec(
            "run",
            "Run",
            &["Todo"],
            "runtime_verdict: Done | Blocked | Needs Review | Canceled",
            "status_todo_and_loop_bound",
            "runtime",
            "none",
            false,
            false,
            true,
            "entrance hive issue run {issue_id} --runtime {runtime} --compact",
        ),
        issue_transition_action_policy_spec(
            "comment",
            "Comment",
            &[
                "Todo",
                "Doing",
                "Blocked",
                "Needs Review",
                "Done",
                "Canceled",
            ],
            "same_status",
            "issue_exists",
            "human_options",
            "body",
            false,
            false,
            false,
            "entrance hive issue comment {issue_id} --body <text> --compact",
        ),
        issue_transition_action_policy_spec(
            "retry",
            "Retry",
            &["Blocked", "Needs Review", "Canceled"],
            "Todo, then runtime_verdict",
            "human_confirmed_retry_boundary",
            "human_options",
            "note",
            false,
            true,
            true,
            "entrance hive issue retry-run {issue_id} --body <note> --human-confirmed --compact",
        ),
        issue_transition_action_policy_spec(
            "request-review",
            "Review",
            &["Blocked"],
            "Needs Review",
            "human_confirmed_review_boundary",
            "human_options",
            "note",
            false,
            true,
            false,
            "entrance hive issue decide {issue_id} request-review --body <note> --human-confirmed --compact",
        ),
        issue_transition_action_policy_spec(
            "cancel",
            "Cancel",
            &["Todo", "Blocked", "Needs Review"],
            "Canceled",
            "human_confirmed_cancel_boundary",
            "human_options",
            "note",
            true,
            true,
            false,
            "entrance hive issue decide {issue_id} cancel --body <note> --human-confirmed --compact",
        ),
    ];
    let reviewer_fallback = IssueTransitionReviewerFallbackPolicy {
        trigger_decision: "reject".to_string(),
        invalid_round_budget: REVIEWER_INVALID_ROUND_BUDGET,
        fallback_status: "Blocked".to_string(),
        human_decision_statuses: vec!["Blocked".to_string(), "Needs Review".to_string()],
    };
    let state_machine =
        issue_transition_state_machine_specs(&state_classes, &actions, &reviewer_fallback);
    IssueTransitionPolicyRegistry {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        owner: "hive-kernel".to_string(),
        scope: "issue.status.transition".to_string(),
        state_classes,
        actions,
        state_machine,
        confirmation: IssueTransitionConfirmationSpec {
            required_actions: vec![
                "cancel".to_string(),
                "request-review".to_string(),
                "retry".to_string(),
            ],
            confirmation_arg: OPERATOR_ACTION_CONFIRMATION_ARG.to_string(),
            receipt_schema: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
            policy_schema_version: OPERATOR_ACTION_POLICY_SCHEMA_VERSION.to_string(),
            policy_resource: "entrance://policy/mcp-permissions".to_string(),
            actor_identity_resource: "entrance://policy/actor-identity".to_string(),
        },
        reviewer_fallback,
        resource_template: "entrance://issues/{issue_id}/transition-policy".to_string(),
    }
}

fn issue_transition_state_machine_specs(
    state_classes: &[IssueTransitionStateClassSpec],
    actions: &[IssueTransitionActionPolicySpec],
    reviewer_fallback: &IssueTransitionReviewerFallbackPolicy,
) -> Vec<IssueTransitionStateMachineSpec> {
    state_classes
        .iter()
        .flat_map(|state| {
            state.statuses.iter().map(|status| {
                let allowed = issue_transition_state_machine_action_names(status)
                    .into_iter()
                    .filter_map(|action| {
                        actions
                            .iter()
                            .find(|policy| policy.action == action)
                            .map(|policy| issue_transition_state_machine_action(status, policy))
                    })
                    .collect::<Vec<_>>();
                let allowed_names = allowed
                    .iter()
                    .map(|action| action.action.as_str())
                    .collect::<BTreeSet<_>>();
                let blocked_actions = actions
                    .iter()
                    .filter(|policy| !allowed_names.contains(policy.action.as_str()))
                    .map(|policy| policy.action.clone())
                    .collect::<Vec<_>>();
                IssueTransitionStateMachineSpec {
                    status: status.clone(),
                    state_class: state.class.clone(),
                    terminal: state.terminal,
                    human_decision_required: state.human_decision_required
                        || reviewer_fallback.human_decision_statuses.contains(status),
                    allowed_actions: allowed,
                    blocked_actions,
                }
            })
        })
        .collect()
}

fn issue_transition_state_machine_action_names(status: &str) -> Vec<&'static str> {
    match status {
        "Todo" => vec!["run", "comment", "cancel"],
        "Doing" => vec!["comment"],
        "Blocked" => vec!["comment", "retry", "request-review", "cancel"],
        "Needs Review" => vec!["comment", "retry", "cancel"],
        "Done" => vec!["comment"],
        "Canceled" => vec!["comment", "retry"],
        _ => vec!["comment"],
    }
}

fn issue_transition_state_machine_action(
    status: &str,
    policy: &IssueTransitionActionPolicySpec,
) -> IssueTransitionStateMachineActionSpec {
    let to_status = if policy.to_status == "same_status" {
        status.to_string()
    } else {
        policy.to_status.clone()
    };
    IssueTransitionStateMachineActionSpec {
        action: policy.action.clone(),
        label: policy.label.clone(),
        to_status,
        gate: policy.gate.clone(),
        source: policy.source.clone(),
        input: policy.input.clone(),
        destructive: policy.destructive,
        requires_confirmation: policy.requires_confirmation,
        runtime_required: policy.runtime_required,
        command_template: policy.command_template.clone(),
        condition: issue_transition_state_machine_condition(status, policy),
    }
}

fn issue_transition_state_machine_condition(
    status: &str,
    policy: &IssueTransitionActionPolicySpec,
) -> Option<String> {
    match (status, policy.action.as_str()) {
        ("Todo", "run") => Some("requires loop-bound issue".to_string()),
        ("Canceled", "retry") => Some(
            "only when the runtime verdict still exposes retry; human-canceled issues are comment-only"
                .to_string(),
        ),
        _ => None,
    }
}

fn issue_transition_state_class_spec(
    class: &str,
    statuses: &[&str],
    terminal: bool,
    human_decision_required: bool,
) -> IssueTransitionStateClassSpec {
    IssueTransitionStateClassSpec {
        class: class.to_string(),
        statuses: statuses
            .iter()
            .map(|status| (*status).to_string())
            .collect(),
        terminal,
        human_decision_required,
    }
}

#[allow(clippy::too_many_arguments)]
fn issue_transition_action_policy_spec(
    action: &str,
    label: &str,
    from_statuses: &[&str],
    to_status: &str,
    gate: &str,
    source: &str,
    input: &str,
    destructive: bool,
    requires_confirmation: bool,
    runtime_required: bool,
    command_template: &str,
) -> IssueTransitionActionPolicySpec {
    IssueTransitionActionPolicySpec {
        action: action.to_string(),
        label: label.to_string(),
        from_statuses: from_statuses
            .iter()
            .map(|status| (*status).to_string())
            .collect(),
        to_status: to_status.to_string(),
        gate: gate.to_string(),
        source: source.to_string(),
        input: input.to_string(),
        destructive,
        requires_confirmation,
        runtime_required,
        command_template: command_template.to_string(),
    }
}

fn issue_transition_action_policy(action: &str) -> Option<IssueTransitionActionPolicySpec> {
    issue_transition_policy_registry()
        .actions
        .into_iter()
        .find(|policy| policy.action == action)
}

fn runtime_policy_registry() -> RuntimePolicyRegistry {
    RuntimePolicyRegistry {
        schema_version: POLICY_SCHEMA_VERSION.to_string(),
        supported: vec![
            RuntimePolicySpec {
                name: "local".to_string(),
                mode: "deterministic-worker".to_string(),
                description: "In-process deterministic worker for local loop smoke tests."
                    .to_string(),
                command: None,
                required_worker_context: Vec::new(),
                sandbox: RuntimeSandboxSpec {
                    filesystem: "in-process".to_string(),
                    network: "none".to_string(),
                    writes_artifacts: false,
                },
            },
            RuntimePolicySpec {
                name: "codex".to_string(),
                mode: "codex-exec".to_string(),
                description: "External codex exec role worker with read-only filesystem sandbox."
                    .to_string(),
                command: Some("codex exec --sandbox read-only -".to_string()),
                required_worker_context: vec![
                    "command".to_string(),
                    "cwd".to_string(),
                    "output_last_message_path".to_string(),
                    "prompt_chars".to_string(),
                ],
                sandbox: RuntimeSandboxSpec {
                    filesystem: "read-only".to_string(),
                    network: "codex-runtime-default".to_string(),
                    writes_artifacts: true,
                },
            },
        ],
        worker: WorkerPolicySpec {
            default_timeout_secs: DEFAULT_WORKER_TIMEOUT_SECS,
            max_timeout_secs: MAX_WORKER_TIMEOUT_SECS,
            timeout_env: "ENTRANCE_HIVE_WORKER_TIMEOUT_SECS".to_string(),
            default_attempts: DEFAULT_WORKER_ATTEMPTS,
            max_attempts: MAX_WORKER_ATTEMPTS,
            attempts_env: "ENTRANCE_HIVE_WORKER_ATTEMPTS".to_string(),
            required_receipt_fields: vec![
                "kind".to_string(),
                "mode".to_string(),
                "role".to_string(),
                "ok".to_string(),
                "timeout_secs".to_string(),
                "attempt_count".to_string(),
                "max_attempts".to_string(),
                "receipt.ok".to_string(),
                "receipt.role".to_string(),
                "receipt.action".to_string(),
                "receipt.evidence_summary".to_string(),
                "receipt.gates".to_string(),
            ],
        },
    }
}
