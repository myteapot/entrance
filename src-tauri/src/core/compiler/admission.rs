use super::{
    evidence::{EvidenceVerdict, GateEvidenceRef, StoredGateEvidence},
    lowering::{
        DispatchLineage, DispatchRouting, LoweredDispatch, SandboxConfig, DISPATCH_SCOPE_PRIMITIVES,
    },
    packet::TypedActionPacket,
};
use crate::core::{action::ActorRole, parallel_budget::BudgetCheckResult};

/// Marker type: a LoweredDispatch that has passed admission.
/// Downstream routing (M6.2) and gate enforcement (M7.2)
/// accept only this type, not raw LoweredDispatch.
#[derive(Debug, Clone)]
pub struct AdmittedDispatch {
    inner: LoweredDispatch,
    admitted_at: String,
    gate_evidence: Option<GateEvidenceRef>,
}

impl AdmittedDispatch {
    pub fn inner(&self) -> &LoweredDispatch {
        &self.inner
    }

    pub fn admitted_at(&self) -> &str {
        &self.admitted_at
    }

    pub fn gate_evidence(&self) -> Option<&GateEvidenceRef> {
        self.gate_evidence.as_ref()
    }

    pub fn packet(&self) -> &TypedActionPacket {
        &self.inner.packet
    }

    pub fn lineage(&self) -> &DispatchLineage {
        &self.inner.lineage
    }

    pub fn sandbox(&self) -> &SandboxConfig {
        &self.inner.sandbox
    }

