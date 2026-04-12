import { invoke } from "@desktop/core";

export type ChatArchivePolicy = "off" | "summary" | "full";

export interface StoredCadenceObject {
  id: number;
  cadence_kind: string;
  title: string;
  summary: string;
  payload_json: string;
  scope_type: string;
  scope_ref: string;
  source_type: string;
  source_ref: string;
  admission_policy: string;
  projection_policy: string;
  status: string;
  is_current: boolean;
  created_at: string;
  updated_at: string;
}

export interface RepoContext {
  project_dir: string;
  git_branch: string | null;
  git_head: string | null;
}

export interface NotaCheckpointPayload {
  stable_level: string;
  landed: string[];
  remaining: string[];
  human_continuity_bus: string;
  selected_trunk: string | null;
  next_start_hints: string[];
  repo_context: RepoContext | null;
}

export interface NotaCheckpointRecord {
  cadence_object: StoredCadenceObject;
  payload: NotaCheckpointPayload;
}

export interface NotaCheckpointListReport {
  checkpoint_count: number;
  current_checkpoint_id: number | null;
  checkpoints: NotaCheckpointRecord[];
}

export interface StoredNotaRuntimeTransaction {
  id: number;
  actor_role: string;
  surface_action: string;
  transaction_kind: string;
  title: string;
  payload_json: string;
  status: string;
  forge_task_id: number | null;
  cadence_checkpoint_id: number | null;
  created_at: string;
  updated_at: string;
}

export interface NotaRuntimeTransactionsReport {
  transaction_count: number;
  transactions: StoredNotaRuntimeTransaction[];
}

export interface StoredDecisionRecord {
  id: number;
  title: string;
  statement: string;
  rationale: string;
  decision_type: string;
  decision_status: string;
  scope_type: string;
  scope_ref: string;
  source_ref: string;
  decided_by: string;
  enforcement_level: string;
  actor_scope: string;
  confidence: number;
  created_at: string;
  updated_at: string;
}

export interface StoredDecisionLink {
  id: number;
  src_decision_id: number;
  dst_decision_id: number;
  relation_type: string;
  status: string;
  created_at: string;
}

export interface DesignDecisionListReport {
  decision_count: number;
  link_count: number;
  decisions: StoredDecisionRecord[];
  links: StoredDecisionLink[];
}

export interface StoredChatArchiveSetting {
  id: number;
  scope_type: string;
  scope_ref: string;
  archive_policy: ChatArchivePolicy;
  updated_at: string;
}

export interface ChatArchivePolicyReport {
  setting: StoredChatArchiveSetting;
}

export interface StoredChatCaptureRecord {
  id: number;
  session_ref: string;
  role: string;
  capture_mode: string;
  archive_policy: ChatArchivePolicy;
  content: string;
  summary: string;
  scope_type: string;
  scope_ref: string;
  linked_decision_id: number | null;
  status: string;
  created_at: string;
}

export interface ChatCaptureListReport {
  capture_count: number;
  captures: StoredChatCaptureRecord[];
}

export interface ChatCaptureReport {
  policy: ChatArchivePolicy;
  stored: boolean;
  record?: StoredChatCaptureRecord | null;
}

export interface NotaRuntimeBoundaryStep {
  state: string;
  transaction_id: number;
  allocation_id: number;
  lineage_ref: string;
  child_dispatch_role: string;
  execution_host: string;
  target_kind: string;
  target_ref: string;
}

export interface NotaRuntimeReview extends NotaRuntimeBoundaryStep {
  verdict?: string;
  summary?: string;
}

export interface NotaRuntimeIntegrate extends NotaRuntimeBoundaryStep {
  outcome?: string;
  summary?: string;
}

export interface NotaRuntimeFinalize extends NotaRuntimeBoundaryStep {
  summary?: string;
}

export interface NotaRuntimeNextStep {
  step: string;
  transaction_id: number;
  allocation_id: number;
  lineage_ref: string;
  child_dispatch_role: string;
  execution_host: string;
  target_kind: string;
  target_ref: string;
}

export interface NotaCheckpointRequest {
  title?: string;
  stable_level: string;
  landed: string[];
  remaining: string[];
  human_continuity_bus: string;
  selected_trunk?: string | null;
  next_start_hints: string[];
}

export interface NotaFrontDoorProgressTrack {
  id: string;
  label: string;
  value: number;
  tone: string;
  summary: string;
}

export interface NotaFrontDoorProjection {
  posture: string;
  summary: string;
  next_action_label: string;
  next_action_detail: string;
  dashboard_hook: string;
  progress_tracks: NotaFrontDoorProgressTrack[];
}

export interface NotaRuntimeStatus {
  chat_policy: ChatArchivePolicyReport;
  checkpoint_count: number;
  current_checkpoint_id: number | null;
  current_checkpoint?: NotaCheckpointRecord | null;
  transaction_count: number;
  latest_transaction?: StoredNotaRuntimeTransaction | null;
  allocation_count: number;
  receipt_count: number;
  decision_count: number;
  latest_decision?: StoredDecisionRecord | null;
  chat_capture_count: number;
  vision_count: number;
  todo_count: number;
  recommended_checkpoint?: NotaCheckpointRequest | null;
  review?: NotaRuntimeReview | null;
  integrate?: NotaRuntimeIntegrate | null;
  finalize?: NotaRuntimeFinalize | null;
  next_step?: NotaRuntimeNextStep | null;
  front_door: NotaFrontDoorProjection;
}

export interface NotaRuntimeOverview {
  chat_policy: ChatArchivePolicyReport;
  checkpoints: NotaCheckpointListReport;
  transactions: NotaRuntimeTransactionsReport;
  decisions: DesignDecisionListReport;
  chat_captures: ChatCaptureListReport;
  recommended_checkpoint?: NotaCheckpointRequest | null;
  review?: NotaRuntimeReview | null;
  integrate?: NotaRuntimeIntegrate | null;
  finalize?: NotaRuntimeFinalize | null;
  next_step?: NotaRuntimeNextStep | null;
  front_door: NotaFrontDoorProjection;
}

export const fetchNotaRuntimeOverview = () =>
  invoke<NotaRuntimeOverview>("nota_runtime_overview");

export const fetchNotaRuntimeStatus = () =>
  invoke<NotaRuntimeStatus>("nota_runtime_status");

export const fetchChatArchivePolicy = () =>
  invoke<ChatArchivePolicyReport>("nota_get_chat_archive_policy");

export const updateChatArchivePolicy = (archivePolicy: ChatArchivePolicy) =>
  invoke<ChatArchivePolicyReport>("nota_set_chat_archive_policy", {
    archivePolicy,
  });

export const captureChatMessage = (
  role: string,
  content: string,
  summary?: string,
  sessionRef?: string,
) =>
  invoke<ChatCaptureReport>("nota_capture_chat_message", {
    role,
    content,
    summary,
    sessionRef,
  });
