import { invoke } from "@tauri-apps/api/core";

export interface LandingImportReport {
  ingest_run_id: number;
  source_system: string;
  source_workspace: string;
  source_project: string;
  artifact_path: string;
  artifact_sha256: string;
  snapshot_artifact_id: number;
  imported_issue_count: number;
  imported_document_count: number;
  imported_milestone_count: number;
  imported_planning_item_count: number;
}

export const importLandingSnapshot = (path: string) =>
  invoke<LandingImportReport>("landing_import_snapshot", { path });
