import type { Page } from "@playwright/test";

const mockCheckpoint = {
  cadence_object: {
    id: 7,
    cadence_kind: "checkpoint",
    title: "Stabilize layered dashboard",
    summary: "Keep the front door and dashboard on the same truth plane.",
    payload_json: "{}",
    scope_type: "repo",
    scope_ref: "entrance",
    source_type: "runtime",
    source_ref: "nota",
    admission_policy: "strict",
    projection_policy: "runtime",
    status: "active",
    is_current: true,
    created_at: "2026-04-05T07:00:00.000Z",
    updated_at: "2026-04-05T08:30:00.000Z",
  },
  payload: {
    stable_level: "L1",
    landed: ["Runtime truth plane is shared", "Dashboard cards render live status"],
    remaining: ["Wire browser-only test pipeline"],
    human_continuity_bus: "Human relay stays calm when the dashboard mirrors runtime truth.",
    selected_trunk: "feat/t1-test-pipeline",
    next_start_hints: ["Run smoke checks", "Verify dashboard cards"],
    repo_context: {
      project_dir: "/mnt/a/Publish/entrance",
      git_branch: "feat/t1-test-pipeline",
      git_head: "deadbeef",
    },
  },
};

const mockFrontDoor = {
  posture: "Runtime aligned",
  summary: "The current boundary is readable and ready for UI verification.",
  next_action_label: "Run browser smoke",
  next_action_detail: "Confirm navigation, dashboard cards, and layered progress visuals.",
  dashboard_hook: "Dashboard stays grounded in the same checkpoint continuity as Chat.",
  progress_tracks: [
    {
      id: "runtime-truth",
      label: "Runtime truth",
      value: 82,
      tone: "steady",
      summary: "Checkpoint, review, and action guidance are all present.",
    },
    {
      id: "ui-readiness",
      label: "UI readiness",
      value: 76,
      tone: "active",
      summary: "The browser surface can validate structure without native IPC.",
    },
  ],
};

const mockStatus = {
  chat_policy: {
    setting: {
      id: 1,
      scope_type: "repo",
      scope_ref: "entrance",
      archive_policy: "summary",
      updated_at: "2026-04-05T08:30:00.000Z",
    },
  },
  checkpoint_count: 1,
  current_checkpoint_id: 7,
  current_checkpoint: mockCheckpoint,
  transaction_count: 4,
  latest_transaction: {
    id: 41,
    actor_role: "dev",
    surface_action: "verify",
    transaction_kind: "test_pipeline",
    title: "Scaffold browser coverage",
    payload_json: JSON.stringify({
      issue_id: "T1",
      issue_title: "Three-layer test pipeline",
      worktree_path: "/mnt/a/Publish/entrance",
    }),
    status: "Done",
    forge_task_id: 18,
    cadence_checkpoint_id: 7,
    created_at: "2026-04-05T08:10:00.000Z",
    updated_at: "2026-04-05T08:20:00.000Z",
  },
  allocation_count: 3,
  receipt_count: 5,
  decision_count: 2,
  latest_decision: {
    id: 5,
    title: "Dashboard truth plane",
    statement: "Dashboard must stay attached to the same runtime status as Chat.",
    rationale: "Avoid a second scheduler.",
    decision_type: "architecture",
    decision_status: "active",
    scope_type: "repo",
    scope_ref: "entrance",
    source_ref: "T1",
    decided_by: "Arch",
    enforcement_level: "hard",
    actor_scope: "repo",
    confidence: 0.92,
    created_at: "2026-04-05T08:00:00.000Z",
    updated_at: "2026-04-05T08:25:00.000Z",
  },
  chat_capture_count: 1,
  vision_count: 1,
  todo_count: 1,
  recommended_checkpoint: null,
  review: {
    state: "approved",
    verdict: "approved",
    transaction_id: 41,
    allocation_id: 11,
    lineage_ref: "lineage/T1",
    child_dispatch_role: "dev",
    execution_host: "local",
    target_kind: "branch",
    target_ref: "feat/t1-test-pipeline",
    summary: "Browser coverage can be scaffolded safely on the current boundary.",
  },
  integrate: {
    state: "ready",
    outcome: "integrated",
    transaction_id: 41,
    allocation_id: 11,
    lineage_ref: "lineage/T1",
    child_dispatch_role: "dev",
    execution_host: "local",
    target_kind: "branch",
    target_ref: "feat/t1-test-pipeline",
    summary: "The branch is ready for validation once tests pass.",
  },
  finalize: {
    state: "open",
    transaction_id: 41,
    allocation_id: 11,
    lineage_ref: "lineage/T1",
    child_dispatch_role: "dev",
    execution_host: "local",
    target_kind: "branch",
    target_ref: "feat/t1-test-pipeline",
    summary: "Keep the boundary open until browser coverage and restore checks pass.",
  },
  next_step: {
    step: "verify",
    transaction_id: 41,
    allocation_id: 11,
    lineage_ref: "lineage/T1",
    child_dispatch_role: "dev",
    execution_host: "local",
    target_kind: "test_suite",
    target_ref: "playwright",
  },
  front_door: mockFrontDoor,
};

