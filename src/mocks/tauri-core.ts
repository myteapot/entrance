/**
 * Mock for @tauri-apps/api/core — browser-mode development.
 *
 * When Vite runs WITHOUT Tauri (plain `pnpm dev`), this module replaces
 * the real `@tauri-apps/api/core` via a resolve alias in vite.config.ts.
 * Every IPC command returns realistic fake data so the full UI renders.
 */

const now = () => new Date().toISOString();
const ago = (minutes: number) =>
  new Date(Date.now() - minutes * 60_000).toISOString();

// ---------------------------------------------------------------------------
// Mock data factories
// ---------------------------------------------------------------------------

const mockNotaRuntimeStatus = () => ({
  chat_policy: {
    setting: {
      id: 1,
      scope_type: "global",
      scope_ref: "*",
      archive_policy: "summary" as const,
      updated_at: ago(120),
    },
  },
  checkpoint_count: 3,
  current_checkpoint_id: 3,
  current_checkpoint: {
    cadence_object: {
      id: 3,
      cadence_kind: "continuity_checkpoint",
      title: "V1 Next — G1 Visual System",
      summary: "ComputeGraph + NotaDialog delivered. Mock bridge in progress.",
      payload_json: "{}",
      scope_type: "project",
      scope_ref: "entrance",
      source_type: "arch",
      source_ref: "session-92b1cb8d",
      admission_policy: "default",
      projection_policy: "default",
      status: "active",
      is_current: true,
      created_at: ago(60),
      updated_at: ago(5),
    },
    payload: {
      stable_level: "alpha",
      landed: [
        "T1 三层测试管线 (Playwright + FlaUI)",
        "D2/M10.1 NOTA Memory Schema (8 tables)",
        "G1-Skeleton 计算图骨架 (事件/Store/Engine)",
        "G1-Skin 学术视觉 (CSS)",
      ],
      remaining: [
        "Mock Bridge (浏览器开发模式)",
        "2A 跨 Agent 路由",
        "D2/M10.2 .agents import pipeline",
      ],
      human_continuity_bus:
        "Arch 验收通过。Mock bridge 是下一个交付节点，直接影响你的开发体验。",
      selected_trunk: "feat/g1-compute-graph",
      next_start_hints: [
        "Mock bridge 零侵入方案",
        "合并 D2 → G1 → T1",
        "视觉验证 Dashboard",
      ],
      repo_context: {
        project_dir: "A:\\Publish\\entrance",
        git_branch: "feat/g1-compute-graph",
        git_head: "d3ae36b",
      },
    },
  },
  transaction_count: 12,
  latest_transaction: {
    id: 12,
    actor_role: "arch",
    surface_action: "dispatch",
    transaction_kind: "dev_dispatch",
    title: "Mock Bridge Implementation",
    payload_json: JSON.stringify({
      issue_id: "#mock-bridge",
      issue_title: "Tauri IPC mock for browser dev",
      worktree_path: "A:\\Publish\\entrance",
      prompt_source: "arch",
    }),
    status: "accepted",
    forge_task_id: 42,
    cadence_checkpoint_id: 3,
    created_at: ago(10),
    updated_at: ago(2),
  },
  allocation_count: 8,
  receipt_count: 6,
  decision_count: 4,
  latest_decision: {
    id: 4,
    title: "Canvas 2D over WebGL for V1",
    statement:
      "Use Canvas 2D + shadowBlur for compute graph rendering. Switch to WebGL (pixi.js) only if >200 nodes cause perf issues.",
    rationale:
      "Simplicity and compatibility outweigh raw performance at current graph scale.",
    decision_type: "architecture",
    decision_status: "accepted",
    scope_type: "project",
    scope_ref: "entrance",
    source_ref: "session-92b1cb8d",
    decided_by: "human",
    enforcement_level: "recommended",
    actor_scope: "all",
    confidence: 0.9,
    created_at: ago(180),
    updated_at: ago(180),
  },
  vision_count: 2,
  todo_count: 5,
  recommended_checkpoint: null,
  review: {
    state: "completed",
    transaction_id: 12,
    allocation_id: 8,
    lineage_ref: "v1-next",
    child_dispatch_role: "dev",
    execution_host: "codex",
    target_kind: "feature",
    target_ref: "g1-compute-graph",
    verdict: "approved",
    summary: "All 4 tasks verified. 202 tests green.",
  },
  integrate: {
    state: "pending",
    transaction_id: 12,
    allocation_id: 8,
    lineage_ref: "v1-next",
    child_dispatch_role: "dev",
    execution_host: "local",
    target_kind: "merge",
    target_ref: "main",
    outcome: "pending_merge",
    summary: "D2 → G1 → T1 squash merge sequence planned.",
  },
  finalize: null,
  next_step: {
    step: "merge_and_verify",
    transaction_id: 12,
    allocation_id: 8,
    lineage_ref: "v1-next",
    child_dispatch_role: "arch",
    execution_host: "local",
    target_kind: "merge",
    target_ref: "main",
  },
  front_door: {
    posture: "Delivering",
    summary:
      "V1 Next 四个 Task 全部交付。Mock bridge 将解锁浏览器开发模式，是合并前的最后一步。",
    next_action_label: "Mock + Merge",
    next_action_detail:
      "创建 Tauri mock bridge，然后按 D2 → G1 → T1 顺序合并到 main。",
    dashboard_hook:
      "Dashboard 正在通过 mock IPC 渲染，所有卡片应该有内容。",
    progress_tracks: [
      {
        id: "test-pipeline",
        label: "Test pipeline",
        value: 100,
        tone: "steady",
        summary: "L2 Playwright + L3 FlaUI skeleton complete.",
      },
      {
        id: "compute-graph",
        label: "Compute graph",
        value: 90,
        tone: "active",
        summary: "Skeleton + skin done. EventBus wiring is G1-followup.",
      },
      {
        id: "memory-schema",
        label: "Memory schema",
        value: 100,
        tone: "steady",
        summary: "8 tables created. M10.2 import pipeline is next.",
      },
      {
        id: "mock-bridge",
        label: "Mock bridge",
        value: 50,
        tone: "active",
        summary: "In progress — Vite alias approach, zero business code changes.",
      },
    ],
  },
});

