export type View =
  | "panel"
  | "reviews"
  | "diagnostics"
  | "status"
  | "drawer"
  | "hive"
  | "loops"
  | "launcher";
export type CommentSurface = "detail" | "board";
export type ActiveCommentComposer = {
  issueId: number;
  surface: CommentSurface;
};

export type AppStatus = {
  app_root: string;
  db_path: string;
  schema: StoreSchemaStatus;
  drawer_entries: number;
  hive_runs: number;
  hive_loops: number;
  launcher_entries: number;
  generated_at: string;
};

export type StoreSchemaStatus = {
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

export type DrawerSummary = {
  mode: string;
  root: string;
  items: number;
};

export type DrawerHistory = {
  commits: Array<{
    id: string;
    summary: string;
  }>;
};

export type DrawerItem = {
  id: number;
  title: string;
  kind: string;
  storage_path: string | null;
  tags: string[];
  updated_at: string;
};

export type HiveRun = {
  id: number;
  title: string;
  status: string;
  project_dir: string | null;
  summary: string | null;
  updated_at: string;
};

export type HiveSummary = {
  total_runs: number;
  ready_runs: number;
  returned_runs: number;
};

export type HiveLoop = {
  id: number;
  title: string;
  goal: string;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
};

export type IssueCard = {
  issue: {
    id: number;
    loop_id: number | null;
    title: string;
    status: string;
    summary: string | null;
    assignee: string | null;
    claim_role: string | null;
    claim_source: string | null;
    claimed_at: string | null;
    created_at: string;
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
};

export type IssueAction = {
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

export type IssueTransitionPolicyReport = {
  schema_version: string;
  issue: IssueCard["issue"];
  loop_id: number | null;
  policy_owner: string;
  policy_scope: string;
  registry: {
    schema_version: string;
    owner: string;
    scope: string;
    actions: Array<{
      action: string;
      gate: string;
      from_statuses: string[];
      to_status: string;
      requires_confirmation: boolean;
    }>;
    reviewer_fallback: {
      invalid_round_budget: number;
      fallback_status: string;
    };
  };
  state_class: string;
  human_decision_required: boolean;
  summary: string;
  allowed_actions: Array<{
    action: IssueAction;
    from_status: string;
    to_status: string | null;
    gate: string;
    requires_human: boolean;
    rationale: string;
  }>;
  blocked_actions: Array<{
    action: string;
    required_statuses: string[];
    reason: string;
    hint: string | null;
  }>;
  confirmation: {
    required: boolean;
    required_actions: string[];
    confirmation_arg: string;
    receipt_schema: string;
    policy_schema_version: string;
    policy_resource: string;
    review_queue_resource: string;
    actor_identity_resource: string;
  };
  reviewer_budget: {
    current_round: number;
    reviewer_invalid_rounds_used: number;
    reviewer_invalid_round_budget: number;
    reviewer_invalid_budget_exhausted: boolean;
    fallback_status: string;
    current_decision: string | null;
    reason_code: string | null;
  } | null;
  resources: {
    issue: string;
    issue_control: string;
    transition_policy: string;
    issue_timeline: string;
    loop_dashboard: string | null;
    worker_lifecycle: string | null;
    runtime_preflight: string | null;
    review_queue: string;
    policy_registry: string;
  };
  next_actions: string[];
};

export type OperatorEvent = {
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

export type IssueDoctorSummary = {
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

export type LoopDashboardReport = {
  schema_version: string;
  loop_id: number;
  issue: LoopDashboardIssue | null;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
  dashboard_state: string;
  summary: string;
  kernel: {
    preflight_state: string;
    gate: string;
    gate_passed: boolean | null;
    route_from: string;
    route_to: string;
    object_kind: string;
    blocker: string | null;
    failures: string[];
  };
  agents: LoopDashboardAgent[];
  reviewer: {
    decision: string | null;
    reason_code: string | null;
    score_vector: Array<{
      name: string;
      value: number | null;
    }>;
    human_options: string[];
    reviewer_invalid_rounds_used: number;
    reviewer_invalid_round_budget: number;
    reviewer_invalid_budget_exhausted: boolean;
    fallback_status: string;
  };
  human_decision: {
    required: boolean;
    issue_status: string | null;
    options: string[];
    actions: IssueAction[];
  };
  health: {
    health: string;
    audit_failed_count: number;
    failed_checks: string[];
    audit_failure_details: string[];
    missing_receipts: string[];
    worker_failures: string[];
  };
  rounds: LoopDashboardRound[];
  comments_count: number;
  latest_comment: {
    id: number;
    author: string;
    body: string;
    created_at: string;
    payload?: Record<string, unknown>;
  } | null;
  resources: {
    loop_dashboard: string;
    evidence_drilldown: string;
    evidence_manifest: string;
    runtime_preflight: string;
    worker_lifecycle: string;
    issue: string | null;
    issue_control: string | null;
    review_queue: string;
  };
  primary_next_action: string | null;
  next_actions: string[];
};

export type LoopDashboardIssue = {
  id: number;
  loop_id: number | null;
  title: string;
  status: string;
  summary: string | null;
  updated_at: string;
};

export type LoopDashboardRound = {
  round: number;
  current: boolean;
  status: string;
  decision: string | null;
  reason_code: string | null;
  retry_lineage: string | null;
  blocker: string | null;
  packet_count: number;
  admission_count: number;
  evidence_count: number;
  verdict_count: number;
  rejected_count: number;
  receipt_missing_count: number;
  worker_count: number;
  worker_ok_count: number;
  groups: {
    packets: LoopDashboardRoundPacket[];
    admissions: LoopDashboardRoundAdmission[];
    evidence: LoopDashboardRoundEvidence[];
    verdicts: LoopDashboardRoundVerdict[];
  };
};

export type LoopDashboardRoundPacket = {
  id: number;
  object_kind: string;
  writer_role: string;
  route_from: string;
  route_to: string;
  state_code: string;
  admission_result: string | null;
};

export type LoopDashboardRoundAdmission = {
  id: number;
  packet_id: number;
  result: string;
  gate: string | null;
  gate_passed: boolean | null;
  reason: string;
  missing_receipts: string[];
};

export type LoopDashboardRoundEvidence = {
  id: number;
  stage_role: string | null;
  kind: string;
  admission_result: string | null;
  blocked_phase: string | null;
  worker_ok: boolean | null;
  summary: string;
};

export type LoopDashboardRoundVerdict = {
  id: number;
  decision: string;
  reason_code: string | null;
  score_vector: Array<{
    name: string;
    value: number | null;
  }>;
  summary: string;
};

export type LoopDashboardAgent = {
  role: string;
  state: string;
  evidence_id: number | null;
  worker_kind: string | null;
  worker_mode: string | null;
  ok: boolean | null;
  receipt_ok: boolean | null;
  timed_out: boolean | null;
  retry_exhausted: boolean | null;
  summary: string | null;
};

export type LoopControlPacket = {
  schema_version: string;
  loop_id: number;
  state: {
    issue_id: number | null;
    issue_status: string | null;
    loop_status: string | null;
    active_phase: string | null;
    current_round: number | null;
    dashboard_state: string | null;
    lifecycle_state: string | null;
    runtime_preflight_state: string | null;
    evidence_manifest_state: string | null;
    reviewer_decision: string | null;
    reviewer_reason_code: string | null;
    reviewer_invalid_rounds_used: number | null;
    reviewer_invalid_round_budget: number | null;
    reviewer_invalid_budget_exhausted: boolean;
    fallback_status: string | null;
    needs_human_decision: boolean;
    primary_action: string;
  };
  reviewer_gate_surface: {
    role: string;
    allowed_decisions: string[];
    gates: {
      runtime_preflight?: {
        resource: string;
        state: string | null;
        gate: string | null;
        passed: boolean | null;
      };
      worker_lifecycle?: {
        resource: string;
        state: string | null;
        expected_roles?: string[] | null;
        observed_roles?: string[] | null;
        missing_roles?: string[] | null;
        failures?: string[] | null;
      };
      evidence_manifest?: {
        resource: string;
        state: string | null;
        coverage?: Record<string, number> | null;
      };
    };
    score_vector: Array<{
      name: string;
      value?: number | null;
      score?: number | null;
      summary?: string | null;
    }>;
    evidence_links: Record<string, string>;
    target_drift_check: {
      state: string;
      source: string;
      note: string;
    };
    budget_policy: {
      invalid_round_budget: number | null;
      invalid_rounds_used: number | null;
      exhausted: boolean;
      fallback_status: string;
    };
  };
  human_decision_boundary: {
    required: boolean;
    issue_status: string | null;
    actions: unknown[];
    options: string[];
    confirmation_arg: string;
    policy_resource: string;
    review_queue_resource: string;
    instruction: string;
  };
  operator_decision_surface: {
    primary_action: string;
    options: Array<{
      key: string;
      label: string;
      enabled: boolean;
      summary: string;
      tool?: string | null;
      call?: {
        name?: string;
        arguments?: Record<string, unknown>;
      } | null;
      resources?: Record<string, string>;
    }>;
    blocked_fallback: {
      condition: string;
      status: string;
      active: boolean;
    };
  };
  resources: Record<string, string | null>;
};

export type EvidenceDrilldownReport = {
  schema_version: string;
  loop_id: number;
  issue_id: number | null;
  issue_status: string | null;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
  drilldown_state: string;
  summary: string;
  evidence_count: number;
  items: EvidenceDrilldownItem[];
  blockers: Array<{
    evidence_id: number | null;
    scope: string;
    round: number;
    kind: string;
    phase: string | null;
    reason: string;
    operator_options: string[];
    decision_surface: {
      required: boolean;
      issue_status: string | null;
      primary_action: string | null;
      actions: Array<{
        issue_action: IssueAction;
        recommended: boolean;
        operator_option: string | null;
        reason: string;
      }>;
      policy_resource: string;
      review_queue_resource: string;
      confirmation_arg: string;
      summary: string;
    };
  }>;
  human_decision: {
    required: boolean;
    issue_status: string | null;
    options: string[];
    actions: IssueAction[];
  };
  resources: {
    evidence_drilldown: string;
    evidence_manifest: string;
    loop_dashboard: string;
    worker_lifecycle: string;
    runtime_preflight: string;
    issue: string | null;
    issue_control: string | null;
    review_queue: string;
  };
  next_actions: string[];
};

export type EvidenceDrilldownItem = {
  id: number;
  round: number;
  stage_role: string | null;
  kind: string;
  summary: string;
  created_at: string;
  path: string | null;
  schema_version: string | null;
  admission_result: string | null;
  blocked_phase: string | null;
  blocker: string | null;
  operator_options: string[];
  worker: {
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
  } | null;
  receipt: {
    schema_version: string | null;
    role: string | null;
    action: string | null;
    ok: boolean | null;
    evidence_summary: string | null;
    gates: Array<{ name: string; value: unknown }>;
    raw_excerpt: string | null;
  } | null;
  artifacts: Array<{
    kind: string;
    path: string | null;
    summary: string | null;
    manifest: unknown;
  }>;
  payload: {
    top_level_keys: string[];
    excerpt: string;
    diff_from_previous: {
      relative_to_evidence_id: number | null;
      added_keys: string[];
      removed_keys: string[];
      changed_keys: string[];
    };
  };
};

export type EvidenceManifestReport = {
  schema_version: string;
  loop_id: number;
  issue_id: number | null;
  issue_status: string | null;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
  manifest_state: string;
  summary: string;
  coverage: EvidenceManifestCoverage;
  entries: EvidenceManifestEntry[];
  resources: {
    evidence_manifest: string;
    evidence_drilldown: string;
    loop_dashboard: string;
    worker_lifecycle: string;
    runtime_preflight: string;
    issue: string | null;
    issue_control: string | null;
    review_queue: string;
  };
  next_actions: string[];
};

export type EvidenceManifestCoverage = {
  evidence_count: number;
  entry_count: number;
  payload_count: number;
  receipt_count: number;
  transcript_count: number;
  artifact_count: number;
  path_count: number;
  path_present_count: number;
  path_missing_count: number;
  path_unverified_count: number;
  path_none_count: number;
  digest_count: number;
};

export type EvidenceManifestEntry = {
  id: string;
  evidence_id: number;
  round: number;
  stage_role: string | null;
  kind: string;
  source: string;
  entry_kind: string;
  label: string;
  summary: string;
  path: string | null;
  path_status: string;
  schema_version: string | null;
  sha256: string | null;
  size_bytes: number | null;
  required: boolean;
  verified: boolean;
  details: unknown;
};

export type IssueTimelineReport = {
  schema_version: string;
  issue: IssueCard["issue"];
  loop_id: number | null;
  timeline_state: string;
  summary: string;
  counts: {
    item_count: number;
    comment_count: number;
    evidence_count: number;
    verdict_count: number;
    operator_event_count: number;
    blocker_count: number;
    receipt_issue_count: number;
    decision_receipt_count: number;
  };
  rounds: IssueTimelineRoundGroup[];
  human_decision: IssueTimelineHumanDecision;
  decision_receipts: IssueTimelineDecisionReceipt[];
  items: IssueTimelineItem[];
  resources: {
    issue: string;
    issue_control: string;
    issue_timeline: string;
    loop_dashboard: string | null;
    evidence_drilldown: string | null;
    evidence_manifest: string | null;
    runtime_preflight: string | null;
    worker_lifecycle: string | null;
    review_queue: string;
  };
  next_actions: string[];
};

export type IssueTimelineRoundGroup = {
  round: number | null;
  label: string;
  state: string;
  item_ids: string[];
  item_count: number;
  comment_count: number;
  evidence_count: number;
  verdict_count: number;
  operator_event_count: number;
  blocker_count: number;
  first_timestamp: string | null;
  last_timestamp: string | null;
  phases: string[];
  decisions: string[];
};

export type IssueTimelineHumanDecision = {
  required: boolean;
  issue_status: string | null;
  primary_action: string | null;
  actions: Array<{
    issue_action: IssueAction;
    recommended: boolean;
    operator_option: string | null;
    reason: string;
  }>;
  receipt_count: number;
  last_receipt: IssueTimelineDecisionReceipt | null;
  policy_resource: string;
  review_queue_resource: string;
  issue_control_resource: string;
  confirmation_arg: string;
  summary: string;
};

export type IssueTimelineDecisionReceipt = {
  id: string;
  source: string;
  timestamp: string;
  round: number | null;
  action: string | null;
  author: string | null;
  comment_id: number | null;
  evidence_id: number | null;
  receipt_schema_version: string | null;
  receipt_source: string | null;
  policy_schema_version: string | null;
  confirmation_arg: string | null;
  human_confirmed: boolean | null;
  client_name: string | null;
  actor_label: string | null;
  actor_trust: string | null;
  note_excerpt: string | null;
  linked_resource: string;
  details: unknown;
};

export type IssueTimelineItem = {
  id: string;
  permalink: string;
  sequence: number;
  timestamp: string;
  source: string;
  event_kind: string;
  actor: string;
  round: number | null;
  status: string | null;
  phase: string | null;
  title: string;
  summary: string;
  body_excerpt: string | null;
  schema_version: string | null;
  comment_id: number | null;
  evidence_id: number | null;
  verdict_id: number | null;
  action: string | null;
  decision: string | null;
  blocker: string | null;
  linked_resource: string | null;
  details: unknown;
};

export type RuntimePreflightReport = {
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
    capability_preview: RuntimeCapabilityPreview;
  };
  current: RuntimePreflightObservation | null;
  failures: string[];
  next_actions: string[];
};

export type RuntimePreflightObservation = {
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
  capability_preview: Record<string, unknown> | null;
};

export type RuntimeCapabilityPreview = {
  schema_version: string;
  runtime: string;
  worker_spawn_ready: boolean;
  worker_spawn_blockers: string[];
  admission_scope: string[];
  worker_mode: string | null;
  sandbox: {
    filesystem: string;
    network: string;
    writes_artifacts: boolean;
    process_isolation: string;
    write_scope: string;
  };
  artifact_capture: {
    expected: boolean;
    mode: string;
    archive_ready: boolean;
    resource: string;
    next_action: string;
  };
  human_boundary: {
    review_surface: string;
    autonomy_level: string;
    confirmation_arg: string;
    human_decision_statuses: string[];
    protected_actions: string[];
    reviewer_invalid_round_budget: number;
    fallback_status: string;
  };
  worker_context: {
    required: string[];
    supplied_by_driver: string[];
    missing_before_spawn: string[];
    required_receipt_fields: string[];
  };
};

export type WorkerLifecycleReport = {
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
    compat_roles: string[];
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

export type WorkerLifecycleRound = {
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

export type WorkerLifecycleWorker = {
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

export type LauncherResult = {
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

export type IssueComment = IssueCard["comments"][number];
export type LoopRunArgs = {
  runtime?: string;
  workerTimeoutSecs?: number;
  workerAttempts?: number;
};
export type CommentPill = {
  label: string;
  evidenceId?: number;
};

export const ISSUE_STATUSES = ["Todo", "Doing", "Needs Review", "Blocked", "Done", "Canceled"] as const;
export const COMMENT_CARD_PREVIEW_LIMIT = 132;
export const COMMENT_DETAIL_PREVIEW_LIMIT = 360;

export const COMMENT_ACTION_LABELS: Record<string, string> = {
  retry: "retry",
  "request-review": "review",
  cancel: "cancel",
};

export const compactText = (value: string, limit: number) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= limit) return normalized;
  return `${normalized.slice(0, Math.max(0, limit - 1)).trimEnd()}...`;
};

export const commentPayloadString = (comment: IssueComment, field: string) => {
  const value = comment.payload?.[field];
  return typeof value === "string" && value.trim() ? value : null;
};

export const commentPayloadNumber = (comment: IssueComment, field: string) => {
  const value = comment.payload?.[field];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
};

export const commentSchemaLabel = (comment: IssueComment) => {
  const schema = commentPayloadString(comment, "schema_version");
  return schema ? schema.split(".").slice(-2).join(".") : null;
};

export const issueStatusTestId = (statusName: string) => statusName.toLowerCase().replace(/\s+/g, "-");

export const commentPills = (comment: IssueComment) => {
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

export const commentPreview = (comment: IssueComment, limit: number) => {
  if (limit === COMMENT_DETAIL_PREVIEW_LIMIT && commentPayloadString(comment, "source") === "operator") {
    return comment.body;
  }
  return compactText(comment.body, limit);
};

export const operatorActionLabel = (action: string | null) =>
  action ? COMMENT_ACTION_LABELS[action] ?? action : "comment";

export const operatorEventLabel = (event: OperatorEvent | null) => {
  if (!event) return null;
  const author = event.author ?? "operator";
  return `${author} ${operatorActionLabel(event.action)}`;
};

export const operatorEventStatusLabel = (event: OperatorEvent) => {
  const status = event.issue_status ?? event.loop_status;
  return status ? `-> ${status}` : `round ${event.round}`;
};
