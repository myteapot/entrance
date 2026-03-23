use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::core::data_store::{
    DataStore, NewDecisionLink, NewDecisionRecord, StoredDecisionLink, StoredDecisionRecord,
};

#[derive(Debug, Clone)]
pub struct DesignDecisionRequest {
    pub title: String,
    pub statement: String,
    pub rationale: String,
    pub decision_type: String,
    pub decision_status: String,
    pub scope_type: String,
    pub scope_ref: String,
    pub source_ref: String,
    pub decided_by: String,
    pub enforcement_level: String,
    pub actor_scope: String,
    pub confidence: f64,
    pub supersedes: Vec<i64>,
    pub conflicts_with: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesignDecisionWriteReport {
    pub decision: StoredDecisionRecord,
    pub links: Vec<StoredDecisionLink>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesignDecisionListReport {
    pub decision_count: usize,
    pub link_count: usize,
    pub decisions: Vec<StoredDecisionRecord>,
    pub links: Vec<StoredDecisionLink>,
}

pub fn record_design_decision(
    data_store: &DataStore,
    request: DesignDecisionRequest,
) -> Result<DesignDecisionWriteReport> {
    let title = request.title.trim().to_string();
    let statement = request.statement.trim().to_string();
    if title.is_empty() {
        return Err(anyhow!("`title` must not be empty"));
    }
    if statement.is_empty() {
        return Err(anyhow!("`statement` must not be empty"));
    }

    let decision = data_store.insert_decision_record(NewDecisionRecord {
        title: &title,
        statement: &statement,
        rationale: request.rationale.trim(),
        decision_type: request.decision_type.trim(),
        decision_status: normalize_or_default(&request.decision_status, "accepted"),
        scope_type: request.scope_type.trim(),
        scope_ref: request.scope_ref.trim(),
        source_ref: request.source_ref.trim(),
        decided_by: normalize_or_default(&request.decided_by, "NOTA"),
        enforcement_level: normalize_or_default(&request.enforcement_level, "runtime_canonical"),
        actor_scope: normalize_or_default(&request.actor_scope, "system"),
        confidence: request.confidence,
    })?;

    let mut links = Vec::new();
    for target_id in request.supersedes {
        ensure_decision_exists(data_store, target_id)?;
        links.push(data_store.insert_decision_link(NewDecisionLink {
            src_decision_id: decision.id,
            dst_decision_id: target_id,
            relation_type: "supersedes",
            status: "active",
        })?);
    }
    for target_id in request.conflicts_with {
        ensure_decision_exists(data_store, target_id)?;
        links.push(data_store.insert_decision_link(NewDecisionLink {
            src_decision_id: decision.id,
            dst_decision_id: target_id,
            relation_type: "conflicts_with",
            status: "active",
        })?);
    }

    Ok(DesignDecisionWriteReport { decision, links })
}

pub fn list_design_decisions(data_store: &DataStore) -> Result<DesignDecisionListReport> {
    let decisions = data_store.list_decision_records()?;
    let links = data_store.list_decision_links()?;
    Ok(DesignDecisionListReport {
        decision_count: decisions.len(),
        link_count: links.len(),
        decisions,
        links,
    })
}

fn ensure_decision_exists(data_store: &DataStore, id: i64) -> Result<()> {
    if data_store.get_decision_record(id)?.is_none() {
        return Err(anyhow!("decision `{id}` does not exist"));
    }

    Ok(())
}

fn normalize_or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::core::data_store::MigrationPlan;

    use super::{list_design_decisions, record_design_decision, DesignDecisionRequest};

    #[test]
    fn design_decisions_persist_with_supersession_and_conflict_links() -> Result<()> {
        let store = crate::core::data_store::DataStore::in_memory(MigrationPlan::new(&[]))?;

        let first = record_design_decision(
            &store,
            DesignDecisionRequest {
                title: "Chat and Do only".to_string(),
                statement: "Human-facing surface should shrink to Chat / Do.".to_string(),
                rationale: "Reduce ingress sprawl.".to_string(),
                decision_type: "ui_surface".to_string(),
                decision_status: "accepted".to_string(),
                scope_type: "project".to_string(),
                scope_ref: "Entrance".to_string(),
                source_ref: "nota:test:first".to_string(),
                decided_by: "NOTA".to_string(),
                enforcement_level: "runtime_canonical".to_string(),
                actor_scope: "system".to_string(),
                confidence: 0.95,
                supersedes: Vec::new(),
                conflicts_with: Vec::new(),
            },
        )?;
        let second = record_design_decision(
            &store,
            DesignDecisionRequest {
                title: "Cadence cut is separate".to_string(),
                statement: "Cadence continuity must not live in memory_fragments.".to_string(),
                rationale: "Keep continuity reconstructable.".to_string(),
                decision_type: "storage".to_string(),
                decision_status: "accepted".to_string(),
                scope_type: "project".to_string(),
                scope_ref: "Entrance".to_string(),
                source_ref: "nota:test:second".to_string(),
                decided_by: "NOTA".to_string(),
                enforcement_level: "runtime_canonical".to_string(),
                actor_scope: "system".to_string(),
                confidence: 0.98,
                supersedes: vec![first.decision.id],
                conflicts_with: vec![first.decision.id],
            },
        )?;

        assert_eq!(second.links.len(), 2);

        let listed = list_design_decisions(&store)?;
        assert_eq!(listed.decision_count, 2);
        assert_eq!(listed.link_count, 2);
        assert_eq!(listed.links[0].relation_type, "supersedes");
        assert_eq!(listed.links[1].relation_type, "conflicts_with");

        Ok(())
    }
}