const mockNotaRuntimeOverview = () => {
  const status = mockNotaRuntimeStatus();
  return {
    chat_policy: status.chat_policy,
    checkpoints: {
      checkpoint_count: status.checkpoint_count,
      current_checkpoint_id: status.current_checkpoint_id,
      checkpoints: [status.current_checkpoint!],
    },
    transactions: {
      transaction_count: status.transaction_count,
      transactions: [
        status.latest_transaction!,
        {
          id: 11,
          actor_role: "dev",
          surface_action: "receipt",
          transaction_kind: "dev_receipt",
          title: "G1-Skin Academic Visual",
          payload_json: JSON.stringify({
            issue_id: "#g1-skin",
            issue_title: "Academic visual system CSS",
          }),
          status: "integrated",
          forge_task_id: 41,
          cadence_checkpoint_id: 3,
          created_at: ago(30),
          updated_at: ago(25),
        },
        {
          id: 10,
          actor_role: "dev",
          surface_action: "receipt",
          transaction_kind: "dev_receipt",
          title: "G1-Skeleton Compute Graph",
          payload_json: JSON.stringify({
            issue_id: "#g1-skeleton",
            issue_title: "Canvas renderer + d3-force engine",
            worktree_path: "A:\\Publish\\entrance",
          }),
          status: "integrated",
          forge_task_id: 40,
          cadence_checkpoint_id: 3,
          created_at: ago(60),
          updated_at: ago(45),
        },
        {
          id: 9,
          actor_role: "dev",
          surface_action: "receipt",
          transaction_kind: "dev_receipt",
          title: "D2/M10.1 Memory Schema",
          payload_json: JSON.stringify({
            issue_id: "#d2-m10.1",
            issue_title: "8-table NOTA memory migration",
          }),
          status: "closed",
          forge_task_id: 39,
          cadence_checkpoint_id: 3,
          created_at: ago(90),
          updated_at: ago(80),
        },
      ],
    },
    decisions: {
      decision_count: status.decision_count,
      link_count: 2,
      decisions: [
        status.latest_decision!,
        {
          id: 3,
          title: "Addons-first architecture",
          statement:
            "All features are delivered as addons. Core is empty.",
          rationale:
            "Maximizes extensibility and maintains the 'empty core' principle.",
          decision_type: "architecture",
          decision_status: "accepted",
          scope_type: "project",
          scope_ref: "entrance",
          source_ref: "session-initial",
          decided_by: "human",
          enforcement_level: "mandatory",
          actor_scope: "all",
          confidence: 1.0,
          created_at: ago(2400),
          updated_at: ago(2400),
        },
      ],
      links: [
        {
          id: 1,
          src_decision_id: 3,
          dst_decision_id: 4,
          relation_type: "informs",
          status: "active",
          created_at: ago(180),
        },
      ],
    },
    chat_captures: { capture_count: 0, captures: [] },
    recommended_checkpoint: status.recommended_checkpoint,
    review: status.review,
    integrate: status.integrate,
    finalize: status.finalize,
    next_step: status.next_step,
    front_door: status.front_door,
  };
};