const mockOverview = {
  chat_policy: mockStatus.chat_policy,
  checkpoints: {
    checkpoint_count: 1,
    current_checkpoint_id: 7,
    checkpoints: [mockCheckpoint],
  },
  transactions: {
    transaction_count: 4,
    transactions: [
      mockStatus.latest_transaction,
      {
        ...mockStatus.latest_transaction,
        id: 40,
        title: "Baseline cargo verification",
        status: "Done",
      },
    ],
  },
  decisions: {
    decision_count: 2,
    link_count: 1,
    decisions: [
      mockStatus.latest_decision,
      {
        ...mockStatus.latest_decision,
        id: 4,
        title: "Chat-first front door",
        statement: "Chat remains the front door while Dashboard mirrors runtime truth.",
      },
    ],
    links: [
      {
        id: 1,
        src_decision_id: 5,
        dst_decision_id: 4,
        relation_type: "depends_on",
        status: "active",
        created_at: "2026-04-05T08:26:00.000Z",
      },
    ],
  },
  chat_captures: {
    capture_count: 1,
    captures: [
      {
        id: 1,
        session_ref: "session-1",
        role: "assistant",
        capture_mode: "summary",
        archive_policy: "summary",
        content: "Dashboard is ready for smoke coverage.",
        summary: "Browser smoke should verify the current shell.",
        scope_type: "repo",
        scope_ref: "entrance",
        linked_decision_id: 5,
        status: "active",
        created_at: "2026-04-05T08:22:00.000Z",
      },
    ],
  },
  recommended_checkpoint: null,
  review: mockStatus.review,
  integrate: mockStatus.integrate,
  finalize: mockStatus.finalize,
  next_step: mockStatus.next_step,
  front_door: mockFrontDoor,
};

const mockDispatchContext = {
  issue_id: "T1",
  issue_status: "Todo",
  issue_status_source: "fallback",
  issue_title: "Three-layer test pipeline",
  project_root: "/mnt/a/Publish/entrance",
  worktree_path: "/mnt/a/Publish/entrance",
  prompt_source: "mock",
  prompt: "Run the smoke pipeline.",
};

const mockMcpConfigs = [
  {
    id: 1,
    name: "Local Entrance",
    transport: "stdio",
    endpoint: "entrance mcp stdio",
    enabled: true,
    created_at: "2026-04-05T08:00:00.000Z",
    updated_at: "2026-04-05T08:00:00.000Z",
  },
];

export const installTauriMocks = async (page: Page) => {
  await page.addInitScript(
    ({ overview, status, dispatchContext, mcpConfigs }) => {
      const browserWindow = window as Window &
        typeof globalThis & {
          __TAURI_INTERNALS__?: Record<string, unknown>;
          __TAURI_EVENT_PLUGIN_INTERNALS__?: Record<string, unknown>;
          isTauri?: boolean;
        };
      const callbacks = new Map<number, (data: unknown) => void>();
      let nextCallbackId = 1;

      const cloneValue = <T,>(value: T): T => {
        if (value === null || value === undefined) {
          return value;
        }

        return JSON.parse(JSON.stringify(value)) as T;
      };

      const dispatchTable: Record<string, unknown> = {
        nota_runtime_overview: overview,
        nota_runtime_status: status,
        forge_list_tasks: [],
        forge_get_task_details: null,
        forge_prepare_agent_dispatch: dispatchContext,
        vault_list_tokens: [],
        vault_list_mcp: mcpConfigs,
        vault_get_token: null,
        vault_get_token_by_provider: null,
        "plugin:dialog|open": null,
        "plugin:dialog|message": null,
        "plugin:dialog|ask": false,
        "plugin:dialog|confirm": false,
        "plugin:process|restart": null,
        "plugin:process|exit": null,
      };

      browserWindow.isTauri = false;
      browserWindow.__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          if (cmd === "plugin:event|listen") {
            return args?.handler ?? nextCallbackId;
          }

          if (cmd === "plugin:event|unlisten" || cmd === "plugin:event|emit") {
            return null;
          }

          if (Object.prototype.hasOwnProperty.call(dispatchTable, cmd)) {
            return cloneValue(dispatchTable[cmd]);
          }

          return null;
        },
        transformCallback: (callback?: (data: unknown) => void, once = false) => {
          const id = nextCallbackId++;
          callbacks.set(id, (data: unknown) => {
            if (once) {
              callbacks.delete(id);
            }

            callback?.(data);
          });
          return id;
        },
        unregisterCallback: (id: number) => {
          callbacks.delete(id);
        },
        convertFileSrc: (filePath: string, protocol = "asset") =>
          `${protocol}://localhost/${encodeURIComponent(filePath)}`,
      };
      browserWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: (_event: string, eventId: number) => {
          callbacks.delete(eventId);
        },
      };
    },
    {
      overview: mockOverview,
      status: mockStatus,
      dispatchContext: mockDispatchContext,
      mcpConfigs: mockMcpConfigs,
    },
  );
};
