use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::core::data_store::{
    DataStore, StoredMemoryFragment, StoredMemoryLink, UpsertMemoryFragmentRecord,
    UpsertMemoryLinkRecord,
};

const SPEC_HYGIENE_KIND: &str = "spec_hygiene";
const SPEC_HYGIENE_SOURCE_TYPE: &str = "runtime_hygiene";
const SPEC_HYGIENE_TARGET_TABLE: &str = "repo_file";
const SPEC_HYGIENE_DST_KIND: &str = "memory_fragments";
const SPEC_HYGIENE_WORKFLOW: &str = "spec_hygiene_v0";
const SPEC_HYGIENE_SCOPE_TYPE: &str = "spec_mount";

const FINDING_MILESTONES_ARCHIVED: i64 = -9201;
const FINDING_LEAD_PRD_HISTORICAL: i64 = -9202;
const FINDING_COMPILER_GOVERNANCE_STALE: i64 = -9203;
const FINDING_OS_AUTH_STALE: i64 = -9204;
const FINDING_HANDOUT_ARCHIVED: i64 = -9205;
const FINDING_ROADMAP_ACTIVE: i64 = -9210;
const FINDING_TOP_LEAD_ACTIVE: i64 = -9211;
const FINDING_TOP_COMPILER_ACTIVE: i64 = -9212;
const FINDING_TOP_OS_ACTIVE: i64 = -9213;