const mockDashboardSummary = () => ({
  app_version: "0.3.1-headless-alpha.1 (mock)",
  launcher_hotkey: "Ctrl+Space",
  enabled_plugin_count: 3,
  running_task_count: 1,
  last_activity_at: ago(2),
  token_count: 2,
  mcp_config_count: 1,
  enabled_mcp_count: 1,
});

type MockAgentInstance = {
  id: number;
  role: string;
  parent_instance_id: number | null;
  agent_tier: string;
  status: string;
  display_name: string;
  config_json: string;
  workspace_path: string | null;
  last_heartbeat_at: string | null;
  created_at: string;
  updated_at: string;
};

const mockAgentInstances = (): MockAgentInstance[] => [
  {
    id: 1,
    role: "nota",
    parent_instance_id: null,
    agent_tier: "FullNota",
    status: "Busy",
    display_name: "NOTA Root",
    config_json: "{}",
    workspace_path: "A:\\Publish\\entrance",
    last_heartbeat_at: ago(1),
    created_at: ago(180),
    updated_at: ago(1),
  },
  {
    id: 2,
    role: "arch",
    parent_instance_id: 1,
    agent_tier: "FullNota",
    status: "Idle",
    display_name: "Arch Lead",
    config_json: "{}",
    workspace_path: "A:\\Publish\\entrance-g2",
    last_heartbeat_at: ago(2),
    created_at: ago(120),
    updated_at: ago(2),
  },
  {
    id: 3,
    role: "dev",
    parent_instance_id: 2,
    agent_tier: "FullNota",
    status: "Busy",
    display_name: "Dev Worker",
    config_json: "{}",
    workspace_path: "A:\\Publish\\entrance-g2",
    last_heartbeat_at: ago(3),
    created_at: ago(90),
    updated_at: ago(3),
  },
  {
    id: 4,
    role: "agent",
    parent_instance_id: 3,
    agent_tier: "FullNota",
    status: "Stale",
    display_name: "Agent Scout",
    config_json: "{}",
    workspace_path: "A:\\Publish\\entrance-g2",
    last_heartbeat_at: ago(12),
    created_at: ago(60),
    updated_at: ago(12),
  },
];

const mockBudgetConfig = () => ({
  max_concurrent_agents: 3,
  capacity_mode: "Queue" as const,
});

const mockInstanceState = mockAgentInstances();
let nextMockInstanceId =
  mockInstanceState.reduce((maxId, instance) => Math.max(maxId, instance.id), 0) + 1;

const snapshotMockInstances = () => mockInstanceState.map((instance) => ({ ...instance }));

const childRoleFor = (role: string) => {
  switch (role.trim().toLowerCase()) {
    case "nota":
      return "arch";
    case "arch":
      return "dev";
    case "dev":
      return "agent";
    default:
      return null;
  }
};

