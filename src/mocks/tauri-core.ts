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