    pub fn routing(&self) -> &DispatchRouting {
        &self.inner.routing
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdmissionContext {
    pub budget_remaining: Option<u32>,
    pub dedup_key: Option<String>,
    pub available_instances: Option<Vec<String>>,
    pub parallel_budget_check: Option<BudgetCheckResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionRejectionReason {
    CapacityExhausted,
    HumanApprovalRequired,
    GateEvidenceNotAccepted,
    WriterNotAuthorized,
    NotDispatchScope,
    TargetInstanceNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRejection {
    pub reason: AdmissionRejectionReason,
    pub summary: String,
}

/// The transport admission gate.
pub fn admit_dispatch(
    lowered: LoweredDispatch,
    gate_evidence: Option<&StoredGateEvidence>,
) -> Result<AdmittedDispatch, AdmissionRejection> {
    admit_dispatch_with_context(lowered, gate_evidence, None)
}

pub fn admit_dispatch_with_context(
    lowered: LoweredDispatch,
    gate_evidence: Option<&StoredGateEvidence>,
    context: Option<&AdmissionContext>,
) -> Result<AdmittedDispatch, AdmissionRejection> {
    let packet = &lowered.packet;
    let record = packet.record();
    let semantics = packet.semantics();

    if !DISPATCH_SCOPE_PRIMITIVES.contains(&record.verb) {
        return Err(AdmissionRejection {
            reason: AdmissionRejectionReason::NotDispatchScope,
            summary: format!(
                "primitive {:?} is outside the dispatch admission scope",
                record.verb
            ),
        });
    }

    if semantics.requires_human_approval {
        return Err(AdmissionRejection {
            reason: AdmissionRejectionReason::HumanApprovalRequired,
            summary: format!(
                "primitive {:?} requires explicit human approval before dispatch admission",
                record.verb
            ),
        });
    }

    if semantics.writes_truth && record.actor_role != ActorRole::Nota {
        return Err(AdmissionRejection {
            reason: AdmissionRejectionReason::WriterNotAuthorized,
            summary: format!(
                "actor role {:?} is not authorized to admit truth-writing dispatches",
                record.actor_role
            ),
        });
    }

    if let Some(target_instance_id) = lowered.lineage.target_instance_id.as_ref() {
        if let Some(available_instances) = context.and_then(|ctx| ctx.available_instances.as_ref())
        {
            if !available_instances
                .iter()
                .any(|instance| instance == target_instance_id)
            {
                return Err(AdmissionRejection {
                    reason: AdmissionRejectionReason::TargetInstanceNotFound,
                    summary: format!(
                        "target agent instance `{target_instance_id}` was not found in admission context"
                    ),
                });
            }
        }
    }

    if let Some(parallel_budget_check) = context.and_then(|ctx| ctx.parallel_budget_check.as_ref())
    {
        match parallel_budget_check {
            BudgetCheckResult::Allowed | BudgetCheckResult::Queued { .. } => {}
            BudgetCheckResult::Rejected { running, limit } => {
                return Err(AdmissionRejection {
                    reason: AdmissionRejectionReason::CapacityExhausted,
                    summary: format!(
                        "parallel capacity exhausted: {running}/{limit} agent tasks are already running"
                    ),
                });
            }
        }
    }

    let gate_evidence = if semantics.requires_admission_gate {
        match gate_evidence {
            Some(gate_evidence) => {
                match EvidenceVerdict::from_str(gate_evidence.verdict.as_str()) {
                    Some(EvidenceVerdict::Accepted) => Some(GateEvidenceRef::from(gate_evidence)),
                    Some(_) | None => {
                        return Err(AdmissionRejection {
                            reason: AdmissionRejectionReason::GateEvidenceNotAccepted,
                            summary: format!(
                                "gate evidence {} has verdict `{}`; only `accepted` satisfies the admission gate",
                                gate_evidence.id, gate_evidence.verdict
                            ),
                        });
                    }
                }
            }
            None => {
                return Err(AdmissionRejection {
                    reason: AdmissionRejectionReason::GateEvidenceNotAccepted,
                    summary: format!(
                        "primitive {:?} requires gate evidence but none was provided",
                        record.verb
                    ),
                });
            }
        }
    } else {
        None
    };

    Ok(AdmittedDispatch {
        admitted_at: packet.created_at().to_string(),
        gate_evidence,
        inner: lowered,
    })
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use serde_json::{json, Value};

    use super::{
        admit_dispatch, admit_dispatch_with_context, AdmissionContext, AdmissionRejection,
        AdmissionRejectionReason, AdmittedDispatch,
    };
    use crate::core::{
        action::{ActionPrimitive, ActionRecord, KnowledgeLayer},
        compiler::{
            evidence::{EvidenceKind, EvidenceVerdict, GateEvidenceRef, StoredGateEvidence},
            lowering::{
                lower_dispatch, lower_dispatch_to_instance, DispatchLineage, DispatchRouting,
                LoweredDispatch, LoweringContext, SandboxConfig, DISPATCH_SCOPE_PRIMITIVES,
            },
            packet::TypedActionPacket,
            registry::lookup_primitive,
            semantics::{RoutingConstraint, SandboxRequirement},
        },
        parallel_budget::BudgetCheckResult,
    };

    fn compile_packet(primitive: ActionPrimitive) -> TypedActionPacket {
        let entry = lookup_primitive(primitive)
            .expect("all test primitives must be present in the compiler registry");
        let record = ActionRecord::new(
            entry.allowed_roles[0],
            primitive,
            entry.allowed_rooms[0],
            KnowledgeLayer::Cold,
        )
        .expect("test packet should satisfy role and room constraints");

        TypedActionPacket::compile(record)
    }

    fn lowering_context(dispatch_lane: &str, allocator_surface: &str) -> LoweringContext {
        LoweringContext {
            transaction_id: 11,
            task_id: 7,
            dispatch_lane: dispatch_lane.to_string(),
            allocator_surface: allocator_surface.to_string(),
            project_root: "A:/Publish/entrance".to_string(),
            consumed_restart_attempts: 0,
            max_restart_attempts: 3,
            has_active_allocation: false,
        }
    }

    fn lowering_context_for_primitive(primitive: ActionPrimitive) -> LoweringContext {
        match primitive {
            ActionPrimitive::Make => lowering_context("nota_do", "do"),
            ActionPrimitive::Prepare | ActionPrimitive::Dispatch | ActionPrimitive::Repair => {
                lowering_context("nota_dev", "dev")
            }
            _ => panic!("non-dispatch primitive should not request a lowering context"),
        }
    }

    fn lowered_dispatch(primitive: ActionPrimitive) -> LoweredDispatch {
        let packet = compile_packet(primitive);
        let context = lowering_context_for_primitive(primitive);

        lower_dispatch(&packet, &context).expect("dispatch scope primitive should lower")
    }

    fn synthetic_lowered(packet: TypedActionPacket) -> LoweredDispatch {
        LoweredDispatch {
            packet,
            lineage: DispatchLineage {
                lineage_ref: "nota/dev/transaction/11/forge-task/7".to_string(),
                child_execution_kind: "forge_task".to_string(),
                child_execution_ref: "7".to_string(),
                return_target_kind: "nota_runtime_transaction".to_string(),
                return_target_ref: "11".to_string(),
                escalation_target_kind: "nota_runtime_transaction".to_string(),
                escalation_target_ref: "11".to_string(),
                target_instance_id: None,
            },
            sandbox: SandboxConfig {
                requirement: SandboxRequirement::None,
                working_dir: None,
            },
            routing: DispatchRouting {
                constraint: RoutingConstraint::Local,
                allocator_role: "nota".to_string(),
                allocation_kind: "forge_dev_dispatch".to_string(),
            },
        }
    }

    fn mutate_lowered_packet(
        lowered: &LoweredDispatch,
        mutate: impl FnOnce(&mut Value),
    ) -> LoweredDispatch {
        let mut value = serde_json::to_value(lowered).expect("lowered dispatch should serialize");
        mutate(&mut value);
        serde_json::from_value(value).expect("mutated lowered dispatch should deserialize")
    }

    fn admitted(primitive: ActionPrimitive) -> AdmittedDispatch {
        admit_dispatch(lowered_dispatch(primitive), None)
            .expect("dispatch scope primitive should admit")
    }

    fn stored_gate_evidence(verdict: &str) -> StoredGateEvidence {
        StoredGateEvidence {
            id: 41,
            allocation_id: 7,
            evidence_kind: EvidenceKind::IntegrationProbe.as_str().to_string(),
            verdict: verdict.to_string(),
            summary: "synthetic integration probe".to_string(),
            payload_json: json!({ "attempt_receipt": true, "artifact_manifest": true }).to_string(),
            created_at: "2026-04-03T08:00:00Z".to_string(),
            updated_at: "2026-04-03T08:00:00Z".to_string(),
        }
    }

    #[test]
    fn admit_dispatch_primitive_succeeds() {
        let _guard = crate::test_env_guard();
        let admitted = admitted(ActionPrimitive::Dispatch);

        assert_eq!(admitted.packet().record().verb, ActionPrimitive::Dispatch);
        assert_eq!(admitted.lineage().child_execution_kind, "forge_task");
    }

    #[test]
    fn admit_make_primitive_succeeds() {
        let _guard = crate::test_env_guard();
        let admitted = admitted(ActionPrimitive::Make);

        assert_eq!(admitted.packet().record().verb, ActionPrimitive::Make);
        assert_eq!(admitted.routing().allocation_kind, "forge_agent_dispatch");
    }

    #[test]
    fn admit_all_dispatch_scope_primitives() {
        let _guard = crate::test_env_guard();
        for primitive in DISPATCH_SCOPE_PRIMITIVES {
            let admitted = admitted(primitive);

            assert_eq!(admitted.packet().record().verb, primitive);
            assert_eq!(admitted.inner().packet.created_at(), admitted.admitted_at());
        }
    }

    #[test]
    fn admitted_dispatch_has_rfc3339_timestamp() {
        let _guard = crate::test_env_guard();
        let admitted = admitted(ActionPrimitive::Dispatch);

        assert!(DateTime::parse_from_rfc3339(admitted.admitted_at()).is_ok());
    }

    #[test]
    fn admission_rejects_non_dispatch_scope() {
        let _guard = crate::test_env_guard();
        let lowered = synthetic_lowered(compile_packet(ActionPrimitive::Chat));

        assert_eq!(
            admit_dispatch(lowered, None).unwrap_err(),
            AdmissionRejection {
                reason: AdmissionRejectionReason::NotDispatchScope,
                summary: "primitive Chat is outside the dispatch admission scope".to_string(),
            }
        );
    }

    #[test]
    fn admission_rejects_human_approval_required() {
        let _guard = crate::test_env_guard();
        let lowered =
            mutate_lowered_packet(&lowered_dispatch(ActionPrimitive::Dispatch), |value| {
                value["packet"]["semantics"]["requires_human_approval"] = json!(true);
            });

        assert_eq!(
            admit_dispatch(lowered, None).unwrap_err(),
            AdmissionRejection {
                reason: AdmissionRejectionReason::HumanApprovalRequired,
                summary:
                    "primitive Dispatch requires explicit human approval before dispatch admission"
                        .to_string(),
            }
        );
    }

    #[test]
    fn admission_rejects_unauthorized_writer() {
        let _guard = crate::test_env_guard();
        let lowered =
            mutate_lowered_packet(&lowered_dispatch(ActionPrimitive::Dispatch), |value| {
                value["packet"]["semantics"]["writes_truth"] = json!(true);
            });

        assert_eq!(
            admit_dispatch(lowered, None).unwrap_err(),
            AdmissionRejection {
                reason: AdmissionRejectionReason::WriterNotAuthorized,
                summary: "actor role Dev is not authorized to admit truth-writing dispatches"
                    .to_string(),
            }
        );
    }

    #[test]
    fn admitted_dispatch_inner_matches_input() {
        let _guard = crate::test_env_guard();
        let lowered = lowered_dispatch(ActionPrimitive::Dispatch);
        let admitted = admit_dispatch(lowered.clone(), None).expect("dispatch should admit");

        assert_eq!(admitted.inner(), &lowered);
        assert_eq!(admitted.packet().created_at(), lowered.packet.created_at());
        assert_eq!(admitted.admitted_at(), lowered.packet.created_at());
        assert!(admitted.gate_evidence().is_none());
    }

    #[test]
    fn gate_required_accepts_accepted_evidence_and_carries_reference() {
        let _guard = crate::test_env_guard();
        let lowered =
            mutate_lowered_packet(&lowered_dispatch(ActionPrimitive::Dispatch), |value| {
                value["packet"]["semantics"]["requires_admission_gate"] = json!(true);
            });
        let evidence = stored_gate_evidence(EvidenceVerdict::Accepted.as_str());

        let admitted = admit_dispatch(lowered, Some(&evidence))
            .expect("accepted evidence should satisfy the admission gate");

        assert_eq!(
            admitted.gate_evidence(),
            Some(&GateEvidenceRef {
                evidence_id: evidence.id,
                evidence_kind: evidence.evidence_kind.clone(),
            })
        );
    }

    #[test]
    fn gate_required_rejects_non_accepted_evidence_verdicts() {
        let _guard = crate::test_env_guard();
        let lowered =
            mutate_lowered_packet(&lowered_dispatch(ActionPrimitive::Dispatch), |value| {
                value["packet"]["semantics"]["requires_admission_gate"] = json!(true);
            });

        for verdict in [
            EvidenceVerdict::Pending,
            EvidenceVerdict::Rejected,
            EvidenceVerdict::Expired,
        ] {
            let evidence = stored_gate_evidence(verdict.as_str());

            assert_eq!(
                admit_dispatch(lowered.clone(), Some(&evidence)).unwrap_err(),
                AdmissionRejection {
                    reason: AdmissionRejectionReason::GateEvidenceNotAccepted,
                    summary: format!(
                        "gate evidence {} has verdict `{}`; only `accepted` satisfies the admission gate",
                        evidence.id, evidence.verdict
                    ),
                }
            );
        }
    }

    #[test]
    fn gate_required_without_evidence_is_rejected() {
        let _guard = crate::test_env_guard();
        let lowered =
            mutate_lowered_packet(&lowered_dispatch(ActionPrimitive::Dispatch), |value| {
                value["packet"]["semantics"]["requires_admission_gate"] = json!(true);
            });

        let rejection =
            admit_dispatch(lowered, None).expect_err("missing evidence should be rejected");

        assert_eq!(
            rejection.reason,
            AdmissionRejectionReason::GateEvidenceNotAccepted
        );
        assert!(rejection
            .summary
            .contains("requires gate evidence but none was provided"));
    }

    #[test]
    fn cross_agent_dispatch_with_valid_instance() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let lowered = lower_dispatch_to_instance(
            &packet,
            &lowering_context_for_primitive(ActionPrimitive::Dispatch),
            Some("slot-1".to_string()),
        )
        .expect("dispatch should lower for an explicit target instance");
        let context = AdmissionContext {
            budget_remaining: None,
            dedup_key: None,
            available_instances: Some(vec!["slot-1".to_string(), "slot-2".to_string()]),
            parallel_budget_check: None,
        };

        let admitted = admit_dispatch_with_context(lowered, None, Some(&context))
            .expect("listed target instance should admit successfully");

        assert_eq!(
            admitted.lineage().target_instance_id.as_deref(),
            Some("slot-1")
        );
    }

    #[test]
    fn cross_agent_dispatch_with_invalid_instance() {
        let _guard = crate::test_env_guard();
        let packet = compile_packet(ActionPrimitive::Dispatch);
        let lowered = lower_dispatch_to_instance(
            &packet,
            &lowering_context_for_primitive(ActionPrimitive::Dispatch),
            Some("slot-3".to_string()),
        )
        .expect("dispatch should lower even before admission validation");
        let context = AdmissionContext {
            budget_remaining: None,
            dedup_key: None,
            available_instances: Some(vec!["slot-1".to_string(), "slot-2".to_string()]),
            parallel_budget_check: None,
        };

        assert_eq!(
            admit_dispatch_with_context(lowered, None, Some(&context)).unwrap_err(),
            AdmissionRejection {
                reason: AdmissionRejectionReason::TargetInstanceNotFound,
                summary: "target agent instance `slot-3` was not found in admission context"
                    .to_string(),
            }
        );
    }

    #[test]
    fn backward_compat_no_instance_id() {
        let _guard = crate::test_env_guard();
        let lowered = lowered_dispatch(ActionPrimitive::Dispatch);
        let admitted = admit_dispatch_with_context(lowered, None, None)
            .expect("dispatch without an explicit target instance should still admit");

        assert_eq!(admitted.lineage().target_instance_id, None);
    }

    #[test]
    fn admission_rejects_when_capacity_exhausted() {
        let _guard = crate::test_env_guard();
        let lowered = lowered_dispatch(ActionPrimitive::Dispatch);
        let context = AdmissionContext {
            budget_remaining: None,
            dedup_key: None,
            available_instances: None,
            parallel_budget_check: Some(BudgetCheckResult::Rejected {
                running: 3,
                limit: 3,
            }),
        };

        let rejection = admit_dispatch_with_context(lowered, None, Some(&context))
            .expect_err("capacity exhaustion should reject dispatch admission");

        assert_eq!(
            rejection.reason,
            AdmissionRejectionReason::CapacityExhausted
        );
        assert_eq!(
            rejection.summary,
            "parallel capacity exhausted: 3/3 agent tasks are already running"
        );
    }
}