const createMockInstanceRecord = (
  role: string,
  displayName: string,
  parentInstanceId: number | null,
  configJson: string,
): MockAgentInstance => {
  const parent = parentInstanceId === null
    ? null
    : mockInstanceState.find((instance) => instance.id === parentInstanceId) ?? null;
  const timestamp = now();
  const created = {
    id: nextMockInstanceId++,
    role,
    parent_instance_id: parentInstanceId,
    agent_tier: parent?.agent_tier ?? "ArchNota",
    status: "Idle",
    display_name: displayName,
    config_json: configJson,
    workspace_path: parent?.workspace_path ?? null,
    last_heartbeat_at: timestamp,
    created_at: timestamp,
    updated_at: timestamp,
  };
  mockInstanceState.push(created);
  return { ...created };
};

const markStoppedRecursively = (id: number) => {
  for (const child of mockInstanceState.filter((instance) => instance.parent_instance_id === id)) {
    markStoppedRecursively(child.id);
  }

  const target = mockInstanceState.find((instance) => instance.id === id);
  if (target) {
    target.status = "Stopped";
    target.updated_at = now();
  }
};

const spawnMockChildren = (parentId: number, count: number) => {
  const parent = mockInstanceState.find((instance) => instance.id === parentId);
  if (!parent) {
    throw new Error(`Instance ${parentId} not found`);
  }

  const childRole = childRoleFor(parent.role);
  if (!childRole) {
    throw new Error(`${parent.role} instances cannot spawn children`);
  }

  const existingSiblingCount = mockInstanceState.filter(
    (instance) => instance.parent_instance_id === parentId,
  ).length;

  return Array.from({ length: count }, (_, index) =>
    createMockInstanceRecord(
      childRole,
      `${childRole}-${parentId}-${existingSiblingCount + index + 1}`,
      parentId,
      "{}",
    ),
  );
};

const mockSystemPulse = () => {
  const totalInstances = mockInstanceState.length;
  const activeInstances = mockInstanceState.filter((instance) =>
    instance.status === "Idle" || instance.status === "Busy"
  ).length;
  const staleInstances = mockInstanceState.filter((instance) => instance.status === "Stale").length;
  const stoppedInstances = mockInstanceState.filter((instance) => instance.status === "Stopped").length;
  const staleTasks = 0;
  const failedUnhandled = 0;
  const health =
    (staleTasks > 0 || staleInstances > 0) && failedUnhandled > 0
      ? ("Red" as const)
      : staleTasks > 0 || staleInstances > 0 || failedUnhandled > 0
        ? ("Yellow" as const)
        : ("Green" as const);

  return {
    timestamp: now(),
    agent_tier: mockInstanceState[0]?.agent_tier ?? "ArchNota",
    active_tasks: 1,
    stale_tasks: staleTasks,
    pending_approvals: 1,
    pending_work: 0,
    total_instances: totalInstances,
    active_instances: activeInstances,
    stale_instances: staleInstances,
    stopped_instances: stoppedInstances,
    health,
    tick_interval_secs: 30,
    stale_threshold_multiplier: 3,
  };
};

const mockForgeTasks = () => [
  {
    id: 42,
    name: "Mock Bridge",
    command: "antigravity",
    args: "--task mock-bridge",
    working_dir: "A:\\Publish\\entrance",
    stdin_text: null,
    required_tokens: "[]",
    metadata: JSON.stringify({ kind: "feature", issue_id: "#mock-bridge" }),
    status: "Running" as const,
    status_message: "Building mock modules...",
    exit_code: null,
    created_at: ago(10),
    finished_at: null,
  },
  {
    id: 41,
    name: "G1-Skin Visual",
    command: "codex",
    args: "--task g1-skin",
    working_dir: "A:\\Publish\\entrance",
    stdin_text: null,
    required_tokens: "[]",
    metadata: JSON.stringify({ kind: "style", issue_id: "#g1-skin" }),
    status: "Done" as const,
    status_message: "CSS academic visual system applied.",
    exit_code: 0,
    created_at: ago(60),
    finished_at: ago(30),
  },
];

// ---------------------------------------------------------------------------
// invoke() dispatcher
// ---------------------------------------------------------------------------

const handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {
  nota_runtime_overview: () => mockNotaRuntimeOverview(),
  nota_runtime_status: () => mockNotaRuntimeStatus(),
  dashboard_summary: () => mockDashboardSummary(),
  list_agent_instances: () => snapshotMockInstances(),
  get_system_pulse: () => mockSystemPulse(),
  get_parallel_budget_config: () => mockBudgetConfig(),
  create_agent_instance: (args) =>
    createMockInstanceRecord(
      String(args?.role ?? "arch").toLowerCase(),
      String(args?.displayName ?? "New Instance"),
      typeof args?.parentInstanceId === "number" ? args.parentInstanceId : null,
      String(args?.configJson ?? "{}"),
    ),
  stop_agent_instance: (args) => {
    if (typeof args?.id === "number") {
      markStoppedRecursively(args.id);
    }
    return null;
  },
  spawn_child_instances: (args) =>
    spawnMockChildren(
      typeof args?.parentId === "number" ? args.parentId : 0,
      typeof args?.count === "number" ? args.count : 1,
    ),
  forge_list_tasks: () => mockForgeTasks(),
  forge_get_task_details: (args) => {
    const tasks = mockForgeTasks();
    const task = tasks.find((t) => t.id === args?.id);
    return task ? { ...task, logs: [] } : null;
  },
  forge_prepare_agent_dispatch: () => ({
    issue_id: "#mock-issue",
    issue_status: "open",
    issue_status_source: "mock",
    issue_title: "Mock dispatch target",
    project_root: "A:\\Publish\\entrance",
    worktree_path: "A:\\Publish\\entrance",
    prompt_source: "mock",
    prompt: "This is a mock dispatch prompt for browser development.",
  }),
  forge_create_task: () => 99,
  forge_cancel_task: () => undefined,
  forge_dispatch_agent: () => 99,
  vault_list_tokens: () => [],
  vault_add_token: () => 1,
  vault_upsert_token: () => 1,
  vault_delete_token: () => undefined,
  vault_get_token: () => null,
  vault_get_token_by_provider: () => null,
  vault_list_mcp: () => [],
  vault_update_mcp: () => ({
    id: 1,
    name: "mock-mcp",
    transport: "stdio",
    endpoint: "mock",
    enabled: true,
    created_at: now(),
    updated_at: now(),
  }),
  launcher_search: () => [],
  launcher_hotkey: () => "Ctrl+Space",
  launcher_launch: () => undefined,
  landing_import_snapshot: () => ({
    ingest_run_id: 1,
    source_system: "mock",
    source_workspace: "mock",
    source_project: "mock",
    artifact_path: "/mock/path",
    artifact_sha256: "0".repeat(64),
    snapshot_artifact_id: 1,
    imported_issue_count: 0,
    imported_document_count: 0,
    imported_milestone_count: 0,
    imported_planning_item_count: 0,
  }),
  // NOTA prayer commands (G1-Skeleton)
  nota_approve_prayer: () => undefined,
  nota_reject_prayer: () => undefined,
  nota_prayer_list: () => [],

  // ── Issue Tracker ─────────────────────────────────────────────
  issue_list: () => [
    {
      id: 1, issue_key: "ENT-1", title: "[S3-BUG] Roleplay 替代模式检验 + 验证模式优化",
      description: "Issue tracked locally in Entrance.", status: "done",
      priority: "high", labels: "[]", assignee: "agent",
      created_at: ago(1440), updated_at: ago(120), closed_at: ago(120),
    },
    {
      id: 2, issue_key: "ENT-2", title: "[S3-4b] Verify 前端 — 交互模式 UI (demo)",
      description: "", status: "done", priority: "medium",
      labels: "[]", assignee: "dev",
      created_at: ago(1200), updated_at: ago(300), closed_at: ago(300),
    },
    {
      id: 3, issue_key: "ENT-3", title: "[S3-4a] Verify 地基 — verify_document command (Codex)",
      description: "Verify the document pipeline end-to-end.", status: "done",
      priority: "medium", labels: "[]", assignee: "agent",
      created_at: ago(960), updated_at: ago(400), closed_at: ago(400),
    },
    {
      id: 4, issue_key: "ENT-4", title: "[S4] 去 Linear 化 — 内置 Issue Tracker MVP",
      description: "Replace hardcoded Linear dependency with built-in issue tracking.\n\nPhase 1: DB + CRUD + MCP tools\nPhase 2: Board Strip UI\nPhase 3: Dispatch prompt refactor",
      status: "in_progress", priority: "urgent", labels: '["p0","release-blocker"]',
      assignee: "arch", created_at: ago(60), updated_at: ago(5), closed_at: null,
    },
    {
      id: 5, issue_key: "ENT-5", title: "[S3-5] 自限控制单元 — Agent 输出限幅",
      description: "", status: "todo", priority: "medium",
      labels: "[]", assignee: "",
      created_at: ago(720), updated_at: ago(720), closed_at: null,
    },
    {
      id: 6, issue_key: "ENT-6", title: "[S3-3] Gallery 前端 — 卡片策略 + 迭代模式",
      description: "", status: "cancelled", priority: "low",
      labels: "[]", assignee: "",
      created_at: ago(1440), updated_at: ago(600), closed_at: ago(600),
    },
    {
      id: 7, issue_key: "ENT-7", title: "[S4-1] Verify 工具链自检 — DB 健康 + MCP Ready",
      description: "Verify the toolchain health check works correctly.",
      status: "todo", priority: "high", labels: "[]", assignee: "dev",
      created_at: ago(30), updated_at: ago(30), closed_at: null,
    },
  ],
  issue_get: (args: Record<string, unknown> | undefined) => {
    const key = args?.issueKey as string;
    return key ? {
      id: 4, issue_key: key, title: "Mock issue " + key,
      description: "Description for " + key, status: "todo",
      priority: "medium", labels: "[]", assignee: "",
      created_at: ago(60), updated_at: ago(5), closed_at: null,
    } : null;
  },
  issue_create: (args: Record<string, unknown> | undefined) => ({
    id: 99, issue_key: "ENT-99", title: (args?.title as string) || "New issue",
    description: (args?.description as string) || "", status: "todo",
    priority: (args?.priority as string) || "none", labels: "[]",
    assignee: "", created_at: now(), updated_at: now(), closed_at: null,
  }),
  issue_update_status: (args: Record<string, unknown> | undefined) => ({
    id: 1, issue_key: (args?.issueKey as string) || "ENT-1",
    title: "Updated issue", description: "", status: (args?.status as string) || "todo",
    priority: "none", labels: "[]", assignee: "",
    created_at: ago(60), updated_at: now(),
    closed_at: (args?.status === "done" || args?.status === "cancelled") ? now() : null,
  }),
  issue_update: (args: Record<string, unknown> | undefined) => ({
    id: 1, issue_key: (args?.issueKey as string) || "ENT-1",
    title: (args?.title as string) || "Updated", description: "",
    status: "todo", priority: "none", labels: "[]", assignee: "",
    created_at: ago(60), updated_at: now(), closed_at: null,
  }),
  issue_delete: () => undefined,
  issue_add_comment: (args: Record<string, unknown> | undefined) => ({
    id: 99, issue_id: 1, author: (args?.author as string) || "human",
    body: (args?.body as string) || "", created_at: now(),
  }),
  issue_list_comments: () => [
    { id: 1, issue_id: 4, author: "arch", body: "DB schema + CRUD done. Running cargo check.", created_at: ago(30) },
    { id: 2, issue_id: 4, author: "human", body: "Looks good. Continue with frontend.", created_at: ago(15) },
  ],
};

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const handler = handlers[cmd];
  if (handler) {
    // Simulate async latency
    await new Promise((r) => setTimeout(r, 50 + Math.random() * 100));
    return handler(args) as T;
  }

  console.warn(`[mock] invoke("${cmd}") — no handler, returning null`, args);
  return null as T;
}

// Re-export anything else that @tauri-apps/api/core exports
export function transformCallback(_callback: unknown, _once?: boolean): number {
  return 0;
}

export class Channel<T = unknown> {
  id = 0;
  onmessage: ((response: T) => void) | undefined;
  toJSON() { return `__CHANNEL__:${this.id}`; }
}

export function convertFileSrc(filePath: string, _protocol?: string): string {
  return filePath;
}
