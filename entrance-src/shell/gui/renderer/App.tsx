import { Match, Show, Switch, createMemo, createResource, createSignal } from "solid-js";
import Nav from "./components/Nav";
import { bridge } from "./lib/bridge";

type View = "status" | "drawer" | "hive" | "panel" | "launcher";
type CommentSurface = "detail" | "board";
type ActiveCommentComposer = {
  issueId: number;
  surface: CommentSurface;
};

type AppStatus = {
  app_root: string;
  db_path: string;
  schema: StoreSchemaStatus;
  drawer_entries: number;
  hive_runs: number;
  hive_loops: number;
  launcher_entries: number;
  generated_at: string;
};

type StoreSchemaStatus = {
  schema_version: string;
  db_path: string;
  user_version: number;
  expected_user_version: number;
  healthy: boolean;
  tables: Array<{
    name: string;
    present: boolean;
    column_count: number;
    required_column_count: number;
    missing_columns: string[];
  }>;
  indexes: Array<{
    name: string;
    table: string;
    present: boolean;
    columns: string[];
  }>;
  missing_tables: string[];
  missing_columns: string[];
  missing_indexes: string[];
  generated_at: string;
};

type DrawerSummary = {
  mode: string;
  root: string;
  items: number;
};

type DrawerHistory = {
  commits: Array<{
    id: string;
    summary: string;
  }>;
};

type DrawerItem = {
  id: number;
  title: string;
  kind: string;
  storage_path: string | null;
  tags: string[];
  updated_at: string;
};

type HiveRun = {
  id: number;
  title: string;
  status: string;
  project_dir: string | null;
  summary: string | null;
  updated_at: string;
};

type HiveSummary = {
  total_runs: number;
  ready_runs: number;
  returned_runs: number;
};

type HiveLoop = {
  id: number;
  title: string;
  goal: string;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
};

type IssueCard = {
  issue: {
    id: number;
    loop_id: number | null;
    title: string;
    status: string;
    summary: string | null;
    updated_at: string;
  };
  comments: Array<{
    id: number;
    author: string;
    body: string;
    created_at: string;
    payload?: Record<string, unknown>;
  }>;
  actions: IssueAction[];
  trace: {
    current_round: number;
    rounds: Array<{
      round: number;
      status: string;
      decision: string | null;
      evidence_count: number;
      rejected_count: number;
      receipt_required_count: number;
      receipt_missing_count: number;
      worker_count: number;
      worker_ok_count: number;
      worker_timeout_count: number;
      worker_retry_exhausted_count: number;
    }>;
    packet_count: number;
    admission_count: number;
    evidence_count: number;
    verdict_count: number;
    round_packet_count: number;
    round_admission_count: number;
    round_evidence_count: number;
    round_verdict_count: number;
    receipt_required_count: number;
    receipt_missing_count: number;
    round_receipt_required_count: number;
    round_receipt_missing_count: number;
    role_worker_count: number;
    role_worker_ok_count: number;
    round_role_worker_count: number;
    round_role_worker_ok_count: number;
    round_worker_duration_ms: number;
    round_worker_timeout_count: number;
    round_worker_retry_exhausted_count: number;
    packet_schema: string | null;
    policy_schema: string | null;
    admission_schema: string | null;
    verdict_schema: string | null;
    last_admission_gate: string | null;
    last_gate_description: string | null;
    last_gate_expected_object_kind: string | null;
    last_admission_passed: boolean | null;
    last_decision: string | null;
    reason_code: string | null;
    score_vector: Array<{
      name: string;
      value: number | null;
    }>;
    human_options: string[];
    operator_event_count: number;
    round_operator_event_count: number;
    last_operator_event: OperatorEvent | null;
    operator_events: OperatorEvent[];
    worker_kind: string | null;
    worker_mode: string | null;
    worker_ok: boolean | null;
    audit_schema: string | null;
    audit_passed: boolean | null;
    audit_failed_count: number;
    audit_failed_checks: string[];
    audit_failure_details: string[];
    evidence: Array<{
      id: number;
      round: number;
      stage_role: string | null;
      kind: string;
      summary: string;
      schema_version: string | null;
      admission_result: string | null;
      blocked_phase: string | null;
      missing_receipts: string[];
      packet_envelope_errors: string[];
      operator_options: string[];
      operator_author: string | null;
      operator_action: string | null;
      worker_kind: string | null;
      worker_mode: string | null;
      worker_ok: boolean | null;
      worker_receipt_ok: boolean | null;
      worker_timed_out: boolean | null;
      worker_status: number | null;
      worker_duration_ms: number | null;
      worker_timeout_secs: number | null;
      worker_attempt_count: number | null;
      worker_max_attempts: number | null;
      worker_retry_exhausted: boolean | null;
      worker_command: string | null;
      worker_cwd: string | null;
      worker_action: string | null;
      worker_evidence_summary: string | null;
      worker_gate_count: number | null;
      worker_receipt_errors: string[];
      transcript_excerpt: string | null;
    }>;
    stages: Array<{
      role: string;
      status: string;
      summary: string | null;
      evidence_kind: string | null;
      evidence_summary: string | null;
      admission_result: string | null;
      worker_kind: string | null;
      worker_mode: string | null;
      worker_ok: boolean | null;
    }>;
  } | null;
  doctor: IssueDoctorSummary | null;
  connector?: IssueConnectorStatus | null;
};

type IssueAction = {
  schema_version: string;
  action: string;
  label: string;
  command: string;
  source: string;
  input: string;
  destructive: boolean;
  runtime: string | null;
  confirmation_required: boolean;
  confirmation_arg?: string | null;
  receipt_schema?: string | null;
  policy_schema_version?: string | null;
};

type IssueConnectorStatus = {
  schema_version: string;
  current: boolean;
  publish_required: boolean | null;
  reason: string;
  failed_checks: string[];
  provider?: string | null;
  review_surface?: string | null;
  path?: string | null;
  current_comment_count?: number | null;
  remote_comment_count?: number | null;
  current_sha256?: string | null;
  remote_sha256?: string | null;
  checks?: AdmissionCheck[] | null;
  remote_readback_checks?: AdmissionCheck[] | null;
  remote_diagnostics?: ConnectorRemoteDiagnostics | null;
  publish_command: string;
  readback_command?: string | null;
  admit_command?: string | null;
  error?: string | null;
};

type ConnectorRegistry = {
  schema_version: string;
  providers: ConnectorProvider[];
  admission: {
    gate: string;
    route_to: string;
    expected_object_kind: string;
    check: string;
    required_receipts: string[];
    required_checks: string[];
    check_registry?: ConnectorAdmissionCheckSpec[];
    dry_run_command: string;
  };
  provider_admissions: ConnectorProviderAdmission[];
};

type ConnectorProvider = {
  name: string;
  display_name: string;
  status: string;
  mode: string;
  review_surface_prefixes: string[];
  auth_required: boolean;
  auth_env: string[];
  configured: boolean;
  supports_status: boolean;
  supports_publish: boolean;
  supports_readback: boolean;
  supports_admission: boolean;
  storage: string;
  notes: string;
};

type ConnectorProviderAdmission = {
  schema_version: string;
  provider: string;
  status: string;
  gate: string;
  route_to: string | null;
  expected_object_kind: string;
  check: string;
  required_receipts: string[];
  required_checks: string[];
  check_registry?: ConnectorAdmissionCheckSpec[];
  blockers: string[];
  dry_run_command: string;
};
type ConnectorAdmissionCheckSpec = {
  name: string;
  severity: string;
  owner: string;
  required_evidence: string[];
  summary: string;
};

type ConnectorRemoteContract = {
  schema_version: string;
  provider: string;
  remote_object_kind: string;
  write: {
    receipt_schema_version: string;
  };
  readback: {
    schema_version: string;
  };
  retry?: {
    max_attempts?: number | null;
    base_backoff_ms?: number | null;
    backoff_strategy?: string | null;
  } | null;
};

type ConnectorRemoteTarget = {
  schema_version?: string;
  provider?: string | null;
  review_surface?: string | null;
  target_kind?: string | null;
  target?: string | null;
  valid?: boolean | null;
  blockers?: string[];
  owner?: string | null;
  repo?: string | null;
  issue_number?: number | null;
  issue_key?: string | null;
  fixture_key?: string | null;
  remote_id?: string | null;
  remote_url?: string | null;
  api_url?: string | null;
  write_mode?: string | null;
};

type ConnectorRemoteWriteOperation = {
  kind?: string | null;
  method?: string | null;
  url?: string | null;
  source?: string | null;
  blocked_by?: string[];
  graphql?: {
    operation?: string | null;
  } | null;
};

type ConnectorRemoteWritePlan = {
  schema_version?: string;
  provider?: string | null;
  remote_object_kind?: string | null;
  executable?: boolean | null;
  blocked_by?: string[];
  operations?: ConnectorRemoteWriteOperation[];
};

type ConnectorRemoteSignal = {
  stage?: string | null;
  tone?: string | null;
  label?: string | null;
  failed_check?: string | null;
  http_status?: number | null;
  attempt_count?: number | null;
  retry?: {
    reason?: string | null;
    retryable?: boolean | null;
    scheduled?: boolean | null;
    attempted?: boolean | null;
    exhausted?: boolean | null;
    rate_limited?: boolean | null;
    backoff_ms?: number | null;
    retry_after_secs?: number | null;
    rate_limit?: Record<string, unknown> | null;
  } | null;
};

type ConnectorRemoteAttempt = {
  attempt?: number | null;
  success?: boolean | null;
  failed_check?: string | null;
  http_status?: number | null;
  error?: string | null;
  retry?: ConnectorRemoteSignal["retry"];
};

type ConnectorRemoteOperationDiagnostics = {
  kind?: string | null;
  method?: string | null;
  graphql_operation?: string | null;
  success?: boolean | null;
  failed_check?: string | null;
  http_status?: number | null;
  attempt_count?: number | null;
  max_attempts?: number | null;
  attempts?: ConnectorRemoteAttempt[];
  retry?: ConnectorRemoteSignal["retry"];
};

type ConnectorRemoteExecutionDiagnostics = {
  schema_version?: string | null;
  stage?: string | null;
  success?: boolean | null;
  failed_checks?: string[];
  operation_count?: number | null;
  primary_operation?: ConnectorRemoteOperationDiagnostics | null;
  signal?: ConnectorRemoteSignal | null;
};

type ConnectorRemoteDiagnostics = {
  schema_version?: string | null;
  write?: ConnectorRemoteExecutionDiagnostics | null;
  readback?: ConnectorRemoteExecutionDiagnostics | null;
  signals?: ConnectorRemoteSignal[];
};

type ConnectorWriterAdapter = {
  schema_version: string;
  provider: string;
  driver: string;
  mode: string | null;
  remote_write: boolean;
  blockers: string[];
  remote_contract?: ConnectorRemoteContract | null;
};

type ConnectorQueueReport = {
  schema_version: string;
  provider_filter: string | null;
  provider_known: boolean;
  total: number;
  current_count: number;
  publish_required_count: number;
  providers: ConnectorQueueProvider[];
  issues: ConnectorQueueIssue[];
  commands?: {
    refresh?: string | null;
    provider?: string | null;
    publish_plan?: string | null;
    roundtrip_plan?: string | null;
  } | null;
};

type ConnectorQueueProvider = {
  name: string;
  display_name: string;
  status: string;
  mode: string;
  configured: boolean;
  supports_publish: boolean;
  supports_admission: boolean;
  admission_status: string | null;
  admission_blockers: string[];
  storage: string;
  adapter?: ConnectorWriterAdapter | null;
  issue_count: number;
  current_count: number;
  publish_required_count: number;
  queue_command: string;
};

type ConnectorQueueIssue = {
  id: number | null;
  loop_id: number | null;
  title: string | null;
  status: string | null;
  provider: string;
  provider_status: string | null;
  configured: boolean | null;
  supports_publish: boolean | null;
  supports_readback?: boolean | null;
  supports_admission?: boolean | null;
  mode?: string | null;
  storage?: string | null;
  can_publish?: boolean | null;
  publish_blockers?: string[];
  adapter?: ConnectorWriterAdapter | null;
  remote_target?: ConnectorRemoteTarget | null;
  remote_write_plan?: ConnectorRemoteWritePlan | null;
  admission_status: string | null;
  admission_blockers: string[];
  checks?: AdmissionCheck[] | null;
  remote_readback_checks?: AdmissionCheck[] | null;
  admission_checks?: AdmissionCheck[] | null;
  remote_diagnostics?: ConnectorRemoteDiagnostics | null;
  review_surface: string | null;
  publish_required: boolean;
  current: boolean | null;
  reason: string | null;
  path: string | null;
  current_sha256?: string | null;
  remote_sha256?: string | null;
  current_comment_count?: number | null;
  remote_comment_count?: number | null;
  failed_checks: string[];
  failed_check_count: number;
  commands: {
    publish: string | null;
    readback: string | null;
    admit: string | null;
  };
};

type ConnectorPublishPlan = {
  schema_version: string;
  plan_id: string;
  provider_filter: string | null;
  provider_known: boolean;
  issue_count: number;
  can_execute: boolean;
  reason: string;
  blockers: string[];
  issues: Array<{
    id: number | null;
    provider: string | null;
    provider_status?: string | null;
    can_publish?: boolean | null;
    publish_blockers?: string[];
    path: string | null;
    current_sha256: string | null;
  }>;
  commands: {
    plan: string;
    execute: string | null;
  };
};

type ConnectorPublishExecuteReport = {
  schema_version: string;
  executed: boolean;
  reason: string;
  plan_id?: string | null;
  current_plan_id?: string | null;
  issue_count?: number | null;
  issue_ids?: number[];
  failed_checks?: string[];
  after?: {
    publish_required_count?: number | null;
    current_count?: number | null;
  };
};

type ConnectorRoundtripPlan = ConnectorPublishPlan;

type ConnectorRoundtripExecuteReport = {
  schema_version: string;
  executed: boolean;
  reason: string;
  plan_id?: string | null;
  current_plan_id?: string | null;
  issue_count?: number | null;
  completed_count?: number | null;
  issue_ids?: number[];
  failed_checks?: string[];
  after?: {
    publish_required_count?: number | null;
    current_count?: number | null;
  };
};

type OperatorEvent = {
  id: number;
  round: number;
  kind: string;
  author: string | null;
  action: string | null;
  issue_status: string | null;
  loop_status: string | null;
  note: string | null;
  summary: string;
};

type IssueDoctorSummary = {
  schema_version: string;
  health: string;
  summary: string;
  next_actions: string[];
  runtime: string;
  current_round: number;
  counts: {
    round_receipt_required_count: number;
    round_receipt_missing_count: number;
    round_role_worker_count: number;
    round_role_worker_ok_count: number;
    round_worker_duration_ms: number;
    round_worker_timeout_count: number;
    round_worker_retry_exhausted_count: number;
    audit_failed_count: number;
  };
  failed_checks: string[];
  audit_failure_details: string[];
  missing_receipts: string[];
  worker_failures: string[];
};

type RuntimePreflightReport = {
  schema_version: string;
  loop_id: number;
  issue_id: number | null;
  issue_status: string | null;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
  preflight_state: string;
  summary: string;
  policy: {
    schema_version: string;
    gate: string;
    object_kind: string;
    route_from: string;
    route_to: string;
    required_receipts: string[];
    supported_runtimes: string[];
  };
  preview: {
    runtime: string;
    supported: boolean;
    probe_ok: boolean;
    blocker: string | null;
    runtime_probe: Record<string, unknown>;
    selected_policy: Record<string, unknown> | null;
  };
  current: RuntimePreflightObservation | null;
  failures: string[];
  next_actions: string[];
};

type RuntimePreflightObservation = {
  packet_id: number;
  admission_id: number | null;
  round: number;
  result: string | null;
  reason: string | null;
  gate: string | null;
  gate_passed: boolean | null;
  receipt_required: string[];
  receipt_missing: string[];
  runtime: string | null;
  supported: boolean | null;
  probe_ok: boolean | null;
  blocker: string | null;
  runtime_probe: Record<string, unknown> | null;
};

type WorkerLifecycleReport = {
  schema_version: string;
  loop_id: number;
  issue_id: number | null;
  issue_status: string | null;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
  lifecycle_state: string;
  summary: string;
  policy: {
    schema_version: string;
    expected_roles: string[];
    legacy_roles: string[];
    default_timeout_secs: number;
    max_timeout_secs: number;
    timeout_env: string;
    default_attempts: number;
    max_attempts: number;
    attempts_env: string;
    reviewer_invalid_round_budget: number;
    fallback_status: string;
    human_decision_statuses: string[];
  };
  current: WorkerLifecycleRound;
  rounds: WorkerLifecycleRound[];
  failures: string[];
  next_actions: string[];
};

type WorkerLifecycleRound = {
  round: number;
  status: string;
  decision: string | null;
  expected_roles: string[];
  observed_roles: string[];
  missing_roles: string[];
  worker_count: number;
  worker_ok_count: number;
  worker_timeout_count: number;
  worker_retry_exhausted_count: number;
  worker_duration_ms: number;
  reviewer_invalid_rounds_used: number;
  reviewer_invalid_budget_exhausted: boolean;
  failures: string[];
  workers: WorkerLifecycleWorker[];
};

type WorkerLifecycleWorker = {
  evidence_id: number;
  round: number;
  role: string;
  stage_role: string | null;
  evidence_kind: string;
  kind: string | null;
  mode: string | null;
  ok: boolean | null;
  receipt_ok: boolean | null;
  timed_out: boolean | null;
  status: number | null;
  duration_ms: number | null;
  timeout_secs: number | null;
  attempt_count: number | null;
  max_attempts: number | null;
  retry_exhausted: boolean | null;
  command: string | null;
  cwd: string | null;
  action: string | null;
  evidence_summary: string | null;
  gate_count: number | null;
  receipt_errors: string[];
  transcript_excerpt: string | null;
};

type LauncherResult = {
  id: number;
  name: string;
  command: string;
  source: string;
  launch_count: number;
  pinned: boolean;
  score: number;
  arguments: string | null;
  working_dir: string | null;
};

type IssueComment = IssueCard["comments"][number];
type LoopRunArgs = {
  runtime?: string;
  workerTimeoutSecs?: number;
  workerAttempts?: number;
};
type IssueMirrorSyncReport = {
  schema_version: string;
  path: string;
  receipt_path: string;
  bytes: number;
  sha256: string;
};
type IssueMirrorPublishReport = Partial<IssueMirrorSyncReport> & {
  schema_version: string;
  published: boolean;
  reason: string;
  failed_checks?: string[];
};
type IssueMirrorVerifyReport = {
  schema_version: string;
  passed: boolean;
  path: string;
  receipt_path: string;
  failures: string[];
  current: {
    sha256: string;
    bytes: number;
  };
};
type IssueMirrorAuditReport = {
  schema_version: string;
  passed: boolean;
  failed_count: number;
  failed_checks: string[];
  verify: IssueMirrorVerifyReport;
};
type IssueMirrorReadbackReport = {
  schema_version: string;
  passed: boolean;
  failed_count: number;
  failed_checks: string[];
  path: string;
  current: {
    digest: {
      sha256: string;
      bytes: number;
    };
    comments: {
      count: number;
    };
  };
  remote: {
    parsed: boolean;
    digest: {
      sha256: string;
      bytes: number;
    } | null;
    surface: {
      comments: {
        count: number;
      };
    } | null;
  };
  recorded?: {
    comment_id: number;
    evidence_id: number | null;
    publish?: {
      required: boolean;
      command: string;
    } | null;
  } | null;
};
type AdmissionCheck = {
  name: string;
  passed: boolean;
  summary?: string | null;
  severity?: string | null;
  owner?: string | null;
  required_evidence?: string[] | null;
  policy_summary?: string | null;
};
type IssueMirrorAdmissionReport = {
  schema_version: string;
  admitted: boolean;
  result: string;
  reason: string;
  failed_checks: string[];
  provider_checks?: AdmissionCheck[] | null;
  writer_adapter?: ConnectorWriterAdapter | null;
  remote_contract?: ConnectorRemoteContract | null;
  decision: {
    route_to: string;
  };
  receipt: {
    sha256: string | null;
    path: string | null;
  };
  recorded?: {
    comment_id: number;
    evidence_id: number | null;
    publish?: {
      required: boolean;
      command: string;
    } | null;
  } | null;
};
type IssueMirrorRoundtripReport = {
  schema_version: string;
  completed: boolean;
  result: string;
  stage_count: number;
  passed_stage_count: number;
  failed_stages: string[];
  recorded_evidence_ids: number[];
  remote?: {
    final_readback_passed?: boolean | null;
    object_kind?: string | null;
  };
};
type ConnectorFixtureDemoReport = {
  schema_version: string;
  provider: string;
  review_surface: string;
  completed: boolean;
  result: string;
  issue_id: number;
  loop?: {
    id?: number | null;
  };
  summary?: {
    stage_count?: number | null;
    passed_stage_count?: number | null;
    failed_stages?: string[];
    recorded_evidence_ids?: number[];
    remote_object_kind?: string | null;
    final_readback_passed?: boolean | null;
  };
};
type CommentPill = {
  label: string;
  evidenceId?: number;
};

