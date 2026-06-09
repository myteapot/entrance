#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use entrance_core::{Store, StoreSchemaStatus};

    use super::*;

    fn test_confirmation_receipt(action: &str, author: &str) -> OperatorConfirmationReceipt {
        OperatorConfirmationReceipt {
            schema_version: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
            source: "test".to_string(),
            policy_schema_version: OPERATOR_ACTION_POLICY_SCHEMA_VERSION.to_string(),
            confirmation_arg: OPERATOR_ACTION_CONFIRMATION_ARG.to_string(),
            human_confirmed: true,
            action: action.to_string(),
            author: author.to_string(),
            marker: format!(
                "Test confirmation: human_confirmed=true; action={action}; author={author}; policy={OPERATOR_ACTION_POLICY_SCHEMA_VERSION}"
            ),
            client: None,
            actor: Some(OperatorConfirmationActor {
                id: format!("test:{author}"),
                label: author.to_string(),
                source: "test".to_string(),
                trust: "test_fixture".to_string(),
                verified: false,
            }),
        }
    }

    fn test_issue_with_status(status: &str, loop_id: Option<i64>) -> HiveIssue {
        HiveIssue {
            id: 42,
            loop_id,
            title: format!("{status} issue"),
            status: status.to_string(),
            summary: None,
            assignee: None,
            claim_role: None,
            claim_source: None,
            claimed_at: None,
            created_at: "2026-06-09T00:00:00Z".to_string(),
            updated_at: "2026-06-09T00:00:00Z".to_string(),
        }
    }

    fn test_operator_evidence(action: &str) -> IssueEvidenceSummary {
        IssueEvidenceSummary {
            id: 1,
            round: 1,
            stage_role: None,
            kind: "operator_decision".to_string(),
            summary: format!("operator {action}"),
            schema_version: Some(OPERATOR_DECISION_SCHEMA_VERSION.to_string()),
            admission_result: None,
            blocked_phase: None,
            missing_receipts: Vec::new(),
            packet_envelope_errors: Vec::new(),
            operator_options: Vec::new(),
            operator_author: Some("human".to_string()),
            operator_action: Some(action.to_string()),
            worker_kind: None,
            worker_mode: None,
            worker_ok: None,
            worker_receipt_ok: None,
            worker_timed_out: None,
            worker_status: None,
            worker_duration_ms: None,
            worker_timeout_secs: None,
            worker_attempt_count: None,
            worker_max_attempts: None,
            worker_retry_exhausted: None,
            worker_command: None,
            worker_cwd: None,
            worker_action: None,
            worker_evidence_summary: None,
            worker_gate_count: None,
            worker_receipt_errors: Vec::new(),
            transcript_excerpt: None,
        }
    }

    fn issue_action_names(actions: &[IssueAction]) -> Vec<String> {
        actions.iter().map(|action| action.action.clone()).collect()
    }

    fn state_machine_action_names(
        actions: &[IssueTransitionStateMachineActionSpec],
    ) -> Vec<String> {
        actions.iter().map(|action| action.action.clone()).collect()
    }

    fn packet_by_id<'a>(packets: &'a [HiveLoopPacket]) -> HashMap<i64, &'a HiveLoopPacket> {
        packets
            .iter()
            .map(|packet| (packet.id, packet))
            .collect::<HashMap<_, _>>()
    }

    fn test_gate_context<'a>(
        packets: &'a [HiveLoopPacket],
        admissions: &'a [HiveLoopAdmission],
    ) -> GateEvaluationContext<'a> {
        GateEvaluationContext {
            packets,
            admissions,
        }
    }

    #[test]
    fn policy_registry_and_loop_policies_expose_typed_gate_specs() {
        let registry = policy_registry();
        assert_eq!(registry.schema_version, POLICY_SCHEMA_VERSION);
        assert!(registry.gates.len() >= 8);
        assert_eq!(registry.runtime.schema_version, POLICY_SCHEMA_VERSION);
        let preflight_gate = registry
            .gates
            .iter()
            .find(|gate| gate.name == "runtime_policy_ready")
            .expect("runtime preflight gate should be registered");
        assert_eq!(
            preflight_gate.expected_object_kind.as_deref(),
            Some("PREFLIGHT_PACKET")
        );
        assert_eq!(preflight_gate.check, "runtime_policy_ready");
        assert!(preflight_gate
            .required_receipts
            .iter()
            .any(|receipt| receipt == "runtime_policy"));
        assert_eq!(
            registry.runtime.worker.default_timeout_secs,
            DEFAULT_WORKER_TIMEOUT_SECS
        );
        assert_eq!(
            registry.runtime.worker.max_timeout_secs,
            MAX_WORKER_TIMEOUT_SECS
        );
        assert_eq!(registry.runtime.worker.max_attempts, MAX_WORKER_ATTEMPTS);
        assert_eq!(
            registry.issue_transitions.schema_version,
            POLICY_SCHEMA_VERSION
        );
        assert_eq!(registry.issue_transitions.owner, "hive-kernel");
        assert_eq!(registry.issue_transitions.scope, "issue.status.transition");
        assert_eq!(
            registry
                .issue_transitions
                .actions
                .iter()
                .map(|action| action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["run", "comment", "retry", "request-review", "cancel"]
        );
        let retry_transition = registry
            .issue_transitions
            .actions
            .iter()
            .find(|action| action.action == "retry")
            .expect("retry transition policy should be registered");
        assert_eq!(retry_transition.gate, "human_confirmed_retry_boundary");
        assert_eq!(retry_transition.input, "note");
        assert!(retry_transition.requires_confirmation);
        assert_eq!(
            registry.issue_transitions.confirmation.required_actions,
            vec!["cancel", "request-review", "retry"]
        );
        assert_eq!(
            registry
                .issue_transitions
                .reviewer_fallback
                .invalid_round_budget,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert_eq!(
            registry.issue_transitions.reviewer_fallback.fallback_status,
            "Blocked"
        );
        assert_eq!(
            registry.issue_transitions.resource_template,
            "entrance://issues/{issue_id}/transition-policy"
        );
        let codex_runtime = registry
            .runtime
            .supported
            .iter()
            .find(|runtime| runtime.name == "codex")
            .expect("codex runtime policy should be registered");
        assert_eq!(codex_runtime.mode, "codex-exec");
        assert_eq!(codex_runtime.sandbox.filesystem, "read-only");
        assert!(codex_runtime
            .required_worker_context
            .iter()
            .any(|field| field == "command"));
        assert!(codex_runtime
            .required_worker_context
            .iter()
            .any(|field| field == "cwd"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "timeout_secs"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "role"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "receipt.action"));
        assert!(registry
            .runtime
            .worker
            .required_receipt_fields
            .iter()
            .any(|field| field == "receipt.gates"));
        let verdict_gate = registry
            .gates
            .iter()
            .find(|gate| gate.name == "verdict_receipts_present")
            .expect("verdict gate should be registered");
        assert_eq!(
            verdict_gate.expected_object_kind.as_deref(),
            Some("VERDICT_PACKET")
        );
        assert_eq!(verdict_gate.check, "receipt_requirements_satisfied");
        assert!(verdict_gate
            .required_receipts
            .iter()
            .any(|receipt| receipt == "score"));
        let candidate_binding_gate = registry
            .gates
            .iter()
            .find(|gate| gate.name == ACCEPTED_CANDIDATE_BOUND_GATE)
            .expect("accepted candidate binding gate should be registered");
        assert_eq!(
            candidate_binding_gate.expected_object_kind.as_deref(),
            Some("EXECUTION_PACKET")
        );
        assert_eq!(candidate_binding_gate.check, "accepted_candidate_bound");
        assert!(candidate_binding_gate
            .required_receipts
            .iter()
            .any(|receipt| receipt == "accepted_candidate"));

        let root = std::env::temp_dir().join(format!(
            "entrance-hive-policy-registry-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");
        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Policy loop".to_string(),
                goal: "Expose active loop policies".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let report = policies(&store, created.contract.id).expect("loop policies should resolve");
        assert_eq!(report.loop_id, created.contract.id);
        assert_eq!(report.policies.len(), 4);
        assert!(report.policies.iter().all(|card| card
            .gate_spec
            .as_ref()
            .is_some_and(|spec| spec.schema_version == POLICY_SCHEMA_VERSION)));
        assert_eq!(
            report.policies[0]
                .gate_spec
                .as_ref()
                .expect("preflight gate spec should exist")
                .required_receipts,
            vec![
                "runtime",
                "runtime_probe",
                "runtime_policy",
                "capability_preview"
            ]
        );
        assert_eq!(
            report.policies[1]
                .gate_spec
                .as_ref()
                .expect("candidate gate spec should exist")
                .required_receipts,
            vec!["candidate", "constraints", "role_worker"]
        );
        assert_eq!(
            report.policies[2]
                .gate_spec
                .as_ref()
                .expect("developer binding gate spec should exist")
                .required_receipts,
            vec![
                "accepted_candidate",
                "runtime_probe",
                "runtime_worker",
                "artifact",
                "role_worker"
            ]
        );
        let audit_report =
            super::audit(&store, created.contract.id).expect("policy audit should resolve");
        let policy_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "active_policy_registry")
            .expect("policy registry check should exist");
        assert!(policy_check.passed);
        assert!(policy_check
            .details
            .pointer("/policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.is_empty()));
        let transition_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_transition_policy")
            .expect("issue transition policy check should exist");
        assert!(transition_check.passed);
        assert_eq!(
            transition_check
                .details
                .pointer("/registry_owner")
                .and_then(|value| value.as_str()),
            Some("hive-kernel")
        );
        assert_eq!(
            transition_check
                .details
                .pointer("/registry_scope")
                .and_then(|value| value.as_str()),
            Some("issue.status.transition")
        );
        assert!(transition_check
            .details
            .pointer("/issue_transition_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.is_empty()));

        let explorer_policy = report
            .policies
            .iter()
            .find(|card| card.policy.object_kind == "EXPLORATION_PACKET")
            .expect("explorer policy should exist");
        store
            .update_hive_loop_policy_gate(explorer_policy.policy.id, "runtime_receipts_present")
            .expect("policy gate should update");
        let bad_audit =
            super::audit(&store, created.contract.id).expect("bad policy audit should resolve");
        let bad_policy_check = bad_audit
            .checks
            .iter()
            .find(|check| check.name == "active_policy_registry")
            .expect("policy registry check should exist");
        assert!(!bad_policy_check.passed);
        assert!(bad_policy_check
            .details
            .pointer("/policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("gate.expected_object_kind"))))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_transition_state_machine_exposes_registry_backed_status_matrix() {
        let registry = issue_transition_policy_registry();
        assert_eq!(
            registry
                .state_machine
                .iter()
                .map(|state| state.status.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Todo",
                "Doing",
                "Blocked",
                "Needs Review",
                "Done",
                "Canceled"
            ]
        );
        let registry_actions = registry
            .actions
            .iter()
            .map(|action| action.action.clone())
            .collect::<BTreeSet<_>>();
        let cases = [
            (
                "Todo",
                "runnable",
                false,
                false,
                vec!["run", "comment", "cancel"],
                vec!["retry", "request-review"],
            ),
            (
                "Doing",
                "running",
                false,
                false,
                vec!["comment"],
                vec!["run", "retry", "request-review", "cancel"],
            ),
            (
                "Blocked",
                "needs_human",
                false,
                true,
                vec!["comment", "retry", "request-review", "cancel"],
                vec!["run"],
            ),
            (
                "Needs Review",
                "needs_human",
                false,
                true,
                vec!["comment", "retry", "cancel"],
                vec!["run", "request-review"],
            ),
            (
                "Done",
                "terminal",
                true,
                false,
                vec!["comment"],
                vec!["run", "retry", "request-review", "cancel"],
            ),
            (
                "Canceled",
                "terminal",
                true,
                false,
                vec!["comment", "retry"],
                vec!["run", "request-review", "cancel"],
            ),
        ];

        for (status, class, terminal, human_required, allowed, blocked) in cases {
            let state = registry
                .state_machine
                .iter()
                .find(|state| state.status == status)
                .expect("state machine row should exist");
            assert_eq!(state.state_class, class);
            assert_eq!(state.terminal, terminal);
            assert_eq!(state.human_decision_required, human_required);
            assert_eq!(state_machine_action_names(&state.allowed_actions), allowed);
            assert_eq!(state.blocked_actions, blocked);

            let allowed_set = state
                .allowed_actions
                .iter()
                .map(|action| action.action.clone())
                .collect::<BTreeSet<_>>();
            let blocked_set = state
                .blocked_actions
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            assert!(allowed_set.is_disjoint(&blocked_set));
            assert_eq!(
                allowed_set
                    .union(&blocked_set)
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                registry_actions
            );

            for action in &state.allowed_actions {
                let policy = registry
                    .actions
                    .iter()
                    .find(|policy| policy.action == action.action)
                    .expect("allowed state-machine action should be in registry");
                assert!(policy.from_statuses.contains(&state.status));
                assert_eq!(action.label, policy.label);
                assert_eq!(action.gate, policy.gate);
                assert_eq!(action.requires_confirmation, policy.requires_confirmation);
                assert_eq!(action.runtime_required, policy.runtime_required);
            }
        }

        let todo_run = registry
            .state_machine
            .iter()
            .find(|state| state.status == "Todo")
            .and_then(|state| {
                state
                    .allowed_actions
                    .iter()
                    .find(|action| action.action == "run")
            })
            .expect("Todo should expose conditional run");
        assert_eq!(
            todo_run.condition.as_deref(),
            Some("requires loop-bound issue")
        );
        let canceled_retry = registry
            .state_machine
            .iter()
            .find(|state| state.status == "Canceled")
            .and_then(|state| {
                state
                    .allowed_actions
                    .iter()
                    .find(|action| action.action == "retry")
            })
            .expect("Canceled should document retryable runtime rejection");
        assert!(canceled_retry
            .condition
            .as_deref()
            .is_some_and(|condition| condition.contains("runtime verdict")));
    }

    #[test]
    fn issue_transition_state_machine_matches_status_action_surface() {
        let registry = issue_transition_policy_registry();
        let cases = [
            ("Todo", vec!["run", "comment", "cancel"]),
            ("Doing", vec!["comment"]),
            (
                "Blocked",
                vec!["comment", "retry", "request-review", "cancel"],
            ),
            ("Needs Review", vec!["comment", "retry", "cancel"]),
            ("Done", vec!["comment"]),
            ("Canceled", vec!["comment"]),
        ];

        for (status, expected_actions) in cases {
            let issue = test_issue_with_status(status, Some(7));
            let actions = issue_actions(&issue, None, None);
            assert_eq!(issue_action_names(&actions), expected_actions);

            let allowed = actions
                .iter()
                .map(|action| issue_transition_policy_action(&issue, action))
                .collect::<Vec<_>>();
            for action in &allowed {
                assert_eq!(
                    issue_transition_allowed_action_policy_errors(&issue, action, &registry),
                    Vec::<String>::new()
                );
            }
            let blocked = issue_transition_blocked_actions(&issue, &actions);
            let allowed_set = actions
                .iter()
                .map(|action| action.action.clone())
                .collect::<BTreeSet<_>>();
            let blocked_set = blocked
                .iter()
                .map(|action| action.action.clone())
                .collect::<BTreeSet<_>>();
            let registry_set = registry
                .actions
                .iter()
                .map(|action| action.action.clone())
                .collect::<BTreeSet<_>>();
            assert!(allowed_set.is_disjoint(&blocked_set));
            assert_eq!(
                allowed_set
                    .union(&blocked_set)
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                registry_set
            );
        }

        let todo_without_loop = test_issue_with_status("Todo", None);
        assert_eq!(
            issue_action_names(&issue_actions(&todo_without_loop, None, None)),
            vec!["comment", "cancel"]
        );
        let canceled = test_issue_with_status("Canceled", Some(7));
        assert_eq!(
            issue_human_options(Some(&canceled), &option_list(&["comment", "retry"]), &[]),
            option_list(&["comment", "retry"])
        );
        assert_eq!(
            issue_human_options(
                Some(&canceled),
                &option_list(&["comment", "retry"]),
                &[test_operator_evidence("cancel")]
            ),
            option_list(&["comment"])
        );
    }

    #[test]
    fn loop_audit_and_doctor_gate_on_store_schema_health() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-schema-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");
        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Schema audit loop".to_string(),
                goal: "Gate loop audit on SQLite schema health".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let schema_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "store_schema")
            .expect("store schema audit check should exist");
        assert!(schema_check.passed);
        assert_eq!(
            schema_check.details.pointer("/missing_tables"),
            Some(&serde_json::json!([]))
        );
        assert_eq!(
            schema_check.details.pointer("/missing_columns"),
            Some(&serde_json::json!([]))
        );
        assert_eq!(
            schema_check.details.pointer("/missing_indexes"),
            Some(&serde_json::json!([]))
        );
        assert!(schema_check
            .details
            .pointer("/expected_index_count")
            .and_then(|value| value.as_u64())
            .is_some_and(|count| count > 0));

        let doctor_report =
            super::doctor(&store, created.contract.id).expect("doctor should resolve");
        assert!(doctor_report
            .checks
            .iter()
            .any(|check| check.name == "store_schema" && check.passed));
        assert!(!doctor_report
            .failed_checks
            .iter()
            .any(|check| check == "store_schema"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn store_schema_audit_check_fails_closed_when_schema_drifts() {
        let check = super::store_schema_audit_check(&StoreSchemaStatus {
            schema_version: "entrance.sqlite.core.v1".to_string(),
            db_path: "/tmp/drifted.db".to_string(),
            user_version: 0,
            expected_user_version: 1,
            healthy: false,
            tables: Vec::new(),
            indexes: Vec::new(),
            missing_tables: vec!["hive_loop_packets".to_string()],
            missing_columns: vec!["hive_loop_packets.payload".to_string()],
            missing_indexes: vec!["idx_hive_loop_packets_loop_round".to_string()],
            generated_at: "2026-06-01T00:00:00Z".to_string(),
        });

        assert_eq!(check.name, "store_schema");
        assert!(!check.passed);
        assert_eq!(
            check.details.pointer("/errors"),
            Some(&serde_json::json!([
                "schema.user_version",
                "schema.missing_tables",
                "schema.missing_columns",
                "schema.missing_indexes"
            ]))
        );
    }

    #[test]
    fn pending_next_actions_prefer_issue_compact_run() {
        assert_eq!(
            doctor_next_actions("pending", 4, Some(9), "codex", true),
            vec!["entrance hive issue run 9 --runtime codex --compact"]
        );
        assert_eq!(
            doctor_next_actions("pending", 4, None, "local", true),
            vec!["entrance hive loop run 4 --runtime local --compact"]
        );
    }

    #[test]
    fn doctor_next_actions_prefer_compact_audit_gate() {
        assert_eq!(
            doctor_next_actions("ok", 4, Some(9), "codex", true),
            vec![
                "entrance hive loop audit 4 --compact",
                "entrance hive loop trace 4",
                "entrance hive loop evidence 4"
            ]
        );
        assert_eq!(
            doctor_next_actions("audit_failed", 4, Some(9), "codex", false),
            vec![
                "entrance hive loop audit 4 --compact",
                "entrance hive loop evidence 4"
            ]
        );
    }

    #[test]
    fn runtime_preflight_preview_exposes_capability_boundaries_before_spawn() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-runtime-capability-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Capability preview".to_string(),
                goal: "Expose pre-worker constraints".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: "local-hive-panel".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let preflight = runtime_preflight(&store, created.contract.id)
            .expect("runtime preflight should resolve before run");
        let capability = preflight.preview.capability_preview;

        assert_eq!(
            capability.schema_version,
            RUNTIME_CAPABILITY_PREVIEW_SCHEMA_VERSION
        );
        assert!(capability.worker_spawn_ready);
        assert!(capability.worker_spawn_blockers.is_empty());
        assert_eq!(capability.sandbox.filesystem, "in-process");
        assert_eq!(capability.artifact_capture.mode, "ledger-only");
        assert!(!capability.artifact_capture.archive_ready);
        assert_eq!(
            capability.human_boundary.confirmation_arg,
            "human_confirmed"
        );
        assert_eq!(
            capability.human_boundary.reviewer_invalid_round_budget,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert_eq!(capability.human_boundary.fallback_status, "Blocked");
        assert!(capability.worker_context.required.is_empty());
        assert!(capability
            .worker_context
            .required_receipt_fields
            .iter()
            .any(|field| field == "role"));
    }

    #[test]
    fn runtime_preflight_blocks_unsupported_runtime_before_workers_spawn() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-runtime-capability-block-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Blocked capability preview".to_string(),
                goal: "Block unsupported runtimes before worker spawn".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: "local-hive-panel".to_string(),
                autonomy_level: "run-approved-candidates".to_string(),
                runtime: "ghost-runtime".to_string(),
            },
        )
        .expect("loop should be created");

        let preflight = runtime_preflight(&store, created.contract.id)
            .expect("runtime preflight should resolve before run");
        assert_eq!(preflight.preflight_state, "blocked");
        assert!(!preflight.preview.supported);
        assert!(!preflight.preview.capability_preview.worker_spawn_ready);
        assert!(preflight
            .preview
            .capability_preview
            .worker_spawn_blockers
            .iter()
            .any(|blocker| blocker == "runtime.unsupported"));

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("ghost-runtime".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
            },
        )
        .expect("unsupported runtime should return a blocked report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "kernel");
        assert_eq!(report.packets.len(), 1);
        assert_eq!(report.admissions.len(), 1);
        assert_eq!(report.admissions[0].result, "rejected");
        assert!(report.admissions[0]
            .reason
            .contains("capability_ready=false"));
        assert!(report.admissions[0].reason.contains("runtime.unsupported"));
        assert_eq!(report.issues[0].issue.status, "Blocked");
        assert!(report.stages.iter().all(|stage| stage.role == "kernel"));
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/runtime_policy/capability_ready")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn local_loop_records_stages_evidence_verdict_and_issue() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Test loop".to_string(),
                goal: "Run the local loop".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(7),
                worker_attempts: Some(2),
            },
        )
        .expect("loop should run");

        assert_eq!(report.contract.status, "kept");
        assert_eq!(report.contract.active_phase, "complete");
        assert_eq!(report.policies.len(), 4);
        assert_eq!(report.policies[0].gate, "runtime_policy_ready");
        assert_eq!(report.policies[1].gate, "candidate_receipts_present");
        assert_eq!(report.policies[2].gate, ACCEPTED_CANDIDATE_BOUND_GATE);
        assert_eq!(report.policies[3].gate, "verdict_receipts_present");
        assert_eq!(report.packets.len(), 4);
        assert!(report.packets.iter().all(|packet| packet
            .payload
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(PACKET_SCHEMA_VERSION)));
        assert_eq!(
            report.packets[0]
                .payload
                .get("object_kind")
                .and_then(|value| value.as_str()),
            Some("PREFLIGHT_PACKET")
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/runtime_policy/supported")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/capability_preview/schema_version")
                .and_then(|value| value.as_str()),
            Some(RUNTIME_CAPABILITY_PREVIEW_SCHEMA_VERSION)
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/capability_preview/worker_spawn_ready")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/capability_preview/sandbox/filesystem")
                .and_then(|value| value.as_str()),
            Some("in-process")
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/capability_preview/human_boundary/fallback_status")
                .and_then(|value| value.as_str()),
            Some("Blocked")
        );
        assert_eq!(
            report.packets[2]
                .payload
                .pointer("/body/accepted_candidate")
                .and_then(|value| value.as_str()),
            Some("Run a local MVP loop through Hive")
        );
        assert!(report.packets[0]
            .payload
            .get("receipt_requirements")
            .and_then(|value| value.as_array())
            .is_some_and(|receipts| receipts
                .iter()
                .any(|receipt| receipt.as_str() == Some("capability_preview"))));
        assert_eq!(
            report.packets[1]
                .payload
                .pointer("/body/role_worker/role")
                .and_then(|value| value.as_str()),
            Some("explorer")
        );
        assert_eq!(
            report.packets[1]
                .payload
                .pointer("/body/candidate")
                .and_then(|value| value.as_str()),
            Some("Run a local MVP loop through Hive")
        );
        assert_eq!(
            report.packets[2]
                .payload
                .pointer("/receipt_requirements/4")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );
        assert_eq!(
            report.packets[3]
                .payload
                .pointer("/body/role_worker/role")
                .and_then(|value| value.as_str()),
            Some("reviewer")
        );
        assert_eq!(report.admissions.len(), 4);
        assert!(report
            .admissions
            .iter()
            .all(|admission| admission.result == "admitted"));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(ADMISSION_SCHEMA_VERSION)));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/policy/schema_version")
            .and_then(|value| value.as_str())
            == Some(POLICY_SCHEMA_VERSION)));
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/spec/check")
                .and_then(|value| value.as_str()),
            Some("runtime_policy_ready")
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/spec/expected_object_kind")
                .and_then(|value| value.as_str()),
            Some("PREFLIGHT_PACKET")
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/packet/schema_version")
                .and_then(|value| value.as_str()),
            Some(PACKET_SCHEMA_VERSION)
        );
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/packet/envelope/valid")
            .and_then(|value| value.as_bool())
            == Some(true)));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/packet/envelope/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.is_empty())));
        assert!(report.admissions.iter().all(|admission| admission
            .policy
            .pointer("/receipt/satisfied")
            .and_then(|value| value.as_bool())
            == Some(true)));
        assert_eq!(
            report.admissions[2]
                .policy
                .pointer("/receipt/required/4")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );
        assert_eq!(
            report.admissions[2]
                .policy
                .pointer("/target_binding/passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.admissions[2]
                .policy
                .pointer("/target_binding/reason")
                .and_then(|value| value.as_str()),
            Some("accepted_candidate_matches_explorer_candidate")
        );
        assert_eq!(
            report.admissions[2]
                .policy
                .pointer("/receipt/missing")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(report.stages.len(), 3);
        assert_eq!(report.evidence.len(), 3);
        let execution_evidence = report
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("execution evidence should exist");
        assert_eq!(
            execution_evidence
                .payload
                .pointer("/worker/kind")
                .and_then(|value| value.as_str()),
            Some("local")
        );
        assert_eq!(
            execution_evidence
                .payload
                .pointer("/worker/receipt/action")
                .and_then(|value| value.as_str()),
            Some("implement-admitted-candidate")
        );
        assert_eq!(
            execution_evidence
                .payload
                .pointer("/worker/receipt/gates/role_bound")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "keep");
        let trace = report.issues[0]
            .trace
            .as_ref()
            .expect("issue trace should be present");
        assert_eq!(trace.current_round, 1);
        assert_eq!(trace.packet_count, 4);
        assert_eq!(trace.admission_count, 4);
        assert_eq!(trace.verdict_count, 1);
        assert_eq!(trace.round_packet_count, 4);
        assert_eq!(trace.round_admission_count, 4);
        assert_eq!(trace.round_evidence_count, 3);
        assert_eq!(trace.round_verdict_count, 1);
        assert_eq!(trace.receipt_required_count, 16);
        assert_eq!(trace.receipt_missing_count, 0);
        assert_eq!(trace.round_receipt_required_count, 16);
        assert_eq!(trace.round_receipt_missing_count, 0);
        assert_eq!(trace.role_worker_count, 3);
        assert_eq!(trace.role_worker_ok_count, 3);
        assert_eq!(trace.round_role_worker_count, 3);
        assert_eq!(trace.round_role_worker_ok_count, 3);
        assert_eq!(trace.round_worker_duration_ms, 0);
        assert_eq!(trace.round_worker_timeout_count, 0);
        assert_eq!(trace.round_worker_retry_exhausted_count, 0);
        assert_eq!(trace.packet_schema.as_deref(), Some(PACKET_SCHEMA_VERSION));
        assert_eq!(trace.policy_schema.as_deref(), Some(POLICY_SCHEMA_VERSION));
        assert_eq!(
            trace.admission_schema.as_deref(),
            Some(ADMISSION_SCHEMA_VERSION)
        );
        assert_eq!(
            trace.verdict_schema.as_deref(),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            trace.last_admission_gate.as_deref(),
            Some("verdict_receipts_present")
        );
        assert_eq!(
            trace.last_gate_expected_object_kind.as_deref(),
            Some("VERDICT_PACKET")
        );
        assert!(trace
            .last_gate_description
            .as_deref()
            .is_some_and(|description| description.contains("Reviewer packets")));
        assert_eq!(trace.last_admission_passed, Some(true));
        assert_eq!(trace.last_decision.as_deref(), Some("keep"));
        assert_eq!(trace.score_vector.len(), 9);
        assert_eq!(
            trace
                .score_vector
                .iter()
                .find(|metric| metric.name == "runtime_readiness")
                .and_then(|metric| metric.value),
            Some(1.0)
        );
        assert_eq!(
            trace
                .score_vector
                .iter()
                .find(|metric| metric.name == "target_alignment")
                .and_then(|metric| metric.value),
            Some(1.0)
        );
        assert_eq!(trace.human_options, vec!["comment"]);
        assert_eq!(trace.operator_event_count, 0);
        assert_eq!(trace.round_operator_event_count, 0);
        assert!(trace.last_operator_event.is_none());
        assert!(trace.operator_events.is_empty());
        assert_eq!(trace.worker_kind.as_deref(), Some("local"));
        assert_eq!(trace.worker_ok, Some(true));
        assert_eq!(trace.evidence.len(), 3);
        let doer_evidence = trace
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("developer evidence summary should exist");
        assert_eq!(doer_evidence.stage_role.as_deref(), Some("developer"));
        assert_eq!(doer_evidence.admission_result.as_deref(), Some("admitted"));
        assert_eq!(doer_evidence.worker_kind.as_deref(), Some("local"));
        assert_eq!(doer_evidence.worker_ok, Some(true));
        assert_eq!(doer_evidence.worker_duration_ms, Some(0));
        assert_eq!(doer_evidence.worker_timeout_secs, Some(7));
        assert_eq!(doer_evidence.worker_attempt_count, Some(1));
        assert_eq!(doer_evidence.worker_max_attempts, Some(2));
        assert_eq!(doer_evidence.worker_retry_exhausted, None);
        assert_eq!(
            doer_evidence.worker_action.as_deref(),
            Some("implement-admitted-candidate")
        );
        assert!(doer_evidence
            .worker_evidence_summary
            .as_deref()
            .is_some_and(|summary| summary.contains("Local developer worker")));
        assert_eq!(doer_evidence.worker_gate_count, Some(3));
        assert!(doer_evidence.worker_receipt_errors.is_empty());
        let mut receipt_error_trace = trace.clone();
        receipt_error_trace
            .evidence
            .iter_mut()
            .find(|evidence| evidence.kind == "execution_packet")
            .expect("developer evidence summary should exist")
            .worker_receipt_errors = vec!["action".to_string()];
        assert!(doctor_worker_failures(&receipt_error_trace)
            .iter()
            .any(|failure| failure.contains("receipt_errors=action")));
        assert_eq!(
            doctor_health("kept", Some("Done"), Some("keep"), true, true),
            "worker_failed"
        );
        assert!(doer_evidence
            .transcript_excerpt
            .as_deref()
            .is_some_and(|excerpt| excerpt.contains("Local developer worker")));
        assert_eq!(
            trace
                .stages
                .iter()
                .map(|stage| stage.role.as_str())
                .collect::<Vec<_>>(),
            vec!["explorer", "developer", "reviewer"]
        );
        let doer_trace = trace
            .stages
            .iter()
            .find(|stage| stage.role == "developer")
            .expect("developer stage trace should exist");
        assert_eq!(
            doer_trace.evidence_kind.as_deref(),
            Some("execution_packet")
        );
        assert_eq!(doer_trace.admission_result.as_deref(), Some("admitted"));
        assert_eq!(doer_trace.worker_kind.as_deref(), Some("local"));
        assert_eq!(doer_trace.worker_ok, Some(true));
        let shown_issue =
            issue(&store, report.issues[0].issue.id).expect("single issue report should resolve");
        assert_eq!(shown_issue.issue.id, report.issues[0].issue.id);
        assert_eq!(
            shown_issue
                .trace
                .as_ref()
                .expect("shown issue should include trace")
                .stages
                .len(),
            3
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/runtime_readiness")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/stage_completeness")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/evidence_presence")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/admission_integrity")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/target_alignment")
                .and_then(|value| value.as_f64()),
            Some(1.0)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/gate_results/target_bound")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/gate_results/target_binding_reason")
                .and_then(|value| value.as_str()),
            Some("accepted_candidate_matches_explorer_candidate")
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/gate_results/current_round_admission_count")
                .and_then(|value| value.as_u64()),
            Some(3)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/gate_results/prior_stage_evidence_count")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/gate_results/review_gates_passed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            report.verdicts[0]
                .evidence
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(report.issues[0].issue.status, "Done");
        assert!(report.issues[0].comments.len() >= 3);
        assert_eq!(
            report.issues[0]
                .comments
                .iter()
                .filter_map(|comment| comment
                    .payload
                    .get("stage_role")
                    .and_then(|value| value.as_str()))
                .collect::<Vec<_>>(),
            vec!["explorer", "developer", "reviewer"]
        );
        assert_eq!(
            report.issues[0]
                .comments
                .iter()
                .filter_map(|comment| comment
                    .payload
                    .get("evidence_id")
                    .and_then(|value| value.as_i64()))
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(report.issues[0].comments.iter().any(|comment| {
            comment.body == "Explorer admitted a candidate for this round."
                && comment
                    .payload
                    .get("evidence_kind")
                    .and_then(|value| value.as_str())
                    == Some("exploration_packet")
        }));
        assert!(report.issues[0].comments.iter().any(|comment| {
            comment.body == "Developer admitted the execution packet."
                && comment
                    .payload
                    .get("evidence_kind")
                    .and_then(|value| value.as_str())
                    == Some("execution_packet")
        }));
        assert!(report.issues[0].comments.iter().any(|comment| {
            comment.body == "Reviewer admitted the verdict packet."
                && comment
                    .payload
                    .get("evidence_kind")
                    .and_then(|value| value.as_str())
                    == Some("verdict_packet")
        }));
        assert!(report.issues[0].comments.iter().all(|comment| comment
            .payload
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(SYSTEM_COMMENT_SCHEMA_VERSION)));
        let issue_timeline = super::issue_timeline(&store, report.issues[0].issue.id)
            .expect("issue timeline should resolve");
        assert_eq!(issue_timeline.schema_version, ISSUE_TIMELINE_SCHEMA_VERSION);
        assert_eq!(issue_timeline.timeline_state, "closed");
        assert_eq!(
            issue_timeline.counts.comment_count,
            report.issues[0].comments.len()
        );
        assert_eq!(issue_timeline.counts.evidence_count, 3);
        assert_eq!(issue_timeline.counts.verdict_count, 1);
        assert_eq!(issue_timeline.counts.blocker_count, 0);
        assert!(!issue_timeline.human_decision.required);
        assert_eq!(
            issue_timeline.human_decision.issue_status.as_deref(),
            Some("Done")
        );
        assert_eq!(
            issue_timeline.human_decision.primary_action.as_deref(),
            Some("comment")
        );
        let issue_round = issue_timeline
            .rounds
            .iter()
            .find(|round| round.round == Some(1))
            .expect("round 1 timeline group should exist");
        assert_eq!(issue_round.evidence_count, 3);
        assert_eq!(issue_round.verdict_count, 1);
        assert!(issue_round.comment_count >= 3);
        assert!(issue_round.phases.iter().any(|phase| phase == "developer"));
        assert!(issue_round
            .decisions
            .iter()
            .any(|decision| decision == "keep"));
        assert_eq!(
            issue_timeline.resources.issue_timeline,
            format!("entrance://issues/{}/timeline", report.issues[0].issue.id)
        );
        assert!(issue_timeline.items.iter().any(|item| {
            item.source == "comment"
                && item.event_kind == "stage_comment"
                && item.evidence_id == Some(2)
        }));
        assert!(issue_timeline.items.iter().any(|item| {
            item.source == "evidence"
                && item.event_kind == "execution_packet"
                && item.actor == "developer"
                && item.status.as_deref() == Some("admitted")
                && item.permalink.starts_with(&format!(
                    "entrance://issues/{}/timeline/items/evidence-",
                    report.issues[0].issue.id
                ))
        }));
        let execution_item = issue_timeline
            .items
            .iter()
            .find(|item| item.event_kind == "execution_packet")
            .expect("execution packet timeline item should exist");
        let item_report =
            super::issue_timeline_item(&store, report.issues[0].issue.id, &execution_item.id)
                .expect("timeline item permalink should resolve");
        assert_eq!(
            item_report.schema_version,
            ISSUE_TIMELINE_ITEM_SCHEMA_VERSION
        );
        assert_eq!(item_report.item.id, execution_item.id);
        assert_eq!(item_report.item.permalink, execution_item.permalink);
        assert_eq!(
            item_report.round.as_ref().and_then(|round| round.round),
            Some(1)
        );
        assert_eq!(
            item_report.resources.item_permalink,
            execution_item.permalink
        );
        assert!(item_report.previous_item_id.is_some());
        assert!(item_report.next_item_id.is_some());
        assert!(item_report
            .next_actions
            .iter()
            .any(|action| action.contains("issue timeline-item")));
        assert!(issue_timeline.items.iter().any(|item| {
            item.source == "verdict"
                && item.decision.as_deref() == Some("keep")
                && item.phase.as_deref() == Some("reviewer")
        }));
        assert!(issue_timeline.next_actions.iter().any(|action| {
            action == &format!("entrance hive issue timeline {}", report.issues[0].issue.id)
        }));
        let issue_doctor = report.issues[0]
            .doctor
            .as_ref()
            .expect("issue card should include doctor summary");
        assert_eq!(issue_doctor.health, "ok");
        assert_eq!(issue_doctor.runtime, "local");
        assert_eq!(issue_doctor.counts.round_role_worker_ok_count, 3);
        assert!(issue_doctor.worker_failures.is_empty());
        let trace_report =
            super::trace(&store, created.contract.id).expect("loop trace report should resolve");
        assert_eq!(trace_report.contract.status, "kept");
        assert_eq!(
            trace_report
                .issue
                .as_ref()
                .map(|issue| issue.status.as_str()),
            Some("Done")
        );
        assert_eq!(trace_report.trace.last_decision.as_deref(), Some("keep"));
        assert_eq!(trace_report.trace.round_receipt_missing_count, 0);
        assert_eq!(trace_report.trace.score_vector.len(), 9);
        assert_eq!(
            trace_report.trace.last_gate_expected_object_kind.as_deref(),
            Some("VERDICT_PACKET")
        );
        assert_eq!(
            trace_report.trace.audit_schema.as_deref(),
            Some(AUDIT_SCHEMA_VERSION)
        );
        assert_eq!(trace_report.trace.audit_passed, Some(true));
        assert_eq!(trace_report.trace.audit_failed_count, 0);
        assert!(trace_report.trace.audit_failed_checks.is_empty());
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("loop evidence report should resolve");
        assert_eq!(evidence_report.evidence.len(), 3);
        assert!(evidence_report.evidence.iter().any(|evidence| {
            evidence.stage_role.as_deref() == Some("reviewer")
                && evidence.kind == "verdict_packet"
                && evidence.worker_ok == Some(true)
        }));
        let evidence_drilldown = super::evidence_drilldown(&store, created.contract.id)
            .expect("loop evidence drilldown should resolve");
        assert_eq!(
            evidence_drilldown.schema_version,
            EVIDENCE_DRILLDOWN_SCHEMA_VERSION
        );
        assert_eq!(evidence_drilldown.drilldown_state, "complete");
        assert_eq!(evidence_drilldown.evidence_count, 3);
        assert!(evidence_drilldown.blockers.is_empty());
        assert!(!evidence_drilldown.human_decision.required);
        assert_eq!(
            evidence_drilldown.resources.evidence_drilldown,
            format!(
                "entrance://loops/{}/evidence-drilldown",
                created.contract.id
            )
        );
        let developer_drilldown = evidence_drilldown
            .items
            .iter()
            .find(|item| item.stage_role.as_deref() == Some("developer"))
            .expect("developer evidence drilldown should exist");
        assert_eq!(developer_drilldown.kind, "execution_packet");
        assert_eq!(
            developer_drilldown
                .worker
                .as_ref()
                .and_then(|worker| worker.kind.as_deref()),
            Some("local")
        );
        assert_eq!(
            developer_drilldown
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.role.as_deref()),
            Some("developer")
        );
        assert!(developer_drilldown
            .payload
            .top_level_keys
            .iter()
            .any(|key| key == "worker"));
        assert!(developer_drilldown
            .payload
            .diff_from_previous
            .relative_to_evidence_id
            .is_some());
        assert!(evidence_drilldown.next_actions.iter().any(|action| {
            action
                == &format!(
                    "entrance hive loop evidence-drilldown {}",
                    created.contract.id
                )
        }));
        assert_eq!(
            evidence_drilldown.resources.evidence_manifest,
            format!("entrance://loops/{}/evidence-manifest", created.contract.id)
        );
        let evidence_manifest = super::evidence_manifest(&store, created.contract.id)
            .expect("loop evidence manifest should resolve");
        assert_eq!(
            evidence_manifest.schema_version,
            EVIDENCE_MANIFEST_SCHEMA_VERSION
        );
        assert_eq!(evidence_manifest.manifest_state, "ok");
        assert_eq!(evidence_manifest.coverage.evidence_count, 3);
        assert_eq!(evidence_manifest.coverage.payload_count, 3);
        assert_eq!(evidence_manifest.coverage.receipt_count, 3);
        assert!(evidence_manifest.coverage.digest_count >= 6);
        assert_eq!(
            evidence_manifest.resources.evidence_manifest,
            format!("entrance://loops/{}/evidence-manifest", created.contract.id)
        );
        assert!(evidence_manifest.entries.iter().any(|entry| {
            entry.source == "evidence.payload"
                && entry.entry_kind == "payload"
                && entry
                    .sha256
                    .as_deref()
                    .is_some_and(|digest| digest.len() == 64)
                && entry.verified
        }));
        assert!(evidence_manifest.entries.iter().any(|entry| {
            entry.source == "worker.receipt"
                && entry.entry_kind == "receipt"
                && entry.stage_role.as_deref() == Some("developer")
                && entry
                    .sha256
                    .as_deref()
                    .is_some_and(|digest| digest.len() == 64)
                && entry.verified
        }));
        assert!(evidence_manifest.next_actions.iter().any(|action| {
            action
                == &format!(
                    "entrance hive loop evidence-manifest {}",
                    created.contract.id
                )
        }));
        let audit_report =
            super::audit(&store, created.contract.id).expect("loop audit should resolve");
        assert_eq!(audit_report.schema_version, AUDIT_SCHEMA_VERSION);
        assert!(audit_report.passed);
        assert_eq!(audit_report.failed_count, 0);
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "packet_envelopes"
                && check.passed
                && check
                    .details
                    .pointer("/packet_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "worker_receipts"
                && check.passed
                && check
                    .details
                    .pointer("/worker_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "runtime_policy"
                && check.passed
                && check
                    .details
                    .pointer("/runtime_policy_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "issue_surface"
                && check.passed
                && check
                    .details
                    .pointer("/comment_count")
                    .and_then(|value| value.as_u64())
                    .is_some_and(|count| count >= 3)
                && check
                    .details
                    .pointer("/issue_surface_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        assert!(audit_report.checks.iter().any(|check| {
            check.name == "issue_transition_policy"
                && check.passed
                && check
                    .details
                    .pointer("/registry_owner")
                    .and_then(|value| value.as_str())
                    == Some("hive-kernel")
                && check
                    .details
                    .pointer("/issue_transition_policy_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.is_empty())
        }));
        let doctor_report =
            super::doctor(&store, created.contract.id).expect("loop doctor should resolve");
        assert_eq!(doctor_report.schema_version, DOCTOR_SCHEMA_VERSION);
        assert_eq!(doctor_report.health, "ok");
        assert_eq!(doctor_report.status, "kept");
        assert_eq!(doctor_report.decision.as_deref(), Some("keep"));
        assert_eq!(doctor_report.counts.round_packet_count, 4);
        assert_eq!(doctor_report.counts.round_role_worker_ok_count, 3);
        assert_eq!(doctor_report.counts.round_receipt_missing_count, 0);
        assert_eq!(doctor_report.counts.round_worker_duration_ms, 0);
        assert_eq!(doctor_report.counts.round_worker_timeout_count, 0);
        assert_eq!(doctor_report.counts.round_worker_retry_exhausted_count, 0);
        assert_eq!(doctor_report.counts.audit_failed_count, 0);
        assert!(doctor_report.failed_checks.is_empty());
        assert!(doctor_report.missing_receipts.is_empty());
        assert!(doctor_report.worker_failures.is_empty());
        assert!(doctor_report.next_actions.iter().any(
            |action| action == &format!("entrance hive loop evidence {}", created.contract.id)
        ));
        let lifecycle_report = super::worker_lifecycle(&store, created.contract.id)
            .expect("loop worker lifecycle should resolve");
        assert_eq!(
            lifecycle_report.schema_version,
            WORKER_LIFECYCLE_SCHEMA_VERSION
        );
        assert_eq!(lifecycle_report.lifecycle_state, "succeeded");
        assert_eq!(
            lifecycle_report.policy.expected_roles,
            vec!["explorer", "developer", "reviewer"]
        );
        assert_eq!(
            lifecycle_report.policy.reviewer_invalid_round_budget,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert_eq!(lifecycle_report.current.round, 1);
        assert_eq!(lifecycle_report.current.worker_count, 3);
        assert_eq!(lifecycle_report.current.worker_ok_count, 3);
        assert!(lifecycle_report.current.missing_roles.is_empty());
        assert_eq!(
            lifecycle_report.current.observed_roles,
            vec!["developer", "explorer", "reviewer"]
        );
        assert!(lifecycle_report.current.failures.is_empty());
        let developer_worker = lifecycle_report
            .current
            .workers
            .iter()
            .find(|worker| worker.role == "developer")
            .expect("developer worker should be visible");
        assert_eq!(developer_worker.kind.as_deref(), Some("local"));
        assert_eq!(developer_worker.ok, Some(true));
        assert_eq!(developer_worker.timeout_secs, Some(7));
        assert_eq!(developer_worker.attempt_count, Some(1));
        assert_eq!(developer_worker.max_attempts, Some(2));
        assert_eq!(
            developer_worker.action.as_deref(),
            Some("implement-admitted-candidate")
        );
        assert!(lifecycle_report.next_actions.iter().any(|action| {
            action
                == &format!(
                    "entrance hive loop worker-lifecycle {}",
                    created.contract.id
                )
        }));
        let dashboard_report =
            super::dashboard(&store, created.contract.id).expect("loop dashboard should resolve");
        assert_eq!(
            dashboard_report.schema_version,
            LOOP_DASHBOARD_SCHEMA_VERSION
        );
        assert_eq!(dashboard_report.dashboard_state, "done");
        assert_eq!(dashboard_report.kernel.preflight_state, "admitted");
        assert_eq!(dashboard_report.kernel.gate, "runtime_policy_ready");
        assert_eq!(dashboard_report.kernel.gate_passed, Some(true));
        assert_eq!(
            dashboard_report
                .agents
                .iter()
                .map(|agent| (agent.role.as_str(), agent.state.as_str()))
                .collect::<Vec<_>>(),
            vec![("explorer", "ok"), ("developer", "ok"), ("reviewer", "ok")]
        );
        assert_eq!(dashboard_report.reviewer.decision.as_deref(), Some("keep"));
        assert_eq!(dashboard_report.reviewer.reviewer_invalid_rounds_used, 0);
        assert_eq!(
            dashboard_report.reviewer.reviewer_invalid_round_budget,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert!(!dashboard_report.human_decision.required);
        assert_eq!(
            dashboard_report.resources.loop_dashboard,
            format!("entrance://loops/{}/dashboard", created.contract.id)
        );
        assert_eq!(
            dashboard_report.resources.evidence_drilldown,
            format!(
                "entrance://loops/{}/evidence-drilldown",
                created.contract.id
            )
        );
        assert_eq!(
            dashboard_report.resources.evidence_manifest,
            format!("entrance://loops/{}/evidence-manifest", created.contract.id)
        );
        assert_eq!(dashboard_report.rounds.len(), 1);
        let dashboard_round = dashboard_report
            .rounds
            .first()
            .expect("dashboard should include current round");
        assert!(dashboard_round.current);
        assert_eq!(dashboard_round.status, "kept");
        assert_eq!(dashboard_round.packet_count, 4);
        assert_eq!(dashboard_round.admission_count, 4);
        assert_eq!(dashboard_round.evidence_count, 3);
        assert_eq!(dashboard_round.verdict_count, 1);
        assert!(dashboard_round.blocker.is_none());
        assert!(dashboard_round.retry_lineage.is_none());
        assert!(dashboard_round.groups.packets.iter().any(|packet| {
            packet.object_kind == "PREFLIGHT_PACKET"
                && packet.writer_role == "kernel"
                && packet.admission_result.as_deref() == Some("admitted")
        }));
        assert!(dashboard_round.groups.admissions.iter().any(|admission| {
            admission.gate.as_deref() == Some("runtime_policy_ready")
                && admission.gate_passed == Some(true)
        }));
        assert!(dashboard_round.groups.evidence.iter().any(|evidence| {
            evidence.stage_role.as_deref() == Some("developer")
                && evidence.kind == "execution_packet"
                && evidence.worker_ok == Some(true)
        }));
        assert!(dashboard_round.groups.verdicts.iter().any(|verdict| {
            verdict.decision == "keep" && verdict.reason_code.as_deref() == Some("all_gates_passed")
        }));
        assert!(dashboard_report.primary_next_action.is_some());
        assert!(dashboard_report.next_actions.iter().any(|action| {
            action == &format!("entrance hive loop dashboard {}", created.contract.id)
        }));

        let rerun = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("completed loop run should be idempotent");
        assert_eq!(rerun.contract.status, "kept");
        assert_eq!(rerun.packets.len(), report.packets.len());
        assert_eq!(rerun.admissions.len(), report.admissions.len());
        assert_eq!(rerun.evidence.len(), report.evidence.len());
        assert_eq!(rerun.verdicts.len(), report.verdicts.len());
        assert_eq!(
            rerun.issues[0].comments.len(),
            report.issues[0].comments.len()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_ok_reads_final_json_receipt() {
        assert_eq!(
            worker_receipt_ok(
                r#"{"ok":true,"role":"doer","action":"execute","evidence_summary":"done","gates":{"accepted":true}}"#
            ),
            Some(true)
        );
        assert_eq!(worker_receipt_ok(r#"{"ok":true}"#), Some(false));
        assert_eq!(
            worker_receipt_ok(
                r#"{"ok":true,"role":"doer","action":{"accepted":"execute"},"evidence_summary":"done","gates":{"accepted":true}}"#
            ),
            Some(false)
        );
        let object_action_receipt = serde_json::json!({
            "ok": true,
            "role": "doer",
            "action": { "accepted": "execute" },
            "evidence_summary": "done",
            "gates": { "accepted": true }
        });
        assert_eq!(
            worker_receipt_contract_errors(&object_action_receipt, Some("doer")),
            vec!["action"]
        );
        assert_eq!(
            worker_receipt_ok("prefix {\"ok\":false,\"reason\":\"blocked\"} suffix"),
            Some(false)
        );
        assert_eq!(worker_receipt_ok("not json"), None);
    }

    #[test]
    fn codex_worker_prompt_declares_strict_receipt_schema() {
        let contract = HiveLoopContract {
            id: 42,
            title: "Prompt contract".to_string(),
            goal: "Keep worker receipts typed".to_string(),
            boundary: "No writes".to_string(),
            approach_space: vec!["Use strict JSON".to_string()],
            eval_space: vec!["action is a string".to_string()],
            review_surface: "local-hive-panel".to_string(),
            autonomy_level: "run-approved-candidates".to_string(),
            runtime: "codex".to_string(),
            status: "todo".to_string(),
            active_phase: "explorer".to_string(),
            current_round: 1,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let prompt = codex_worker_prompt(&contract, "explorer");

        assert!(prompt.contains(r#""role": "explorer""#));
        assert!(prompt.contains(r#""action": "compile-candidate""#));
        assert!(prompt.contains("action` must be a non-empty JSON string"));
        assert!(prompt.contains("Never return `action` as an object"));
    }

    #[test]
    fn evidence_summary_exposes_codex_worker_command_context() {
        let evidence = HiveLoopEvidence {
            id: 9,
            loop_id: 3,
            stage_id: None,
            round: 1,
            kind: "execution_packet".to_string(),
            summary: "Doer ran `codex` runtime worker.".to_string(),
            path: None,
            payload: serde_json::json!({
                "worker": {
                    "ok": true,
                    "kind": "codex",
                    "mode": "codex-exec",
                    "role": "doer",
                    "command": "codex -a never exec --sandbox read-only <prompt>",
                    "cwd": "/tmp/entrance-src",
                    "receipt_ok": true,
                    "receipt": {
                        "ok": true,
                        "role": "doer",
                        "action": "record-local-loop-ledger",
                        "evidence_summary": "codex accepted the packet",
                        "gates": { "packet_received": true }
                    }
                }
            }),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let summary = issue_evidence_summary(&evidence, &HashMap::new());

        assert_eq!(
            summary.worker_command.as_deref(),
            Some("codex -a never exec --sandbox read-only <prompt>")
        );
        assert_eq!(summary.worker_cwd.as_deref(), Some("/tmp/entrance-src"));
        assert_eq!(
            summary.worker_action.as_deref(),
            Some("record-local-loop-ledger")
        );
    }

    #[test]
    fn codex_worker_requires_explicit_ok_receipt() {
        let output = TimedCommandOutput {
            status_success: true,
            status_code: Some(0),
            timed_out: false,
            duration_ms: 12,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(codex_worker_success(&output, Some(true)));
        assert!(!codex_worker_success(&output, Some(false)));
        assert!(!codex_worker_success(&output, None));

        let timed_out = TimedCommandOutput {
            timed_out: true,
            ..output
        };
        assert!(!codex_worker_success(&timed_out, Some(true)));
    }

    #[test]
    fn doctor_retry_action_uses_more_attempts_for_codex() {
        assert_eq!(
            retry_run_command(7, "codex"),
            "entrance hive issue retry-run 7 --body <note> --human-confirmed --runtime codex --worker-attempts 2 --compact"
        );
        assert_eq!(
            retry_run_command(7, "local"),
            "entrance hive issue retry-run 7 --body <note> --human-confirmed --compact"
        );
    }

    #[test]
    fn verdict_audit_rejects_inconsistent_score_contract() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-verdict-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict audit loop".to_string(),
                goal: "Detect inconsistent typed verdict score contracts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");

        let mut verdict = report.verdicts[0].clone();
        verdict.score["gates_passed"] = serde_json::json!(false);
        verdict.score["human_options"] = serde_json::json!(["comment", "retry"]);
        verdict.score["score_vector"]["runtime_readiness"] = serde_json::json!(1.5);
        verdict.evidence["decision"] = serde_json::json!("blocked");
        verdict.evidence["reason_code"] = serde_json::json!("different_reason");

        let errors = verdict_audit_errors(&verdict).expect("verdict should fail audit");
        let fields = errors
            .get("errors")
            .and_then(|value| value.as_array())
            .expect("verdict audit should return error fields")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"score.gates_passed"));
        assert!(fields.contains(&"score.human_options"));
        assert!(fields.contains(&"score.score_vector.runtime_readiness"));
        assert!(fields.contains(&"evidence.decision_binding"));
        assert!(fields.contains(&"reason_code.binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reviewer_keep_requires_ledger_gate_assessment() {
        let assessment = ReviewerGateAssessment {
            stage_completeness: 2.0 / 3.0,
            runtime_readiness: 1.0,
            evidence_presence: 0.5,
            admission_integrity: 1.0,
            target_alignment: 0.0,
            goal_alignment: 1.0,
            acceptance_evidence: 0.5,
            implementation_specificity: 1.0,
            regression_risk: 1.0,
            three_stages_recorded: false,
            evidence_recorded: false,
            runtime_ready: true,
            admissions_clean: true,
            target_bound: false,
            semantic_gates_passed: false,
            review_gates_passed: false,
            observed_stage_roles: vec!["explorer".to_string(), "developer".to_string()],
            missing_stage_roles: vec!["reviewer".to_string()],
            expected_candidate: Some("Candidate A".to_string()),
            accepted_candidate: Some("Candidate B".to_string()),
            target_binding_reason: "accepted_candidate_mismatch".to_string(),
            current_round_admission_count: 3,
            rejected_admission_count: 0,
            receipt_missing_count: 0,
            prior_stage_evidence_count: 1,
            expected_prior_stage_evidence_count: 2,
            failure_reasons: vec![
                "missing_stage_roles=reviewer".to_string(),
                "prior_stage_evidence=1/2".to_string(),
                "target_binding=accepted_candidate_mismatch".to_string(),
            ],
        };

        let verdict = build_verdict(
            Some(VerdictDecision::Keep),
            true,
            None,
            "local",
            1,
            assessment,
            0,
        );

        assert_eq!(verdict.decision, VerdictDecision::Reject);
        assert_eq!(verdict.reason_code, "review_gates_failed");
        assert_eq!(verdict.reviewer_invalid_rounds_used, 1);
        let score = verdict.score_payload();
        assert_eq!(
            score
                .pointer("/score_vector/stage_completeness")
                .and_then(|value| value.as_f64()),
            Some(2.0 / 3.0)
        );
        assert_eq!(
            score
                .pointer("/score_vector/evidence_presence")
                .and_then(|value| value.as_f64()),
            Some(0.5)
        );
        assert_eq!(
            score
                .pointer("/score_vector/target_alignment")
                .and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(
            score
                .pointer("/gate_results/review_gates_passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            score
                .pointer("/gate_results/target_bound")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert!(score
            .pointer("/gate_results/failure_reasons")
            .and_then(|value| value.as_array())
            .is_some_and(|reasons| reasons
                .iter()
                .any(|reason| reason.as_str() == Some("missing_stage_roles=reviewer"))));
    }

    #[test]
    fn verdict_audit_rejects_drifted_evidence_bindings() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-verdict-evidence-binding-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict evidence binding loop".to_string(),
                goal: "Detect drifted verdict evidence bindings".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let verdict = report
            .verdicts
            .first()
            .expect("run should record a verdict");
        let mut evidence = verdict.evidence.clone();
        evidence["evidence_count"] = serde_json::json!(999);
        evidence["runtime_ready"] = serde_json::json!(false);
        evidence["reviewer_invalid_rounds_used"] = serde_json::json!(99);
        evidence["reviewer_invalid_budget_exhausted"] = serde_json::json!(true);
        evidence["role_worker"]["ok"] = serde_json::json!(false);
        store
            .insert_hive_loop_verdict(HiveLoopVerdictCreate {
                loop_id: verdict.loop_id,
                round: verdict.round,
                decision: verdict.decision.clone(),
                summary: verdict.summary.clone(),
                score: verdict.score.clone(),
                evidence,
            })
            .expect("drifted verdict should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let verdict_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "verdict_packets")
            .expect("verdict audit should exist");
        assert!(!verdict_check.passed);
        for expected in [
            "evidence.count",
            "evidence.runtime_ready",
            "evidence.reviewer_invalid_rounds_used",
            "evidence.reviewer_invalid_budget_exhausted",
            "evidence.role_worker_binding",
        ] {
            assert!(
                verdict_check
                    .details
                    .pointer("/verdict_errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|errors| errors.iter().any(|error| error
                        .pointer("/errors")
                        .and_then(|value| value.as_array())
                        .is_some_and(|fields| fields
                            .iter()
                            .any(|field| field.as_str() == Some(expected))))),
                "expected verdict evidence binding error {expected}"
            );
        }
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "verdict_packets:verdict_evidence:evidence.count"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verdict_audit_recomputes_reviewer_budget_from_ledger() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-verdict-budget-binding-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict budget binding loop".to_string(),
                goal: "Detect reviewer budget receipts that drift from verdict history".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let first_invalid = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("first invalid review should run");
        decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id: first_invalid.issues[0].issue.id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: Some("retry after first invalid review".to_string()),
                confirmation_receipt: Some(test_confirmation_receipt("retry", "human")),
            },
        )
        .expect("first retry should be admitted");
        let second_invalid = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("second invalid review should run");
        decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id: second_invalid.issues[0].issue.id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: Some("retry after second invalid review".to_string()),
                confirmation_receipt: Some(test_confirmation_receipt("retry", "human")),
            },
        )
        .expect("second retry should be admitted");
        let exhausted = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("third invalid review should exhaust budget");

        let second_verdict = exhausted
            .verdicts
            .iter()
            .find(|verdict| verdict.round == 2)
            .expect("second invalid verdict should exist");
        let mut drifted_second = second_verdict.clone();
        drifted_second.score["reviewer_invalid_rounds_used"] = serde_json::json!(1);
        drifted_second.score["gate_results"]["reviewer_invalid_rounds_used"] = serde_json::json!(1);
        drifted_second.evidence["reviewer_invalid_rounds_used"] = serde_json::json!(1);
        let second_errors = standard_verdict_binding_errors(
            &drifted_second,
            &exhausted.verdicts,
            &exhausted.packets,
            &exhausted.evidence,
        );
        assert!(second_errors.contains(&"reviewer_budget.rounds_used_binding".to_string()));
        assert!(!second_errors.contains(&"evidence.reviewer_invalid_rounds_used".to_string()));

        let exhausted_verdict = exhausted
            .verdicts
            .iter()
            .find(|verdict| verdict.round == REVIEWER_INVALID_ROUND_BUDGET)
            .expect("budget exhausted verdict should exist");
        let mut drifted_exhausted = exhausted_verdict.clone();
        drifted_exhausted.score["reviewer_invalid_rounds_used"] = serde_json::json!(2);
        drifted_exhausted.score["gate_results"]["reviewer_invalid_rounds_used"] =
            serde_json::json!(2);
        drifted_exhausted.evidence["reviewer_invalid_rounds_used"] = serde_json::json!(2);
        drifted_exhausted.score["reviewer_invalid_budget_exhausted"] = serde_json::json!(false);
        drifted_exhausted.score["gate_results"]["reviewer_invalid_budget_exhausted"] =
            serde_json::json!(false);
        drifted_exhausted.evidence["reviewer_invalid_budget_exhausted"] = serde_json::json!(false);
        let exhausted_errors = standard_verdict_binding_errors(
            &drifted_exhausted,
            &exhausted.verdicts,
            &exhausted.packets,
            &exhausted.evidence,
        );
        assert!(exhausted_errors.contains(&"reviewer_budget.rounds_used_binding".to_string()));
        assert!(exhausted_errors.contains(&"reviewer_budget.exhausted_binding".to_string()));
        assert!(
            !exhausted_errors.contains(&"evidence.reviewer_invalid_budget_exhausted".to_string())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verdict_audit_rejects_contract_status_drift_from_current_verdict() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-verdict-contract-binding-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict contract binding loop".to_string(),
                goal: "Detect a terminal contract status that drifted from the verdict".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let issue_id = report.issues[0].issue.id;
        let verdict = report
            .verdicts
            .first()
            .expect("run should record a verdict");
        assert_eq!(verdict.decision, "keep");
        assert_eq!(report.contract.status, "kept");
        store
            .update_hive_loop_contract_state(
                report.contract.id,
                "blocked",
                "complete",
                report.contract.current_round,
            )
            .expect("contract status should drift for audit probe");
        store
            .update_hive_issue_status(
                issue_id,
                "Blocked",
                Some("drifted issue status matching the contract"),
            )
            .expect("issue status should be mutated with contract status");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let verdict_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "verdict_packets")
            .expect("verdict audit should exist");
        assert!(!verdict_check.passed);
        assert!(verdict_check
            .details
            .pointer("/verdict_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("contract.status_binding"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "verdict_packets:verdict_contract:contract.status_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verdict_audit_rejects_replayed_round_verdicts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-verdict-replay-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Verdict replay audit loop".to_string(),
                goal: "Catch replayed verdicts in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let verdict = report
            .verdicts
            .first()
            .expect("run should record a verdict");
        store
            .insert_hive_loop_verdict(HiveLoopVerdictCreate {
                loop_id: verdict.loop_id,
                round: verdict.round,
                decision: verdict.decision.clone(),
                summary: verdict.summary.clone(),
                score: verdict.score.clone(),
                evidence: verdict.evidence.clone(),
            })
            .expect("replayed verdict should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let verdict_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "verdict_packets")
            .expect("verdict audit should exist");
        assert!(!verdict_check.passed);
        assert!(verdict_check
            .details
            .pointer("/verdict_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("verdict.round_duplicate"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "verdict_packets:verdict_round:verdict.round_duplicate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_rejects_packets_missing_required_receipts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-receipt-gate-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Receipt gate loop".to_string(),
                goal: "Reject incomplete execution packets".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let admission = emit_and_admit(
            &store,
            &created.contract,
            "EXECUTION_PACKET",
            "developer",
            "developer",
            "reviewer",
            serde_json::json!({
                "runtime_probe": {
                    "ok": true,
                    "kind": "local"
                },
                "artifact": "hive-loop-ledger"
            }),
        )
        .expect("admission should be recorded");

        assert_eq!(admission.result, "rejected");
        assert_eq!(
            admission.reason,
            "accepted_candidate_bound failed: missing or invalid receipts accepted_candidate, runtime_worker, role_worker"
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/satisfied")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/missing/0")
                .and_then(|value| value.as_str()),
            Some("accepted_candidate")
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/missing/1")
                .and_then(|value| value.as_str()),
            Some("runtime_worker")
        );
        assert_eq!(
            admission
                .policy
                .pointer("/receipt/missing/2")
                .and_then(|value| value.as_str()),
            Some("role_worker")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_rejects_success_worker_with_incomplete_structured_receipt() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-worker-receipt-gate-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Worker receipt gate loop".to_string(),
                goal: "Reject incomplete successful worker receipts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let runtime_probe = serde_json::json!({
            "ok": true,
            "kind": "local"
        });
        let runtime_worker = run_role_worker(
            "local",
            "developer",
            &created.contract,
            &runtime_probe,
            DEFAULT_WORKER_TIMEOUT_SECS,
            DEFAULT_WORKER_ATTEMPTS,
        );
        let mut role_worker = runtime_worker.clone();
        role_worker
            .pointer_mut("/receipt")
            .and_then(|value| value.as_object_mut())
            .expect("role worker receipt should be an object")
            .remove("action");

        let admission = emit_and_admit(
            &store,
            &created.contract,
            "EXECUTION_PACKET",
            "developer",
            "developer",
            "reviewer",
            serde_json::json!({
                "accepted_candidate": "Run a local MVP loop through Hive",
                "runtime_probe": runtime_probe,
                "runtime_worker": runtime_worker,
                "role_worker": role_worker,
                "artifact": "hive-loop-ledger"
            }),
        )
        .expect("admission should be recorded");

        assert_eq!(admission.result, "rejected");
        assert_eq!(
            admission.reason,
            "accepted_candidate_bound failed: missing or invalid receipts role_worker"
        );
        assert_eq!(
            string_array_at(&admission.policy, "/receipt/missing"),
            vec!["role_worker"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_rejects_developer_packet_with_drifted_candidate() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-target-drift-gate-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Target drift gate loop".to_string(),
                goal: "Reject Developer work that is not bound to the accepted candidate"
                    .to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let runtime_probe = serde_json::json!({
            "ok": true,
            "kind": "local"
        });
        let explorer_worker = run_role_worker(
            "local",
            "explorer",
            &created.contract,
            &runtime_probe,
            DEFAULT_WORKER_TIMEOUT_SECS,
            DEFAULT_WORKER_ATTEMPTS,
        );
        let explorer_admission = emit_and_admit(
            &store,
            &created.contract,
            "EXPLORATION_PACKET",
            "explorer",
            "explorer",
            "developer",
            serde_json::json!({
                "candidate": "Accepted candidate A",
                "constraints": ["stay inside the accepted candidate"],
                "role_worker": explorer_worker
            }),
        )
        .expect("explorer admission should be recorded");
        assert_eq!(explorer_admission.result, "admitted");

        let developer_worker = run_role_worker(
            "local",
            "developer",
            &created.contract,
            &runtime_probe,
            DEFAULT_WORKER_TIMEOUT_SECS,
            DEFAULT_WORKER_ATTEMPTS,
        );
        let developer_admission = emit_and_admit(
            &store,
            &created.contract,
            "EXECUTION_PACKET",
            "developer",
            "developer",
            "reviewer",
            serde_json::json!({
                "accepted_candidate": "Different candidate B",
                "runtime_probe": runtime_probe,
                "runtime_worker": developer_worker,
                "role_worker": developer_worker,
                "artifact": "hive-loop-ledger"
            }),
        )
        .expect("developer admission should be recorded");

        assert_eq!(developer_admission.result, "rejected");
        assert_eq!(
            developer_admission.reason,
            "accepted_candidate_bound failed: accepted_candidate_mismatch expected_candidate=Accepted candidate A accepted_candidate=Different candidate B"
        );
        assert_eq!(
            developer_admission
                .policy
                .pointer("/receipt/satisfied")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            developer_admission
                .policy
                .pointer("/target_binding/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            developer_admission
                .policy
                .pointer("/target_binding/reason")
                .and_then(|value| value.as_str()),
            Some("accepted_candidate_mismatch")
        );
        assert_eq!(
            developer_admission
                .policy
                .pointer("/target_binding/expected_candidate")
                .and_then(|value| value.as_str()),
            Some("Accepted candidate A")
        );
        assert_eq!(
            developer_admission
                .policy
                .pointer("/target_binding/accepted_candidate")
                .and_then(|value| value.as_str()),
            Some("Different candidate B")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_audit_rejects_corrupt_receipt_policy_and_gate_bindings() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-admission-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Admission audit loop".to_string(),
                goal: "Detect drift between admission receipt, policy, gate, and packet"
                    .to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let packet_by_id = packet_by_id(&report.packets);
        let mut bad_admission = report.admissions[0].clone();
        bad_admission.policy["packet"]["object_kind"] = serde_json::json!("EXECUTION_PACKET");
        bad_admission.policy["policy"]["route_to"] = serde_json::json!("complete");
        bad_admission.policy["policy"]["gate"] = serde_json::json!("runtime_receipts_present");
        bad_admission.policy["gate"]["passed"] = serde_json::json!(false);
        bad_admission.policy["receipt"]["required"] = serde_json::json!(["candidate"]);
        bad_admission.policy["receipt"]["missing"] = serde_json::json!(["constraints"]);
        bad_admission.policy["receipt"]["satisfied"] = serde_json::json!(true);

        let errors = admission_audit_errors(
            &bad_admission,
            &packet_by_id,
            test_gate_context(&report.packets, &report.admissions),
        )
        .expect("corrupt admission should fail audit");
        let fields = errors
            .get("errors")
            .and_then(|value| value.as_array())
            .expect("admission audit should return error fields")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"packet.object_kind"));
        assert!(fields.contains(&"policy.route_to"));
        assert!(fields.contains(&"policy.gate_binding"));
        assert!(fields.contains(&"receipt.required_binding"));
        assert!(fields.contains(&"receipt.satisfied_binding"));
        assert!(fields.contains(&"result.admission_conditions"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_audit_rejects_drifted_target_binding_receipt() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-target-binding-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Target binding audit loop".to_string(),
                goal: "Detect drifted target binding admission receipts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let execution_packet_id = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("execution packet should exist")
            .id;
        let packet_by_id = packet_by_id(&report.packets);
        let mut bad_admission = report
            .admissions
            .iter()
            .find(|admission| admission.packet_id == execution_packet_id)
            .expect("execution admission should exist")
            .clone();
        bad_admission.policy["target_binding"]["passed"] = serde_json::json!(false);
        bad_admission.policy["target_binding"]["reason"] =
            serde_json::json!("accepted_candidate_mismatch");
        bad_admission.policy["target_binding"]["expected_candidate"] =
            serde_json::json!("Another candidate");

        let errors = admission_audit_errors(
            &bad_admission,
            &packet_by_id,
            test_gate_context(&report.packets, &report.admissions),
        )
        .expect("drifted target binding should fail admission audit");
        let fields = errors
            .get("errors")
            .and_then(|value| value.as_array())
            .expect("admission audit should return error fields")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"target_binding.passed"));
        assert!(fields.contains(&"target_binding.reason"));
        assert!(fields.contains(&"target_binding.expected_candidate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn admission_audit_recomputes_gate_result_and_missing_receipts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-admission-gate-recompute-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Admission gate recompute loop".to_string(),
                goal: "Detect admission receipts that lie about the packet gate".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let mut packets = report.packets.clone();
        let execution_packet = packets
            .iter_mut()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("execution packet should exist");
        execution_packet
            .payload
            .pointer_mut("/body")
            .and_then(|value| value.as_object_mut())
            .expect("execution packet body should be an object")
            .remove("accepted_candidate");
        let execution_packet_id = execution_packet.id;
        let packet_by_id = packet_by_id(&packets);
        let admission = report
            .admissions
            .iter()
            .find(|admission| admission.packet_id == execution_packet_id)
            .expect("execution admission should exist");

        let errors = admission_audit_errors(
            admission,
            &packet_by_id,
            test_gate_context(&packets, &report.admissions),
        )
        .expect("drifted packet should fail admission audit");
        let fields = errors
            .get("errors")
            .and_then(|value| value.as_array())
            .expect("admission audit should return error fields")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"receipt.missing_binding"));
        assert!(fields.contains(&"receipt.satisfied_packet_binding"));
        assert!(fields.contains(&"gate.passed_binding"));
        assert!(fields.contains(&"reason.gate_binding"));
        assert!(fields.contains(&"result.gate_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typed_packet_envelope_diagnostics_explain_schema_breaks() {
        let malformed = serde_json::json!({
            "schema_version": "entrance.hive.packet.v0",
            "object_kind": " ",
            "writer": {
                "role": ""
            },
            "route": {
                "from": "explorer"
            },
            "state_code": "draft",
            "body": {
                "candidate": "local-loop-mvp"
            }
        });
        assert_eq!(
            typed_packet_envelope_errors(&malformed),
            vec![
                "schema_version",
                "loop_id",
                "round",
                "object_kind",
                "writer.role",
                "route.to",
                "state_code"
            ]
        );
        assert!(!typed_packet_envelope_valid(&malformed));
        assert!(!gate_passes("candidate_receipts_present", &malformed));
        assert_eq!(
            gate_failure_reason("candidate_receipts_present", &malformed),
            "candidate_receipts_present failed: typed packet envelope invalid: schema_version, loop_id, round, object_kind, writer.role, route.to, state_code"
        );

        let packet = HiveLoopPacket {
            id: 42,
            loop_id: 7,
            round: 3,
            object_kind: "EXPLORATION_PACKET".to_string(),
            writer_role: "explorer".to_string(),
            route_from: "explorer".to_string(),
            route_to: "doer".to_string(),
            state_code: "submitted".to_string(),
            payload: malformed.clone(),
            created_at: "2026-05-31T00:00:00Z".to_string(),
        };
        let receipt = typed_admission_receipt(
            &packet,
            &malformed,
            None,
            "rejected",
            "bad packet",
            None,
            None,
            GateEvaluationContext {
                packets: &[],
                admissions: &[],
            },
        );
        assert_eq!(
            receipt
                .pointer("/packet/envelope/valid")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            receipt
                .pointer("/packet/envelope/errors/0")
                .and_then(|value| value.as_str()),
            Some("schema_version")
        );
        assert_eq!(
            receipt
                .pointer("/packet/envelope/errors/6")
                .and_then(|value| value.as_str()),
            Some("state_code")
        );
    }

    #[test]
    fn unsupported_runtime_records_blocked_verdict_and_issue() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-blocked-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Blocked loop".to_string(),
                goal: "Block unsupported runtime".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("loop should be created");

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(2),
            },
        )
        .expect("blocked loop should still return a report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "kernel");
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "blocked");
        assert_eq!(
            report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("admission_rejected")
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/admission_integrity")
                .and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(report.issues[0].issue.status, "Blocked");
        let issue_doctor = report.issues[0]
            .doctor
            .as_ref()
            .expect("blocked issue should include doctor summary");
        assert_eq!(issue_doctor.health, "blocked");
        assert!(issue_doctor.missing_receipts.is_empty());
        assert!(report
            .issues
            .first()
            .expect("issue should exist")
            .comments
            .iter()
            .any(|comment| comment.body.contains("runtime_policy_ready failed")));
        assert_eq!(report.admissions.len(), 1);
        assert_eq!(report.admissions[0].result, "rejected");
        assert!(report.admissions[0]
            .reason
            .contains("runtime_policy_ready failed"));
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/name")
                .and_then(|value| value.as_str()),
            Some("runtime_policy_ready")
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/gate/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            report.admissions[0]
                .policy
                .pointer("/receipt/missing")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(report.packets.len(), 1);
        assert_eq!(
            report.packets[0]
                .payload
                .get("object_kind")
                .and_then(|value| value.as_str()),
            Some("PREFLIGHT_PACKET")
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/runtime_policy/supported")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            report.packets[0]
                .payload
                .pointer("/body/runtime_policy/blocker")
                .and_then(|value| value.as_str()),
            Some("runtime.unsupported")
        );
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("blocked evidence report should resolve");
        let blocked_evidence = evidence_report
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "admission_rejection")
            .expect("admission rejection evidence should be summarized");
        assert_eq!(blocked_evidence.blocked_phase.as_deref(), Some("kernel"));
        assert!(blocked_evidence.missing_receipts.is_empty());
        assert_eq!(blocked_evidence.worker_kind, None);
        assert_eq!(blocked_evidence.worker_ok, None);
        assert_eq!(blocked_evidence.worker_timeout_secs, None);
        assert_eq!(blocked_evidence.worker_attempt_count, None);
        assert_eq!(blocked_evidence.worker_max_attempts, None);
        assert_eq!(blocked_evidence.worker_retry_exhausted, None);
        assert!(blocked_evidence
            .operator_options
            .iter()
            .any(|option| option == "request-human-review"));
        let drilldown = super::evidence_drilldown(&store, created.contract.id)
            .expect("blocked evidence drilldown should resolve");
        assert_eq!(drilldown.drilldown_state, "needs_human");
        assert_eq!(drilldown.blockers.len(), 1);
        let blocker = drilldown
            .blockers
            .first()
            .expect("blocked evidence should produce a blocker");
        assert_eq!(blocker.scope, "evidence");
        assert_eq!(blocker.evidence_id, Some(blocked_evidence.id));
        assert_eq!(blocker.phase.as_deref(), Some("kernel"));
        assert!(blocker.reason.contains("runtime_policy_ready failed"));
        assert!(blocker.decision_surface.required);
        assert_eq!(
            blocker.decision_surface.issue_status.as_deref(),
            Some("Blocked")
        );
        assert_eq!(
            blocker.decision_surface.primary_action.as_deref(),
            Some("retry")
        );
        assert!(blocker
            .decision_surface
            .actions
            .iter()
            .any(|action| action.issue_action.action == "retry"
                && action.issue_action.command.contains("issue retry-run")
                && action.recommended));
        assert!(blocker
            .decision_surface
            .actions
            .iter()
            .any(|action| action.issue_action.action == "request-review"
                && action.operator_option.as_deref() == Some("request-human-review")
                && action.issue_action.confirmation_required));
        let blocked_timeline = super::issue_timeline(&store, report.issues[0].issue.id)
            .expect("blocked issue timeline should resolve");
        assert_eq!(blocked_timeline.timeline_state, "needs_human");
        assert!(blocked_timeline.human_decision.required);
        assert_eq!(
            blocked_timeline.human_decision.issue_status.as_deref(),
            Some("Blocked")
        );
        assert_eq!(
            blocked_timeline.human_decision.primary_action.as_deref(),
            Some("retry")
        );
        assert!(blocked_timeline
            .human_decision
            .actions
            .iter()
            .any(|action| action.issue_action.action == "retry"
                && action.issue_action.command.contains("issue retry-run")
                && action.recommended));
        assert!(blocked_timeline
            .human_decision
            .actions
            .iter()
            .any(|action| action.issue_action.action == "request-review"
                && action.issue_action.confirmation_required));
        assert!(blocked_timeline
            .rounds
            .iter()
            .any(|round| round.round == Some(1) && round.blocker_count > 0));
        let doctor_report =
            super::doctor(&store, created.contract.id).expect("blocked doctor should resolve");
        assert_eq!(doctor_report.health, "blocked");
        assert_eq!(doctor_report.status, "blocked");
        assert_eq!(doctor_report.issue_status.as_deref(), Some("Blocked"));
        assert_eq!(doctor_report.decision.as_deref(), Some("blocked"));
        assert!(doctor_report.missing_receipts.is_empty());
        assert!(doctor_report.worker_failures.is_empty());
        assert!(doctor_report
            .next_actions
            .iter()
            .any(|action| action.contains("issue retry-run")));
        assert!(doctor_report.next_actions.iter().any(|action| action
            == &format!(
                "entrance hive issue decide {} request-review --body <note> --human-confirmed --compact",
                doctor_report
                    .issue_id
                    .expect("blocked doctor should have issue")
            )));
        let audit_report =
            super::audit(&store, created.contract.id).expect("blocked audit should resolve");
        let runtime_policy_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "runtime_policy")
            .expect("runtime policy audit should be present");
        assert!(!runtime_policy_check.passed);
        assert!(runtime_policy_check
            .details
            .pointer("/runtime_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| !errors.is_empty()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn decision_override_records_reject_and_needs_review_verdicts() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-decision-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let rejected = create(
            &store,
            HiveLoopCreateRequest {
                title: "Rejected loop".to_string(),
                goal: "Reject a candidate".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let rejected_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: rejected.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("reject loop should run");

        assert_eq!(rejected_report.contract.status, "rejected");
        assert_eq!(rejected_report.verdicts[0].decision, "reject");
        assert_eq!(rejected_report.issues[0].issue.status, "Canceled");
        assert_eq!(
            rejected_report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("quality_gate_failed")
        );
        assert_eq!(
            rejected_report.verdicts[0]
                .score
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            rejected_report.verdicts[0]
                .score
                .pointer("/human_options/1")
                .and_then(|value| value.as_str()),
            Some("retry")
        );
        assert_eq!(
            rejected_report.issues[0]
                .trace
                .as_ref()
                .expect("rejected issue trace should exist")
                .human_options,
            vec!["comment", "retry"]
        );

        let review = create(
            &store,
            HiveLoopCreateRequest {
                title: "Review loop".to_string(),
                goal: "Ask for human review".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let review_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: review.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("needs-review".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("review loop should run");

        assert_eq!(review_report.contract.status, "needs-review");
        assert_eq!(review_report.verdicts[0].decision, "needs-review");
        assert_eq!(review_report.issues[0].issue.status, "Needs Review");
        assert_eq!(
            review_report.verdicts[0]
                .score
                .get("operator_review_needed")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            review_report.verdicts[0]
                .score
                .pointer("/human_options/2")
                .and_then(|value| value.as_str()),
            Some("cancel")
        );
        assert_eq!(
            review_report.issues[0]
                .trace
                .as_ref()
                .expect("review issue trace should exist")
                .human_options,
            vec!["comment", "retry", "cancel"]
        );

        let exhausted = create(
            &store,
            HiveLoopCreateRequest {
                title: "Exhausted review loop".to_string(),
                goal: "Block after repeated invalid reviews".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let jumped = create(
            &store,
            HiveLoopCreateRequest {
                title: "Jumped review loop".to_string(),
                goal: "Do not block from round number alone".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        store
            .update_hive_loop_contract_state(
                jumped.contract.id,
                "todo",
                "explorer",
                REVIEWER_INVALID_ROUND_BUDGET,
            )
            .expect("test should move loop to budget-numbered round");
        let jumped_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: jumped.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("jumped reject loop should run");
        assert_eq!(jumped_report.contract.status, "rejected");
        assert_eq!(jumped_report.contract.current_round, 3);
        assert_eq!(
            jumped_report
                .verdicts
                .last()
                .expect("jumped verdict should exist")
                .score
                .get("reviewer_invalid_rounds_used")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(
            jumped_report
                .verdicts
                .last()
                .expect("jumped verdict should exist")
                .score
                .get("reviewer_invalid_budget_exhausted")
                .and_then(|value| value.as_bool()),
            Some(false)
        );

        let first_invalid_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: exhausted.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("first invalid review should run");
        assert_eq!(first_invalid_report.contract.status, "rejected");
        assert_eq!(first_invalid_report.contract.current_round, 1);
        assert_eq!(
            first_invalid_report
                .verdicts
                .last()
                .expect("first invalid verdict should exist")
                .score
                .get("reviewer_invalid_rounds_used")
                .and_then(|value| value.as_i64()),
            Some(1)
        );
        decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id: first_invalid_report.issues[0].issue.id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: Some("retry after first invalid review".to_string()),
                confirmation_receipt: Some(test_confirmation_receipt("retry", "human")),
            },
        )
        .expect("first retry should be admitted");
        let second_invalid_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: exhausted.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("second invalid review should run");
        assert_eq!(second_invalid_report.contract.status, "rejected");
        assert_eq!(second_invalid_report.contract.current_round, 2);
        assert_eq!(
            second_invalid_report
                .verdicts
                .last()
                .expect("second invalid verdict should exist")
                .score
                .get("reviewer_invalid_rounds_used")
                .and_then(|value| value.as_i64()),
            Some(2)
        );
        decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id: second_invalid_report.issues[0].issue.id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: Some("retry after second invalid review".to_string()),
                confirmation_receipt: Some(test_confirmation_receipt("retry", "human")),
            },
        )
        .expect("second retry should be admitted");
        let exhausted_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: exhausted.contract.id,
                runtime: Some("local".to_string()),
                decision: Some("reject".to_string()),
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("exhausted reject loop should run");

        assert_eq!(exhausted_report.contract.status, "blocked");
        assert_eq!(exhausted_report.contract.current_round, 3);
        let exhausted_verdict = exhausted_report
            .verdicts
            .last()
            .expect("exhausted verdict should exist");
        assert_eq!(exhausted_verdict.decision, "blocked");
        assert_eq!(exhausted_report.issues[0].issue.status, "Blocked");
        assert_eq!(
            exhausted_verdict
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("review_budget_exhausted")
        );
        assert_eq!(
            exhausted_verdict
                .score
                .get("reviewer_invalid_rounds_used")
                .and_then(|value| value.as_i64()),
            Some(REVIEWER_INVALID_ROUND_BUDGET)
        );
        assert_eq!(
            exhausted_verdict
                .score
                .get("reviewer_invalid_budget_exhausted")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(exhausted_verdict
            .summary
            .contains("still invalid after 3 review rounds"));
        let exhausted_lifecycle = super::worker_lifecycle(&store, exhausted.contract.id)
            .expect("exhausted worker lifecycle should resolve");
        assert_eq!(exhausted_lifecycle.lifecycle_state, "blocked");
        assert_eq!(exhausted_lifecycle.issue_status.as_deref(), Some("Blocked"));
        assert_eq!(exhausted_lifecycle.policy.fallback_status, "Blocked");
        assert_eq!(
            exhausted_lifecycle.current.round,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert_eq!(
            exhausted_lifecycle.current.reviewer_invalid_rounds_used,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert!(
            exhausted_lifecycle
                .current
                .reviewer_invalid_budget_exhausted
        );
        assert!(exhausted_lifecycle
            .current
            .observed_roles
            .iter()
            .any(|role| role == "reviewer"));
        let exhausted_drilldown = super::evidence_drilldown(&store, exhausted.contract.id)
            .expect("exhausted evidence drilldown should resolve");
        assert_eq!(exhausted_drilldown.drilldown_state, "needs_human");
        assert_eq!(exhausted_drilldown.issue_status.as_deref(), Some("Blocked"));
        let loop_blocker = exhausted_drilldown
            .blockers
            .iter()
            .find(|blocker| blocker.scope == "loop")
            .expect("review budget fallback should create a loop-level blocker");
        assert_eq!(loop_blocker.evidence_id, None);
        assert_eq!(loop_blocker.phase.as_deref(), Some("reviewer"));
        assert!(loop_blocker
            .reason
            .contains("Reviewer invalid budget exhausted"));
        assert_eq!(
            loop_blocker.decision_surface.primary_action.as_deref(),
            Some("retry")
        );
        assert!(loop_blocker
            .decision_surface
            .actions
            .iter()
            .any(|action| action.issue_action.action == "request-review"));
        let exhausted_timeline = super::issue_timeline(&store, exhausted_report.issues[0].issue.id)
            .expect("exhausted issue timeline should resolve");
        assert_eq!(exhausted_timeline.timeline_state, "needs_human");
        assert!(exhausted_timeline.human_decision.required);
        assert_eq!(
            exhausted_timeline.human_decision.primary_action.as_deref(),
            Some("retry")
        );
        assert!(exhausted_timeline
            .human_decision
            .summary
            .contains("requires a human decision"));
        let exhausted_round = exhausted_timeline
            .rounds
            .iter()
            .find(|round| round.round == Some(REVIEWER_INVALID_ROUND_BUDGET))
            .expect("budget-exhausted round should exist in timeline");
        assert_eq!(exhausted_round.verdict_count, 1);
        assert!(exhausted_round
            .decisions
            .iter()
            .any(|decision| decision == "blocked"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_audit_rejects_role_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-worker-role-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Worker role audit loop".to_string(),
                goal: "Catch worker receipt role drift".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        let mut payload = doer_packet.payload.clone();
        *payload
            .pointer_mut("/body/role_worker/role")
            .expect("role worker role should exist") = serde_json::json!("explorer");
        *payload
            .pointer_mut("/body/runtime_worker/role")
            .expect("runtime worker role should exist") = serde_json::json!("explorer");
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: report.contract.id,
                round: report.contract.current_round,
                object_kind: "EXECUTION_PACKET".to_string(),
                writer_role: "doer".to_string(),
                route_from: "doer".to_string(),
                route_to: "evaluator".to_string(),
                state_code: "submitted".to_string(),
                payload,
            })
            .expect("drifted packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let worker_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "worker_receipts")
            .expect("worker receipt audit should exist");
        assert!(!worker_check.passed);
        assert!(worker_check
            .details
            .pointer("/worker_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("role_binding"))))));
        let runtime_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "runtime_policy")
            .expect("runtime policy audit should exist");
        assert!(!runtime_check.passed);
        assert!(runtime_check
            .details
            .pointer("/runtime_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("role_binding"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "worker_receipts:role_binding"));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "runtime_policy:worker_receipt:role_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn worker_receipt_audit_rejects_missing_structured_receipt_fields() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-worker-receipt-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Worker receipt audit loop".to_string(),
                goal: "Catch incomplete structured worker receipts".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        let mut payload = doer_packet.payload.clone();
        payload
            .pointer_mut("/body/role_worker/receipt")
            .and_then(|value| value.as_object_mut())
            .expect("role worker receipt should be an object")
            .remove("action");
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: report.contract.id,
                round: report.contract.current_round,
                object_kind: "EXECUTION_PACKET".to_string(),
                writer_role: "doer".to_string(),
                route_from: "doer".to_string(),
                route_to: "evaluator".to_string(),
                state_code: "submitted".to_string(),
                payload,
            })
            .expect("drifted packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let worker_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "worker_receipts")
            .expect("worker receipt audit should exist");
        assert!(!worker_check.passed);
        assert!(worker_check
            .details
            .pointer("/worker_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("receipt.action"))))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_policy_audit_rejects_codex_workers_missing_command_context() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-codex-context-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Codex context audit loop".to_string(),
                goal: "Catch codex worker context drift".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        let mut payload = doer_packet.payload.clone();
        for pointer in ["/body/role_worker", "/body/runtime_worker"] {
            let worker = payload
                .pointer_mut(pointer)
                .and_then(|value| value.as_object_mut())
                .expect("worker should be an object");
            worker.insert("kind".to_string(), serde_json::json!("codex"));
            worker.insert("mode".to_string(), serde_json::json!("codex-exec"));
        }
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: report.contract.id,
                round: report.contract.current_round,
                object_kind: "EXECUTION_PACKET".to_string(),
                writer_role: "doer".to_string(),
                route_from: "doer".to_string(),
                route_to: "evaluator".to_string(),
                state_code: "submitted".to_string(),
                payload,
            })
            .expect("drifted packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let runtime_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "runtime_policy")
            .expect("runtime policy audit should exist");

        assert!(!runtime_check.passed);
        assert!(runtime_check
            .details
            .pointer("/runtime_policy_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("context.command"))))));
        assert!(audit_failure_details(&audit_report)
            .iter()
            .any(|detail| detail == "runtime_policy:worker_receipt:context.command"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_evidence_audit_rejects_codex_evidence_missing_command_context() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-codex-evidence-context-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Codex evidence context loop".to_string(),
                goal: "Catch codex evidence context drift".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_stage = report
            .stages
            .iter()
            .find(|stage| stage.role == "developer")
            .expect("developer stage should exist");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: report.contract.id,
                stage_id: Some(doer_stage.id),
                round: report.contract.current_round,
                kind: "execution_packet".to_string(),
                summary: "Drifted codex evidence without command context.".to_string(),
                path: None,
                payload: serde_json::json!({
                    "runtime": "codex",
                    "worker": {
                        "ok": true,
                        "kind": "codex",
                        "mode": "codex-exec",
                        "role": "developer",
                        "timeout_secs": 60,
                        "attempt_count": 1,
                        "max_attempts": 1,
                        "receipt_ok": true,
                        "receipt": {
                            "ok": true,
                            "role": "developer",
                            "action": "implement-admitted-candidate",
                            "evidence_summary": "codex evidence drifted",
                            "gates": { "packet_received": true }
                        }
                    }
                }),
            })
            .expect("drifted evidence should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let evidence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "stage_evidence")
            .expect("stage evidence audit should exist");

        assert!(!evidence_check.passed);
        assert!(evidence_check
            .details
            .pointer("/stage_evidence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .get("scope")
                .and_then(|value| value.as_str())
                == Some("evidence_worker")
                && error
                    .pointer("/errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|fields| fields
                        .iter()
                        .any(|field| field.as_str() == Some("context.command"))))));
        assert!(audit_failure_details(&audit_report)
            .iter()
            .any(|detail| detail == "stage_evidence:evidence_worker:context.command"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_evidence_audit_rejects_duplicate_stage_evidence() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-stage-evidence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Stage evidence audit loop".to_string(),
                goal: "Catch duplicated stage evidence in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_evidence = report
            .evidence
            .iter()
            .find(|row| row.kind == "execution_packet")
            .expect("doer evidence should exist");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: doer_evidence.loop_id,
                stage_id: doer_evidence.stage_id,
                round: doer_evidence.round,
                kind: doer_evidence.kind.clone(),
                summary: doer_evidence.summary.clone(),
                path: doer_evidence.path.clone(),
                payload: doer_evidence.payload.clone(),
            })
            .expect("duplicated evidence should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let evidence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "stage_evidence")
            .expect("stage evidence audit should exist");
        assert!(!evidence_check.passed);
        assert!(evidence_check
            .details
            .pointer("/stage_evidence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.stage_duplicate"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| { detail == "stage_evidence:evidence_stage:evidence.stage_duplicate" }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_sequence_audit_rejects_replayed_stages() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-stage-sequence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Stage replay audit loop".to_string(),
                goal: "Catch replayed stages in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_stage = report
            .stages
            .iter()
            .find(|stage| stage.role == "developer")
            .expect("developer stage should exist");
        store
            .insert_hive_loop_stage(HiveLoopStageCreate {
                loop_id: doer_stage.loop_id,
                round: doer_stage.round,
                role: doer_stage.role.clone(),
                status: doer_stage.status.clone(),
                summary: doer_stage.summary.clone(),
                input: doer_stage.input.clone(),
                output: doer_stage.output.clone(),
                started_at: doer_stage.started_at.clone(),
                completed_at: doer_stage.completed_at.clone(),
            })
            .expect("replayed stage should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let sequence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "stage_sequence")
            .expect("stage sequence audit should exist");
        assert!(!sequence_check.passed);
        assert!(sequence_check
            .details
            .pointer("/stage_sequence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("stage.role_duplicate"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "stage_sequence:stage_role:stage.role_duplicate"));
        let doctor_report = super::doctor(&store, created.contract.id)
            .expect("doctor should include audit details");
        assert!(doctor_report
            .failed_checks
            .iter()
            .any(|check| check == "stage_sequence"));
        assert!(doctor_report
            .audit_failure_details
            .iter()
            .any(|detail| detail == "stage_sequence:stage_role:stage.role_duplicate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn packet_sequence_audit_rejects_replayed_packets() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-packet-sequence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Packet replay audit loop".to_string(),
                goal: "Catch replayed packets in one round".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_packet = report
            .packets
            .iter()
            .find(|packet| packet.object_kind == "EXECUTION_PACKET")
            .expect("doer packet should exist");
        store
            .insert_hive_loop_packet(HiveLoopPacketCreate {
                loop_id: doer_packet.loop_id,
                round: doer_packet.round,
                object_kind: doer_packet.object_kind.clone(),
                writer_role: doer_packet.writer_role.clone(),
                route_from: doer_packet.route_from.clone(),
                route_to: doer_packet.route_to.clone(),
                state_code: doer_packet.state_code.clone(),
                payload: doer_packet.payload.clone(),
            })
            .expect("replayed packet should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let sequence_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "packet_sequence")
            .expect("packet sequence audit should exist");
        assert!(!sequence_check.passed);
        assert!(sequence_check
            .details
            .pointer("/packet_sequence_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("packet.route_duplicate"))))));
        let admission_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "admission_receipts")
            .expect("admission audit should exist");
        assert!(!admission_check.passed);
        assert!(admission_check
            .details
            .pointer("/admission_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("packet.admission_missing"))))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "packet_sequence:packet_route:packet.route_duplicate"));
        assert!(
            trace_report
                .trace
                .audit_failure_details
                .iter()
                .any(|detail| detail
                    == "admission_receipts:packet_admission:packet.admission_missing")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejected_admission_records_blocked_report_and_issue() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-admission-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Admission loop".to_string(),
                goal: "Block on a policy gate".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");

        let execution_policy = created
            .policies
            .iter()
            .find(|policy| policy.object_kind == "EXECUTION_PACKET")
            .expect("execution policy should exist");
        store
            .update_hive_loop_policy_gate(execution_policy.id, "unknown_gate")
            .expect("policy gate should be updated");

        let report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("admission rejection should still return a report");

        assert_eq!(report.contract.status, "blocked");
        assert_eq!(report.contract.active_phase, "developer");
        assert_eq!(report.issues[0].issue.status, "Blocked");
        assert_eq!(report.verdicts.len(), 1);
        assert_eq!(report.verdicts[0].decision, "blocked");
        assert_eq!(
            report.verdicts[0]
                .score
                .get("reason_code")
                .and_then(|value| value.as_str()),
            Some("admission_rejected")
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert_eq!(
            report.verdicts[0]
                .score
                .pointer("/score_vector/admission_integrity")
                .and_then(|value| value.as_f64()),
            Some(0.0)
        );
        assert_eq!(
            report.verdicts[0]
                .evidence
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(VERDICT_SCHEMA_VERSION)
        );
        assert!(report
            .admissions
            .iter()
            .any(|admission| admission.result == "rejected"
                && admission.reason == "unknown_gate failed"));
        let rejected_admission = report
            .admissions
            .iter()
            .find(|admission| admission.result == "rejected")
            .expect("rejected admission should be recorded");
        assert_eq!(
            rejected_admission
                .policy
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(ADMISSION_SCHEMA_VERSION)
        );
        assert_eq!(
            rejected_admission
                .policy
                .pointer("/packet/object_kind")
                .and_then(|value| value.as_str()),
            Some("EXECUTION_PACKET")
        );
        assert_eq!(
            rejected_admission
                .policy
                .pointer("/gate/passed")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert!(report
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "admission_rejection"));
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("blocked evidence report should resolve");
        let blocked_evidence = evidence_report
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "admission_rejection")
            .expect("admission rejection evidence should be summarized");
        assert_eq!(blocked_evidence.blocked_phase.as_deref(), Some("developer"));
        assert!(blocked_evidence
            .operator_options
            .iter()
            .any(|option| option == "retry"));
        assert!(blocked_evidence.missing_receipts.is_empty());
        assert!(report
            .issues
            .first()
            .expect("issue should exist")
            .comments
            .iter()
            .any(|comment| comment
                .body
                .contains("Compiler admission blocked at developer")));
        let blocked_trace = report.issues[0]
            .trace
            .as_ref()
            .expect("blocked issue should include trace");
        assert_eq!(blocked_trace.audit_passed, Some(false));
        assert!(blocked_trace
            .audit_failed_checks
            .iter()
            .any(|check| check == "active_policy_registry"));
        assert!(blocked_trace
            .audit_failed_checks
            .iter()
            .any(|check| check == "admission_receipts"));
        let audit_report =
            super::audit(&store, created.contract.id).expect("loop audit should resolve");
        assert!(!audit_report.passed);
        assert!(audit_report
            .checks
            .iter()
            .any(|check| check.name == "active_policy_registry" && !check.passed));
        assert!(audit_report
            .checks
            .iter()
            .any(|check| check.name == "admission_receipts" && !check.passed));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_decisions_update_issue_comment_and_loop_contract() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-decision-action-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Human decision loop".to_string(),
                goal: "Exercise issue decisions".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("loop should be created");
        let blocked = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should block");
        let issue_id = blocked.issues[0].issue.id;
        assert_eq!(
            blocked.issues[0]
                .actions
                .iter()
                .map(|action| action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["comment", "retry", "request-review", "cancel"]
        );
        let review_action = blocked.issues[0]
            .actions
            .iter()
            .find(|action| action.action == "request-review")
            .expect("blocked issue should expose review action");
        assert_eq!(review_action.schema_version, ISSUE_ACTION_SCHEMA_VERSION);
        assert_eq!(review_action.source, "human_options");
        assert_eq!(review_action.input, "note");
        assert!(review_action.confirmation_required);
        assert_eq!(
            review_action.confirmation_arg.as_deref(),
            Some(OPERATOR_ACTION_CONFIRMATION_ARG)
        );
        assert_eq!(
            review_action.receipt_schema.as_deref(),
            Some(OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION)
        );
        assert_eq!(
            review_action.policy_schema_version.as_deref(),
            Some(OPERATOR_ACTION_POLICY_SCHEMA_VERSION)
        );
        let comment_action = blocked.issues[0]
            .actions
            .iter()
            .find(|action| action.action == "comment")
            .expect("blocked issue should expose comment action");
        assert!(!comment_action.confirmation_required);
        assert!(comment_action.receipt_schema.is_none());
        let blocked_policy = issue_transition_policy(&store, issue_id)
            .expect("blocked issue transition policy should resolve");
        assert_eq!(
            blocked_policy.schema_version,
            ISSUE_TRANSITION_POLICY_SCHEMA_VERSION
        );
        assert_eq!(
            blocked_policy.registry.schema_version,
            POLICY_SCHEMA_VERSION
        );
        assert_eq!(blocked_policy.policy_owner, blocked_policy.registry.owner);
        assert_eq!(blocked_policy.policy_scope, blocked_policy.registry.scope);
        assert_eq!(
            blocked_policy
                .registry
                .actions
                .iter()
                .map(|action| action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["run", "comment", "retry", "request-review", "cancel"]
        );
        assert_eq!(
            blocked_policy
                .registry
                .reviewer_fallback
                .invalid_round_budget,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert_eq!(
            blocked_policy.registry.reviewer_fallback.fallback_status,
            "Blocked"
        );
        assert_eq!(blocked_policy.state_class, "needs_human");
        assert!(blocked_policy.human_decision_required);
        assert_eq!(
            blocked_policy
                .allowed_actions
                .iter()
                .map(|action| action.action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["comment", "retry", "request-review", "cancel"]
        );
        assert!(blocked_policy
            .blocked_actions
            .iter()
            .any(|action| action.action == "run"
                && action
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("retry-run"))));
        assert!(blocked_policy.confirmation.required);
        assert_eq!(
            blocked_policy.confirmation.required_actions,
            vec!["cancel", "request-review", "retry"]
        );
        let blocked_budget = blocked_policy
            .reviewer_budget
            .as_ref()
            .expect("loop issue should expose reviewer budget");
        assert_eq!(
            blocked_budget.reviewer_invalid_round_budget,
            REVIEWER_INVALID_ROUND_BUDGET
        );
        assert_eq!(blocked_budget.fallback_status, "Blocked");
        assert_eq!(
            blocked_policy.resources.transition_policy,
            format!("entrance://issues/{issue_id}/transition-policy")
        );
        let mut corrupt_actions = blocked.issues[0].actions.clone();
        corrupt_actions.retain(|action| action.action != "request-review");
        corrupt_actions[0].schema_version = "bad.schema".to_string();
        corrupt_actions[1].source = "status_fallback".to_string();
        corrupt_actions
            .iter_mut()
            .find(|action| action.action == "cancel")
            .expect("cancel action should exist")
            .destructive = false;
        corrupt_actions
            .iter_mut()
            .find(|action| action.action == "retry")
            .expect("retry action should exist")
            .confirmation_required = false;
        let action_error = issue_action_audit_error(
            &blocked.issues[0].issue,
            &blocked.contract,
            blocked.issues[0]
                .trace
                .as_ref()
                .expect("blocked issue should include trace"),
            &corrupt_actions,
        )
        .expect("corrupt action metadata should fail audit");
        let action_error_fields = action_error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .expect("action errors should be listed")
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(action_error_fields.contains(&"action.sequence"));
        assert!(action_error_fields.contains(&"action.schema_version"));
        assert!(action_error_fields.contains(&"action.source"));
        assert!(action_error_fields.contains(&"action.destructive"));
        assert!(action_error_fields.contains(&"action.confirmation_required"));

        let review_receipt = OperatorConfirmationReceipt {
            schema_version: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
            source: "mcp".to_string(),
            policy_schema_version: "entrance.mcp.permission_policy.v1".to_string(),
            confirmation_arg: "human_confirmed".to_string(),
            human_confirmed: true,
            action: "request-review".to_string(),
            author: "human".to_string(),
            marker: "MCP confirmation: human_confirmed=true; action=request-review; author=human; policy=entrance.mcp.permission_policy.v1".to_string(),
            client: None,
            actor: Some(OperatorConfirmationActor {
                id: "mcp:human".to_string(),
                label: "human".to_string(),
                source: "author_arg".to_string(),
                trust: "self_reported".to_string(),
                verified: false,
            }),
        };
        let review_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "request-review".to_string(),
                author: "human".to_string(),
                body: Some("Need policy owner".to_string()),
                confirmation_receipt: Some(review_receipt.clone()),
            },
        )
        .expect("issue should move to review");
        let review_contract = store
            .get_hive_loop_contract(created.contract.id)
            .expect("contract query should succeed")
            .expect("contract should exist");
        assert_eq!(review_card.issue.status, "Needs Review");
        assert_eq!(review_contract.status, "needs-review");
        assert_eq!(review_contract.active_phase, "human-review");
        let review_policy = issue_transition_policy(&store, issue_id)
            .expect("review issue transition policy should resolve");
        assert_eq!(review_policy.state_class, "needs_human");
        assert!(review_policy.human_decision_required);
        assert!(review_policy
            .allowed_actions
            .iter()
            .any(|action| action.action.action == "retry"
                && action.gate == "human_confirmed_retry_boundary"));
        assert!(review_policy
            .blocked_actions
            .iter()
            .any(|action| action.action == "request-review"));
        assert_eq!(
            review_card
                .actions
                .iter()
                .map(|action| action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["comment", "retry", "cancel"]
        );
        let review_doctor = review_card
            .doctor
            .as_ref()
            .expect("review card should include doctor summary");
        assert_eq!(review_doctor.health, "needs_review");
        assert_eq!(review_doctor.counts.audit_failed_count, 1);
        assert_eq!(review_doctor.failed_checks, vec!["runtime_policy"]);
        assert!(review_doctor
            .next_actions
            .iter()
            .any(|action| action.contains("issue retry-run")));
        assert!(review_doctor
            .next_actions
            .iter()
            .any(|action| action.contains("issue show") && action.contains("--compact")));
        assert!(!review_doctor
            .next_actions
            .iter()
            .any(|action| action.contains("request-review")));
        assert!(review_card
            .comments
            .iter()
            .any(|comment| comment.body.contains("Need policy owner")));
        assert!(review_card.comments.iter().any(|comment| {
            comment.body.contains("Need policy owner")
                && comment
                    .payload
                    .get("schema_version")
                    .and_then(|value| value.as_str())
                    == Some(OPERATOR_DECISION_SCHEMA_VERSION)
                && comment
                    .payload
                    .get("action")
                    .and_then(|value| value.as_str())
                    == Some("request-review")
                && comment.payload.get("confirmation_receipt")
                    == Some(&serde_json::to_value(&review_receipt).expect("receipt should encode"))
        }));
        let review_decision_comment = review_card
            .comments
            .iter()
            .find(|comment| {
                comment
                    .payload
                    .get("action")
                    .and_then(|value| value.as_str())
                    == Some("request-review")
            })
            .expect("review decision comment should be visible");
        assert_eq!(
            review_decision_comment
                .payload
                .pointer("/transition_admission/schema_version")
                .and_then(|value| value.as_str()),
            Some(ISSUE_TRANSITION_ADMISSION_SCHEMA_VERSION)
        );
        assert_eq!(
            review_decision_comment
                .payload
                .pointer("/transition_admission/action")
                .and_then(|value| value.as_str()),
            Some("request-review")
        );
        assert_eq!(
            review_decision_comment
                .payload
                .pointer("/transition_admission/requires_confirmation")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        let review_evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        assert!(review_evidence.iter().any(|evidence| {
            evidence.kind == "operator_decision"
                && evidence
                    .payload
                    .get("schema_version")
                    .and_then(|value| value.as_str())
                    == Some(OPERATOR_DECISION_SCHEMA_VERSION)
                && evidence
                    .payload
                    .pointer("/operator/action")
                    .and_then(|value| value.as_str())
                    == Some("request-review")
                && evidence.payload.pointer("/operator/confirmation_receipt")
                    == Some(&serde_json::to_value(&review_receipt).expect("receipt should encode"))
                && evidence
                    .payload
                    .pointer("/transition_admission/action")
                    .and_then(|value| value.as_str())
                    == Some("request-review")
        }));
        let review_timeline =
            super::issue_timeline(&store, issue_id).expect("review timeline should resolve");
        assert_eq!(review_timeline.timeline_state, "needs_human");
        assert_eq!(review_timeline.counts.decision_receipt_count, 1);
        assert_eq!(review_timeline.decision_receipts.len(), 1);
        let timeline_receipt = review_timeline
            .decision_receipts
            .first()
            .expect("review timeline should expose decision receipt");
        assert_eq!(timeline_receipt.action.as_deref(), Some("request-review"));
        assert_eq!(timeline_receipt.author.as_deref(), Some("human"));
        assert_eq!(timeline_receipt.source, "comment+evidence");
        assert_eq!(
            timeline_receipt.comment_id,
            Some(review_card.comments.last().unwrap().id)
        );
        assert!(timeline_receipt.evidence_id.is_some());
        assert_eq!(
            timeline_receipt.receipt_schema_version.as_deref(),
            Some(OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION)
        );
        assert_eq!(timeline_receipt.receipt_source.as_deref(), Some("mcp"));
        assert_eq!(timeline_receipt.human_confirmed, Some(true));
        assert_eq!(
            timeline_receipt.actor_trust.as_deref(),
            Some("self_reported")
        );
        assert!(timeline_receipt
            .linked_resource
            .contains("/evidence-drilldown"));
        assert_eq!(review_timeline.human_decision.receipt_count, 1);
        assert_eq!(
            review_timeline
                .human_decision
                .last_receipt
                .as_ref()
                .and_then(|receipt| receipt.action.as_deref()),
            Some("request-review")
        );
        let review_trace = review_card
            .trace
            .as_ref()
            .expect("review card should retain loop trace");
        assert_eq!(review_trace.audit_failed_count, 1);
        assert_eq!(review_trace.operator_event_count, 1);
        assert_eq!(review_trace.round_operator_event_count, 1);
        assert_eq!(
            review_trace
                .last_operator_event
                .as_ref()
                .and_then(|event| event.action.as_deref()),
            Some("request-review")
        );
        let review_event = review_trace
            .last_operator_event
            .as_ref()
            .expect("review trace should expose last operator event");
        assert_eq!(
            review_event.admission_action.as_deref(),
            Some("request-review")
        );
        assert_eq!(
            review_event.admission_gate.as_deref(),
            Some("human_confirmed_review_boundary")
        );
        assert_eq!(
            review_event.admission_from_status.as_deref(),
            Some("Blocked")
        );
        assert_eq!(
            review_event.admission_to_status.as_deref(),
            Some("Needs Review")
        );
        assert_eq!(review_event.admission_requires_confirmation, Some(true));
        assert_eq!(
            review_event.admission_policy_resource.as_deref(),
            Some("entrance://policy/registry")
        );
        assert_eq!(
            review_event.admission_transition_policy_resource.as_deref(),
            Some(format!("entrance://issues/{issue_id}/transition-policy").as_str())
        );
        assert_eq!(
            review_trace
                .operator_events
                .first()
                .and_then(|event| event.issue_status.as_deref()),
            Some("Needs Review")
        );
        let review_decision_summary = review_trace
            .evidence
            .iter()
            .find(|evidence| evidence.kind == "operator_decision")
            .expect("review decision should be summarized");
        assert_eq!(
            review_decision_summary.operator_author.as_deref(),
            Some("human")
        );
        assert_eq!(
            review_decision_summary.operator_action.as_deref(),
            Some("request-review")
        );
        let review_audit =
            super::audit(&store, created.contract.id).expect("review audit should resolve");
        assert!(!review_audit.passed);
        assert!(review_audit
            .checks
            .iter()
            .any(|check| check.name == "stage_sequence" && check.passed));
        assert!(review_audit
            .checks
            .iter()
            .any(|check| check.name == "stage_evidence" && check.passed));
        assert!(review_audit
            .checks
            .iter()
            .any(|check| check.name == "runtime_policy" && !check.passed));

        let retry_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: None,
                confirmation_receipt: Some(test_confirmation_receipt("retry", "human")),
            },
        )
        .expect("issue should retry");
        let retry_contract = store
            .get_hive_loop_contract(created.contract.id)
            .expect("contract query should succeed")
            .expect("contract should exist");
        assert_eq!(retry_card.issue.status, "Todo");
        assert_eq!(retry_contract.status, "todo");
        assert_eq!(retry_contract.active_phase, "explorer");
        assert_eq!(
            retry_contract.current_round,
            blocked.contract.current_round + 1
        );
        let retry_trace = retry_card
            .trace
            .as_ref()
            .expect("retry card should retain loop trace");
        assert_eq!(retry_trace.current_round, retry_contract.current_round);
        assert_eq!(retry_trace.packet_count, 1);
        assert_eq!(retry_trace.admission_count, 1);
        assert_eq!(retry_trace.evidence_count, 3);
        assert_eq!(retry_trace.verdict_count, 1);
        assert_eq!(retry_trace.round_packet_count, 0);
        assert_eq!(retry_trace.round_admission_count, 0);
        assert_eq!(retry_trace.round_evidence_count, 1);
        assert_eq!(retry_trace.round_verdict_count, 0);
        assert_eq!(retry_trace.round_receipt_required_count, 0);
        assert_eq!(retry_trace.round_receipt_missing_count, 0);
        assert_eq!(retry_trace.role_worker_count, 0);
        assert_eq!(retry_trace.role_worker_ok_count, 0);
        assert_eq!(retry_trace.round_role_worker_count, 0);
        assert_eq!(retry_trace.round_role_worker_ok_count, 0);
        assert_eq!(retry_trace.round_worker_duration_ms, 0);
        assert_eq!(retry_trace.round_worker_timeout_count, 0);
        assert_eq!(retry_trace.round_worker_retry_exhausted_count, 0);
        assert_eq!(retry_trace.verdict_schema, None);
        assert_eq!(retry_trace.last_decision, None);
        assert_eq!(retry_trace.worker_kind, None);
        assert_eq!(retry_trace.human_options, vec!["comment", "cancel"]);
        assert_eq!(retry_trace.operator_event_count, 2);
        assert_eq!(retry_trace.round_operator_event_count, 1);
        assert_eq!(
            retry_trace
                .last_operator_event
                .as_ref()
                .and_then(|event| event.action.as_deref()),
            Some("retry")
        );
        let retry_event = retry_trace
            .last_operator_event
            .as_ref()
            .expect("retry trace should expose last operator event");
        assert_eq!(retry_event.admission_action.as_deref(), Some("retry"));
        assert_eq!(
            retry_event.admission_gate.as_deref(),
            Some("human_confirmed_retry_boundary")
        );
        assert_eq!(
            retry_event.admission_from_status.as_deref(),
            Some("Needs Review")
        );
        assert_eq!(
            retry_event.admission_to_status.as_deref(),
            Some("Todo, then runtime_verdict")
        );
        assert_eq!(retry_event.admission_requires_confirmation, Some(true));
        assert_eq!(
            retry_event.admission_policy_resource.as_deref(),
            Some("entrance://policy/registry")
        );
        assert_eq!(
            retry_event.admission_transition_policy_resource.as_deref(),
            Some(format!("entrance://issues/{issue_id}/transition-policy").as_str())
        );

        let cancel_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "cancel".to_string(),
                author: "human".to_string(),
                body: None,
                confirmation_receipt: Some(test_confirmation_receipt("cancel", "human")),
            },
        )
        .expect("issue should cancel");
        let cancel_contract = store
            .get_hive_loop_contract(created.contract.id)
            .expect("contract query should succeed")
            .expect("contract should exist");
        assert_eq!(cancel_card.issue.status, "Canceled");
        assert_eq!(cancel_contract.status, "rejected");
        assert_eq!(cancel_contract.active_phase, "complete");
        let cancel_policy = issue_transition_policy(&store, issue_id)
            .expect("canceled issue transition policy should resolve");
        assert_eq!(cancel_policy.state_class, "terminal");
        assert!(!cancel_policy.human_decision_required);
        assert_eq!(
            cancel_policy
                .allowed_actions
                .iter()
                .map(|action| action.action.action.as_str())
                .collect::<Vec<_>>(),
            vec!["comment"]
        );
        assert_eq!(
            cancel_card
                .trace
                .as_ref()
                .expect("cancel card should retain trace")
                .human_options,
            vec!["comment"]
        );
        let cancel_trace = cancel_card
            .trace
            .as_ref()
            .expect("cancel card should retain trace");
        assert_eq!(cancel_trace.operator_event_count, 3);
        assert_eq!(cancel_trace.round_operator_event_count, 2);
        assert_eq!(
            cancel_trace
                .last_operator_event
                .as_ref()
                .and_then(|event| event.action.as_deref()),
            Some("cancel")
        );
        let cancel_event = cancel_trace
            .last_operator_event
            .as_ref()
            .expect("cancel trace should expose last operator event");
        assert_eq!(cancel_event.admission_action.as_deref(), Some("cancel"));
        assert_eq!(
            cancel_event.admission_gate.as_deref(),
            Some("human_confirmed_cancel_boundary")
        );
        assert_eq!(cancel_event.admission_from_status.as_deref(), Some("Todo"));
        assert_eq!(
            cancel_event.admission_to_status.as_deref(),
            Some("Canceled")
        );
        assert_eq!(cancel_event.admission_requires_confirmation, Some(true));
        assert_eq!(
            cancel_event.admission_policy_resource.as_deref(),
            Some("entrance://policy/registry")
        );
        assert_eq!(
            cancel_event.admission_transition_policy_resource.as_deref(),
            Some(format!("entrance://issues/{issue_id}/transition-policy").as_str())
        );
        assert_eq!(
            cancel_trace
                .operator_events
                .iter()
                .filter_map(|event| event.action.as_deref())
                .collect::<Vec<_>>(),
            vec!["retry", "cancel"]
        );
        assert!(cancel_card
            .comments
            .iter()
            .any(|comment| comment.body.contains("Human canceled")));
        let decision_evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        assert!(decision_evidence.iter().any(|evidence| {
            evidence.kind == "operator_decision"
                && evidence.round == retry_contract.current_round
                && evidence
                    .payload
                    .pointer("/issue/comment_id")
                    .and_then(|value| value.as_i64())
                    .is_some()
                && evidence
                    .payload
                    .pointer("/operator/action")
                    .and_then(|value| value.as_str())
                    == Some("cancel")
        }));
        let audit_report =
            super::audit(&store, created.contract.id).expect("decision audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(issue_surface_check.passed);
        let retry_after_cancel = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "retry".to_string(),
                author: "human".to_string(),
                body: None,
                confirmation_receipt: None,
            },
        );
        assert!(retry_after_cancel
            .expect_err("human-canceled issue should not retry")
            .to_string()
            .contains("not admitted"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_run_executes_todo_and_retry_control_flow() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-run-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let todo = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue run todo loop".to_string(),
                goal: "Run a Todo issue from the issue surface".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("todo loop should be created");
        let todo_report = run_issue(
            &store,
            IssueRunRequest {
                issue_id: todo.issues[0].issue.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
                retry: false,
                author: "human".to_string(),
                body: None,
                confirmation_receipt: None,
            },
        )
        .expect("todo issue should run");
        assert_eq!(todo_report.contract.status, "kept");
        assert_eq!(todo_report.issues[0].issue.status, "Done");

        let blocked = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue retry-run loop".to_string(),
                goal: "Retry a blocked issue from the issue surface".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("blocked loop should be created");
        let blocked_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: blocked.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("unsupported runtime should block");
        let blocked_issue_id = blocked_report.issues[0].issue.id;
        let blocked_run = run_issue(
            &store,
            IssueRunRequest {
                issue_id: blocked_issue_id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
                retry: false,
                author: "human".to_string(),
                body: None,
                confirmation_receipt: None,
            },
        );
        assert!(blocked_run
            .expect_err("blocked issue should require retry-run")
            .to_string()
            .contains("retry-run"));

        let retry_report = run_issue(
            &store,
            IssueRunRequest {
                issue_id: blocked_issue_id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: Some(5),
                worker_attempts: Some(1),
                retry: true,
                author: "human".to_string(),
                body: Some("Retry with local runtime".to_string()),
                confirmation_receipt: Some(test_confirmation_receipt("retry", "human")),
            },
        )
        .expect("retry-run should record decision and execute");
        assert_eq!(retry_report.contract.status, "kept");
        assert_eq!(retry_report.contract.current_round, 2);
        assert_eq!(retry_report.issues[0].issue.status, "Done");
        assert!(retry_report.issues[0]
            .comments
            .iter()
            .any(|comment| comment.body.contains("Retry with local runtime")));
        let operator_decisions = store
            .list_hive_loop_evidence(blocked.contract.id)
            .expect("evidence should list")
            .into_iter()
            .filter(|evidence| evidence.kind == "operator_decision")
            .collect::<Vec<_>>();
        assert_eq!(operator_decisions.len(), 1);
        assert_eq!(operator_decisions[0].round, 2);
        let audit_report =
            super::audit(&store, blocked.contract.id).expect("retry audit should resolve");
        assert!(audit_report.passed);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_comments_record_operator_evidence() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-comment-evidence-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Human comment loop".to_string(),
                goal: "Capture issue comments as loop evidence".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "unsupported-agent".to_string(),
            },
        )
        .expect("loop should be created");
        let blocked = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("unsupported-agent".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should block");
        let issue_id = blocked.issues[0].issue.id;

        let comment_card = add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "operator".to_string(),
                body: "  Please inspect the missing role worker receipt.  ".to_string(),
            },
        )
        .expect("comment should be recorded");
        let operator_comment = comment_card
            .comments
            .iter()
            .find(|comment| comment.author == "operator")
            .expect("operator comment should be visible");
        assert_eq!(
            operator_comment.body,
            "Please inspect the missing role worker receipt."
        );
        assert_eq!(
            operator_comment
                .payload
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(OPERATOR_COMMENT_SCHEMA_VERSION)
        );
        assert_eq!(
            operator_comment
                .payload
                .get("loop_id")
                .and_then(|value| value.as_i64()),
            Some(created.contract.id)
        );
        assert_eq!(
            operator_comment
                .payload
                .get("round")
                .and_then(|value| value.as_i64()),
            Some(blocked.contract.current_round)
        );
        assert_eq!(
            operator_comment
                .payload
                .get("status")
                .and_then(|value| value.as_str()),
            Some("Blocked")
        );
        assert_eq!(
            operator_comment
                .payload
                .get("phase")
                .and_then(|value| value.as_str()),
            Some(blocked.contract.active_phase.as_str())
        );
        assert_eq!(
            operator_comment
                .payload
                .pointer("/transition_admission/schema_version")
                .and_then(|value| value.as_str()),
            Some(ISSUE_TRANSITION_ADMISSION_SCHEMA_VERSION)
        );
        assert_eq!(
            operator_comment
                .payload
                .pointer("/transition_admission/action")
                .and_then(|value| value.as_str()),
            Some("comment")
        );
        assert_eq!(
            operator_comment
                .payload
                .pointer("/transition_admission/requires_confirmation")
                .and_then(|value| value.as_bool()),
            Some(false)
        );

        let evidence = store
            .list_hive_loop_evidence(created.contract.id)
            .expect("loop evidence should list");
        let comment_evidence = evidence
            .iter()
            .find(|evidence| evidence.kind == "operator_comment")
            .expect("operator comment should be ledger evidence");
        assert_eq!(comment_evidence.round, blocked.contract.current_round);
        assert_eq!(
            comment_evidence
                .payload
                .get("schema_version")
                .and_then(|value| value.as_str()),
            Some(OPERATOR_COMMENT_SCHEMA_VERSION)
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/operator/comment_body")
                .and_then(|value| value.as_str()),
            Some("Please inspect the missing role worker receipt.")
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/issue/comment_id")
                .and_then(|value| value.as_i64()),
            Some(operator_comment.id)
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/loop/round")
                .and_then(|value| value.as_i64()),
            Some(blocked.contract.current_round)
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/loop/phase")
                .and_then(|value| value.as_str()),
            Some(blocked.contract.active_phase.as_str())
        );
        assert_eq!(
            comment_evidence
                .payload
                .pointer("/transition_admission/action")
                .and_then(|value| value.as_str()),
            Some("comment")
        );
        let evidence_report = super::evidence_report(&store, created.contract.id)
            .expect("evidence report should resolve");
        assert!(evidence_report.evidence.iter().any(|evidence| {
            evidence.kind == "operator_comment"
                && evidence.summary == "Please inspect the missing role worker receipt."
                && evidence.schema_version.as_deref() == Some(OPERATOR_COMMENT_SCHEMA_VERSION)
                && evidence.operator_author.as_deref() == Some("operator")
        }));
        let audit_report =
            super::audit(&store, created.contract.id).expect("comment audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(issue_surface_check.passed);
        assert!(issue_surface_check
            .details
            .pointer("/operator_evidence_count")
            .and_then(|value| value.as_u64())
            .is_some_and(|count| count >= 1));
        assert!(add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "operator".to_string(),
                body: "   ".to_string(),
            },
        )
        .is_err());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_untyped_comments() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-surface-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue surface audit loop".to_string(),
                goal: "Detect untyped control-plane comments".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;
        store
            .insert_hive_comment(HiveCommentCreate {
                issue_id,
                author: "human".to_string(),
                body: "untyped compatibility note".to_string(),
                payload: serde_json::json!({}),
            })
            .expect("untyped comment should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        assert!(issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .is_some_and(|errors| errors.iter().any(|error| error
                .pointer("/errors")
                .and_then(|value| value.as_array())
                .is_some_and(|fields| fields
                    .iter()
                    .any(|field| field.as_str() == Some("comment.payload.schema_version"))))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_issue_contract_status_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-issue-status-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Issue status drift loop".to_string(),
                goal: "Detect drift between contract and issue status".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let run_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let issue_id = run_report.issues[0].issue.id;
        assert_eq!(run_report.contract.status, "kept");
        assert_eq!(run_report.issues[0].issue.status, "Done");

        store
            .update_hive_issue_status(issue_id, "Todo", Some("drifted issue status"))
            .expect("issue status should be mutated for audit probe");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        let errors = issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .expect("issue surface errors should be listed");
        assert!(errors.iter().any(|error| {
            error
                .pointer("/expected_status")
                .and_then(|value| value.as_str())
                == Some("Done")
                && error
                    .pointer("/actual_status")
                    .and_then(|value| value.as_str())
                    == Some("Todo")
                && error
                    .pointer("/errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|fields| {
                        fields
                            .iter()
                            .any(|field| field.as_str() == Some("issue.contract_status_binding"))
                    })
        }));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:issue:issue.contract_status_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_stage_system_comment_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-stage-comment-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Stage comment audit loop".to_string(),
                goal: "Detect drift between stage comments and stage evidence".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;
        let run_report = run(
            &store,
            HiveLoopRunRequest {
                loop_id: created.contract.id,
                runtime: Some("local".to_string()),
                decision: None,
                worker_timeout_secs: None,
                worker_attempts: None,
            },
        )
        .expect("loop should run");
        let doer_evidence = run_report
            .evidence
            .iter()
            .find(|row| row.kind == "execution_packet")
            .expect("developer evidence should exist");
        let doer_worker = doer_evidence
            .payload
            .get("worker")
            .cloned()
            .expect("developer evidence should carry a worker receipt");

        store
            .insert_hive_comment(HiveCommentCreate {
                issue_id,
                author: "hive".to_string(),
                body: "Developer admitted the execution packet.".to_string(),
                payload: serde_json::json!({
                    "schema_version": SYSTEM_COMMENT_SCHEMA_VERSION,
                    "source": "hive",
                    "loop_id": created.contract.id,
                    "round": 1,
                    "phase": "developer",
                    "stage_role": "developer",
                    "evidence_kind": "verdict_packet",
                    "evidence_id": doer_evidence.id,
                    "admission": "admitted",
                    "worker": doer_worker
                }),
            })
            .expect("drifted stage comment should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        let errors = issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .expect("issue surface errors should be listed");
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("comment.stage.evidence_kind"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("comment.stage.evidence_binding")))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:comment:comment.stage.evidence_binding"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn issue_surface_audit_rejects_operator_evidence_drift() {
        let root = std::env::temp_dir().join(format!(
            "entrance-hive-loop-operator-evidence-audit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should be created");
        let store = Store::open(root.join("entrance.db")).expect("store should open");

        let created = create(
            &store,
            HiveLoopCreateRequest {
                title: "Operator evidence audit loop".to_string(),
                goal: "Detect drift between operator comments and evidence".to_string(),
                boundary: String::new(),
                approach_space: Vec::new(),
                eval_space: Vec::new(),
                review_surface: String::new(),
                autonomy_level: String::new(),
                runtime: "local".to_string(),
            },
        )
        .expect("loop should be created");
        let issue_id = created.issues[0].issue.id;

        let comment_card = add_comment(
            &store,
            IssueCommentRequest {
                issue_id,
                author: "human".to_string(),
                body: "Keep the operator trail honest".to_string(),
            },
        )
        .expect("operator comment should be recorded");
        let operator_comment = comment_card
            .comments
            .iter()
            .find(|comment| comment.author == "human")
            .expect("operator comment should be visible");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_comment".to_string(),
                summary: "drifted comment evidence".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "status": comment_card.issue.status,
                        "comment_id": operator_comment.id
                    },
                    "loop": {
                        "id": created.contract.id,
                        "status": created.contract.status,
                        "phase": created.contract.active_phase,
                        "round": created.contract.current_round
                    },
                    "operator": {
                        "author": "different-human",
                        "comment_body": "drifted body"
                    }
                }),
            })
            .expect("drifted operator comment evidence should insert");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_comment".to_string(),
                summary: "drifted comment loop binding".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_COMMENT_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "status": "Drifted",
                        "comment_id": operator_comment.id
                    },
                    "loop": {
                        "id": created.contract.id + 99,
                        "status": "blocked",
                        "phase": "doer",
                        "round": created.contract.current_round + 99
                    },
                    "operator": {
                        "author": "human",
                        "comment_body": operator_comment.body
                    }
                }),
            })
            .expect("drifted operator comment binding evidence should insert");

        let cancel_receipt = OperatorConfirmationReceipt {
            schema_version: OPERATOR_CONFIRMATION_RECEIPT_SCHEMA_VERSION.to_string(),
            source: "mcp".to_string(),
            policy_schema_version: "entrance.mcp.permission_policy.v1".to_string(),
            confirmation_arg: "human_confirmed".to_string(),
            human_confirmed: true,
            action: "cancel".to_string(),
            author: "human".to_string(),
            marker: "MCP confirmation: human_confirmed=true; action=cancel; author=human; policy=entrance.mcp.permission_policy.v1".to_string(),
            client: None,
            actor: None,
        };
        let cancel_card = decide_issue(
            &store,
            IssueDecisionRequest {
                issue_id,
                action: "cancel".to_string(),
                author: "human".to_string(),
                body: Some("No longer needed".to_string()),
                confirmation_receipt: Some(cancel_receipt),
            },
        )
        .expect("todo issue should cancel");
        let cancel_comment = cancel_card
            .comments
            .iter()
            .find(|comment| {
                comment
                    .payload
                    .get("action")
                    .and_then(|value| value.as_str())
                    == Some("cancel")
            })
            .expect("cancel decision comment should be visible");
        let mut drifted_transition_admission = cancel_comment
            .payload
            .get("transition_admission")
            .cloned()
            .expect("cancel decision should carry transition admission");
        drifted_transition_admission["from_status"] = serde_json::json!("Blocked");
        drifted_transition_admission["to_status"] = serde_json::json!("Todo");
        drifted_transition_admission["policy_resource"] =
            serde_json::json!("entrance://policy/drifted");
        drifted_transition_admission["transition_policy_resource"] =
            serde_json::json!("entrance://issues/999/transition-policy");
        drifted_transition_admission["allowed_actions"] = serde_json::json!(["comment"]);
        let mut drifted_transition_payload = serde_json::json!({
            "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
            "source": "issue/status/comment",
            "issue": {
                "id": issue_id,
                "comment_id": cancel_comment.id,
                "from_status": "Todo",
                "to_status": "Canceled"
            },
            "loop": {
                "id": created.contract.id,
                "next_status": "rejected",
                "next_phase": "complete",
                "round": created.contract.current_round
            },
            "operator": {
                "author": "human",
                "action": "cancel",
                "note": "drifted transition admission",
                "comment_body": cancel_comment.body
            },
            "transition_admission": drifted_transition_admission
        });
        if let Some(receipt) = cancel_comment.payload.get("confirmation_receipt") {
            drifted_transition_payload["operator"]["confirmation_receipt"] = receipt.clone();
        }
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_decision".to_string(),
                summary: "drifted transition admission evidence".to_string(),
                path: None,
                payload: drifted_transition_payload,
            })
            .expect("drifted transition admission evidence should insert");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_decision".to_string(),
                summary: "drifted decision evidence".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "comment_id": cancel_comment.id,
                        "from_status": "Todo",
                        "to_status": "Todo"
                    },
                    "loop": {
                        "id": created.contract.id,
                        "next_status": "todo",
                        "next_phase": "explorer",
                        "round": created.contract.current_round
                    },
                    "operator": {
                        "author": "human",
                        "action": "retry",
                        "note": "wrong action",
                        "comment_body": cancel_comment.body
                    }
                }),
            })
            .expect("drifted operator decision evidence should insert");
        store
            .insert_hive_loop_evidence(HiveLoopEvidenceCreate {
                loop_id: created.contract.id,
                stage_id: None,
                round: created.contract.current_round,
                kind: "operator_decision".to_string(),
                summary: "drifted retry round binding".to_string(),
                path: None,
                payload: serde_json::json!({
                    "schema_version": OPERATOR_DECISION_SCHEMA_VERSION,
                    "source": "issue/status/comment",
                    "issue": {
                        "id": issue_id,
                        "comment_id": cancel_comment.id,
                        "from_status": "Todo",
                        "to_status": "Canceled"
                    },
                    "loop": {
                        "id": created.contract.id + 99,
                        "next_status": "rejected",
                        "next_phase": "explorer",
                        "round": created.contract.current_round + 99
                    },
                    "operator": {
                        "author": "human",
                        "action": "cancel",
                        "note": "wrong round",
                        "comment_body": cancel_comment.body
                    }
                }),
            })
            .expect("drifted decision round evidence should insert");

        let audit_report = super::audit(&store, created.contract.id).expect("audit should resolve");
        let issue_surface_check = audit_report
            .checks
            .iter()
            .find(|check| check.name == "issue_surface")
            .expect("issue surface audit should exist");
        assert!(!issue_surface_check.passed);
        let errors = issue_surface_check
            .details
            .pointer("/issue_surface_errors")
            .and_then(|value| value.as_array())
            .expect("issue surface errors should be listed");
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.author_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.comment_body_binding")))));
        assert!(errors.iter().any(|error| {
            error.pointer("/kind").and_then(|value| value.as_str()) == Some("operator_comment")
                && error
                    .pointer("/errors")
                    .and_then(|value| value.as_array())
                    .is_some_and(|fields| {
                        fields
                            .iter()
                            .any(|field| field.as_str() == Some("evidence.loop_id_binding"))
                            && fields
                                .iter()
                                .any(|field| field.as_str() == Some("evidence.loop_round_binding"))
                            && fields.iter().any(|field| {
                                field.as_str() == Some("evidence.comment_round_binding")
                            })
                            && fields.iter().any(|field| {
                                field.as_str() == Some("evidence.comment_status_binding")
                            })
                            && fields.iter().any(|field| {
                                field.as_str() == Some("evidence.comment_phase_binding")
                            })
                    })
        }));
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.action_binding")))));
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.confirmation_receipt_binding")))));
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields.iter().any(|field| {
                field.as_str() == Some("transition_admission.from_status_binding")
            }) && fields.iter().any(|field| {
                field.as_str() == Some("transition_admission.to_status_binding")
            }) && fields
                .iter()
                .any(|field| field.as_str() == Some("transition_admission.policy_resource"))
                && fields.iter().any(|field| {
                    field.as_str() == Some("transition_admission.transition_policy_resource")
                })
                && fields.iter().any(|field| {
                    field.as_str() == Some("transition_admission.allowed_action_binding")
                }))));
        assert!(errors.iter().any(|error| error
            .pointer("/errors")
            .and_then(|value| value.as_array())
            .is_some_and(|fields| fields
                .iter()
                .any(|field| field.as_str() == Some("evidence.loop_id_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.loop_round_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.loop_phase_binding"))
                && fields
                    .iter()
                    .any(|field| field.as_str() == Some("evidence.comment_next_round_binding")))));
        let trace_report =
            super::trace(&store, created.contract.id).expect("trace should include audit details");
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.author_binding"));
        assert!(trace_report.trace.audit_failure_details.iter().any(
            |detail| detail == "issue_surface:operator_evidence:evidence.comment_body_binding"
        ));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.action_binding"));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| {
                detail == "issue_surface:operator_evidence:evidence.confirmation_receipt_binding"
            }));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| {
                detail == "issue_surface:operator_evidence:transition_admission.from_status_binding"
            }));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| {
                detail == "issue_surface:operator_evidence:transition_admission.to_status_binding"
            }));
        assert!(trace_report
            .trace
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.loop_round_binding"));
        assert!(trace_report.trace.audit_failure_details.iter().any(
            |detail| detail == "issue_surface:operator_evidence:evidence.comment_round_binding"
        ));
        let doctor_report = super::doctor(&store, created.contract.id)
            .expect("doctor should include audit details");
        assert!(doctor_report.audit_failure_details.iter().any(
            |detail| detail == "issue_surface:operator_evidence:evidence.comment_body_binding"
        ));
        let issue_card = issue(&store, issue_id).expect("issue card should include doctor details");
        let issue_doctor = issue_card
            .doctor
            .expect("issue doctor should be present for linked loop");
        assert!(issue_doctor
            .audit_failure_details
            .iter()
            .any(|detail| detail == "issue_surface:operator_evidence:evidence.action_binding"));

        let _ = fs::remove_dir_all(root);
    }
}
