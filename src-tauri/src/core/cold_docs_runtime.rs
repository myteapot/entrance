use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use walkdir::WalkDir;

use crate::core::data_store::{DataStore, StoredDocumentRecord, UpsertDocumentRecordBySlug};
use crate::core::projection_runtime::{
    build_projection_status_report, record_projection_failure, record_projection_success,
    ProjectionTargetSpec, ProjectionTruthRevision, COLD_DOC_PROJECTION_CLASS,
    OPTIONAL_PROJECTION_POLICY,
};

pub const COLD_DOC_CATEGORY: &str = "cold_doc";

#[derive(Debug, Clone, Serialize)]
pub struct NotaColdDocRecord {
    #[serde(flatten)]
    pub document: StoredDocumentRecord,
    pub projection_state: String,
    pub projection_fresh: bool,
    pub projection_dirty: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaColdDocListReport {
    pub cold_doc_count: usize,
    pub fresh_projection_count: usize,
    pub dirty_projection_count: usize,
    pub docs: Vec<NotaColdDocRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaColdDocCanonicalizationReport {
    pub source_root: String,
    pub imported_count: usize,
    pub docs: Vec<StoredDocumentRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotaColdDocExportReport {
    pub export_root: String,
    pub exported_count: usize,
    pub exported_paths: Vec<String>,
}

pub fn list_cold_documents(
    data_store: &DataStore,
    current_truth_revision: ProjectionTruthRevision,
) -> Result<NotaColdDocListReport> {
    let docs = data_store.list_document_records_by_category(COLD_DOC_CATEGORY)?;
    let projection_report = build_projection_status_report(data_store, current_truth_revision)?;

    let mut fresh_projection_count = 0usize;
    let mut dirty_projection_count = 0usize;
    let mut cold_docs = Vec::with_capacity(docs.len());

    for document in docs {
        let projection_status = projection_report.targets.iter().find(|target| {
            target.target.projection_class == COLD_DOC_PROJECTION_CLASS
                && target.target.target_key == cold_doc_target_key(&document.slug)
        });

        let projection_state = projection_status
            .map(|status| status.state.clone())
            .unwrap_or_else(|| "unprojected".to_string());
        let projection_fresh = projection_status.map(|status| status.fresh).unwrap_or(false);
        let projection_dirty = projection_status
            .map(|status| status.dirty)
            .unwrap_or(false);
        let target_path = projection_status.map(|status| status.target.target_path.clone());

        if projection_fresh {
            fresh_projection_count += 1;
        }
        if projection_dirty {
            dirty_projection_count += 1;
        }

        cold_docs.push(NotaColdDocRecord {
            document,
            projection_state,
            projection_fresh,
            projection_dirty,
            target_path,
        });
    }

    Ok(NotaColdDocListReport {
        cold_doc_count: cold_docs.len(),
        fresh_projection_count,
        dirty_projection_count,
        docs: cold_docs,
    })
}

pub fn canonicalize_cold_docs_from_repo(
    data_store: &DataStore,
    project_dir: &str,
) -> Result<NotaColdDocCanonicalizationReport> {
    let cold_root = Path::new(project_dir).join("specs").join("cold");
    if !cold_root.exists() {
        return Err(anyhow!(
            "cold-doc root `{}` does not exist",
            cold_root.display()
        ));
    }

    let mut docs = Vec::new();
    for entry in WalkDir::new(&cold_root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("md"))
            != Some(true)
        {
            continue;
        }

        let relative_path = entry
            .path()
            .strip_prefix(project_dir)
            .with_context(|| {
                format!(
                    "failed to derive repo-relative path for cold doc `{}`",
                    entry.path().display()
                )
            })?;
        let slug = normalize_relative_path(relative_path);
        let content = fs::read_to_string(entry.path()).with_context(|| {
            format!("failed to read cold doc source at {}", entry.path().display())
        })?;
        let title = derive_markdown_title(&content)
            .unwrap_or_else(|| derive_title_from_path(entry.path()));
        let stored = data_store.upsert_document_record_by_slug(UpsertDocumentRecordBySlug {
            slug: &slug,
            title: &title,
            content: &content,
            category: COLD_DOC_CATEGORY,
        })?;
        docs.push(stored);
    }

    docs.sort_by(|left, right| left.slug.cmp(&right.slug).then(left.id.cmp(&right.id)));

    Ok(NotaColdDocCanonicalizationReport {
        source_root: cold_root.to_string_lossy().replace('\\', "/"),
        imported_count: docs.len(),
        docs,
    })
}

pub fn export_cold_docs_to_repo(
    data_store: &DataStore,
    project_dir: &str,
    truth_revision: &ProjectionTruthRevision,
) -> Result<NotaColdDocExportReport> {
    let docs = data_store.list_document_records_by_category(COLD_DOC_CATEGORY)?;
    let mut exported_paths = Vec::with_capacity(docs.len());

    for document in docs {
        let target_path = Path::new(project_dir).join(&document.slug);
        if let Some(parent) = target_path.parent() {
            if let Err(error) = fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create cold-doc parent directory at {}",
                    parent.display()
                )
            }) {
                record_cold_doc_projection_failure(
                    data_store,
                    truth_revision,
                    &document,
                    &target_path,
                    &error.to_string(),
                )?;
                return Err(error);
            }
        }