#[derive(Debug, Clone, Serialize)]
pub struct SpecHygieneFindingSummary {
    pub memory_fragment_id: i64,
    pub title: String,
    pub status: String,
    pub triage_status: String,
    pub target_ref: String,
    pub scope_ref: String,
    pub notes: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecHygieneRelationSummary {
    pub memory_link_id: i64,
    pub relation_type: String,
    pub source_memory_fragment_id: i64,
    pub source_target_ref: String,
    pub target_memory_fragment_id: i64,
    pub target_target_ref: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecHygieneReport {
    pub workflow: String,
    pub finding_count: usize,
    pub relation_count: usize,
    pub findings: Vec<SpecHygieneFindingSummary>,
    pub relations: Vec<SpecHygieneRelationSummary>,
}

struct SpecHygieneSeed {
    id: i64,
    title: &'static str,
    content: &'static str,
    source_ref: &'static str,
    source_hash: &'static str,
    scope_ref: &'static str,
    target_ref: &'static str,
    status: &'static str,
    triage_status: &'static str,
    tags: &'static str,
    notes: &'static str,
    confidence: f64,
}

struct SpecHygieneRelationSeed {
    id: i64,
    src_id: i64,
    dst_id: i64,
    relation_type: &'static str,
}

const SPEC_HYGIENE_SEEDS: [SpecHygieneSeed; 9] = [
    SpecHygieneSeed {
        id: FINDING_MILESTONES_ARCHIVED,
        title: "Archive lead-model milestones mount residue",
        content: "The mounted milestones doc has decayed into a mojibake-prone V1/V2 roadmap and should no longer steer current Entrance truth.",
        source_ref: "specs/top/2.2-lead-model-3.md",
        source_hash: "spec-hygiene-v0:lead-model-milestones",
        scope_ref: "specs/top/2.2-lead-model-3.md",
        target_ref: "specs/chore/2.2-lead-model-3/milestones.md",
        status: "archived",
        triage_status: "accepted",
        tags: "spec_hygiene,mount_decay,lead_model",
        notes: "Keep as historical residue only; the active roadmap anchor is specs/chore/entrance_v0_headless_system_roadmap.md.",
        confidence: 0.99,
    },
    SpecHygieneSeed {
        id: FINDING_LEAD_PRD_HISTORICAL,
        title: "Demote lead-model PRD to historical reference",
        content: "The mounted lead-model PRD is still useful as historical recovery material but should not be read as active program truth.",
        source_ref: "specs/top/2.2-lead-model-3.md",
        source_hash: "spec-hygiene-v0:lead-model-prd",
        scope_ref: "specs/top/2.2-lead-model-3.md",
        target_ref: "specs/cold/2.2-lead-model-3/prd.md",
        status: "historical",
        triage_status: "accepted",
        tags: "spec_hygiene,historical_reference,lead_model",
        notes: "Read only for lineage questions; do not let it override the current hot control-slot summary.",
        confidence: 0.95,
    },
    SpecHygieneSeed {
        id: FINDING_COMPILER_GOVERNANCE_STALE,
        title: "Mark compiler governance mount as stale residue",
        content: "The GitLab governance draft is process residue and no longer fits as active compiler/action IR detail.",
        source_ref: "specs/top/1.3-compiler-action-ir.md",
        source_hash: "spec-hygiene-v0:compiler-governance",
        scope_ref: "specs/top/1.3-compiler-action-ir.md",
        target_ref: "specs/chore/1.3-compiler-action-ir/gitlab_mr_based_governance.md",
        status: "stale",
        triage_status: "accepted",
        tags: "spec_hygiene,mount_decay,compiler",
        notes: "Keep recoverable in DB, but stop treating it as current compiler-layer truth.",
        confidence: 0.94,
    },
    SpecHygieneSeed {
        id: FINDING_OS_AUTH_STALE,
        title: "Mark OS connector-auth mount as stale residue",
        content: "The GitLab connector-auth design note is operational residue rather than current OS/core truth.",
        source_ref: "specs/top/1.1-os-core.md",
        source_hash: "spec-hygiene-v0:os-auth",
        scope_ref: "specs/top/1.1-os-core.md",
        target_ref: "specs/chore/1.1-os-core/gitlab_connector_auth.md",
        status: "stale",
        triage_status: "accepted",
        tags: "spec_hygiene,mount_decay,os_core",
        notes: "Retain for recovery context only; do not let it reheat the OS/core root.",
        confidence: 0.93,
    },
    SpecHygieneSeed {
        id: FINDING_HANDOUT_ARCHIVED,
        title: "Archive expired self-cycle handout as historical handoff",
        content: "The top self-cycle handout contains expired operational state and should be treated as historical handoff rather than active execution truth.",
        source_ref: "specs/chore/README.md",
        source_hash: "spec-hygiene-v0:top-self-cycle-handout",
        scope_ref: "specs/chore/README.md",
        target_ref: "specs/chore/top_self_cycle_handout.md",
        status: "archived",
        triage_status: "accepted",
        tags: "spec_hygiene,historical_handoff,chore",
        notes: "Keep reconstructable, but route current execution through the headless V0 roadmap instead.",
        confidence: 0.98,
    },
    SpecHygieneSeed {
        id: FINDING_ROADMAP_ACTIVE,
        title: "Keep headless system roadmap as active self-clean anchor",
        content: "The headless Entrance V0 roadmap remains the active execution anchor for current system work.",
        source_ref: "specs/chore/README.md",
        source_hash: "spec-hygiene-v0:roadmap-anchor",
        scope_ref: "specs/chore/README.md",
        target_ref: "specs/chore/entrance_v0_headless_system_roadmap.md",
        status: "active_reference",
        triage_status: "accepted",
        tags: "spec_hygiene,active_anchor,chore",
        notes: "Use this as the live roadmap surface while older handouts stay historical.",
        confidence: 0.99,
    },
    SpecHygieneSeed {
        id: FINDING_TOP_LEAD_ACTIVE,
        title: "Keep lead-model top doc as active mount owner",
        content: "The lead-model top doc remains the active mount owner; only its stale subordinate docs should be demoted.",
        source_ref: "specs/top/control.md",
        source_hash: "spec-hygiene-v0:top-lead-owner",
        scope_ref: "specs/top/control.md",
        target_ref: "specs/top/2.2-lead-model-3.md",
        status: "active_reference",
        triage_status: "accepted",
        tags: "spec_hygiene,active_anchor,lead_model",
        notes: "Preserve the mounted top summary while demoting stale subordinate residue.",
        confidence: 0.96,
    },
    SpecHygieneSeed {
        id: FINDING_TOP_COMPILER_ACTIVE,
        title: "Keep compiler top doc as active mount owner",
        content: "The compiler/action IR top doc remains the active mount owner even though one mounted chore doc has decayed.",
        source_ref: "specs/top/machine.md",
        source_hash: "spec-hygiene-v0:top-compiler-owner",
        scope_ref: "specs/top/machine.md",
        target_ref: "specs/top/1.3-compiler-action-ir.md",
        status: "active_reference",
        triage_status: "accepted",
        tags: "spec_hygiene,active_anchor,compiler",
        notes: "Demote only the stale governance residue, not the mounted top summary itself.",
        confidence: 0.96,
    },
    SpecHygieneSeed {
        id: FINDING_TOP_OS_ACTIVE,
        title: "Keep OS top doc as active mount owner",
        content: "The OS/core top doc remains an active mount owner even though one chore note has decayed into historical residue.",
        source_ref: "specs/top/machine.md",
        source_hash: "spec-hygiene-v0:top-os-owner",
        scope_ref: "specs/top/machine.md",
        target_ref: "specs/top/1.1-os-core.md",
        status: "active_reference",
        triage_status: "accepted",
        tags: "spec_hygiene,active_anchor,os_core",
        notes: "Retain the mounted top boundary summary while pruning stale sub-doc influence.",
        confidence: 0.96,
    },
];

const SPEC_HYGIENE_RELATION_SEEDS: [SpecHygieneRelationSeed; 8] = [
    SpecHygieneRelationSeed {
        id: -9301,
        src_id: FINDING_MILESTONES_ARCHIVED,
        dst_id: FINDING_TOP_LEAD_ACTIVE,
        relation_type: "mounted_under",
    },
    SpecHygieneRelationSeed {
        id: -9302,
        src_id: FINDING_LEAD_PRD_HISTORICAL,
        dst_id: FINDING_TOP_LEAD_ACTIVE,
        relation_type: "mounted_under",
    },
    SpecHygieneRelationSeed {
        id: -9303,
        src_id: FINDING_COMPILER_GOVERNANCE_STALE,
        dst_id: FINDING_TOP_COMPILER_ACTIVE,
        relation_type: "mounted_under",
    },
    SpecHygieneRelationSeed {
        id: -9304,
        src_id: FINDING_OS_AUTH_STALE,
        dst_id: FINDING_TOP_OS_ACTIVE,
        relation_type: "mounted_under",
    },
    SpecHygieneRelationSeed {
        id: -9305,
        src_id: FINDING_HANDOUT_ARCHIVED,
        dst_id: FINDING_ROADMAP_ACTIVE,
        relation_type: "superseded_by",
    },
    SpecHygieneRelationSeed {
        id: -9306,
        src_id: FINDING_MILESTONES_ARCHIVED,
        dst_id: FINDING_ROADMAP_ACTIVE,
        relation_type: "superseded_by",
    },
    SpecHygieneRelationSeed {
        id: -9307,
        src_id: FINDING_MILESTONES_ARCHIVED,
        dst_id: FINDING_ROADMAP_ACTIVE,
        relation_type: "conflicts_with",
    },
    SpecHygieneRelationSeed {
        id: -9308,
        src_id: FINDING_HANDOUT_ARCHIVED,
        dst_id: FINDING_ROADMAP_ACTIVE,
        relation_type: "historical_replaced_by",
    },
];

pub fn run_spec_hygiene_v0(data_store: &DataStore) -> Result<SpecHygieneReport> {
    let now = Utc::now().to_rfc3339();

    for seed in &SPEC_HYGIENE_SEEDS {
        data_store.upsert_memory_fragment_record(UpsertMemoryFragmentRecord {
            id: seed.id,
            title: seed.title,
            content: seed.content,
            kind: SPEC_HYGIENE_KIND,
            source_type: SPEC_HYGIENE_SOURCE_TYPE,
            source_ref: seed.source_ref,
            source_hash: seed.source_hash,
            scope_type: SPEC_HYGIENE_SCOPE_TYPE,
            scope_ref: seed.scope_ref,
            target_table: SPEC_HYGIENE_TARGET_TABLE,
            target_ref: seed.target_ref,
            status: seed.status,
            triage_status: seed.triage_status,
            temperature: "cold",
            tags: seed.tags,
            notes: seed.notes,
            confidence: seed.confidence,
            created_at: &now,
            updated_at: &now,
        })?;
    }

    for relation in &SPEC_HYGIENE_RELATION_SEEDS {
        data_store.upsert_memory_link_record(UpsertMemoryLinkRecord {
            id: relation.id,
            src_kind: SPEC_HYGIENE_DST_KIND,
            src_id: relation.src_id,
            dst_kind: SPEC_HYGIENE_DST_KIND,
            dst_id: relation.dst_id,
            relation_type: relation.relation_type,
            status: "active",
            created_at: &now,
        })?;
    }

    list_spec_hygiene_v0(data_store)
}

pub fn list_spec_hygiene_v0(data_store: &DataStore) -> Result<SpecHygieneReport> {
    let findings = data_store
        .list_memory_fragment_records()?
        .into_iter()
        .filter(is_spec_hygiene_fragment)
        .map(to_finding_summary)
        .collect::<Vec<_>>();
    let finding_targets = findings
        .iter()
        .map(|finding| (finding.memory_fragment_id, finding.target_ref.clone()))
        .collect::<HashMap<_, _>>();

    let relations = data_store
        .list_memory_link_records()?
        .into_iter()
        .filter(|link| is_spec_hygiene_link(link, &finding_targets))
        .map(|link| SpecHygieneRelationSummary {
            memory_link_id: link.id,
            relation_type: link.relation_type,
            source_memory_fragment_id: link.src_id,
            source_target_ref: finding_targets
                .get(&link.src_id)
                .cloned()
                .unwrap_or_default(),
            target_memory_fragment_id: link.dst_id,
            target_target_ref: finding_targets
                .get(&link.dst_id)
                .cloned()
                .unwrap_or_default(),
            status: link.status,
        })
        .collect::<Vec<_>>();

    Ok(SpecHygieneReport {
        workflow: SPEC_HYGIENE_WORKFLOW.to_string(),
        finding_count: findings.len(),
        relation_count: relations.len(),
        findings,
        relations,
    })
}

fn is_spec_hygiene_fragment(fragment: &StoredMemoryFragment) -> bool {
    fragment.kind == SPEC_HYGIENE_KIND
        && fragment.source_type == SPEC_HYGIENE_SOURCE_TYPE
        && fragment.target_table == SPEC_HYGIENE_TARGET_TABLE
}

fn is_spec_hygiene_link(link: &StoredMemoryLink, finding_targets: &HashMap<i64, String>) -> bool {
    link.src_kind == SPEC_HYGIENE_DST_KIND
        && link.dst_kind == SPEC_HYGIENE_DST_KIND
        && finding_targets.contains_key(&link.src_id)
        && finding_targets.contains_key(&link.dst_id)
}

fn to_finding_summary(fragment: StoredMemoryFragment) -> SpecHygieneFindingSummary {
    SpecHygieneFindingSummary {
        memory_fragment_id: fragment.id,
        title: fragment.title,
        status: fragment.status,
        triage_status: fragment.triage_status,
        target_ref: fragment.target_ref,
        scope_ref: fragment.scope_ref,
        notes: fragment.notes,
        updated_at: fragment.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::core::data_store::{DataStore, MigrationPlan};

    use super::{list_spec_hygiene_v0, run_spec_hygiene_v0};

    #[test]
    fn spec_hygiene_v0_persists_self_clean_findings_and_relations() -> Result<()> {
        let store = DataStore::in_memory(MigrationPlan::new(&[]))?;

        let report = run_spec_hygiene_v0(&store)?;
        assert_eq!(report.workflow, "spec_hygiene_v0");
        assert_eq!(report.finding_count, 9);
        assert_eq!(report.relation_count, 8);
        assert!(report.findings.iter().any(|finding| finding.target_ref
            == "specs/chore/top_self_cycle_handout.md"
            && finding.status == "archived"));
        assert!(report
            .relations
            .iter()
            .any(|relation| relation.relation_type == "superseded_by"
                && relation.source_target_ref == "specs/chore/top_self_cycle_handout.md"
                && relation.target_target_ref
                    == "specs/chore/entrance_v0_headless_system_roadmap.md"));

        let listed = list_spec_hygiene_v0(&store)?;
        assert_eq!(listed.finding_count, report.finding_count);
        assert_eq!(listed.relation_count, report.relation_count);

        Ok(())
    }
}