const ISSUE_STATUSES = ["Todo", "Doing", "Blocked", "Needs Review", "Done", "Canceled"] as const;
const COMMENT_CARD_PREVIEW_LIMIT = 132;
const COMMENT_DETAIL_PREVIEW_LIMIT = 360;

const COMMENT_ACTION_LABELS: Record<string, string> = {
  retry: "retry",
  "request-review": "review",
  cancel: "cancel",
};

const compactText = (value: string, limit: number) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= limit) return normalized;
  return `${normalized.slice(0, Math.max(0, limit - 1)).trimEnd()}...`;
};

const commentPayloadString = (comment: IssueComment, field: string) => {
  const value = comment.payload?.[field];
  return typeof value === "string" && value.trim() ? value : null;
};

const commentPayloadNumber = (comment: IssueComment, field: string) => {
  const value = comment.payload?.[field];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
};

const commentSchemaLabel = (comment: IssueComment) => {
  const schema = commentPayloadString(comment, "schema_version");
  return schema ? schema.split(".").slice(-2).join(".") : null;
};

const issueStatusTestId = (statusName: string) => statusName.toLowerCase().replace(/\s+/g, "-");

const commentPills = (comment: IssueComment) => {
  const source = commentPayloadString(comment, "source") ?? comment.author;
  const action = commentPayloadString(comment, "action");
  const decision = commentPayloadString(comment, "decision");
  const phase =
    commentPayloadString(comment, "phase") ?? commentPayloadString(comment, "next_phase");
  const evidenceId = commentPayloadNumber(comment, "evidence_id");
  const schema = commentSchemaLabel(comment);
  return [
    source ? { label: source } : null,
    action ? { label: COMMENT_ACTION_LABELS[action] ?? action } : null,
    decision && decision !== action ? { label: decision } : null,
    phase ? { label: phase } : null,
    evidenceId ? { label: `E#${evidenceId}`, evidenceId } : null,
    schema ? { label: schema } : null,
  ].filter((value): value is CommentPill => Boolean(value?.label));
};

const commentPreview = (comment: IssueComment, limit: number) => {
  if (limit === COMMENT_DETAIL_PREVIEW_LIMIT && commentPayloadString(comment, "source") === "operator") {
    return comment.body;
  }
  return compactText(comment.body, limit);
};

const operatorActionLabel = (action: string | null) =>
  action ? COMMENT_ACTION_LABELS[action] ?? action : "comment";

const operatorEventLabel = (event: OperatorEvent | null) => {
  if (!event) return null;
  const author = event.author ?? "operator";
  return `${author} ${operatorActionLabel(event.action)}`;
};

const operatorEventStatusLabel = (event: OperatorEvent) => {
  const status = event.issue_status ?? event.loop_status;
  return status ? `-> ${status}` : `round ${event.round}`;
};