        if let Err(error) = fs::write(&target_path, &document.content).with_context(|| {
            format!("failed to write cold-doc projection at {}", target_path.display())
        }) {
            record_cold_doc_projection_failure(
                data_store,
                truth_revision,
                &document,
                &target_path,
                &error.to_string(),
            )?;
            return Err(error);
        }

        record_projection_success(
            data_store,
            cold_doc_projection_spec(&document, &target_path),
            truth_revision,
            "cold_doc_export",
            "Cold doc projection is current with DB truth.",
        )?;
        exported_paths.push(target_path.to_string_lossy().replace('\\', "/"));
    }

    Ok(NotaColdDocExportReport {
        export_root: Path::new(project_dir)
            .join("specs")
            .join("cold")
            .to_string_lossy()
            .replace('\\', "/"),
        exported_count: exported_paths.len(),
        exported_paths,
    })
}

fn record_cold_doc_projection_failure(
    data_store: &DataStore,
    truth_revision: &ProjectionTruthRevision,
    document: &StoredDocumentRecord,
    target_path: &Path,
    error_message: &str,
) -> Result<()> {
    record_projection_failure(
        data_store,
        cold_doc_projection_spec(document, target_path),
        truth_revision,
        "cold_doc_export",
        "Cold doc projection failed.",
        error_message,
    )?;
    Ok(())
}

fn cold_doc_projection_spec<'a>(
    document: &'a StoredDocumentRecord,
    target_path: &'a Path,
) -> ProjectionTargetSpec<'a> {
    let target_path = target_path.to_string_lossy().replace('\\', "/");
    ProjectionTargetSpec {
        projection_class: COLD_DOC_PROJECTION_CLASS.into(),
        target_key: cold_doc_target_key(&document.slug).into(),
        title: format!("Cold doc: {}", document.slug).into(),
        target_path: target_path.into(),
        source_scope: "runtime:Entrance".into(),
        repair_action: "entrance nota export-cold-docs --project-dir <path>".into(),
        projection_policy: OPTIONAL_PROJECTION_POLICY.into(),
        is_required: false,
    }
}

fn cold_doc_target_key(slug: &str) -> String {
    format!("cold_doc:{slug}")
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn derive_markdown_title(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
}

fn derive_title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.replace('_', " "))
        .unwrap_or_else(|| "cold doc".to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::core::data_store::{DataStore, MigrationPlan};
    use crate::core::projection_runtime::ProjectionTruthRevision;

    use super::{
        canonicalize_cold_docs_from_repo, export_cold_docs_to_repo, list_cold_documents,
        COLD_DOC_CATEGORY,
    };

    struct TempRoot {
        root: PathBuf,
        db_path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let root = std::env::temp_dir().join(format!(
                "entrance-cold-docs-{label}-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            let db_path = root.join("appdata").join("data").join("entrance.db");
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent)?;
            }
            Ok(Self { root, db_path })
        }

        fn db_path(&self) -> &Path {
            &self.db_path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn cold_docs_can_be_canonicalized_and_reprojected() -> Result<()> {
        let temp_root = TempRoot::new("canonicalize")?;
        let migration_plan = MigrationPlan::new(crate::plugins::forge::migrations());
        let store = DataStore::open(temp_root.db_path(), migration_plan)?;

        let project_dir = temp_root.root.join("Entrance");
        let cold_doc_path = project_dir
            .join("specs")
            .join("cold")
            .join("1.1-os-core")
            .join("projection_boundary.md");
        if let Some(parent) = cold_doc_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &cold_doc_path,
            "# Projection Boundary\n\nDB truth first, files second.\n",
        )?;

        let import_report = canonicalize_cold_docs_from_repo(
            &store,
            project_dir.to_str().expect("project path should be valid UTF-8"),
        )?;
        assert_eq!(import_report.imported_count, 1);
        assert_eq!(import_report.docs[0].category, COLD_DOC_CATEGORY);
        assert_eq!(
            import_report.docs[0].slug,
            "specs/cold/1.1-os-core/projection_boundary.md"
        );

        fs::remove_file(&cold_doc_path)?;
        let export_report = export_cold_docs_to_repo(
            &store,
            project_dir.to_str().expect("project path should be valid UTF-8"),
            &ProjectionTruthRevision::default(),
        )?;
        assert_eq!(export_report.exported_count, 1);
        assert!(cold_doc_path.exists());

        let listed = list_cold_documents(&store, ProjectionTruthRevision::default())?;
        assert_eq!(listed.cold_doc_count, 1);
        assert_eq!(listed.fresh_projection_count, 1);
        assert_eq!(listed.docs[0].projection_state, "fresh");

        Ok(())
    }
}