export default function App() {
  let issueDetailPanel: HTMLElement | undefined;
  const evidenceRows = new Map<number, HTMLElement>();

  const [view, setView] = createSignal<View>("status");
  const [launcherQuery, setLauncherQuery] = createSignal("");
  const [hiveTitle, setHiveTitle] = createSignal("");
  const [hiveProject, setHiveProject] = createSignal("");
  const [loopTitle, setLoopTitle] = createSignal("");
  const [loopGoal, setLoopGoal] = createSignal("");
  const [loopRuntime, setLoopRuntime] = createSignal("codex");
  const [loopWorkerTimeoutSecs, setLoopWorkerTimeoutSecs] = createSignal("");
  const [loopWorkerAttempts, setLoopWorkerAttempts] = createSignal("");
  const [selectedIssueId, setSelectedIssueId] = createSignal<number | null>(null);
  const [selectedEvidenceId, setSelectedEvidenceId] = createSignal<number | null>(null);
  const [activeCommentComposer, setActiveCommentComposer] =
    createSignal<ActiveCommentComposer | null>(null);
  const [commentBody, setCommentBody] = createSignal("");
  const [pendingLoopActions, setPendingLoopActions] = createSignal<Record<number, string>>({});
  const [pendingIssueActions, setPendingIssueActions] = createSignal<Record<number, string>>({});
  const [pendingDemoAction, setPendingDemoAction] = createSignal<string | null>(null);
  const [pendingFixtureAction, setPendingFixtureAction] = createSignal<string | null>(null);
  const [drawerTitle, setDrawerTitle] = createSignal("");
  const [drawerBody, setDrawerBody] = createSignal("");
  const [banner, setBanner] = createSignal<string>("");
  const [connectorPublishPlan, setConnectorPublishPlan] =
    createSignal<ConnectorPublishPlan | null>(null);
  const [connectorPublishAction, setConnectorPublishAction] = createSignal<string | null>(null);
  const [connectorRoundtripPlan, setConnectorRoundtripPlan] =
    createSignal<ConnectorRoundtripPlan | null>(null);
  const [connectorRoundtripAction, setConnectorRoundtripAction] = createSignal<string | null>(null);

  const [status, { refetch: refetchStatus }] = createResource(async () =>
    bridge.invoke<AppStatus>("status"),
  );
  const [drawerSummary, { refetch: refetchDrawerSummary }] = createResource(async () =>
    bridge.invoke<DrawerSummary>("drawer_summary"),
  );
  const [drawerItems, { refetch: refetchDrawerItems }] = createResource(async () =>
    bridge.invoke<DrawerItem[]>("drawer_list", {}),
  );
  const [drawerHistory, { refetch: refetchDrawerHistory }] = createResource(async () =>
    bridge.invoke<DrawerHistory>("drawer_history"),
  );
  const [hiveRuns, { refetch: refetchHiveRuns }] = createResource(async () =>
    bridge.invoke<HiveRun[]>("hive_list"),
  );
  const [hiveSummary, { refetch: refetchHiveSummary }] = createResource(async () =>
    bridge.invoke<HiveSummary>("hive_summary"),
  );
  const [hiveLoops, { refetch: refetchHiveLoops }] = createResource(async () =>
    bridge.invoke<HiveLoop[]>("hive_loop_list"),
  );
  const [issueCards, { refetch: refetchIssueCards }] = createResource(async () =>
    bridge.invoke<IssueCard[]>("hive_panel"),
  );
  const [connectorRegistry, { refetch: refetchConnectorRegistry }] = createResource(async () =>
    bridge.invoke<ConnectorRegistry>("hive_connector_registry"),
  );
  const [connectorQueue, { refetch: refetchConnectorQueue }] = createResource(async () =>
    bridge.invoke<ConnectorQueueReport>("hive_connector_queue", {}),
  );
  const [launcherItems, { refetch: refetchLauncher }] = createResource(launcherQuery, async (query) =>
    bridge.invoke<LauncherResult[]>("launcher_search", { query, limit: 12 }),
  );

  const selectedIssueCard = createMemo(() => {
    const cards = issueCards() ?? [];
    if (cards.length === 0) return null;
    const issueId = selectedIssueId();
    return cards.find((card) => card.issue.id === issueId) ?? cards[0];
  });
  const selectedIssueDoctor = createMemo(() => selectedIssueCard()?.doctor ?? null);
  const selectedIssuePreflightKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card?.issue.loop_id) return null;
    return [
      card.issue.loop_id,
      card.issue.updated_at,
      card.trace?.current_round ?? 0,
      card.trace?.admission_count ?? 0,
      card.trace?.last_admission_passed ?? "pending",
    ].join(":");
  });
  const [selectedRuntimePreflight] = createResource(selectedIssuePreflightKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<RuntimePreflightReport>("hive_loop_runtime_preflight", { id: loopId });
  });
  const selectedIssueRuntimePreflight = createMemo(() => {
    const preflight = selectedRuntimePreflight();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return preflight && preflight.loop_id === loopId ? preflight : null;
  });
  const selectedIssueLifecycleKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card?.issue.loop_id) return null;
    return [
      card.issue.loop_id,
      card.issue.updated_at,
      card.trace?.current_round ?? 0,
      card.trace?.evidence_count ?? 0,
      card.trace?.role_worker_count ?? 0,
    ].join(":");
  });
  const [selectedWorkerLifecycle] = createResource(selectedIssueLifecycleKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<WorkerLifecycleReport>("hive_loop_worker_lifecycle", { id: loopId });
  });
  const selectedIssueWorkerLifecycle = createMemo(() => {
    const lifecycle = selectedWorkerLifecycle();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return lifecycle && lifecycle.loop_id === loopId ? lifecycle : null;
  });
  const issueCardsForStatus = (statusName: string) =>
    (issueCards() ?? []).filter((card) => card.issue.status === statusName);
  const reviewQueueCards = createMemo(() =>
    (issueCards() ?? []).filter((card) =>
      card.issue.status === "Blocked" || card.issue.status === "Needs Review",
    ),
  );
  const connectorQueueIssues = createMemo(() => connectorQueue()?.issues ?? []);
  const connectorQueueProviders = createMemo(() => connectorQueue()?.providers ?? []);
  const connectorPublishQueue = createMemo(() => {
    const cards = issueCards() ?? [];
    const queuedIds = new Set(
      connectorQueueIssues()
        .map((issue) => issue.id)
        .filter((id): id is number => typeof id === "number"),
    );
    if (connectorQueue()) {
      return cards.filter((card) => queuedIds.has(card.issue.id));
    }
    return cards.filter((card) => card.connector?.publish_required === true);
  });
  const connectorPublishRequiredCount = createMemo(
    () => connectorQueue()?.publish_required_count ?? connectorPublishQueue().length,
  );
  const connectorProviders = createMemo(() => connectorRegistry()?.providers ?? []);
  const connectorProviderAdmissions = createMemo(() => connectorRegistry()?.provider_admissions ?? []);
  const activeConnectorCount = createMemo(
    () => connectorProviders().filter((provider) => provider.status === "active").length,
  );
  const connectorProviderAdmission = (provider: ConnectorProvider) =>
    connectorProviderAdmissions().find((admission) => admission.provider === provider.name) ?? null;
  const connectorQueueIssueById = (issueId: number) =>
    connectorQueueIssues().find((issue) => issue.id === issueId) ?? null;
  const connectorQueueIssueCanPublish = (issueId: number) =>
    connectorQueueIssueById(issueId)?.can_publish !== false;
  const connectorQueueIssueTarget = (issueId: number) =>
    connectorQueueIssueById(issueId)?.remote_target ?? null;
  const connectorQueueIssueWritePlan = (issueId: number) =>
    connectorQueueIssueById(issueId)?.remote_write_plan ?? null;
  const connectorRemoteTargetIdentity = (target?: ConnectorRemoteTarget | null) => {
    if (!target) return null;
    if (target.issue_key) return target.issue_key;
    if (target.owner && target.repo) {
      return `${target.owner}/${target.repo}${target.issue_number ? `#${target.issue_number}` : ""}`;
    }
    if (target.fixture_key) return target.fixture_key;
    return target.target ?? target.remote_id ?? target.review_surface ?? null;
  };
  const connectorRemoteTargetLabel = (target?: ConnectorRemoteTarget | null) => {
    if (!target) return null;
    const identity = connectorRemoteTargetIdentity(target);
    if (target.valid === false) {
      return identity ? `target invalid ${identity}` : "target invalid";
    }
    return identity ? `target ${identity}` : null;
  };
  const connectorRemoteTargetTitle = (target?: ConnectorRemoteTarget | null) => {
    if (!target) return undefined;
    const parts = [
      target.target_kind ? `kind: ${target.target_kind}` : null,
      target.write_mode ? `mode: ${target.write_mode}` : null,
      target.review_surface ? `surface: ${target.review_surface}` : null,
      target.remote_url ? `url: ${target.remote_url}` : null,
      target.api_url ? `api: ${target.api_url}` : null,
    ].filter((part): part is string => Boolean(part));
    if (target.valid === false && target.blockers?.length) {
      parts.push(`blockers: ${target.blockers.join(", ")}`);
    }
    return parts.length ? parts.join(" | ") : undefined;
  };
  const connectorRemoteTargetTone = (target?: ConnectorRemoteTarget | null) =>
    target?.valid === false ? "warn" : "ok";
  const connectorRemoteTargetChip = (
    target: ConnectorRemoteTarget | null | undefined,
    testId: string,
  ) => {
    const label = connectorRemoteTargetLabel(target);
    return label ? (
      <span
        class={`connector-target connector-target--${connectorRemoteTargetTone(target)}`}
        data-testid={testId}
        title={connectorRemoteTargetTitle(target)}
      >
        {label}
      </span>
    ) : null;
  };
  const connectorRemoteWriteOperationLabel = (operation?: ConnectorRemoteWriteOperation | null) => {
    if (!operation) return null;
    const graphqlOperation = operation.graphql?.operation;
    if (graphqlOperation) return `plan ${graphqlOperation}`;
    return operation.method ? `plan ${operation.method}` : null;
  };
  const connectorRemoteWritePlanLabel = (plan?: ConnectorRemoteWritePlan | null) => {
    if (!plan) return null;
    const primary = connectorRemoteWriteOperationLabel(plan.operations?.[0]);
    if (!primary) return null;
    return plan.executable === false ? `${primary} blocked` : primary;
  };
  const connectorRemoteWritePlanTitle = (plan?: ConnectorRemoteWritePlan | null) => {
    if (!plan) return undefined;
    const operations = plan.operations
      ?.map((operation) =>
        [
          operation.kind,
          operation.method,
          operation.graphql?.operation,
          operation.url,
          operation.blocked_by?.length ? `blocked: ${operation.blocked_by.join(", ")}` : null,
        ]
          .filter(Boolean)
          .join(" "),
      )
      .filter(Boolean);
    const parts = [
      plan.schema_version,
      plan.remote_object_kind,
      plan.blocked_by?.length ? `plan blocked: ${plan.blocked_by.join(", ")}` : null,
      operations?.length ? `ops: ${operations.join(" | ")}` : null,
    ].filter((part): part is string => Boolean(part));
    return parts.length ? parts.join(" | ") : undefined;
  };
  const connectorRemoteWritePlanTone = (plan?: ConnectorRemoteWritePlan | null) =>
    plan?.executable === false ||
    Boolean(plan?.blocked_by?.length) ||
    Boolean(plan?.operations?.some((operation) => operation.blocked_by?.length))
      ? "warn"
      : "ok";
  const connectorRemoteWritePlanChip = (
    plan: ConnectorRemoteWritePlan | null | undefined,
    testId: string,
  ) => {
    const label = connectorRemoteWritePlanLabel(plan);
    return label ? (
      <span
        class={`connector-write-plan connector-write-plan--${connectorRemoteWritePlanTone(plan)}`}
        data-testid={testId}
        title={connectorRemoteWritePlanTitle(plan)}
      >
        {label}
      </span>
    ) : null;
  };
  const selectedIssueConnector = (issueId: number) =>
    (issueCards() ?? []).find((card) => card.issue.id === issueId)?.connector ?? null;
  const connectorRemoteDiagnostics = (issueId: number) =>
    connectorQueueIssueById(issueId)?.remote_diagnostics ??
    selectedIssueConnector(issueId)?.remote_diagnostics ??
    null;
  const connectorRemoteSignalTone = (signal: ConnectorRemoteSignal) =>
    signal.tone === "warn" ? "warn" : "info";
  const connectorRemoteSignalTitle = (signal: ConnectorRemoteSignal) => {
    const retry = signal.retry;
    const parts = [
      signal.stage ? `stage: ${signal.stage}` : null,
      signal.failed_check ? `failed: ${signal.failed_check}` : null,
      signal.http_status ? `http: ${signal.http_status}` : null,
      signal.attempt_count ? `attempts: ${signal.attempt_count}` : null,
      retry?.reason ? `retry: ${retry.reason}` : null,
      retry?.retry_after_secs ? `retry-after: ${retry.retry_after_secs}s` : null,
      retry?.backoff_ms ? `backoff: ${retry.backoff_ms}ms` : null,
      retry?.rate_limited ? "rate limited" : null,
      retry?.exhausted ? "retry exhausted" : null,
    ].filter((part): part is string => Boolean(part));
    return parts.length ? parts.join(" | ") : undefined;
  };
  const connectorRemoteDiagnosticChips = (
    diagnostics: ConnectorRemoteDiagnostics | null | undefined,
    testIdPrefix: string,
  ) =>
    diagnostics?.signals?.slice(0, 2).map((signal, index) =>
      signal.label ? (
        <span
          class={`connector-remote-signal connector-remote-signal--${connectorRemoteSignalTone(signal)}`}
          data-testid={`${testIdPrefix}-${index}`}
          title={connectorRemoteSignalTitle(signal)}
        >
          {compactText(signal.label, 72)}
        </span>
      ) : null,
    ) ?? [];
  const connectorRemoteAttemptLabel = (attempt: ConnectorRemoteAttempt) => {
    const retry = attempt.retry;
    const parts = [
      attempt.attempt ? `#${attempt.attempt}` : "attempt",
      attempt.success === true ? "ok" : attempt.success === false ? "failed" : null,
      attempt.http_status ? `HTTP ${attempt.http_status}` : null,
      attempt.failed_check,
      retry?.reason ? `retry ${retry.reason}` : null,
      retry?.scheduled ? "scheduled" : null,
      retry?.backoff_ms ? `${retry.backoff_ms}ms` : null,
      retry?.retry_after_secs ? `retry after ${retry.retry_after_secs}s` : null,
    ].filter((part): part is string => Boolean(part));
    return parts.join(" / ");
  };
  const connectorRemoteOperationName = (
    operation: ConnectorRemoteOperationDiagnostics | null | undefined,
  ) => {
    if (!operation) return "operation";
    return operation.graphql_operation ?? operation.kind ?? operation.method ?? "operation";
  };
  const connectorRemoteExecutionRows = (diagnostics: ConnectorRemoteDiagnostics | null | undefined) =>
    [
      diagnostics?.write ? { label: "write", execution: diagnostics.write } : null,
      diagnostics?.readback ? { label: "readback", execution: diagnostics.readback } : null,
    ].filter(
      (
        row,
      ): row is {
        label: string;
        execution: ConnectorRemoteExecutionDiagnostics;
      } => Boolean(row?.execution?.primary_operation?.attempts?.length),
    );
  const connectorRemoteAttemptDetails = (
    diagnostics: ConnectorRemoteDiagnostics | null | undefined,
    testIdPrefix: string,
  ) => {
    const rows = connectorRemoteExecutionRows(diagnostics);
    if (!rows.length) return null;
    return (
      <div class="connector-remote-drilldown" data-testid={testIdPrefix}>
        {rows.map(({ label, execution }) => {
          const operation = execution.primary_operation;
          const attempts = operation?.attempts ?? [];
          return (
            <details
              class="connector-remote-attempts"
              data-testid={`${testIdPrefix}-${label}`}
            >
              <summary>
                {label} attempts {attempts.length}/{operation?.max_attempts ?? attempts.length}
              </summary>
              <div class="connector-remote-attempt-head">
                <span>{connectorRemoteOperationName(operation)}</span>
                {operation?.failed_check ? <span>{operation.failed_check}</span> : null}
                {operation?.http_status ? <span>HTTP {operation.http_status}</span> : null}
              </div>
              <div class="connector-remote-attempt-list">
                {attempts.map((attempt) => (
                  <span
                    class={
                      attempt.success === false ||
                      attempt.retry?.rate_limited ||
                      attempt.retry?.exhausted
                        ? "connector-remote-attempt connector-remote-attempt--warn"
                        : "connector-remote-attempt"
                    }
                    data-testid={`${testIdPrefix}-${label}-attempt-${attempt.attempt ?? "n"}`}
                    title={attempt.error ?? undefined}
                  >
                    {connectorRemoteAttemptLabel(attempt)}
                  </span>
                ))}
              </div>
            </details>
          );
        })}
      </div>
    );
  };
  const connectorQueueIssuePublishTitle = (card: IssueCard) => {
    const queueIssue = connectorQueueIssueById(card.issue.id);
    if (queueIssue?.can_publish === false && queueIssue.publish_blockers?.length) {
      return `Publish blocked: ${queueIssue.publish_blockers.join(", ")}`;
    }
    return (
      queueIssue?.commands.publish ??
      card.connector?.publish_command ??
      issueMirrorPublishCommand(card)
    );
  };
  const revealIssueDetail = () => {
    window.setTimeout(() => {
      issueDetailPanel?.scrollIntoView({ block: "start", behavior: "auto" });
    }, 50);
  };
  const focusEvidence = (issueId: number, evidenceId: number) => {
    setSelectedIssueId(issueId);
    setSelectedEvidenceId(evidenceId);
    window.setTimeout(() => {
      (evidenceRows.get(evidenceId) ?? issueDetailPanel)?.scrollIntoView({
        block: "center",
        behavior: "auto",
      });
    }, 50);
  };
  const commentPillNode = (issueId: number, pill: CommentPill) =>
    pill.evidenceId ? (
      <button
        type="button"
        class="comment-tag-button"
        aria-label={`Show evidence #${pill.evidenceId} for issue #${issueId}`}
        onClick={() => focusEvidence(issueId, pill.evidenceId ?? 0)}
      >
        {pill.label}
      </button>
    ) : (
      <span>{pill.label}</span>
    );

  const refreshAll = async () => {
    setConnectorPublishPlan(null);
    setConnectorRoundtripPlan(null);
    await Promise.all([
      refetchStatus(),
      refetchDrawerSummary(),
      refetchDrawerItems(),
      refetchDrawerHistory(),
      refetchHiveRuns(),
      refetchHiveSummary(),
      refetchHiveLoops(),
      refetchIssueCards(),
      refetchConnectorRegistry(),
      refetchConnectorQueue(),
      refetchLauncher(),
    ]);
  };
  const refetchLoopSurfaces = async () => {
    await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchConnectorQueue(), refetchStatus()]);
  };
  const refetchIssueCardsQuietly = () => {
    void Promise.all([refetchIssueCards(), refetchConnectorQueue()]).catch(() => undefined);
  };
  const pollLoopSurfaces = () => {
    void refetchLoopSurfaces().catch(() => undefined);
  };
  const withLoopProgressPolling = async <T,>(work: Promise<T>) => {
    pollLoopSurfaces();
    const interval = window.setInterval(pollLoopSurfaces, 2500);
    try {
      return await work;
    } finally {
      window.clearInterval(interval);
    }
  };

  const addDrawerNote = async () => {
    await bridge.invoke("drawer_add_note", {
      title: drawerTitle() || "Untitled Note",
      body: drawerBody(),
    });
    setDrawerTitle("");
    setDrawerBody("");
    setBanner("Drawer note created.");
    await Promise.all([refetchDrawerSummary(), refetchDrawerItems(), refetchDrawerHistory(), refetchStatus()]);
  };

  const dispatchHive = async () => {
    await bridge.invoke("hive_dispatch", {
      title: hiveTitle() || "Untitled dispatch",
      projectDir: hiveProject() || undefined,
    });
    setHiveTitle("");
    setHiveProject("");
    setBanner("Hive dispatch persisted.");
    await Promise.all([refetchHiveRuns(), refetchHiveSummary(), refetchStatus()]);
  };

  const createHiveLoop = async () => {
    const report = await bridge.invoke<{ issues: IssueCard[] }>("hive_loop_create", {
      title: loopTitle() || "Untitled loop",
      goal: loopGoal() || loopTitle() || "Run an Entrance loop",
      runtime: loopRuntime(),
      approachSpace: ["Explore the smallest runnable MVP"],
      evalSpace: ["CLI loop run produces a keep/reject/block verdict"],
    });
    const createdIssueId = report.issues[0]?.issue.id ?? null;
    if (createdIssueId !== null) {
      setSelectedIssueId(createdIssueId);
    }
    setLoopTitle("");
    setLoopGoal("");
    setBanner(
      createdIssueId === null
        ? "Loop contract created."
        : `Loop contract created as issue #${createdIssueId}.`,
    );
    await refetchLoopSurfaces();
    if (createdIssueId !== null) {
      revealIssueDetail();
    }
  };

  const setPendingLoop = (loopId: number, label: string | null) => {
    setPendingLoopActions((current) => {
      const next = { ...current };
      if (label) {
        next[loopId] = label;
      } else {
        delete next[loopId];
      }
      return next;
    });
  };

  const setPendingIssue = (issueId: number, label: string | null) => {
    setPendingIssueActions((current) => {
      const next = { ...current };
      if (label) {
        next[issueId] = label;
      } else {
        delete next[issueId];
      }
      return next;
    });
  };

  const loopPendingLabel = (loopId: number) => pendingLoopActions()[loopId] ?? null;
  const issuePendingLabel = (issueId: number) => pendingIssueActions()[issueId] ?? null;
  const issueDecisionNote = (issueId: number) =>
    activeCommentComposer()?.issueId === issueId ? commentBody().trim() : "";
  const commentSubmitDisabled = (issueId: number) =>
    Boolean(issuePendingLabel(issueId)) || !commentBody().trim();
  const clearIssueComposer = (issueId: number) => {
    if (activeCommentComposer()?.issueId === issueId) {
      setCommentBody("");
      setActiveCommentComposer(null);
    }
  };
  const commentComposerActive = (issueId: number, surface: CommentSurface) => {
    const active = activeCommentComposer();
    return active?.issueId === issueId && active.surface === surface;
  };
  const actionErrorMessage = (error: unknown) =>
    error instanceof Error ? error.message : String(error);
  const writeClipboardText = async (text: string) => {
    if (navigator.clipboard?.writeText) {
      try {
        await navigator.clipboard.writeText(text);
        return;
      } catch {
        // Browser and Electron focus rules can reject the async clipboard API.
        // Fall back to a selected textarea so the control remains usable.
      }
    }
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "true");
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    document.body.appendChild(textarea);
    window.focus();
    textarea.focus({ preventScroll: true });
    textarea.select();
    const copied = document.execCommand("copy");
    textarea.remove();
    if (!copied) {
      throw new Error("clipboard is unavailable");
    }
  };
  const copyCommandAction = async (label: string, action: string) => {
    try {
      await writeClipboardText(action);
      setBanner(`Copied ${label}: ${compactText(action, 96)}`);
    } catch {
      setBanner(`Copy unavailable; command: ${compactText(action, 120)}`);
    }
  };
  const copyDoctorAction = (action: string) => copyCommandAction("next action", action);
  const syncIssueMirror = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Syncing");
    try {
      const report = await bridge.invoke<IssueMirrorSyncReport>("hive_issue_mirror_sync", {
        issueId: card.issue.id,
      });
      setBanner(`Synced issue mirror ${compactText(report.sha256, 12)}: ${compactText(report.path, 86)}`);
    } catch (error) {
      setBanner(`Issue #${card.issue.id} sync failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
      refetchIssueCardsQuietly();
    }
  };
  const publishIssueMirror = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Publishing");
    setConnectorPublishPlan(null);
    setConnectorRoundtripPlan(null);
    try {
      const report = await bridge.invoke<IssueMirrorPublishReport>("hive_issue_mirror_publish", {
        issueId: card.issue.id,
      });
      if (report.published === false) {
        const blockers = report.failed_checks?.length
          ? `: ${report.failed_checks.slice(0, 3).join(", ")}`
          : "";
        setBanner(`Connector publish blocked${blockers}`);
      } else {
        setBanner(
          `Published connector mirror ${compactText(report.sha256 ?? "", 12)}: ${compactText(report.path ?? "", 86)}`,
        );
      }
    } catch (error) {
      setBanner(`Issue #${card.issue.id} publish failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
      refetchIssueCardsQuietly();
    }
  };
  const planConnectorPublish = async () => {
    if (connectorPublishAction()) return;
    setConnectorPublishAction("Planning");
    try {
      const plan = await bridge.invoke<ConnectorPublishPlan>("hive_connector_publish_plan", {});
      setConnectorPublishPlan(plan);
      const blockers = plan.blockers?.length
        ? `: ${plan.blockers.slice(0, 3).join(", ")}`
        : "";
      setBanner(
        plan.can_execute
          ? `Connector publish plan ${compactText(plan.plan_id, 12)}: ${plan.issue_count} issues.`
          : `Connector publish plan blocked: ${plan.reason}${blockers}`,
      );
    } catch (error) {
      setBanner(`Connector publish plan failed: ${actionErrorMessage(error)}`);
    } finally {
      setConnectorPublishAction(null);
      void refetchConnectorQueue();
    }
  };
  const executeConnectorPublishPlan = async () => {
    const plan = connectorPublishPlan();
    if (!plan?.can_execute || connectorPublishAction()) return;
    setConnectorPublishAction("Executing");
    try {
      const report = await bridge.invoke<ConnectorPublishExecuteReport>(
        "hive_connector_publish_execute",
        { planId: plan.plan_id },
      );
      if (report.executed) {
        setBanner(
          `Connector publish executed ${compactText(plan.plan_id, 12)}: ${report.issue_count ?? 0} issues.`,
        );
        setConnectorPublishPlan(null);
      } else {
        setBanner(`Connector publish skipped: ${report.reason}`);
        if (report.current_plan_id) {
          setConnectorPublishPlan(null);
        }
      }
    } catch (error) {
      setBanner(`Connector publish execute failed: ${actionErrorMessage(error)}`);
    } finally {
      setConnectorPublishAction(null);
      await Promise.all([refetchIssueCards(), refetchConnectorQueue()]);
    }
  };
  const planConnectorRoundtrip = async () => {
    if (connectorRoundtripAction()) return;
    setConnectorRoundtripAction("Planning");
    try {
      const plan = await bridge.invoke<ConnectorRoundtripPlan>("hive_connector_roundtrip_plan", {});
      setConnectorRoundtripPlan(plan);
      const blockers = plan.blockers?.length
        ? `: ${plan.blockers.slice(0, 3).join(", ")}`
        : "";
      setBanner(
        plan.can_execute
          ? `Connector roundtrip plan ${compactText(plan.plan_id, 12)}: ${plan.issue_count} issues.`
          : `Connector roundtrip plan blocked: ${plan.reason}${blockers}`,
      );
    } catch (error) {
      setBanner(`Connector roundtrip plan failed: ${actionErrorMessage(error)}`);
    } finally {
      setConnectorRoundtripAction(null);
      void refetchConnectorQueue();
    }
  };
  const executeConnectorRoundtripPlan = async () => {
    const plan = connectorRoundtripPlan();
    if (!plan?.can_execute || connectorRoundtripAction()) return;
    setConnectorRoundtripAction("Executing");
    try {
      const report = await bridge.invoke<ConnectorRoundtripExecuteReport>(
        "hive_connector_roundtrip_execute",
        { planId: plan.plan_id },
      );
      if (report.executed) {
        setBanner(
          `Connector roundtrip executed ${compactText(plan.plan_id, 12)}: ${report.completed_count ?? 0}/${report.issue_count ?? 0} completed.`,
        );
        setConnectorRoundtripPlan(null);
        setConnectorPublishPlan(null);
      } else {
        setBanner(`Connector roundtrip skipped: ${report.reason}`);
        if (report.current_plan_id) {
          setConnectorRoundtripPlan(null);
        }
      }
    } catch (error) {
      setBanner(`Connector roundtrip execute failed: ${actionErrorMessage(error)}`);
    } finally {
      setConnectorRoundtripAction(null);
      await Promise.all([refetchIssueCards(), refetchConnectorQueue()]);
    }
  };
  const verifyIssueMirror = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Verifying");
    try {
      const report = await bridge.invoke<IssueMirrorAuditReport>("hive_issue_mirror_audit", {
        issueId: card.issue.id,
      });
      if (report.passed) {
        setBanner(`Verified issue mirror ${compactText(report.verify.current.sha256, 12)}.`);
      } else {
        setBanner(`Issue #${card.issue.id} mirror audit failed: ${compactText(report.failed_checks.join(", "), 96)}`);
      }
    } catch (error) {
      setBanner(`Issue #${card.issue.id} verify failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
      refetchIssueCardsQuietly();
    }
  };
  const readbackIssueMirror = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Reading");
    try {
      const report = await bridge.invoke<IssueMirrorReadbackReport>("hive_issue_mirror_readback", {
        issueId: card.issue.id,
        record: true,
      });
      const evidenceLabel = report.recorded?.evidence_id ? ` (E#${report.recorded.evidence_id})` : "";
      const publishLabel = report.recorded?.publish?.required ? "; publish required" : "";
      if (report.passed) {
        setBanner(
          `Read back issue mirror ${compactText(report.current.digest.sha256, 12)}: ${report.remote.surface?.comments.count ?? 0} comments${evidenceLabel}${publishLabel}`,
        );
      } else {
        setBanner(
          `Issue #${card.issue.id} readback failed: ${compactText(report.failed_checks.join(", "), 96)}${evidenceLabel}${publishLabel}`,
        );
      }
    } catch (error) {
      setBanner(`Issue #${card.issue.id} readback failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
      refetchIssueCardsQuietly();
    }
  };
  const admitIssueMirror = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Admitting");
    try {
      const report = await bridge.invoke<IssueMirrorAdmissionReport>("hive_issue_mirror_admit", {
        issueId: card.issue.id,
        record: true,
      });
      const checkLabel = admissionCheckLabel(report.provider_checks);
      const checkSuffix = checkLabel ? `, ${checkLabel}` : "";
      if (report.admitted) {
        const evidenceLabel = report.recorded?.evidence_id ? `E#${report.recorded.evidence_id}` : "recorded";
        const publishLabel = report.recorded?.publish?.required ? "; publish required" : "";
        setBanner(
          `Admitted connector mirror ${compactText(report.receipt.sha256 ?? "no-sha", 12)}: ${report.decision.route_to} (${evidenceLabel}${checkSuffix})${publishLabel}`,
        );
      } else {
        const evidenceLabel = report.recorded?.evidence_id ? ` (E#${report.recorded.evidence_id})` : "";
        const publishLabel = report.recorded?.publish?.required ? "; publish required" : "";
        setBanner(`Connector admission rejected${checkLabel ? ` (${checkLabel})` : ""}: ${compactText(report.failed_checks.join(", "), 96)}${evidenceLabel}${publishLabel}`);
      }
    } catch (error) {
      setBanner(`Issue #${card.issue.id} admission failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
      refetchIssueCardsQuietly();
    }
  };
  const roundtripIssueMirror = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Roundtrip");
    setConnectorPublishPlan(null);
    setConnectorRoundtripPlan(null);
    try {
      const report = await bridge.invoke<IssueMirrorRoundtripReport>("hive_issue_mirror_roundtrip", {
        issueId: card.issue.id,
        record: true,
      });
      const evidenceLabel = report.recorded_evidence_ids?.length
        ? `; E#${report.recorded_evidence_ids.join(", E#")}`
        : "";
      const stageLabel = `${report.passed_stage_count}/${report.stage_count} stages`;
      if (report.completed) {
        const remoteLabel = report.remote?.object_kind ? ` ${report.remote.object_kind}` : "";
        setBanner(`Connector roundtrip complete${remoteLabel}: ${stageLabel}${evidenceLabel}`);
      } else {
        const failed = report.failed_stages?.length ? `: ${report.failed_stages.join(", ")}` : "";
        setBanner(`Connector roundtrip blocked (${stageLabel})${failed}${evidenceLabel}`);
      }
    } catch (error) {
      setBanner(`Issue #${card.issue.id} roundtrip failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
      await Promise.all([refetchIssueCards(), refetchConnectorQueue()]);
    }
  };
  const workerLimitRunArgs = (): LoopRunArgs => {
    const timeoutText = loopWorkerTimeoutSecs().trim();
    const workerTimeoutSecs = timeoutText ? Number.parseInt(timeoutText, 10) : undefined;
    const attemptsText = loopWorkerAttempts().trim();
    const workerAttempts = attemptsText ? Number.parseInt(attemptsText, 10) : undefined;
    return {
      workerTimeoutSecs: workerTimeoutSecs && workerTimeoutSecs > 0 ? workerTimeoutSecs : undefined,
      workerAttempts: workerAttempts && workerAttempts > 0 ? workerAttempts : undefined,
    };
  };
  const loopRunArgs = (): LoopRunArgs => ({
    runtime: loopRuntime(),
    ...workerLimitRunArgs(),
  });
  const commandRunArgs = (command: string | undefined): LoopRunArgs => {
    if (!command) return {};
    const tokens = command.split(/\s+/);
    const valueAfter = (flag: string) => {
      const index = tokens.indexOf(flag);
      return index >= 0 ? tokens[index + 1] : undefined;
    };
    const parsePositive = (value: string | undefined) => {
      if (!value) return undefined;
      const parsed = Number.parseInt(value, 10);
      return parsed > 0 ? parsed : undefined;
    };
    return {
      runtime: valueAfter("--runtime"),
      workerTimeoutSecs: parsePositive(valueAfter("--worker-timeout-secs")),
      workerAttempts: parsePositive(valueAfter("--worker-attempts")),
    };
  };
  const issueActionByName = (card: IssueCard, actionName: string) =>
    card.actions.find((action) => action.action === actionName);
  const doctorRunArgs = (card: IssueCard, commandNeedles: string | string[]) => {
    const needles = Array.isArray(commandNeedles) ? commandNeedles : [commandNeedles];
    return commandRunArgs(
      card.doctor?.next_actions.find((action) =>
        needles.some((needle) => action.includes(needle)),
      ),
    );
  };
  const mergeRunArgs = (...argsList: LoopRunArgs[]) => {
    const merged: LoopRunArgs = {};
    argsList.forEach((args) => {
      if (args.runtime) merged.runtime = args.runtime;
      if (args.workerTimeoutSecs) merged.workerTimeoutSecs = args.workerTimeoutSecs;
      if (args.workerAttempts) merged.workerAttempts = args.workerAttempts;
    });
    return merged;
  };
  const demoRunArgs = () =>
    mergeRunArgs({ runtime: "codex", workerTimeoutSecs: 90, workerAttempts: 1 }, workerLimitRunArgs());
  const hasRunArgs = (args: LoopRunArgs) =>
    Boolean(args.runtime || args.workerTimeoutSecs || args.workerAttempts);
  const issueRunArgs = (card: IssueCard) => {
    const actionArgs = commandRunArgs(issueActionByName(card, "run")?.command);
    if (hasRunArgs(actionArgs)) return mergeRunArgs(actionArgs, workerLimitRunArgs());
    const doctorArgs = doctorRunArgs(card, ["entrance hive issue run", "entrance hive loop run"]);
    return hasRunArgs(doctorArgs)
      ? mergeRunArgs(doctorArgs, workerLimitRunArgs())
      : loopRunArgs();
  };
  const issueRetryRunArgs = (card: IssueCard) =>
    mergeRunArgs(
      commandRunArgs(issueActionByName(card, "retry")?.command),
      doctorRunArgs(card, "entrance hive issue retry-run"),
      workerLimitRunArgs(),
    );
  const runArgsSummary = (args: LoopRunArgs, fallbackRuntime?: string) => {
    const runtime = args.runtime ?? fallbackRuntime;
    const parts = runtime ? [runtime] : [];
    if (args.workerAttempts && args.workerAttempts > 1) parts.push(`${args.workerAttempts}x`);
    if (args.workerTimeoutSecs) parts.push(`${args.workerTimeoutSecs}s`);
    return parts.join(" ");
  };
  const issueRuntimeSummary = (card: IssueCard, retry: boolean) =>
    runArgsSummary(
      retry ? issueRetryRunArgs(card) : issueRunArgs(card),
      card.doctor?.runtime || card.trace?.worker_kind || undefined,
    );
  const issueRuntimeActionLabel = (card: IssueCard, retry: boolean) => {
    const pending = issuePendingLabel(card.issue.id);
    if (pending) return pending;
    const verb = retry ? "Retry" : "Run";
    const summary = issueRuntimeSummary(card, retry);
    return summary ? `${verb} ${summary}` : verb;
  };
  const issueRuntimeActionAriaLabel = (card: IssueCard, retry: boolean, surface: string) => {
    const verb = retry ? "Retry" : "Run";
    const summary = issueRuntimeSummary(card, retry);
    return summary
      ? `${verb} issue #${card.issue.id} from ${surface} with ${summary}`
      : `${verb} issue #${card.issue.id} from ${surface}`;
  };

  const startDemoLoop = async () => {
    if (pendingDemoAction()) return;
    setView("panel");
    setPendingDemoAction("Running Demo");
    setBanner("Running Entrance MVP demo.");
    try {
      const runArgs = demoRunArgs();
      const report = await bridge.invoke<{ contract?: HiveLoop; issues: IssueCard[] }>("hive_loop_create", {
        title: "Entrance MVP demo",
        goal: "Run the Entrance Explorer -> Developer -> Reviewer loop and expose it on the issue/status/comment panel.",
        boundary: "Use the local Hive SQLite ledger, typed receipts, compact CLI output, and the local Panel surface.",
        runtime: "codex",
        approachSpace: [
          "Compile the natural-language goal into a typed candidate",
          "Develop only the admitted candidate",
          "Review the evidence with keep/reject/block gates",
        ],
        evalSpace: [
          "Explorer, Developer, and Reviewer each produce role receipts",
          "Admissions bind packets to policy gates",
          "Panel shows issue status, comments, evidence, verdict, and recovery actions",
        ],
      });
      const issue = report.issues[0];
      if (!issue) throw new Error("Demo loop did not create an issue.");
      setSelectedIssueId(issue.issue.id);
      await refetchLoopSurfaces();
      await withLoopProgressPolling(bridge.invoke("hive_issue_run", {
        issueId: issue.issue.id,
        ...runArgs,
      }));
      const loopId = issue.issue.loop_id ?? report.contract?.id;
      setBanner(loopId ? `Demo loop #${loopId} finished.` : "Demo loop finished.");
      await refetchLoopSurfaces();
      revealIssueDetail();
    } catch (error) {
      setBanner(`Demo loop failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingDemoAction(null);
    }
  };

  const runConnectorFixtureDemo = async () => {
    if (pendingFixtureAction()) return;
    setView("panel");
    setPendingFixtureAction("Running Fixture");
    setConnectorPublishPlan(null);
    setConnectorRoundtripPlan(null);
    setBanner("Running remote fixture roundtrip.");
    try {
      const report = await bridge.invoke<ConnectorFixtureDemoReport>(
        "hive_connector_fixture_demo",
        { record: true },
      );
      setSelectedIssueId(report.issue_id);
      const evidenceLabel = report.summary?.recorded_evidence_ids?.length
        ? `; E#${report.summary.recorded_evidence_ids.join(", E#")}`
        : "";
      const stageLabel = `${report.summary?.passed_stage_count ?? 0}/${report.summary?.stage_count ?? 0} stages`;
      const remoteLabel = report.summary?.remote_object_kind
        ? ` ${report.summary.remote_object_kind}`
        : "";
      setBanner(
        report.completed
          ? `Remote fixture roundtrip complete${remoteLabel}: ${stageLabel}${evidenceLabel}`
          : `Remote fixture roundtrip blocked: ${stageLabel}${evidenceLabel}`,
      );
      await refetchLoopSurfaces();
      revealIssueDetail();
    } catch (error) {
      setBanner(`Remote fixture demo failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingFixtureAction(null);
    }
  };

  const runHiveLoop = async (loop: HiveLoop) => {
    if (loopPendingLabel(loop.id)) return;
    setPendingLoop(loop.id, "Running");
    try {
      const runArgs = loopRunArgs();
      await withLoopProgressPolling(bridge.invoke("hive_loop_run", {
        id: loop.id,
        ...runArgs,
        runtime: loop.runtime || runArgs.runtime,
      }));
      setBanner(`Loop #${loop.id} finished.`);
      await refetchLoopSurfaces();
    } catch (error) {
      setBanner(`Loop #${loop.id} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingLoop(loop.id, null);
    }
  };

  const runIssueLoop = async (card: IssueCard) => {
    if (!card.issue.loop_id || issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Running");
    try {
      await withLoopProgressPolling(bridge.invoke("hive_issue_run", {
        issueId: card.issue.id,
        ...issueRunArgs(card),
      }));
      setBanner(`Loop #${card.issue.loop_id} finished.`);
      await refetchLoopSurfaces();
    } catch (error) {
      setBanner(`Loop #${card.issue.loop_id} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
    }
  };

  const decideIssue = async (issueId: number, action: string) => {
    if (issuePendingLabel(issueId)) return;
    setSelectedIssueId(issueId);
    setPendingIssue(issueId, issuePendingActionLabel(action));
    try {
      await bridge.invoke("hive_issue_decide", {
        issueId,
        action,
        author: "human",
        body: issueDecisionNote(issueId) || undefined,
      });
      clearIssueComposer(issueId);
      setBanner(`Issue #${issueId} ${issueActionLabel(action)}.`);
      await refetchLoopSurfaces();
    } catch (error) {
      setBanner(`Issue #${issueId} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(issueId, null);
    }
  };

  const retryIssueLoop = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Retrying");
    try {
      await withLoopProgressPolling(bridge.invoke("hive_issue_run", {
        issueId: card.issue.id,
        retry: true,
        author: "human",
        body: issueDecisionNote(card.issue.id) || undefined,
        ...issueRetryRunArgs(card),
      }));
      clearIssueComposer(card.issue.id);
      setBanner(`Issue #${card.issue.id} retried.`);
      await refetchLoopSurfaces();
    } catch (error) {
      setBanner(`Issue #${card.issue.id} retry failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
    }
  };

  const openIssueComment = (issueId: number, surface: CommentSurface) => {
    if (activeCommentComposer()?.issueId !== issueId) {
      setCommentBody("");
    }
    setSelectedIssueId(issueId);
    setActiveCommentComposer({ issueId, surface });
    focusIssueComment(issueId, surface);
  };

  const closeIssueComment = (issueId: number) => {
    clearIssueComposer(issueId);
  };
  const focusIssueComment = (issueId: number, surface: CommentSurface) => {
    window.setTimeout(() => {
      document
        .querySelector<HTMLTextAreaElement>(`[data-testid="issue-comment-${surface}-${issueId}"]`)
        ?.focus();
    }, 0);
  };
  const handleCommentKeyDown = (event: KeyboardEvent, issueId: number) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void addIssueComment(issueId);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      closeIssueComment(issueId);
    }
  };

  const addIssueComment = async (issueId: number) => {
    const body = commentBody().trim();
    if (!body || issuePendingLabel(issueId)) return;
    setSelectedIssueId(issueId);
    setPendingIssue(issueId, "Sending");
    try {
      await bridge.invoke("hive_issue_comment", {
        issueId,
        author: "human",
        body,
      });
      setCommentBody("");
      setActiveCommentComposer(null);
      setBanner(`Commented on issue #${issueId}.`);
      await refetchIssueCards();
    } catch (error) {
      setBanner(`Comment failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(issueId, null);
    }
  };

  const refreshLauncherIndex = async () => {
    await bridge.invoke("launcher_refresh", {});
    setBanner("Launcher index refreshed.");
    await Promise.all([refetchLauncher(), refetchStatus()]);
  };

  const launchItem = async (item: LauncherResult) => {
    await bridge.invoke("launcher_launch", {
      command: item.command,
      arguments: item.arguments,
      workingDir: item.working_dir,
    });
    setBanner(`Launched ${item.name}.`);
    await refetchLauncher();
  };

  const pinItem = async (item: LauncherResult) => {
    await bridge.invoke("launcher_pin", {
      command: item.command,
      pinned: !item.pinned,
    });
    await refetchLauncher();
  };

  const issueActionLabel = (action: string) =>
    ({
      retry: "retried",
      "request-review": "sent to review",
      cancel: "canceled",
    })[action] ?? action;

  const issuePendingActionLabel = (action: string) =>
    ({
      retry: "Retrying",
      "request-review": "Reviewing",
      cancel: "Canceling",
    })[action] ?? "Working";

  const issueDecisionButtonLabel = (card: IssueCard, action: IssueAction) =>
    action.action === "retry"
      ? issueRuntimeActionLabel(card, true)
      : issuePendingLabel(card.issue.id) ?? action.label;
  const issueActionContractLabel = (action: IssueAction) =>
    [
      action.action,
      action.source,
      action.input,
      action.runtime ?? "no-runtime",
      action.confirmation_required ? "confirmed" : "no-confirm",
      action.destructive ? "destructive" : "non-destructive",
    ].join(" / ");
  const issueActionTitle = (action: IssueAction) =>
    [
      `schema=${action.schema_version}`,
      `source=${action.source}`,
      `input=${action.input}`,
      `runtime=${action.runtime ?? "none"}`,
      `confirmation_required=${action.confirmation_required ? "true" : "false"}`,
      action.confirmation_arg ? `confirmation_arg=${action.confirmation_arg}` : null,
      action.receipt_schema ? `receipt_schema=${action.receipt_schema}` : null,
      action.policy_schema_version ? `policy=${action.policy_schema_version}` : null,
      `destructive=${action.destructive ? "true" : "false"}`,
      action.command,
    ]
      .filter((value): value is string => Boolean(value))
      .join(" | ");
  const issueActionButtonAttrs = (action: IssueAction | undefined) =>
    action
      ? {
          "data-action-destructive": action.destructive ? "true" : "false",
          "data-action-input": action.input,
          "data-action-runtime": action.runtime ?? "",
          "data-action-schema-version": action.schema_version,
          "data-action-source": action.source,
          "data-action-confirmation-required": action.confirmation_required ? "true" : "false",
          "data-action-confirmation-arg": action.confirmation_arg ?? "",
          "data-action-receipt-schema": action.receipt_schema ?? "",
          "data-action-policy-schema-version": action.policy_schema_version ?? "",
          title: issueActionTitle(action),
        }
      : {};
  const issueActionContractChips = (card: IssueCard, surface: string) => (
    <div
      class="action-contracts"
      data-testid={`issue-action-contracts-${surface}-${card.issue.id}`}
    >
      {card.actions.map((action) => (
        <span
          class={`action-contract${action.destructive ? " action-contract--destructive" : ""}`}
          data-testid={`issue-action-contract-${surface}-${action.action}-${card.issue.id}`}
          title={issueActionTitle(action)}
        >
          {issueActionContractLabel(action)}
        </span>
      ))}
    </div>
  );

  const issueOptionDisabled = (card: IssueCard, action: IssueAction) =>
    Boolean(issuePendingLabel(card.issue.id)) ||
    (action.action === "retry" && !card.issue.loop_id);
  const issueHumanActions = (card: IssueCard) =>
    card.actions.filter((action) => action.source !== "runtime" && action.action !== "run");
  const issueDecisionActions = (card: IssueCard) =>
    issueHumanActions(card).filter((action) => action.action !== "comment");

  const runIssueAction = (card: IssueCard, action: IssueAction) => {
    const option = action.action;
    setSelectedIssueId(card.issue.id);
    if (option === "comment") {
      openIssueComment(card.issue.id, "detail");
      return;
    }
    if (option === "retry") {
      void retryIssueLoop(card);
      return;
    }
    if (option === "request-review" || option === "cancel") {
      void decideIssue(card.issue.id, option);
      return;
    }
    setBanner(`Unsupported issue option ${option}.`);
  };

  const schemaLabel = (schema: string | null | undefined) =>
    schema ? schema.split(".").slice(-2).join(".") : "pending";

  const workerLabel = (card: IssueCard) => {
    if (!card.trace?.worker_kind) return null;
    const state = card.trace.worker_ok === true ? "ok" : "blocked";
    return `${card.trace.worker_kind}/${state}`;
  };

  const receiptLabel = (card: IssueCard) => {
    if (!card.trace) return null;
    if (card.trace.round_receipt_required_count === 0) return "receipts pending";
    return card.trace.round_receipt_missing_count === 0
      ? "receipts ok"
      : `missing ${card.trace.round_receipt_missing_count}`;
  };

  const gateLabel = (card: IssueCard) => {
    if (!card.trace?.last_admission_gate) return null;
    const state = card.trace.last_admission_passed === true ? "ok" : "blocked";
    return `gate ${state}`;
  };

  const roleWorkerLabel = (card: IssueCard) => {
    if (!card.trace) return null;
    if (card.trace.round_role_worker_count === 0) return "workers pending";
    return `workers ${card.trace.round_role_worker_ok_count}/${card.trace.round_role_worker_count}`;
  };

  const traceCountLabel = (label: string, current: number, total: number) =>
    total > current ? `${label} ${current}/${total}` : `${label} ${current}`;

  const auditLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    if (trace.audit_passed === null) return "audit pending";
    return trace.audit_passed ? "audit ok" : `audit fail ${trace.audit_failed_count}`;
  };

  const reviewQueueDecisionLabel = (card: IssueCard) =>
    card.trace?.last_decision
      ? `${card.trace.last_decision}${card.trace.reason_code ? ` / ${card.trace.reason_code}` : ""}`
      : card.issue.status;

  const reviewQueueBlockerLabel = (card: IssueCard) => {
    const doctor = card.doctor;
    if (!doctor) return "doctor pending";
    const blockers = [
      ...doctor.failed_checks,
      ...doctor.audit_failure_details,
      ...doctor.missing_receipts.map((receipt) => `missing ${receipt}`),
      ...doctor.worker_failures,
    ];
    return blockers.length ? compactText(blockers.join(", "), 96) : doctor.health;
  };

  const reviewQueueEvidence = (card: IssueCard) => card.trace?.evidence.slice(-3) ?? [];

  const stageWorkerLabel = (stage: NonNullable<IssueCard["trace"]>["stages"][number]) => {
    if (!stage.worker_kind) return "worker pending";
    const state = stage.worker_ok === true ? "ok" : "blocked";
    return `${stage.worker_kind}/${state}`;
  };

  const evidenceWorkerLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (!evidence.worker_kind) return "worker pending";
    const state = evidence.worker_ok === true ? "ok" : "blocked";
    return `${evidence.worker_kind}/${state}`;
  };

  const workerReceiptLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_receipt_ok === null) return null;
    return evidence.worker_receipt_ok ? "receipt ok" : "receipt fail";
  };

  const workerStatusLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_status === null) return null;
    return `exit ${evidence.worker_status}`;
  };

  const workerDurationLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_duration_ms === null) return null;
    if (evidence.worker_duration_ms >= 1000) {
      return `${(evidence.worker_duration_ms / 1000).toFixed(1)}s`;
    }
    return `${evidence.worker_duration_ms}ms`;
  };

  const runtimeDurationLabel = (durationMs: number) =>
    durationMs >= 1000 ? `${(durationMs / 1000).toFixed(1)}s` : `${durationMs}ms`;

  const traceRuntimeLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    if (trace.round_role_worker_count === 0) return null;
    return `runtime ${runtimeDurationLabel(trace.round_worker_duration_ms)}`;
  };

  const traceRuntimeWarnLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    const warnings = [];
    if (trace.round_worker_timeout_count) warnings.push(`timeouts ${trace.round_worker_timeout_count}`);
    if (trace.round_worker_retry_exhausted_count) {
      warnings.push(`retry exhausted ${trace.round_worker_retry_exhausted_count}`);
    }
    return warnings.length ? warnings.join(" / ") : null;
  };

  const workerTimeoutLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) =>
    evidence.worker_timeout_secs === null ? null : `limit ${evidence.worker_timeout_secs}s`;

  const workerAttemptLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_attempt_count === null) return null;
    return evidence.worker_max_attempts === null
      ? `attempts ${evidence.worker_attempt_count}`
      : `attempts ${evidence.worker_attempt_count}/${evidence.worker_max_attempts}`;
  };
  const workerCommandLabel = (command: string | null) => {
    if (!command) return null;
    return command.startsWith("codex ")
      ? "cmd codex exec"
      : `cmd ${command.split(/\s+/).slice(0, 2).join(" ")}`;
  };

  const shouldShowTranscriptExcerpt = (
    evidence: NonNullable<IssueCard["trace"]>["evidence"][number],
  ) =>
    Boolean(
      evidence.transcript_excerpt &&
        (evidence.worker_ok === false ||
          evidence.worker_receipt_ok === false ||
          evidence.worker_timed_out === true ||
          evidence.worker_retry_exhausted === true ||
          evidence.worker_receipt_errors.length > 0),
    );

  const scoreMetricLabel = (name: string) =>
    ({
      stage_completeness: "stage",
      runtime_readiness: "runtime",
      evidence_presence: "evidence",
      admission_integrity: "admission",
    })[name] ?? name;

  const scoreValueLabel = (value: number | null) => (value === null ? "n/a" : value.toFixed(2));

  const scoreVectorLabel = (trace: NonNullable<IssueCard["trace"]>) =>
    trace.score_vector.length
      ? trace.score_vector
          .map((metric) => `${scoreMetricLabel(metric.name)} ${scoreValueLabel(metric.value)}`)
          .join(" / ")
      : null;

  const scoreSummaryLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    if (!trace.score_vector.length) return null;
    const healthy = trace.score_vector.filter((metric) => metric.value !== null && metric.value >= 1).length;
    return `score ${healthy}/${trace.score_vector.length}`;
  };

  const doctorHealthLabel = (health: string) =>
    ({
      ok: "ok",
      blocked: "blocked",
      needs_review: "needs review",
      rejected: "rejected",
      audit_failed: "audit failed",
      worker_failed: "worker failed",
      pending: "pending",
      unknown: "unknown",
    })[health] ?? health;

  const doctorHealthTone = (health: string) =>
    health === "ok" ? "ok" : health === "pending" ? "pending" : "warn";

  const doctorReceiptLabel = (doctor: IssueDoctorSummary) => {
    const required = doctor.counts.round_receipt_required_count;
    if (required === 0) return "receipts pending";
    const present = required - doctor.counts.round_receipt_missing_count;
    return `receipts ${present}/${required}`;
  };

  const doctorWorkerLabel = (doctor: IssueDoctorSummary) => {
    const total = doctor.counts.round_role_worker_count;
    if (total === 0) return "workers pending";
    return `workers ${doctor.counts.round_role_worker_ok_count}/${total}`;
  };

  const doctorRuntimeLabel = (doctor: IssueDoctorSummary) => {
    if (doctor.counts.round_role_worker_count === 0) return "runtime pending";
    return `runtime ${runtimeDurationLabel(doctor.counts.round_worker_duration_ms)}`;
  };

  const runtimePreflightStateLabel = (state: string) =>
    ({
      admitted: "admitted",
      ready: "ready",
      blocked: "blocked",
      pending: "pending",
    })[state] ?? state;

  const runtimePreflightTone = (state: string) =>
    state === "admitted" || state === "ready"
      ? "ok"
      : state === "pending"
        ? "pending"
        : "warn";

  const runtimePreflightBoolLabel = (label: string, value: boolean | null | undefined) => {
    if (value === true) return `${label} ok`;
    if (value === false) return `${label} blocked`;
    return `${label} pending`;
  };

  const runtimePreflightGateLabel = (preflight: RuntimePreflightReport) => {
    const gate = preflight.current?.gate ?? preflight.policy.gate;
    const passed = preflight.current?.gate_passed;
    if (passed === true) return `${gate} ok`;
    if (passed === false) return `${gate} blocked`;
    return `${gate} pending`;
  };

  const runtimePreflightProbeLabel = (preflight: RuntimePreflightReport) => {
    const duration = preflight.preview.runtime_probe.duration_ms;
    return typeof duration === "number"
      ? `probe ${preflight.preview.probe_ok ? "ok" : "blocked"} ${runtimeDurationLabel(duration)}`
      : `probe ${preflight.preview.probe_ok ? "ok" : "blocked"}`;
  };

  const workerLifecycleStateLabel = (state: string) =>
    ({
      succeeded: "succeeded",
      blocked: "blocked",
      needs_review: "needs review",
      worker_failed: "worker failed",
      canceled: "canceled",
      running: "running",
      pending: "pending",
      observed: "observed",
    })[state] ?? state;

  const workerLifecycleTone = (state: string) =>
    state === "succeeded" ? "ok" : state === "pending" || state === "running" ? "pending" : "warn";

  const workerLifecycleWorkerState = (worker: WorkerLifecycleWorker | null | undefined) => {
    if (!worker) return "missing";
    if (worker.retry_exhausted) return "retry exhausted";
    if (worker.timed_out) return "timeout";
    if (worker.ok === true && worker.receipt_ok !== false) return "ok";
    if (worker.ok === false || worker.receipt_ok === false || worker.receipt_errors.length) return "blocked";
    return "observed";
  };

  const workerLifecycleRoleTone = (worker: WorkerLifecycleWorker | null | undefined) => {
    const state = workerLifecycleWorkerState(worker);
    return state === "ok" ? "ok" : state === "observed" ? "pending" : "warn";
  };

  const workerLifecycleReceiptLabel = (worker: WorkerLifecycleWorker | null | undefined) => {
    if (!worker) return "receipt missing";
    if (worker.receipt_ok === null) return "receipt pending";
    return worker.receipt_ok ? "receipt ok" : "receipt fail";
  };

  const workerLifecycleAttemptLabel = (worker: WorkerLifecycleWorker | null | undefined) => {
    if (worker?.attempt_count === null || worker?.attempt_count === undefined) return null;
    return worker.max_attempts
      ? `attempts ${worker.attempt_count}/${worker.max_attempts}`
      : `attempts ${worker.attempt_count}`;
  };

  const workerLifecycleDurationLabel = (worker: WorkerLifecycleWorker | null | undefined) =>
    worker?.duration_ms === null || worker?.duration_ms === undefined
      ? null
      : runtimeDurationLabel(worker.duration_ms);

  const workerLifecycleBudgetLabel = (lifecycle: WorkerLifecycleReport) =>
    `review budget ${lifecycle.current.reviewer_invalid_rounds_used}/${lifecycle.policy.reviewer_invalid_round_budget}`;

  const workerLifecycleRoundLabel = (round: WorkerLifecycleRound) => {
    const workers = round.worker_count ? ` ${round.worker_ok_count}/${round.worker_count}` : "";
    const decision = round.decision ? ` ${round.decision}` : "";
    const warn = round.worker_timeout_count || round.worker_retry_exhausted_count ? " retry" : "";
    return `r${round.round} ${round.status}${decision}${workers}${warn}`;
  };

  const workerLifecycleWorkerForRole = (round: WorkerLifecycleRound, role: string) =>
    round.workers.find((worker) => worker.role === role) ?? null;

  const roundHistoryLabel = (card: IssueCard) =>
    card.trace?.rounds.length
      ? card.trace.rounds
          .map((round) => {
            const workers = round.worker_count ? ` ${round.worker_ok_count}/${round.worker_count}` : "";
            const receipts = round.receipt_required_count
              ? ` r${round.receipt_required_count - round.receipt_missing_count}/${round.receipt_required_count}`
              : "";
            const warn = round.worker_timeout_count || round.worker_retry_exhausted_count ? " timeout" : "";
            return `r${round.round} ${round.status}${workers}${receipts}${warn}`;
          })
          .join(" | ")
      : null;

  const roundRecoveryLabel = (card: IssueCard) => {
    const rounds = card.trace?.rounds ?? [];
    if (card.issue.status !== "Done" || !rounds.length) return null;
    const current = card.trace?.current_round ?? 0;
    const failed = rounds
      .filter((round) => round.round < current)
      .filter(
        (round) =>
          round.status !== "kept" &&
          (round.rejected_count ||
            round.receipt_missing_count ||
            round.worker_timeout_count ||
            round.worker_retry_exhausted_count),
      )
      .map((round) => round.round);
    return failed.length ? `recovered r${failed.join(",r")} -> r${current}` : null;
  };

  const cardDoctor = (card: IssueCard) => card.doctor;

  const cardAuditFailureDetails = (card: IssueCard) =>
    card.doctor?.audit_failure_details.length
      ? card.doctor.audit_failure_details
      : card.trace?.audit_failure_details ?? [];
  const connectorStatusTone = (connector?: IssueConnectorStatus | null) => {
    if (!connector) return "pending";
    if (connector.current) return "ok";
    return connector.publish_required ? "warn" : "pending";
  };
  const connectorStatusLabel = (connector?: IssueConnectorStatus | null) => {
    if (!connector) return "connector pending";
    if (connector.current) return "connector current";
    if (connector.publish_required) return "publish required";
    return "connector unknown";
  };
  const connectorCommentLabel = (connector?: IssueConnectorStatus | null) => {
    if (!connector) return null;
    const current = connector.current_comment_count;
    const remote = connector.remote_comment_count;
    if (current == null && remote == null) return null;
    return `comments ${remote ?? 0}/${current ?? 0}`;
  };
  const connectorReasonLabel = (connector?: IssueConnectorStatus | null) => {
    if (!connector || connector.current) return null;
    return connector.reason;
  };
  const connectorCheckSummary = (checks?: AdmissionCheck[] | null) => {
    if (!checks?.length) return null;
    const passed = checks.filter((check) => check.passed).length;
    return `${passed}/${checks.length}`;
  };
  const connectorCheckTitle = (checks?: AdmissionCheck[] | null) =>
    checks
      ?.map((check) => {
        const status = check.passed ? "ok" : "blocked";
        const owner = check.owner ? ` (${check.owner})` : "";
        const severity = check.severity ? ` ${check.severity}` : "";
        const evidence = check.required_evidence?.length
          ? ` evidence ${check.required_evidence.join(", ")}`
          : "";
        const policy = check.policy_summary ? ` policy ${check.policy_summary}` : "";
        return `${status}${severity} ${check.name}${owner}${evidence}${policy}${
          check.summary ? `: ${check.summary}` : ""
        }`;
      })
      .join(" | ") ?? "";
  const connectorCheckTone = (checks?: AdmissionCheck[] | null) =>
    checks?.some((check) => !check.passed) ? "warn" : "ok";
  const connectorCheckChip = (
    label: string,
    checks: AdmissionCheck[] | null | undefined,
    testId: string,
  ) => {
    const summary = connectorCheckSummary(checks);
    if (!summary) return null;
    return (
      <span
        class={`connector-check connector-check--${connectorCheckTone(checks)}`}
        data-testid={testId}
        title={connectorCheckTitle(checks)}
      >
        {label} {summary}
      </span>
    );
  };
  const connectorProviderCapabilityLabel = (provider: ConnectorProvider) => {
    const capabilities = [
      provider.supports_status ? "status" : null,
      provider.supports_publish ? "publish" : null,
      provider.supports_readback ? "readback" : null,
      provider.supports_admission ? "admit" : null,
    ].filter(Boolean);
    if (capabilities.length) return capabilities.join("/");
    if (provider.configured) return "configured/not active";
    if (provider.auth_required) return "auth missing";
    return "not active";
  };
  const connectorProviderAdmissionLabel = (provider: ConnectorProvider) => {
    const admission = connectorProviderAdmission(provider);
    if (!admission) return "admission pending";
    return admission.status === "ready" ? "admit ready" : "admit blocked";
  };
  const connectorProviderTitle = (provider: ConnectorProvider) => {
    const admission = connectorProviderAdmission(provider);
    if (!admission || !admission.blockers.length) return provider.notes;
    return `${provider.notes} Admission blockers: ${admission.blockers.join(", ")}`;
  };
  const connectorAdmissionCheckContract = () => connectorRegistry()?.admission.required_checks ?? [];
  const connectorAdmissionCheckRegistry = () => connectorRegistry()?.admission.check_registry ?? [];
  const connectorAdmissionCheckContractLabel = () => {
    const checks = connectorAdmissionCheckRegistry();
    if (checks.length) return `${checks.length} checks`;
    const fallback = connectorAdmissionCheckContract();
    return fallback.length ? `${fallback.length} checks` : "checks pending";
  };
  const connectorAdmissionCheckSpecTitle = (check: ConnectorAdmissionCheckSpec) => {
    const evidence = check.required_evidence.length
      ? ` evidence ${check.required_evidence.join(", ")}`
      : "";
    return `${check.severity} ${check.name} (${check.owner})${evidence}: ${check.summary}`;
  };
  const connectorAdmissionCheckContractTitle = () => {
    const specs = connectorAdmissionCheckRegistry();
    if (specs.length) return specs.map(connectorAdmissionCheckSpecTitle).join(" | ");
    const checks = connectorAdmissionCheckContract();
    return checks.length ? checks.join(" | ") : "Connector admission check contract pending";
  };
  const connectorProviderTone = (provider: ConnectorProvider) =>
    provider.status === "active" && provider.configured ? "active" : "planned";
  const connectorQueueProviderTone = (provider: ConnectorQueueProvider) =>
    provider.status === "active" && provider.configured ? "active" : "planned";
  const connectorQueueProviderTitle = (provider: ConnectorQueueProvider) => {
    const adapter = provider.adapter;
    const contract = adapter?.remote_contract;
    const parts = [provider.queue_command];
    if (adapter?.blockers?.length) {
      parts.push(`writer blockers: ${adapter.blockers.join(", ")}`);
    }
    if (contract) {
      const retry = contract.retry
        ? `, retry ${contract.retry.max_attempts ?? 1} attempts / ${
            contract.retry.base_backoff_ms ?? 0
          }ms ${contract.retry.backoff_strategy ?? "none"}`
        : "";
      parts.push(
        `remote contract: ${contract.remote_object_kind}, write ${contract.write.receipt_schema_version}, readback ${contract.readback.schema_version}${retry}`,
      );
    }
    return parts.join(" | ");
  };
  const connectorStatusStrip = (card: IssueCard, surface: string) =>
    card.connector ? (
      <div
        class={`connector-strip connector-strip--${connectorStatusTone(card.connector)}`}
        data-testid={`issue-connector-${surface}-${card.issue.id}`}
      >
        <strong>Connector</strong>
        <span>{connectorStatusLabel(card.connector)}</span>
        {connectorCommentLabel(card.connector) ? <span>{connectorCommentLabel(card.connector)}</span> : null}
        {connectorReasonLabel(card.connector) ? <span>{connectorReasonLabel(card.connector)}</span> : null}
        {connectorCheckChip(
          "readback",
          connectorQueueIssueById(card.issue.id)?.checks ?? card.connector.checks,
          `connector-readback-checks-${surface}-${card.issue.id}`,
        )}
        {connectorCheckChip(
          "admit",
          connectorQueueIssueById(card.issue.id)?.admission_checks,
          `connector-admission-checks-${surface}-${card.issue.id}`,
        )}
        {connectorRemoteTargetChip(
          connectorQueueIssueTarget(card.issue.id),
          `connector-target-${surface}-${card.issue.id}`,
        )}
        {connectorRemoteWritePlanChip(
          connectorQueueIssueWritePlan(card.issue.id),
          `connector-write-plan-${surface}-${card.issue.id}`,
        )}
        {connectorRemoteDiagnosticChips(
          connectorRemoteDiagnostics(card.issue.id),
          `connector-remote-signal-${surface}-${card.issue.id}`,
        )}
      </div>
    ) : null;
  const loopAuditCommand = (card: IssueCard) =>
    card.issue.loop_id ? `entrance hive loop audit ${card.issue.loop_id} --compact` : null;
  const loopEvidenceCommand = (card: IssueCard) =>
    card.issue.loop_id ? `entrance hive loop evidence ${card.issue.loop_id}` : null;
  const issueMirrorCommand = (card: IssueCard) =>
    `entrance hive issue mirror ${card.issue.id} --compact`;
  const issueMirrorSyncCommand = (card: IssueCard) =>
    `entrance hive issue mirror-sync ${card.issue.id}`;
  const issueMirrorPublishCommand = (card: IssueCard) =>
    `entrance hive issue mirror-publish ${card.issue.id} --compact`;
  const issueMirrorVerifyCommand = (card: IssueCard) =>
    `entrance hive issue mirror-audit ${card.issue.id} --compact`;
  const issueMirrorReadbackCommand = (card: IssueCard) =>
    `entrance hive issue mirror-readback ${card.issue.id} --record --compact`;
  const issueMirrorAdmitCommand = (card: IssueCard) =>
    `entrance hive issue mirror-admit ${card.issue.id} --record --compact`;
  const issueMirrorRoundtripCommand = (card: IssueCard) =>
    `entrance hive issue mirror-roundtrip ${card.issue.id} --compact`;
  const admissionCheckLabel = (checks?: AdmissionCheck[] | null) => {
    if (!checks?.length) return null;
    const passed = checks.filter((check) => check.passed).length;
    return `${passed}/${checks.length} checks`;
  };
  const issueMirrorSyncLabel = (card: IssueCard) =>
    issuePendingLabel(card.issue.id) === "Syncing" ? "Syncing" : "Sync";
  const issueMirrorPublishLabel = (card: IssueCard) =>
    issuePendingLabel(card.issue.id) === "Publishing" ? "Publishing" : "Publish";
  const issueMirrorVerifyLabel = (card: IssueCard) =>
    issuePendingLabel(card.issue.id) === "Verifying" ? "Verifying" : "Verify";
  const issueMirrorReadbackLabel = (card: IssueCard) =>
    issuePendingLabel(card.issue.id) === "Reading" ? "Reading" : "Readback";
  const issueMirrorAdmitLabel = (card: IssueCard) =>
    issuePendingLabel(card.issue.id) === "Admitting" ? "Admitting" : "Admit";
  const issueMirrorRoundtripLabel = (card: IssueCard) =>
    issuePendingLabel(card.issue.id) === "Roundtrip" ? "Running" : "Roundtrip";
  const connectorPublishPlanLabel = () =>
    connectorPublishAction() === "Planning" ? "Planning" : "Plan";
  const connectorPublishExecuteLabel = () =>
    connectorPublishAction() === "Executing" ? "Executing" : "Execute";
  const connectorRoundtripPlanLabel = () =>
    connectorRoundtripAction() === "Planning" ? "Planning" : "Plan RT";
  const connectorRoundtripExecuteLabel = () =>
    connectorRoundtripAction() === "Executing" ? "Running" : "Run RT";
  const connectorFixtureDemoLabel = () => pendingFixtureAction() ?? "Run Fixture";

  const compactAuditFailureDetail = (detail: string) => {
    const parts = detail.split(":").filter(Boolean);
    if (parts.length < 2) return detail;
    return `${parts[0]} / ${parts[parts.length - 1]}`;
  };

  const issueAuditQuickActions = (card: IssueCard) => {
    const auditCommand = loopAuditCommand(card);
    const evidenceCommand = loopEvidenceCommand(card);
    if (!auditCommand || !evidenceCommand) return null;
    return (
      <div class="audit-preview-actions">
        <button
          type="button"
          aria-label={`Copy compact audit for issue #${card.issue.id}`}
          data-testid={`issue-audit-copy-${card.issue.id}`}
          title={auditCommand}
          onClick={() => void copyCommandAction("audit command", auditCommand)}
        >
          Audit
        </button>
        <button
          type="button"
          aria-label={`Copy evidence command for issue #${card.issue.id}`}
          data-testid={`issue-evidence-copy-${card.issue.id}`}
          title={evidenceCommand}
          onClick={() => void copyCommandAction("evidence command", evidenceCommand)}
        >
          Evidence
        </button>
      </div>
    );
  };

  const issueDetailRows = (card: IssueCard) => {
    const trace = card.trace;
    return [
      ["Status", card.issue.status],
      ["Loop", card.issue.loop_id ? `#${card.issue.loop_id}` : "unlinked"],
      ["Round", trace ? String(trace.current_round) : "pending"],
      [
        "Packets",
        trace ? `${trace.round_packet_count}/${trace.packet_count}` : "pending",
      ],
      [
        "Admissions",
        trace ? `${trace.round_admission_count}/${trace.admission_count}` : "pending",
      ],
      ["Evidence", trace ? `${trace.round_evidence_count}/${trace.evidence_count}` : "pending"],
      ["Verdicts", trace ? `${trace.round_verdict_count}/${trace.verdict_count}` : "pending"],
      [
        "Receipts",
        trace
          ? `${trace.round_receipt_required_count - trace.round_receipt_missing_count}/${trace.round_receipt_required_count}`
          : "pending",
      ],
      [
        "Workers",
        trace ? `${trace.round_role_worker_ok_count}/${trace.round_role_worker_count}` : "pending",
      ],
      ["Runtime", trace ? runtimeDurationLabel(trace.round_worker_duration_ms) : "pending"],
      [
        "Timeouts",
        trace
          ? `${trace.round_worker_timeout_count} timeout / ${trace.round_worker_retry_exhausted_count} exhausted`
          : "pending",
      ],
      ["Gate", trace?.last_admission_gate ?? "pending"],
      ["Gate Rule", trace?.last_gate_description ?? "pending"],
      ["Gate Object", trace?.last_gate_expected_object_kind ?? "pending"],
      ["Decision", trace?.last_decision ?? "pending"],
      ["Reason", trace?.reason_code ?? "pending"],
      ["Scores", trace ? scoreVectorLabel(trace) ?? "pending" : "pending"],
      [
        "Options",
        trace?.human_options.length ? trace.human_options.join(", ") : "pending",
      ],
      [
        "Operator",
        operatorEventLabel(trace?.last_operator_event ?? null) ?? "none",
      ],
      [
        "Operator Events",
        trace
          ? `${trace.round_operator_event_count}/${trace.operator_event_count}`
          : "pending",
      ],
      ["Packet", schemaLabel(trace?.packet_schema)],
      ["Policy", schemaLabel(trace?.policy_schema)],
      ["Admission", schemaLabel(trace?.admission_schema)],
      ["Verdict", schemaLabel(trace?.verdict_schema)],
      ["Audit", trace ? auditLabel(trace) : "pending"],
      ["Audit Schema", schemaLabel(trace?.audit_schema)],
      [
        "Audit Fails",
        trace?.audit_failed_checks.length ? trace.audit_failed_checks.join(", ") : "none",
      ],
      [
        "Audit Details",
        trace?.audit_failure_details.length ? trace.audit_failure_details.join(", ") : "none",
      ],
      ["Worker", workerLabel(card) ?? "pending"],
    ];
  };
  const storeSchemaLabel = () => {
    const schema = status()?.schema;
    if (!schema) return "pending";
    const tables = schema.tables.filter((table) => table.present).length;
    const indexes = schema.indexes.filter((index) => index.present).length;
    return `${schema.healthy ? "ok" : "blocked"} v${schema.user_version}/${schema.expected_user_version} tables ${tables}/${schema.tables.length} indexes ${indexes}/${schema.indexes.length}`;
  };
  const storeSchemaTitle = () => {
    const schema = status()?.schema;
    if (!schema) return "Store schema pending";
    const missing = [
      ...schema.missing_tables.map((value) => `table:${value}`),
      ...schema.missing_columns.map((value) => `column:${value}`),
      ...schema.missing_indexes.map((value) => `index:${value}`),
    ];
    return missing.length ? missing.join(" | ") : schema.schema_version;
  };

  return (
    <div class="app-shell">
      <Nav current={view()} onSelect={setView} />

      <main class="main-shell">
        <header class="hero-panel">
          <div>
            <p class="hero-kicker">Refactor target</p>
            <h2>Core / Plugins / Shell</h2>
            <p class="hero-copy">
              This GUI talks only to the unified `entrance daemon` protocol.
            </p>
          </div>
          <button type="button" class="hero-action" onClick={() => void refreshAll()}>
            Refresh
          </button>
        </header>

        {banner() ? <p class="banner">{banner()}</p> : null}

        <Switch>
          <Match when={view() === "status"}>
            <section class="panel-grid panel-grid--status">
              <article class="panel">
                <p class="panel-kicker">Kernel</p>
                <h3>Runtime status</h3>
                <dl class="metric-list">
                  <div><dt>App root</dt><dd>{status()?.app_root ?? "..."}</dd></div>
                  <div><dt>Database</dt><dd>{status()?.db_path ?? "..."}</dd></div>
                  <div><dt>Schema</dt><dd title={storeSchemaTitle()}>{storeSchemaLabel()}</dd></div>
                  <div><dt>Drawer</dt><dd>{status()?.drawer_entries ?? 0}</dd></div>
                  <div><dt>Hive</dt><dd>{status()?.hive_runs ?? 0}</dd></div>
                  <div><dt>Loops</dt><dd>{status()?.hive_loops ?? 0}</dd></div>
                  <div><dt>Launcher</dt><dd>{status()?.launcher_entries ?? 0}</dd></div>
                </dl>
              </article>

              <article class="panel">
                <p class="panel-kicker">Drawer</p>
                <h3>Storage mode</h3>
                <p class="big-copy">{drawerSummary()?.mode ?? "..."}</p>
                <p class="muted">{drawerSummary()?.root ?? "..."}</p>
              </article>

              <article class="panel">
                <p class="panel-kicker">Identity</p>
                <h3>Microkernel cutover</h3>
                <p class="muted">
                  Runtime, daemon, and GUI now share the same single-binary command contract.
                </p>
              </article>
            </section>
          </Match>

          <Match when={view() === "drawer"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Drawer</p>
                <h3>Add note</h3>
                <input
                  value={drawerTitle()}
                  onInput={(event) => setDrawerTitle(event.currentTarget.value)}
                  placeholder="Title"
                />
                <textarea
                  value={drawerBody()}
                  onInput={(event) => setDrawerBody(event.currentTarget.value)}
                  placeholder="Write a note for the drawer"
                />
                <button type="button" class="primary-button" onClick={() => void addDrawerNote()}>
                  Create Note
                </button>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Items</p>
                <h3>Stored entries</h3>
                <ul class="record-list">
                  {(drawerItems() ?? []).map((item) => (
                    <li class="record-card">
                      <strong>{item.title}</strong>
                      <span>{item.kind}</span>
                      <code>{item.storage_path ?? "db-only"}</code>
                    </li>
                  ))}
                </ul>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Versioning</p>
                <h3>Drawer history</h3>
                <ul class="record-list">
                  {(drawerHistory()?.commits ?? []).map((commit) => (
                    <li class="record-card">
                      <strong>{commit.summary}</strong>
                      <code>{commit.id}</code>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>

          <Match when={view() === "hive"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Hive</p>
                <h3>Dispatch</h3>
                <input
                  value={hiveTitle()}
                  onInput={(event) => setHiveTitle(event.currentTarget.value)}
                  placeholder="Task title"
                />
                <input
                  value={hiveProject()}
                  onInput={(event) => setHiveProject(event.currentTarget.value)}
                  placeholder="Project path (optional)"
                />
                <button type="button" class="primary-button" onClick={() => void dispatchHive()}>
                  Persist Dispatch
                </button>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Runs</p>
                <h3>Dispatch ledger</h3>
                <p class="muted">
                  Ready {hiveSummary()?.ready_runs ?? 0} / Total {hiveSummary()?.total_runs ?? 0}
                </p>
                <ul class="record-list">
                  {(hiveRuns() ?? []).map((run) => (
                    <li class="record-card">
                      <strong>{run.title}</strong>
                      <span>{run.status}</span>
                      <code>{run.project_dir ?? "no project"}</code>
                    </li>
                  ))}
                </ul>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Loops</p>
                <h3>Contracts</h3>
                <ul class="record-list">
                  {(hiveLoops() ?? []).map((loop) => (
                    <li class="record-card">
                      <div class="record-head">
                        <strong>{loop.title}</strong>
                        <span>{loop.status}</span>
                      </div>
                      <span>{loop.active_phase} / round {loop.current_round}</span>
                      <code>{loop.runtime}</code>
                      {loop.status === "todo" ? (
                        <div class="record-actions">
                          <button
                            type="button"
                            disabled={Boolean(loopPendingLabel(loop.id))}
                            onClick={() => void runHiveLoop(loop)}
                          >
                            {loopPendingLabel(loop.id) ?? "Run"}
                          </button>
                        </div>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>

          <Match when={view() === "panel"}>
            <section class="panel-grid panel-grid--board">
              <div class="panel-stack">
                <article class="panel panel--form">
                  <p class="panel-kicker">Loop</p>
                  <h3>Contract</h3>
                  <input
                    aria-label="Loop title"
                    value={loopTitle()}
                    onInput={(event) => setLoopTitle(event.currentTarget.value)}
                    placeholder="Title"
                  />
                  <textarea
                    aria-label="Loop goal"
                    value={loopGoal()}
                    onInput={(event) => setLoopGoal(event.currentTarget.value)}
                    placeholder="Goal"
                  />
                  <select
                    aria-label="Loop runtime"
                    value={loopRuntime()}
                    onChange={(event) => setLoopRuntime(event.currentTarget.value)}
                  >
                    <option value="codex">codex</option>
                    <option value="local">local</option>
                  </select>
                  <input
                    aria-label="Worker timeout seconds"
                    type="number"
                    min="1"
                    value={loopWorkerTimeoutSecs()}
                    onInput={(event) => setLoopWorkerTimeoutSecs(event.currentTarget.value)}
                    placeholder="Worker timeout seconds"
                  />
                  <input
                    aria-label="Worker attempts"
                    type="number"
                    min="1"
                    max="3"
                    value={loopWorkerAttempts()}
                    onInput={(event) => setLoopWorkerAttempts(event.currentTarget.value)}
                    placeholder="Worker attempts"
                  />
                  <div class="form-actions">
                    <button
                      type="button"
                      data-testid="panel-run-demo"
                      disabled={Boolean(pendingDemoAction())}
                      onClick={() => void startDemoLoop()}
                    >
                      {pendingDemoAction() ?? "Run Demo"}
                    </button>
                    <button
                      type="button"
                      data-testid="panel-run-fixture-demo"
                      disabled={Boolean(pendingFixtureAction())}
                      title="entrance hive connector fixture-demo --compact"
                      onClick={() => void runConnectorFixtureDemo()}
                    >
                      {connectorFixtureDemoLabel()}
                    </button>
                    <button type="button" onClick={() => void createHiveLoop()}>
                      Create Loop
                    </button>
                  </div>
                </article>

                <article
                  class="panel panel--detail"
                  ref={(element) => {
                    issueDetailPanel = element;
                  }}
                >
                  <p class="panel-kicker">Issue</p>
                  <Show
                    when={selectedIssueCard()}
                    keyed
                    fallback={
                      <div class="empty-state">
                        <span>No issues</span>
                        <button
                          type="button"
                          data-testid="issue-detail-run-demo"
                          disabled={Boolean(pendingDemoAction())}
                          onClick={() => void startDemoLoop()}
                        >
                          {pendingDemoAction() ?? "Run Demo"}
                        </button>
                        <button
                          type="button"
                          data-testid="issue-detail-run-fixture-demo"
                          disabled={Boolean(pendingFixtureAction())}
                          title="entrance hive connector fixture-demo --compact"
                          onClick={() => void runConnectorFixtureDemo()}
                        >
                          {connectorFixtureDemoLabel()}
                        </button>
                      </div>
                    }
                  >
                    {(card) => (
                      <>
                        <h3>{card.issue.title}</h3>
                        <p class="muted">{card.issue.summary ?? "No summary"}</p>
                        {connectorStatusStrip(card, "detail")}
                        {connectorRemoteAttemptDetails(
                          connectorRemoteDiagnostics(card.issue.id),
                          `connector-remote-attempts-detail-${card.issue.id}`,
                        )}
                        <Show when={selectedIssueDoctor()} keyed>
                          {(doctor) => (
                            <div class={`doctor-summary doctor-summary--${doctorHealthTone(doctor.health)}`}>
                              <div class="stage-row-head">
                                <strong>Doctor</strong>
                                <span>{doctorHealthLabel(doctor.health)}</span>
                              </div>
                              <p>{doctor.summary}</p>
                              <div class="trace-strip">
                                <span class="trace-pill">{schemaLabel(doctor.schema_version)}</span>
                                <span class="trace-pill">round {doctor.current_round}</span>
                                <Show when={roundHistoryLabel(card)}>
                                  {(label) => (
                                    <span class="trace-pill" title={label()}>
                                      rounds {card.trace?.rounds.length ?? 0}
                                    </span>
                                  )}
                                </Show>
                                <Show when={roundRecoveryLabel(card)}>
                                  {(label) => <span class="trace-pill trace-pill--ok">{label()}</span>}
                                </Show>
                                <span class="trace-pill">{doctorWorkerLabel(doctor)}</span>
                                <span class="trace-pill">{doctorRuntimeLabel(doctor)}</span>
                                {doctor.counts.round_worker_timeout_count ||
                                doctor.counts.round_worker_retry_exhausted_count ? (
                                  <span class="trace-pill trace-pill--warn">
                                    {doctor.counts.round_worker_timeout_count} timeout /{" "}
                                    {doctor.counts.round_worker_retry_exhausted_count} exhausted
                                  </span>
                                ) : null}
                                <span
                                  class={
                                    doctor.counts.round_receipt_missing_count === 0
                                      ? "trace-pill"
                                      : "trace-pill trace-pill--warn"
                                  }
                                >
                                  {doctorReceiptLabel(doctor)}
                                </span>
                                <span
                                  class={
                                    doctor.counts.audit_failed_count === 0
                                      ? "trace-pill"
                                      : "trace-pill trace-pill--warn"
                                  }
                                >
                                  audit {doctor.counts.audit_failed_count}
                                </span>
                              </div>
                              {doctor.failed_checks.length ||
                              doctor.audit_failure_details.length ||
                              doctor.missing_receipts.length ||
                              doctor.worker_failures.length ? (
                                <div class="doctor-lines">
                                  {doctor.failed_checks.map((check) => (
                                    <span>check {check}</span>
                                  ))}
                                  {doctor.audit_failure_details.map((detail) => (
                                    <span>detail {detail}</span>
                                  ))}
                                  {doctor.missing_receipts.map((receipt) => (
                                    <span>missing {receipt}</span>
                                  ))}
                                  {doctor.worker_failures.map((failure) => (
                                    <span>{failure}</span>
                                  ))}
                                </div>
                              ) : null}
                              {doctor.next_actions.length ? (
                                <div class="doctor-actions">
                                  {doctor.next_actions.slice(0, 3).map((action, index) => (
                                    <div class="doctor-action-row">
                                      <code>{action}</code>
                                      <button
                                        type="button"
                                        aria-label={`Copy doctor action ${action}`}
                                        data-testid={`doctor-action-copy-${card.issue.id}-${index}`}
                                        onClick={() => void copyDoctorAction(action)}
                                      >
                                        Copy
                                      </button>
                                    </div>
                                  ))}
                                </div>
                              ) : null}
                            </div>
                          )}
                        </Show>
                        {card.issue.loop_id ? (
                          <Show
                            when={selectedIssueRuntimePreflight()}
                            keyed
                            fallback={
                              <div
                                class="worker-lifecycle runtime-preflight worker-lifecycle--pending"
                                data-testid={`runtime-preflight-detail-${card.issue.id}`}
                              >
                                <div class="stage-row-head">
                                  <strong>Runtime Preflight</strong>
                                  <span>
                                    {selectedRuntimePreflight.loading ? "loading" : "pending"}
                                  </span>
                                </div>
                                <div class="trace-strip">
                                  <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                  <span class="trace-pill">runtime_preflight.v1</span>
                                </div>
                              </div>
                            }
                          >
                            {(preflight) => (
                              <div
                                class={`worker-lifecycle runtime-preflight worker-lifecycle--${runtimePreflightTone(
                                  preflight.preflight_state,
                                )}`}
                                data-testid={`runtime-preflight-detail-${card.issue.id}`}
                              >
                                <div class="stage-row-head">
                                  <strong>Runtime Preflight</strong>
                                  <span>{runtimePreflightStateLabel(preflight.preflight_state)}</span>
                                </div>
                                <p>{preflight.summary}</p>
                                <div class="trace-strip">
                                  <span class="trace-pill">{schemaLabel(preflight.schema_version)}</span>
                                  <span class="trace-pill">loop #{preflight.loop_id}</span>
                                  <span class="trace-pill">round {preflight.current_round}</span>
                                  <span class="trace-pill">{preflight.runtime}</span>
                                  <span class="trace-pill">
                                    {preflight.policy.route_from}
                                    {" -> "}
                                    {preflight.policy.route_to}
                                  </span>
                                  <span class="trace-pill">{preflight.policy.object_kind}</span>
                                  <span
                                    class={
                                      preflight.current?.gate_passed === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {runtimePreflightGateLabel(preflight)}
                                  </span>
                                  <span
                                    class={
                                      preflight.preview.supported
                                        ? "trace-pill"
                                        : "trace-pill trace-pill--warn"
                                    }
                                  >
                                    {runtimePreflightBoolLabel("policy", preflight.preview.supported)}
                                  </span>
                                  <span
                                    class={
                                      preflight.preview.probe_ok
                                        ? "trace-pill"
                                        : "trace-pill trace-pill--warn"
                                    }
                                  >
                                    {runtimePreflightProbeLabel(preflight)}
                                  </span>
                                  {preflight.current?.result ? (
                                    <span
                                      class={
                                        preflight.current.result === "rejected"
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      {preflight.current.result}
                                    </span>
                                  ) : null}
                                  {preflight.current?.receipt_missing.length ? (
                                    <span class="trace-pill trace-pill--warn">
                                      missing {preflight.current.receipt_missing.join(", ")}
                                    </span>
                                  ) : null}
                                  {preflight.preview.blocker ? (
                                    <span class="trace-pill trace-pill--warn">
                                      {preflight.preview.blocker}
                                    </span>
                                  ) : null}
                                </div>
                                <div class="worker-lifecycle-rounds">
                                  {preflight.policy.supported_runtimes.map((runtime) => (
                                    <span
                                      class={
                                        runtime === preflight.runtime
                                          ? "trace-pill trace-pill--ok"
                                          : "trace-pill"
                                      }
                                    >
                                      {runtime}
                                    </span>
                                  ))}
                                </div>
                                {preflight.failures.length ? (
                                  <div class="doctor-lines">
                                    {preflight.failures.map((failure) => (
                                      <span>{failure}</span>
                                    ))}
                                  </div>
                                ) : null}
                                {preflight.next_actions.length ? (
                                  <div class="doctor-actions">
                                    {preflight.next_actions.slice(0, 2).map((action, index) => (
                                      <div class="doctor-action-row">
                                        <code>{action}</code>
                                        <button
                                          type="button"
                                          aria-label={`Copy runtime preflight action ${action}`}
                                          data-testid={`runtime-preflight-action-copy-${card.issue.id}-${index}`}
                                          onClick={() => void copyDoctorAction(action)}
                                        >
                                          Copy
                                        </button>
                                      </div>
                                    ))}
                                  </div>
                                ) : null}
                              </div>
                            )}
                          </Show>
                        ) : null}
                        {card.issue.loop_id ? (
                          <Show
                            when={selectedIssueWorkerLifecycle()}
                            keyed
                            fallback={
                              <div
                                class="worker-lifecycle worker-lifecycle--pending"
                                data-testid={`worker-lifecycle-detail-${card.issue.id}`}
                              >
                                <div class="stage-row-head">
                                  <strong>Worker Lifecycle</strong>
                                  <span>
                                    {selectedWorkerLifecycle.loading ? "loading" : "pending"}
                                  </span>
                                </div>
                                <div class="trace-strip">
                                  <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                  <span class="trace-pill">worker_lifecycle.v1</span>
                                </div>
                              </div>
                            }
                          >
                            {(lifecycle) => (
                              <div
                                class={`worker-lifecycle worker-lifecycle--${workerLifecycleTone(
                                  lifecycle.lifecycle_state,
                                )}`}
                                data-testid={`worker-lifecycle-detail-${card.issue.id}`}
                              >
                                <div class="stage-row-head">
                                  <strong>Worker Lifecycle</strong>
                                  <span>{workerLifecycleStateLabel(lifecycle.lifecycle_state)}</span>
                                </div>
                                <p>{lifecycle.summary}</p>
                                <div class="trace-strip">
                                  <span class="trace-pill">{schemaLabel(lifecycle.schema_version)}</span>
                                  <span class="trace-pill">loop #{lifecycle.loop_id}</span>
                                  <span class="trace-pill">round {lifecycle.current_round}</span>
                                  <span class="trace-pill">{lifecycle.runtime}</span>
                                  <span
                                    class={
                                      lifecycle.current.reviewer_invalid_budget_exhausted
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {workerLifecycleBudgetLabel(lifecycle)}
                                  </span>
                                  <span class="trace-pill">fallback {lifecycle.policy.fallback_status}</span>
                                  <span class="trace-pill">{lifecycle.current.worker_ok_count}/{lifecycle.current.worker_count} workers</span>
                                  {lifecycle.current.missing_roles.length ? (
                                    <span class="trace-pill trace-pill--warn">
                                      missing {lifecycle.current.missing_roles.join(", ")}
                                    </span>
                                  ) : null}
                                  {lifecycle.current.worker_timeout_count ||
                                  lifecycle.current.worker_retry_exhausted_count ? (
                                    <span class="trace-pill trace-pill--warn">
                                      {lifecycle.current.worker_timeout_count} timeout /{" "}
                                      {lifecycle.current.worker_retry_exhausted_count} exhausted
                                    </span>
                                  ) : null}
                                </div>
                                <div class="worker-lifecycle-roles">
                                  {lifecycle.current.expected_roles.map((role) => {
                                    const worker = workerLifecycleWorkerForRole(lifecycle.current, role);
                                    return (
                                      <div
                                        class={`worker-lifecycle-role worker-lifecycle-role--${workerLifecycleRoleTone(
                                          worker,
                                        )}`}
                                        data-testid={`worker-lifecycle-role-${card.issue.id}-${role}`}
                                      >
                                        <div class="stage-row-head">
                                          <strong>{role}</strong>
                                          <span>{workerLifecycleWorkerState(worker)}</span>
                                        </div>
                                        <p>
                                          {worker?.evidence_summary ??
                                            worker?.action ??
                                            "No worker receipt"}
                                        </p>
                                        <div class="trace-strip">
                                          <span
                                            class={
                                              workerLifecycleWorkerState(worker) === "ok"
                                                ? "trace-pill"
                                                : "trace-pill trace-pill--warn"
                                            }
                                          >
                                            {worker?.kind ?? "missing"}
                                          </span>
                                          <span
                                            class={
                                              worker?.receipt_ok === false || !worker
                                                ? "trace-pill trace-pill--warn"
                                                : "trace-pill"
                                            }
                                          >
                                            {workerLifecycleReceiptLabel(worker)}
                                          </span>
                                          {worker?.evidence_kind ? (
                                            <span class="trace-pill">{worker.evidence_kind}</span>
                                          ) : null}
                                          {worker?.mode ? <span class="trace-pill">{worker.mode}</span> : null}
                                          {workerLifecycleDurationLabel(worker) ? (
                                            <span class="trace-pill">{workerLifecycleDurationLabel(worker)}</span>
                                          ) : null}
                                          {worker?.timeout_secs !== null && worker?.timeout_secs !== undefined ? (
                                            <span class="trace-pill">limit {worker.timeout_secs}s</span>
                                          ) : null}
                                          {workerLifecycleAttemptLabel(worker) ? (
                                            <span class="trace-pill">{workerLifecycleAttemptLabel(worker)}</span>
                                          ) : null}
                                          {worker?.action ? <span class="trace-pill">{worker.action}</span> : null}
                                          {worker?.gate_count !== null && worker?.gate_count !== undefined ? (
                                            <span class="trace-pill">gates {worker.gate_count}</span>
                                          ) : null}
                                          {worker?.timed_out ? (
                                            <span class="trace-pill trace-pill--warn">timeout</span>
                                          ) : null}
                                          {worker?.retry_exhausted ? (
                                            <span class="trace-pill trace-pill--warn">retry exhausted</span>
                                          ) : null}
                                          {worker?.receipt_errors.map((field) => (
                                            <span class="trace-pill trace-pill--warn">receipt {field}</span>
                                          ))}
                                        </div>
                                        {worker?.transcript_excerpt ? (
                                          <p class="muted">{worker.transcript_excerpt}</p>
                                        ) : null}
                                      </div>
                                    );
                                  })}
                                </div>
                                <div class="worker-lifecycle-rounds">
                                  {lifecycle.rounds.map((round) => (
                                    <span
                                      class={
                                        round.failures.length ||
                                        round.worker_timeout_count ||
                                        round.worker_retry_exhausted_count ||
                                        round.reviewer_invalid_budget_exhausted
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                      title={round.failures.join(" | ") || undefined}
                                    >
                                      {workerLifecycleRoundLabel(round)}
                                    </span>
                                  ))}
                                </div>
                                {lifecycle.failures.length ? (
                                  <div class="doctor-lines">
                                    {lifecycle.failures.map((failure) => (
                                      <span>{failure}</span>
                                    ))}
                                  </div>
                                ) : null}
                                {lifecycle.next_actions.length ? (
                                  <div class="doctor-actions">
                                    {lifecycle.next_actions.slice(0, 2).map((action, index) => (
                                      <div class="doctor-action-row">
                                        <code>{action}</code>
                                        <button
                                          type="button"
                                          aria-label={`Copy worker lifecycle action ${action}`}
                                          data-testid={`worker-lifecycle-action-copy-${card.issue.id}-${index}`}
                                          onClick={() => void copyDoctorAction(action)}
                                        >
                                          Copy
                                        </button>
                                      </div>
                                    ))}
                                  </div>
                                ) : null}
                              </div>
                            )}
                          </Show>
                        ) : null}
                        {issueHumanActions(card).length ? (
                          <div class="decision-options">
                            {card.issue.loop_id && card.issue.status === "Todo" ? (
                              <button
                                type="button"
                                aria-label={issueRuntimeActionAriaLabel(card, false, "detail")}
                                {...issueActionButtonAttrs(issueActionByName(card, "run"))}
                                data-testid={`issue-action-detail-run-${card.issue.id}`}
                                disabled={Boolean(issuePendingLabel(card.issue.id))}
                                onClick={() => void runIssueLoop(card)}
                              >
                                {issueRuntimeActionLabel(card, false)}
                              </button>
                            ) : null}
                            {issueHumanActions(card).map((action) => (
                              <button
                                type="button"
                                aria-label={
                                  action.action === "retry"
                                    ? issueRuntimeActionAriaLabel(card, true, "detail")
                                    : `${action.label} issue #${card.issue.id} from detail`
                                }
                                data-testid={`issue-action-detail-${action.action}-${card.issue.id}`}
                                disabled={issueOptionDisabled(card, action)}
                                onClick={() => runIssueAction(card, action)}
                                {...issueActionButtonAttrs(action)}
                              >
                                {issueDecisionButtonLabel(card, action)}
                              </button>
                            ))}
                            <button
                              type="button"
                              aria-label={`Copy issue #${card.issue.id} mirror command from detail`}
                              data-testid={`issue-action-detail-mirror-${card.issue.id}`}
                              title={issueMirrorCommand(card)}
                              onClick={() => void copyCommandAction("issue mirror", issueMirrorCommand(card))}
                            >
                              Mirror
                            </button>
                            <button
                              type="button"
                              aria-label={`Sync issue #${card.issue.id} mirror to file from detail`}
                              data-testid={`issue-action-detail-sync-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              title={issueMirrorSyncCommand(card)}
                              onClick={() => void syncIssueMirror(card)}
                            >
                              {issueMirrorSyncLabel(card)}
                            </button>
                            <button
                              type="button"
                              aria-label={`Publish issue #${card.issue.id} mirror to connector from detail`}
                              data-testid={`issue-action-detail-publish-${card.issue.id}`}
                              disabled={
                                Boolean(issuePendingLabel(card.issue.id)) ||
                                !connectorQueueIssueCanPublish(card.issue.id)
                              }
                              title={connectorQueueIssuePublishTitle(card)}
                              onClick={() => void publishIssueMirror(card)}
                            >
                              {issueMirrorPublishLabel(card)}
                            </button>
                            <button
                              type="button"
                              aria-label={`Verify issue #${card.issue.id} mirror file from detail`}
                              data-testid={`issue-action-detail-verify-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              title={issueMirrorVerifyCommand(card)}
                              onClick={() => void verifyIssueMirror(card)}
                            >
                              {issueMirrorVerifyLabel(card)}
                            </button>
                            <button
                              type="button"
                              aria-label={`Read back issue #${card.issue.id} mirror file from detail`}
                              data-testid={`issue-action-detail-readback-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              title={issueMirrorReadbackCommand(card)}
                              onClick={() => void readbackIssueMirror(card)}
                            >
                              {issueMirrorReadbackLabel(card)}
                            </button>
                            <button
                              type="button"
                              aria-label={`Admit issue #${card.issue.id} mirror for connector from detail`}
                              data-testid={`issue-action-detail-admit-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              title={issueMirrorAdmitCommand(card)}
                              onClick={() => void admitIssueMirror(card)}
                            >
                              {issueMirrorAdmitLabel(card)}
                            </button>
                            <button
                              type="button"
                              aria-label={`Roundtrip issue #${card.issue.id} connector mirror from detail`}
                              data-testid={`issue-action-detail-roundtrip-${card.issue.id}`}
                              disabled={
                                Boolean(issuePendingLabel(card.issue.id)) ||
                                !connectorQueueIssueCanPublish(card.issue.id)
                              }
                              title={issueMirrorRoundtripCommand(card)}
                              onClick={() => void roundtripIssueMirror(card)}
                            >
                              {issueMirrorRoundtripLabel(card)}
                            </button>
                          </div>
                        ) : null}
                        {card.actions.length ? issueActionContractChips(card, "detail") : null}
                        {commentComposerActive(card.issue.id, "detail") ? (
                          <div class="comment-box comment-box--detail">
                            <textarea
                              aria-label={`Detail issue #${card.issue.id} comment`}
                              data-testid={`issue-comment-detail-${card.issue.id}`}
                              value={commentBody()}
                              onInput={(event) => setCommentBody(event.currentTarget.value)}
                              onKeyDown={(event) => handleCommentKeyDown(event, card.issue.id)}
                              placeholder="Comment"
                            />
                            <button
                              type="button"
                              aria-label={`Send detail issue #${card.issue.id} comment`}
                              data-testid={`issue-comment-detail-send-${card.issue.id}`}
                              disabled={commentSubmitDisabled(card.issue.id)}
                              onClick={() => void addIssueComment(card.issue.id)}
                            >
                              {issuePendingLabel(card.issue.id) ?? "Send"}
                            </button>
                            {issueDecisionActions(card).map((action) => (
                              <button
                                type="button"
                                aria-label={
                                  action.action === "retry"
                                    ? issueRuntimeActionAriaLabel(card, true, "detail composer")
                                    : `${action.label} issue #${card.issue.id} from detail composer`
                                }
                                data-testid={`issue-action-detail-composer-${action.action}-${card.issue.id}`}
                                disabled={issueOptionDisabled(card, action)}
                                onClick={() => runIssueAction(card, action)}
                                {...issueActionButtonAttrs(action)}
                              >
                                {issueDecisionButtonLabel(card, action)}
                              </button>
                            ))}
                            <button
                              type="button"
                              aria-label={`Close detail issue #${card.issue.id} comment`}
                              data-testid={`issue-comment-detail-close-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              onClick={() => closeIssueComment(card.issue.id)}
                            >
                              Close
                            </button>
                          </div>
                        ) : null}
                        <div class="comment-stack comment-stack--detail">
                          <h4>Comments</h4>
                          {card.comments.map((comment) => (
                            <div class="comment-line comment-line--detail">
                              <div class="comment-line-head">
                                <strong>{comment.author}</strong>
                                <div class="comment-tags">
                                  {commentPills(comment).map((pill) => commentPillNode(card.issue.id, pill))}
                                </div>
                              </div>
                              <span>{commentPreview(comment, COMMENT_DETAIL_PREVIEW_LIMIT)}</span>
                            </div>
                          ))}
                        </div>
                        <dl class="detail-grid">
                          {issueDetailRows(card).map(([label, value]) => (
                            <div>
                              <dt>{label}</dt>
                              <dd>{value}</dd>
                            </div>
                          ))}
                        </dl>
                        {card.trace?.operator_events.length ? (
                          <div class="operator-trail">
                            <h4>Operator Trail</h4>
                            {card.trace.operator_events.map((event) => (
                              <div class="operator-event">
                                <strong>{operatorEventLabel(event)}</strong>
                                <span>{operatorEventStatusLabel(event)}</span>
                                <p>{event.note ?? event.summary}</p>
                              </div>
                            ))}
                          </div>
                        ) : null}
                        {card.trace?.stages.length ? (
                          <div class="stage-timeline">
                            {card.trace.stages.map((stage) => (
                              <div class="stage-row">
                                <div class="stage-row-head">
                                  <strong>{stage.role}</strong>
                                  <span>{stage.status}</span>
                                </div>
                                <p>{stage.summary ?? "No stage summary"}</p>
                                <div class="trace-strip">
                                  <span class="trace-pill">{stage.evidence_kind ?? "evidence pending"}</span>
                                  <span class="trace-pill">{stage.admission_result ?? "admission pending"}</span>
                                  <span
                                    class={
                                      stage.worker_ok === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {stageWorkerLabel(stage)}
                                  </span>
                                </div>
                                <p class="muted">{stage.evidence_summary ?? "No evidence summary"}</p>
                              </div>
                            ))}
                          </div>
                        ) : null}
                        {card.trace?.evidence.length ? (
                          <div class="evidence-ledger">
                            <h4>Evidence</h4>
                            {card.trace.evidence.map((evidence) => (
                              <div
                                class={
                                  selectedEvidenceId() === evidence.id
                                    ? "evidence-row evidence-row--selected"
                                    : "evidence-row"
                                }
                                data-testid={`evidence-row-${evidence.id}`}
                                ref={(element) => evidenceRows.set(evidence.id, element)}
                              >
                                <div class="stage-row-head">
                                  <strong>{evidence.stage_role ?? evidence.kind}</strong>
                                  <span>#{evidence.id}</span>
                                </div>
                                <p>{evidence.summary}</p>
                                <div class="trace-strip">
                                  <span class="trace-pill">{evidence.kind}</span>
                                  <span class="trace-pill">{evidence.admission_result ?? "admission pending"}</span>
                                  <span
                                    class={
                                      evidence.worker_ok === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {evidenceWorkerLabel(evidence)}
                                  </span>
                                  {evidence.schema_version ? (
                                    <span class="trace-pill">{schemaLabel(evidence.schema_version)}</span>
                                  ) : null}
                                  {workerReceiptLabel(evidence) ? (
                                    <span
                                      class={
                                        evidence.worker_receipt_ok === false
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      {workerReceiptLabel(evidence)}
                                    </span>
                                  ) : null}
                                  {evidence.worker_timed_out === true ? (
                                    <span class="trace-pill trace-pill--warn">timeout</span>
                                  ) : null}
                                  {workerStatusLabel(evidence) ? (
                                    <span class="trace-pill">{workerStatusLabel(evidence)}</span>
                                  ) : null}
                                  {workerDurationLabel(evidence) ? (
                                    <span class="trace-pill">{workerDurationLabel(evidence)}</span>
                                  ) : null}
                                  {workerTimeoutLabel(evidence) ? (
                                    <span class="trace-pill">{workerTimeoutLabel(evidence)}</span>
                                  ) : null}
                                  {workerAttemptLabel(evidence) ? (
                                    <span class="trace-pill">{workerAttemptLabel(evidence)}</span>
                                  ) : null}
                                  {workerCommandLabel(evidence.worker_command) ? (
                                    <span class="trace-pill">{workerCommandLabel(evidence.worker_command)}</span>
                                  ) : null}
                                  {evidence.worker_action ? (
                                    <span class="trace-pill">{evidence.worker_action}</span>
                                  ) : null}
                                  {evidence.worker_gate_count !== null ? (
                                    <span class="trace-pill">gates {evidence.worker_gate_count}</span>
                                  ) : null}
                                  {evidence.worker_retry_exhausted === true ? (
                                    <span class="trace-pill trace-pill--warn">retry exhausted</span>
                                  ) : null}
                                  {evidence.blocked_phase ? (
                                    <span class="trace-pill trace-pill--warn">blocked {evidence.blocked_phase}</span>
                                  ) : null}
                                  {evidence.missing_receipts.map((receipt) => (
                                    <span class="trace-pill trace-pill--warn">missing {receipt}</span>
                                  ))}
                                  {evidence.packet_envelope_errors.map((field) => (
                                    <span class="trace-pill trace-pill--warn">invalid {field}</span>
                                  ))}
                                  {evidence.worker_receipt_errors.map((field) => (
                                    <span class="trace-pill trace-pill--warn">receipt {field}</span>
                                  ))}
                                  {evidence.operator_options.map((option) => (
                                    <span class="trace-pill">{option}</span>
                                  ))}
                                  {evidence.operator_author ? (
                                    <span class="trace-pill">operator {evidence.operator_author}</span>
                                  ) : null}
                                  {evidence.operator_action ? (
                                    <span class="trace-pill">{evidence.operator_action}</span>
                                  ) : null}
                                </div>
                                {evidence.worker_evidence_summary ? (
                                  <p class="muted">{evidence.worker_evidence_summary}</p>
                                ) : null}
                                {evidence.worker_cwd ? (
                                  <p class="muted">cwd {evidence.worker_cwd}</p>
                                ) : null}
                                {shouldShowTranscriptExcerpt(evidence) ? (
                                  <p class="muted">{evidence.transcript_excerpt}</p>
                                ) : null}
                              </div>
                            ))}
                          </div>
                        ) : null}
                      </>
                    )}
                  </Show>
                </article>
              </div>

              <article class="panel panel--board">
                <p class="panel-kicker">Issues</p>
                <h3>Status board</h3>
                <div class="review-queue" data-testid="review-queue">
                  <div class="review-queue-head">
                    <div>
                      <strong>Review queue</strong>
                      <span>
                        {reviewQueueCards().length
                          ? `${reviewQueueCards().length} need decision`
                          : "clear"}
                      </span>
                    </div>
                    <span>Blocked / Needs Review</span>
                  </div>
                  {reviewQueueCards().length ? (
                    <div class="review-queue-list">
                      {reviewQueueCards().map((card) => (
                        <div
                          class="review-queue-item"
                          data-testid={`review-queue-issue-${card.issue.id}`}
                        >
                          <div class="review-queue-item-head">
                            <div>
                              <strong>#{card.issue.id}</strong>
                              <span>{card.issue.title}</span>
                            </div>
                            <span class="trace-pill trace-pill--warn">{card.issue.status}</span>
                          </div>
                          <div class="trace-strip">
                            <span class="trace-pill">R {card.trace?.current_round ?? "?"}</span>
                            <span class="trace-pill">{reviewQueueDecisionLabel(card)}</span>
                            <span class="trace-pill">{reviewQueueBlockerLabel(card)}</span>
                            {card.doctor ? (
                              <span class="trace-pill">{doctorWorkerLabel(card.doctor)}</span>
                            ) : null}
                            {card.doctor ? (
                              <span class="trace-pill">{doctorReceiptLabel(card.doctor)}</span>
                            ) : null}
                          </div>
                          <p class="muted">{card.issue.summary ?? card.doctor?.summary ?? "Decision pending"}</p>
                          {reviewQueueEvidence(card).length ? (
                            <div class="review-queue-evidence">
                              {reviewQueueEvidence(card).map((evidence) => (
                                <button
                                  type="button"
                                  data-testid={`review-queue-evidence-${card.issue.id}-${evidence.id}`}
                                  title={evidence.summary}
                                  onClick={() => focusEvidence(card.issue.id, evidence.id)}
                                >
                                  {evidence.stage_role ?? evidence.kind} E#{evidence.id}
                                </button>
                              ))}
                            </div>
                          ) : null}
                          <div class="record-actions">
                            {issueDecisionActions(card).map((action) => (
                              <button
                                type="button"
                                aria-label={
                                  action.action === "retry"
                                    ? issueRuntimeActionAriaLabel(card, true, "review queue")
                                    : `${action.label} issue #${card.issue.id} from review queue`
                                }
                                data-testid={`review-queue-action-${action.action}-${card.issue.id}`}
                                disabled={issueOptionDisabled(card, action)}
                                onClick={() => runIssueAction(card, action)}
                                {...issueActionButtonAttrs(action)}
                              >
                                {issueDecisionButtonLabel(card, action)}
                              </button>
                            ))}
                            <button
                              type="button"
                              aria-label={`Comment on issue #${card.issue.id} from review queue`}
                              data-testid={`review-queue-action-comment-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              onClick={() => openIssueComment(card.issue.id, "board")}
                              {...issueActionButtonAttrs(issueActionByName(card, "comment"))}
                            >
                              Comment
                            </button>
                            <button
                              type="button"
                              aria-label={`Show issue #${card.issue.id} details from review queue`}
                              data-testid={`review-queue-action-details-${card.issue.id}`}
                              onClick={() => {
                                setSelectedIssueId(card.issue.id);
                                revealIssueDetail();
                              }}
                            >
                              Details
                            </button>
                          </div>
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
                <div class="connector-queue" data-testid="connector-publish-queue">
                  <div>
                    <strong>Connector queue</strong>
                    <span>
                      {connectorPublishRequiredCount()
                        ? `${connectorPublishRequiredCount()} publish required`
                        : "all current"}
                    </span>
                    {connectorPublishPlan() ? (
                      <span title={connectorPublishPlan()?.plan_id}>
                        plan {compactText(connectorPublishPlan()?.plan_id ?? "", 12)}
                      </span>
                    ) : null}
                    {connectorRoundtripPlan() ? (
                      <span title={connectorRoundtripPlan()?.plan_id}>
                        rt {compactText(connectorRoundtripPlan()?.plan_id ?? "", 12)}
                      </span>
                    ) : null}
                  </div>
                  <button
                    type="button"
                    aria-label="Plan connector queue publish"
                    data-testid="connector-publish-queue-plan"
                    disabled={Boolean(connectorPublishAction())}
                    title={connectorQueue()?.commands?.publish_plan ?? "entrance hive connector publish-plan --compact"}
                    onClick={() => void planConnectorPublish()}
                  >
                    {connectorPublishPlanLabel()}
                  </button>
                  <button
                    type="button"
                    aria-label="Execute connector queue publish plan"
                    data-testid="connector-publish-queue-execute"
                    disabled={
                      Boolean(connectorPublishAction()) ||
                      !connectorPublishPlan()?.can_execute
                    }
                    title={connectorPublishPlan()?.commands.execute ?? "publish plan required"}
                    onClick={() => void executeConnectorPublishPlan()}
                  >
                    {connectorPublishExecuteLabel()}
                  </button>
                  <button
                    type="button"
                    aria-label="Plan connector queue roundtrip"
                    data-testid="connector-roundtrip-queue-plan"
                    disabled={Boolean(connectorRoundtripAction())}
                    title={connectorQueue()?.commands?.roundtrip_plan ?? "entrance hive connector roundtrip-plan --compact"}
                    onClick={() => void planConnectorRoundtrip()}
                  >
                    {connectorRoundtripPlanLabel()}
                  </button>
                  <button
                    type="button"
                    aria-label="Run remote fixture connector demo"
                    data-testid="connector-fixture-demo-run"
                    disabled={Boolean(pendingFixtureAction())}
                    title="entrance hive connector fixture-demo --compact"
                    onClick={() => void runConnectorFixtureDemo()}
                  >
                    {connectorFixtureDemoLabel()}
                  </button>
                  <button
                    type="button"
                    aria-label="Execute connector queue roundtrip plan"
                    data-testid="connector-roundtrip-queue-execute"
                    disabled={
                      Boolean(connectorRoundtripAction()) ||
                      !connectorRoundtripPlan()?.can_execute
                    }
                    title={connectorRoundtripPlan()?.commands.execute ?? "roundtrip plan required"}
                    onClick={() => void executeConnectorRoundtripPlan()}
                  >
                    {connectorRoundtripExecuteLabel()}
                  </button>
                  {connectorQueueProviders().map((provider) => (
                    <span
                      class={`connector-provider connector-provider--${connectorQueueProviderTone(provider)}`}
                      title={connectorQueueProviderTitle(provider)}
                    >
                      <strong>{provider.name}</strong>
                      <span>{provider.publish_required_count} queued</span>
                      <span>{provider.admission_status === "ready" ? "admit ready" : "admit blocked"}</span>
                    </span>
                  ))}
                  {connectorPublishQueue().slice(0, 4).map((card) => (
                    <div class="connector-queue-issue">
                      {connectorCheckChip(
                        "readback",
                        connectorQueueIssueById(card.issue.id)?.checks ?? card.connector?.checks,
                        `connector-queue-readback-checks-${card.issue.id}`,
                      )}
                      {connectorCheckChip(
                        "admit",
                        connectorQueueIssueById(card.issue.id)?.admission_checks,
                        `connector-queue-admission-checks-${card.issue.id}`,
                      )}
                      {connectorRemoteTargetChip(
                        connectorQueueIssueTarget(card.issue.id),
                        `connector-queue-target-${card.issue.id}`,
                      )}
                      {connectorRemoteWritePlanChip(
                        connectorQueueIssueWritePlan(card.issue.id),
                        `connector-queue-write-plan-${card.issue.id}`,
                      )}
                      {connectorRemoteDiagnosticChips(
                        connectorRemoteDiagnostics(card.issue.id),
                        `connector-queue-remote-signal-${card.issue.id}`,
                      )}
                      <button
                        type="button"
                        aria-label={`Publish issue #${card.issue.id} from connector queue`}
                        data-testid={`connector-publish-queue-publish-${card.issue.id}`}
                        disabled={
                          Boolean(issuePendingLabel(card.issue.id)) ||
                          !connectorQueueIssueCanPublish(card.issue.id)
                        }
                        title={connectorQueueIssuePublishTitle(card)}
                        onClick={() => void publishIssueMirror(card)}
                      >
                        #{card.issue.id} Publish
                      </button>
                      <button
                        type="button"
                        aria-label={`Roundtrip issue #${card.issue.id} from connector queue`}
                        data-testid={`connector-publish-queue-roundtrip-${card.issue.id}`}
                        disabled={
                          Boolean(issuePendingLabel(card.issue.id)) ||
                          !connectorQueueIssueCanPublish(card.issue.id)
                        }
                        title={issueMirrorRoundtripCommand(card)}
                        onClick={() => void roundtripIssueMirror(card)}
                      >
                        #{card.issue.id} Roundtrip
                      </button>
                    </div>
                  ))}
                </div>
                <div class="connector-registry" data-testid="connector-registry">
                  <div>
                    <strong>Connector registry</strong>
                    <span>
                      {activeConnectorCount()}/{connectorProviders().length} active
                    </span>
                    <span>{connectorRegistry()?.admission.gate ?? "gate pending"}</span>
                    <span title={connectorAdmissionCheckContractTitle()}>
                      {connectorAdmissionCheckContractLabel()}
                    </span>
                  </div>
                  {connectorProviders().map((provider) => (
                    <span
                      class={`connector-provider connector-provider--${connectorProviderTone(provider)}`}
                      title={connectorProviderTitle(provider)}
                    >
                      <strong>{provider.name}</strong>
                      <span>{provider.status}</span>
                      <span>{connectorProviderCapabilityLabel(provider)}</span>
                      <span>{connectorProviderAdmissionLabel(provider)}</span>
                    </span>
                  ))}
                </div>
                <div class="board-columns">
                  {ISSUE_STATUSES.map((statusName) => (
                    <section
                      class="board-column"
                      data-testid={`issue-column-${issueStatusTestId(statusName)}`}
                    >
                      <div class="board-column-head">
                        <strong>{statusName}</strong>
                        <span>{issueCardsForStatus(statusName).length}</span>
                      </div>
                      <ul class="record-list board-column-list">
                        {issueCardsForStatus(statusName).length ? (
                          issueCardsForStatus(statusName).map((card) => (
                            <li
                              class={
                                selectedIssueCard()?.issue.id === card.issue.id
                                  ? "record-card issue-card issue-card--selected"
                                  : "record-card issue-card"
                              }
                            >
                              <div class="record-head">
                                <strong>{card.issue.title}</strong>
                                <span>#{card.issue.id}</span>
                              </div>
                              <p class="muted">{card.issue.summary ?? "No summary"}</p>
                              {connectorStatusStrip(card, "board")}
                              <Show when={cardDoctor(card)} keyed>
                                {(doctor) => (
                                  <div class={`doctor-strip doctor-strip--${doctorHealthTone(doctor.health)}`}>
                                    <strong>Doctor</strong>
                                    <span>{doctorHealthLabel(doctor.health)}</span>
                                    <span>{doctorWorkerLabel(doctor)}</span>
                                    <span>{doctorReceiptLabel(doctor)}</span>
                                  </div>
                                )}
                              </Show>
                              {cardAuditFailureDetails(card).length ? (
                                <div class="audit-preview">
                                  {cardAuditFailureDetails(card)
                                    .slice(0, 2)
                                    .map((detail) => (
                                      <span title={detail}>{compactAuditFailureDetail(detail)}</span>
                                    ))}
                                  {cardAuditFailureDetails(card).length > 2 ? (
                                    <span>+{cardAuditFailureDetails(card).length - 2} more</span>
                                  ) : null}
                                  {issueAuditQuickActions(card)}
                                </div>
                              ) : null}
                              {card.trace ? (
                                <div class="trace-strip">
                                  <span class="trace-pill">R {card.trace.current_round}</span>
                                  <span class="trace-pill">
                                    {traceCountLabel("P", card.trace.round_packet_count, card.trace.packet_count)}
                                  </span>
                                  <span class="trace-pill">
                                    {traceCountLabel("A", card.trace.round_admission_count, card.trace.admission_count)}
                                  </span>
                                  <span class="trace-pill">
                                    {traceCountLabel("E", card.trace.round_evidence_count, card.trace.evidence_count)}
                                  </span>
                                  <span class="trace-pill">
                                    {traceCountLabel("V", card.trace.round_verdict_count, card.trace.verdict_count)}
                                  </span>
                                  <span class="trace-pill">{schemaLabel(card.trace.verdict_schema)}</span>
                                  {scoreSummaryLabel(card.trace) ? (
                                    <span class="trace-pill">{scoreSummaryLabel(card.trace)}</span>
                                  ) : null}
                                  <span
                                    class={
                                      card.trace.audit_passed === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {auditLabel(card.trace)}
                                  </span>
                                  {receiptLabel(card) ? (
                                    <span
                                      class={
                                        card.trace.round_receipt_missing_count === 0
                                          ? "trace-pill"
                                          : "trace-pill trace-pill--warn"
                                      }
                                    >
                                      {receiptLabel(card)}
                                    </span>
                                  ) : null}
                                  {gateLabel(card) ? (
                                    <span
                                      class={
                                        card.trace.last_admission_passed === true
                                          ? "trace-pill"
                                          : "trace-pill trace-pill--warn"
                                      }
                                    >
                                      {gateLabel(card)}
                                    </span>
                                  ) : null}
                                  {roleWorkerLabel(card) ? (
                                    <span
                                      class={
                                        card.trace.round_role_worker_count ===
                                        card.trace.round_role_worker_ok_count
                                          ? "trace-pill"
                                          : "trace-pill trace-pill--warn"
                                      }
                                    >
                                      {roleWorkerLabel(card)}
                                    </span>
                                  ) : null}
                                  {card.trace.last_decision ? (
                                    <span class="trace-pill">{card.trace.last_decision}</span>
                                  ) : null}
                                  {operatorEventLabel(card.trace.last_operator_event) ? (
                                    <span class="trace-pill">
                                      {operatorEventLabel(card.trace.last_operator_event)}
                                    </span>
                                  ) : null}
                                  {workerLabel(card) ? <span class="trace-pill">{workerLabel(card)}</span> : null}
                                  {traceRuntimeLabel(card.trace) ? (
                                    <span class="trace-pill">{traceRuntimeLabel(card.trace)}</span>
                                  ) : null}
                                  {traceRuntimeWarnLabel(card.trace) ? (
                                    <span class="trace-pill trace-pill--warn">
                                      {traceRuntimeWarnLabel(card.trace)}
                                    </span>
                                  ) : null}
                                </div>
                              ) : null}
                              <div class="comment-stack">
                                {card.comments.length > 2 ? (
                                  <div class="comment-more">+{card.comments.length - 2} earlier comments</div>
                                ) : null}
                                {card.comments.slice(-2).map((comment) => (
                                  <div class="comment-line comment-line--compact">
                                    <div class="comment-line-head">
                                      <strong>{comment.author}</strong>
                                      <div class="comment-tags">
                                        {commentPills(comment)
                                          .slice(0, 3)
                                          .map((pill) => commentPillNode(card.issue.id, pill))}
                                      </div>
                                    </div>
                                    <span>{commentPreview(comment, COMMENT_CARD_PREVIEW_LIMIT)}</span>
                                  </div>
                                ))}
                              </div>
                              {commentComposerActive(card.issue.id, "board") ? (
                                <div class="comment-box">
                                  <textarea
                                    aria-label={`Board issue #${card.issue.id} comment`}
                                    data-testid={`issue-comment-board-${card.issue.id}`}
                                    value={commentBody()}
                                    onInput={(event) => setCommentBody(event.currentTarget.value)}
                                    onKeyDown={(event) => handleCommentKeyDown(event, card.issue.id)}
                                    placeholder="Comment"
                                  />
                                  <button
                                    type="button"
                                    aria-label={`Send board issue #${card.issue.id} comment`}
                                    data-testid={`issue-comment-board-send-${card.issue.id}`}
                                    disabled={commentSubmitDisabled(card.issue.id)}
                                    onClick={() => void addIssueComment(card.issue.id)}
                                  >
                                    {issuePendingLabel(card.issue.id) ?? "Send"}
                                  </button>
                                  {issueDecisionActions(card).map((action) => (
                                    <button
                                      type="button"
                                      aria-label={
                                        action.action === "retry"
                                          ? issueRuntimeActionAriaLabel(card, true, "board composer")
                                          : `${action.label} issue #${card.issue.id} from board composer`
                                      }
                                      data-testid={`issue-action-board-composer-${action.action}-${card.issue.id}`}
                                      disabled={issueOptionDisabled(card, action)}
                                      onClick={() => runIssueAction(card, action)}
                                      {...issueActionButtonAttrs(action)}
                                    >
                                      {issueDecisionButtonLabel(card, action)}
                                    </button>
                                  ))}
                                  <button
                                    type="button"
                                    aria-label={`Close board issue #${card.issue.id} comment`}
                                    data-testid={`issue-comment-board-close-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => closeIssueComment(card.issue.id)}
                                  >
                                    Close
                                  </button>
                                </div>
                              ) : (
                                <div class="record-actions">
                                  {card.issue.loop_id && card.issue.status === "Todo" ? (
                                    <button
                                      type="button"
                                      aria-label={issueRuntimeActionAriaLabel(
                                        card,
                                        false,
                                        "board",
                                      )}
                                      data-testid={`issue-action-board-run-${card.issue.id}`}
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void runIssueLoop(card)}
                                      {...issueActionButtonAttrs(issueActionByName(card, "run"))}
                                    >
                                      {issueRuntimeActionLabel(card, false)}
                                    </button>
                                  ) : null}
                                  {issueDecisionActions(card).map((action) => (
                                    <button
                                      type="button"
                                      aria-label={
                                        action.action === "retry"
                                          ? issueRuntimeActionAriaLabel(card, true, "board")
                                          : `${action.label} issue #${card.issue.id} from board`
                                      }
                                      data-testid={`issue-action-board-${action.action}-${card.issue.id}`}
                                      disabled={issueOptionDisabled(card, action)}
                                      onClick={() => runIssueAction(card, action)}
                                      {...issueActionButtonAttrs(action)}
                                    >
                                      {issueDecisionButtonLabel(card, action)}
                                    </button>
                                  ))}
                                  <button
                                    type="button"
                                    aria-label={`Show issue #${card.issue.id} details`}
                                    data-testid={`issue-action-board-details-${card.issue.id}`}
                                    onClick={() => {
                                      setSelectedIssueId(card.issue.id);
                                      revealIssueDetail();
                                    }}
                                  >
                                    Details
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Copy issue #${card.issue.id} mirror command from board`}
                                    data-testid={`issue-action-board-mirror-${card.issue.id}`}
                                    title={issueMirrorCommand(card)}
                                    onClick={() => void copyCommandAction("issue mirror", issueMirrorCommand(card))}
                                  >
                                    Mirror
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Sync issue #${card.issue.id} mirror to file from board`}
                                    data-testid={`issue-action-board-sync-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    title={issueMirrorSyncCommand(card)}
                                    onClick={() => void syncIssueMirror(card)}
                                  >
                                    {issueMirrorSyncLabel(card)}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Publish issue #${card.issue.id} mirror to connector from board`}
                                    data-testid={`issue-action-board-publish-${card.issue.id}`}
                                    disabled={
                                      Boolean(issuePendingLabel(card.issue.id)) ||
                                      !connectorQueueIssueCanPublish(card.issue.id)
                                    }
                                    title={connectorQueueIssuePublishTitle(card)}
                                    onClick={() => void publishIssueMirror(card)}
                                  >
                                    {issueMirrorPublishLabel(card)}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Verify issue #${card.issue.id} mirror file from board`}
                                    data-testid={`issue-action-board-verify-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    title={issueMirrorVerifyCommand(card)}
                                    onClick={() => void verifyIssueMirror(card)}
                                  >
                                    {issueMirrorVerifyLabel(card)}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Read back issue #${card.issue.id} mirror file from board`}
                                    data-testid={`issue-action-board-readback-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    title={issueMirrorReadbackCommand(card)}
                                    onClick={() => void readbackIssueMirror(card)}
                                  >
                                    {issueMirrorReadbackLabel(card)}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Admit issue #${card.issue.id} mirror for connector from board`}
                                    data-testid={`issue-action-board-admit-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    title={issueMirrorAdmitCommand(card)}
                                    onClick={() => void admitIssueMirror(card)}
                                  >
                                    {issueMirrorAdmitLabel(card)}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Roundtrip issue #${card.issue.id} connector mirror from board`}
                                    data-testid={`issue-action-board-roundtrip-${card.issue.id}`}
                                    disabled={
                                      Boolean(issuePendingLabel(card.issue.id)) ||
                                      !connectorQueueIssueCanPublish(card.issue.id)
                                    }
                                    title={issueMirrorRoundtripCommand(card)}
                                    onClick={() => void roundtripIssueMirror(card)}
                                  >
                                    {issueMirrorRoundtripLabel(card)}
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Comment on issue #${card.issue.id} from board`}
                                    data-testid={`issue-action-board-comment-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => openIssueComment(card.issue.id, "board")}
                                    {...issueActionButtonAttrs(issueActionByName(card, "comment"))}
                                  >
                                    Comment
                                  </button>
                                </div>
                              )}
                            </li>
                          ))
                        ) : (
                          <li class="record-card issue-card issue-card--empty">
                            <span>No issues</span>
                            {statusName === "Todo" && !(issueCards() ?? []).length ? (
                              <>
                                <button
                                  type="button"
                                  data-testid="issue-empty-run-demo"
                                  disabled={Boolean(pendingDemoAction())}
                                  onClick={() => void startDemoLoop()}
                                >
                                  {pendingDemoAction() ?? "Run Demo"}
                                </button>
                                <button
                                  type="button"
                                  data-testid="issue-empty-run-fixture-demo"
                                  disabled={Boolean(pendingFixtureAction())}
                                  title="entrance hive connector fixture-demo --compact"
                                  onClick={() => void runConnectorFixtureDemo()}
                                >
                                  {connectorFixtureDemoLabel()}
                                </button>
                              </>
                            ) : null}
                          </li>
                        )}
                      </ul>
                    </section>
                  ))}
                </div>
              </article>
            </section>
          </Match>

          <Match when={view() === "launcher"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Launcher</p>
                <h3>Search index</h3>
                <input
                  value={launcherQuery()}
                  onInput={(event) => setLauncherQuery(event.currentTarget.value)}
                  placeholder="Search indexed apps"
                />
                <button type="button" class="primary-button" onClick={() => void refreshLauncherIndex()}>
                  Refresh Index
                </button>
                <p class="muted">Launcher routes through the unified daemon contract.</p>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Matches</p>
                <h3>Launch surface</h3>
                <ul class="record-list">
                  {(launcherItems() ?? []).map((item) => (
                    <li class="record-card">
                      <div class="record-head">
                        <strong>{item.name}</strong>
                        <span>{item.score.toFixed(2)}</span>
                      </div>
                      <code>{item.command}</code>
                      <div class="record-actions">
                        <button type="button" onClick={() => void launchItem(item)}>
                          Launch
                        </button>
                        <button type="button" onClick={() => void pinItem(item)}>
                          {item.pinned ? "Unpin" : "Pin"}
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>
        </Switch>
      </main>
    </div>
  );
}
