import { A } from "@solidjs/router";
import { invoke } from "@desktop/core";
import { For, Show, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import "./Dashboard.css";
import ComputeGraph from "../components/ComputeGraph";
import type { GraphLayoutMode } from "../components/ComputeGraph";
import NodeInspector from "../components/NodeInspector";
import NotaDialog from "../components/NotaDialog";
import {
  listenToGraphUpdates,
  listenToNotaDialogs,
  listenToSystemPulse,
  type SystemPulseEvent,
} from "../features/dashboard/graphEvents";
import { createGraphStore } from "../features/dashboard/graphStore";
import { createNotaDialogStore } from "../features/dashboard/notaDialogStore";
import { listenToForgeTaskStatus } from "../features/forge/taskFeed";
import {
  fetchNotaRuntimeOverview,
  fetchNotaRuntimeStatus,
  type NotaCheckpointRecord,
  type NotaFrontDoorProgressTrack,
  type NotaRuntimeOverview,
  type NotaRuntimeStatus,
  type StoredDecisionRecord,
  type StoredNotaRuntimeTransaction,
} from "../features/nota/overview";

const DASHBOARD_REFRESH_MS = 15_000;

interface StoredAgentInstance {
  id: number;
  role: string;
  parent_instance_id: number | null;
  status: string;
  display_name: string;
}

const formatTimestamp = (value: string) =>
  new Date(value).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

const formatRelativeTimestamp = (value: string) => {
  const diffMs = Date.now() - new Date(value).getTime();
  const diffMinutes = Math.round(Math.abs(diffMs) / 60_000);

  if (diffMinutes < 1) {
    return "just now";
  }

  if (diffMinutes < 60) {
    return `${diffMinutes}m ago`;
  }

  const diffHours = Math.round(diffMinutes / 60);
  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }

  const diffDays = Math.round(diffHours / 24);
  return `${diffDays}d ago`;
};

const humanizeState = (value?: string | null) => {
  if (!value) {
    return "Not recorded";
  }

  return value
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
};

const parseTransactionPayload = (payloadJson: string) => {
  try {
    const parsed = JSON.parse(payloadJson) as {
      issue_id?: string;
      issue_title?: string | null;
      worktree_path?: string;
      prompt_source?: string;
    };
    if (parsed && typeof parsed === "object") {
      return parsed;
    }
  } catch (error) {
    console.error("Failed to parse NOTA transaction payload", error);
  }

  return {};
};

const renderCheckpointTitle = (checkpoint: NotaCheckpointRecord | null) => {
  if (!checkpoint) {
    return "No active checkpoint yet";
  }

  return `Checkpoint ${checkpoint.cadence_object.id}: ${checkpoint.cadence_object.title}`;
};

const transactionTone = (status: string) => {
  const normalized = status.trim().toLowerCase();
  if (normalized === "accepted" || normalized === "closed" || normalized === "integrated") {
    return "steady";
  }
  if (normalized.includes("failed") || normalized.includes("repair")) {
    return "caution";
  }
  return "active";
};

const instanceStatusToTone = (status: string) => {
  switch (status.trim().toLowerCase()) {
    case "busy":
      return "active";
    case "idle":
      return "steady";
    case "stale":
      return "caution";
    case "stopped":
      return "archived";
    default:
      return "steady";
  }
};

const instanceNodeId = (id: number) => `instance-${id}`;

const Dashboard = () => {
  const [overview, setOverview] = createSignal<NotaRuntimeOverview | null>(null);
  const [status, setStatus] = createSignal<NotaRuntimeStatus | null>(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [lastRefreshedAt, setLastRefreshedAt] = createSignal<string | null>(null);
  const [visibleDepth, setVisibleDepth] = createSignal(4);
  const [selectedNodeId, setSelectedNodeId] = createSignal<string | null>(null);
  const [graphLayout, setGraphLayout] = createSignal<GraphLayoutMode>("tree");
  const graphStore = createGraphStore();
  const dialogStore = createNotaDialogStore();

  const selectedGraphNode = () => {
    const id = selectedNodeId();
    if (!id) return null;
    return graphStore.state.nodes.find((n) => n.id === id) ?? null;
  };

  const handleNodeSelect = (nodeId: string) => {
    setSelectedNodeId(nodeId || null);
  };

  const handleNodeAction = (nodeId: string, kind: string) => {
    if (kind === "nota") {
      /* Double-clicking NOTA node — surface the dialog queue */
      const current = dialogStore.current();
      if (!current) {
        /* No pending dialog; just select */
        setSelectedNodeId(nodeId);
      }
      /* If there is a pending dialog, it's already visible via NotaDialog */
    } else {
      setSelectedNodeId(nodeId);
    }
  };

  const applySystemPulse = (pulse: SystemPulseEvent) => {
    const notaTone =
      pulse.health === "Green"
        ? "nota"
        : pulse.health === "Yellow"
          ? "warming"
          : "caution";
    graphStore.updateNodeTone(
      "nota",
      notaTone,
      `${pulse.active_instances} active, ${pulse.stale_instances} stale, ${pulse.stopped_instances} stopped`,
    );
  };

  const loadDashboard = async () => {
    try {
      setError(null);
      const [nextOverview, nextStatus] = await Promise.all([
        fetchNotaRuntimeOverview(),
        fetchNotaRuntimeStatus(),
      ]);
      setOverview(nextOverview);
      setStatus(nextStatus);

      const transactions = nextOverview.transactions.transactions.slice(0, 10);
      for (const transaction of transactions) {
        graphStore.addNode(
          {
            id: `tx-${transaction.id}`,
            kind: "allocation",
            label: transaction.title,
            detail: transaction.status,
            tone: transactionTone(transaction.status),
          },
          "nota",
        );
      }

      setLastRefreshedAt(new Date().toISOString());
    } catch (loadError) {
      console.error("Failed to fetch NOTA runtime dashboard truth", loadError);
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setIsLoading(false);
    }
  };

  const syncInstanceGraph = async () => {
    try {
      const instances =
        (await invoke<StoredAgentInstance[] | null>("list_agent_instances")) ?? [];
      for (const instance of instances) {
        graphStore.addNode(
          {
            id: instanceNodeId(instance.id),
            kind: instance.role,
            label: instance.display_name,
            detail: instance.status,
            tone: instanceStatusToTone(instance.status),
          },
          instance.parent_instance_id !== null
            ? instanceNodeId(instance.parent_instance_id)
            : undefined,
        );
      }
    } catch (loadError) {
      console.warn("Failed to fetch agent instances for compute graph", loadError);
    }
  };

  const syncSystemPulse = async () => {
    try {
      const pulse = await invoke<SystemPulseEvent | null>("get_system_pulse");
      if (pulse) {
        applySystemPulse(pulse);
      }
    } catch (loadError) {
      console.warn("Failed to fetch initial system pulse", loadError);
    }
  };

  onMount(() => {
    graphStore.setVisibleDepth(visibleDepth());
    void loadDashboard();
    void syncInstanceGraph();
    void syncSystemPulse();

    const timer = window.setInterval(() => {
      void loadDashboard();
    }, DASHBOARD_REFRESH_MS);

    onCleanup(() => window.clearInterval(timer));

    let unlistenGraph: (() => void) | undefined;
    let unlistenDialog: (() => void) | undefined;
    let unlistenPulse: (() => void) | undefined;

    void listenToGraphUpdates((event) => {
      graphStore.handleGraphEvent(event);
    }).then((unlisten) => {
      unlistenGraph = unlisten;
    });

    void listenToNotaDialogs((event) => {
      dialogStore.push(event);
    }).then((unlisten) => {
      unlistenDialog = unlisten;
    });

    void listenToSystemPulse((pulse) => {
      applySystemPulse(pulse);
    }).then((unlisten) => {
      unlistenPulse = unlisten;
    });

    void (async () => {
      const unlistenStatus = await listenToForgeTaskStatus(() => {
        void loadDashboard();
      });

      onCleanup(() => {
        unlistenStatus();
      });
    })();

    onCleanup(() => {
      unlistenGraph?.();
      unlistenDialog?.();
      unlistenPulse?.();
    });
  });

  createEffect(() => {
    graphStore.setVisibleDepth(visibleDepth());
  });

  const currentCheckpoint = createMemo(
    () =>
      status()?.current_checkpoint ??
      overview()?.checkpoints.checkpoints.find((checkpoint) => checkpoint.cadence_object.is_current) ??
      null,
  );
  const frontDoor = createMemo(() => status()?.front_door ?? overview()?.front_door ?? null);
  const review = createMemo(() => status()?.review ?? overview()?.review ?? null);
  const integrate = createMemo(() => status()?.integrate ?? overview()?.integrate ?? null);
  const finalize = createMemo(() => status()?.finalize ?? overview()?.finalize ?? null);
  const nextStep = createMemo(() => status()?.next_step ?? overview()?.next_step ?? null);
  const latestTransactions = createMemo(
    () => overview()?.transactions.transactions.slice(0, 4) ?? [],
  );
  const latestDecisions = createMemo(() => overview()?.decisions.decisions.slice(0, 2) ?? []);
  const refreshLine = createMemo(() => {
    const value = lastRefreshedAt();
    if (!value) {
      return "Syncing runtime truth...";
    }

    return `Refreshed ${formatRelativeTimestamp(value)}`;
  });

  const relaySummary = createMemo(
    () =>
      currentCheckpoint()?.payload.human_continuity_bus ??
      status()?.recommended_checkpoint?.human_continuity_bus ??
      frontDoor()?.progress_tracks.find((track) => track.id === "relay-relief")?.summary ??
      "Human relay guidance will appear once the runtime writes the next checkpoint.",
  );

  const checkpointClosureTrack = createMemo<NotaFrontDoorProgressTrack | null>(() => {
    const checkpoint = currentCheckpoint();
    if (!checkpoint) {
      return null;
    }

    const landedCount = checkpoint.payload.landed.length;
    const remainingCount = checkpoint.payload.remaining.length;
    const totalCount = landedCount + remainingCount;
    const value = totalCount === 0 ? 100 : Math.round((landedCount / totalCount) * 100);

    return {
      id: "checkpoint-closure",
      label: "Checkpoint closure",
      value,
      tone: remainingCount === 0 ? "steady" : "active",
      summary: `${landedCount} landed and ${remainingCount} remaining item${
        remainingCount === 1 ? "" : "s"
      } are recorded on the active checkpoint.`,
    };
  });

  const progressTracks = createMemo(() => {
    const tracks = frontDoor()?.progress_tracks ?? [];
    const closureTrack = checkpointClosureTrack();

    return closureTrack ? [...tracks, closureTrack] : tracks;
  });

  return (
    <section class="page page--dashboard page--mission-board">
      <header class="page__hero page__hero--dashboard page__hero--board">
        <p class="page__eyebrow">Board</p>
        <div class="dashboard-hero__meta" aria-label="Dashboard runtime summary">
          <span class="dashboard-pill">Checkpoint {status()?.current_checkpoint_id ?? "none"}</span>
          <span class="dashboard-pill">{frontDoor()?.posture ?? "Loading board"}</span>
          <span class="dashboard-pill">{frontDoor()?.next_action_label ?? "Current slice"}</span>
          <span class="dashboard-pill">Receipts {status()?.receipt_count ?? 0}</span>
          <span class="dashboard-pill">{refreshLine()}</span>
        </div>
      </header>

      <Show when={error()}>
        {(message) => <div class="board-callout board-callout--error">{message()}</div>}
      </Show>

      <section class="board-layout" aria-label="Mission dashboard">
        <article class="dashboard-card dashboard-card--wide board-map">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Runtime map</p>
            <span class="dashboard-live-indicator">
              <span class="dashboard-live-indicator__dot" aria-hidden="true" />
              Live
            </span>
          </div>

          <div class="dashboard-card__headline">
            <div>
              <h3>{renderCheckpointTitle(currentCheckpoint())}</h3>
              <p>
                {frontDoor()?.dashboard_hook ??
                  "Dashboard now stays on the same runtime truth plane as Chat and the CLI."}
              </p>
            </div>
            <div class="dashboard-card__badges">
              <span class="dashboard-card__badge">{frontDoor()?.next_action_label ?? "Current slice"}</span>
              <span class="dashboard-card__badge dashboard-card__badge--running">
                {nextStep() ? humanizeState(nextStep()?.step) : humanizeState(finalize()?.state)}
              </span>
            </div>
          </div>

          <div class="board-map__graph-shell">
            <div class="graph-filter-bar" aria-label="Instance depth filter">
              <button
                type="button"
                class={`graph-filter-btn graph-filter-btn--mode ${graphLayout() === "tree" ? "is-active" : ""}`}
                onClick={() => setGraphLayout("tree")}
                title="Hierarchy view"
              >
                ⊤
              </button>
              <button
                type="button"
                class={`graph-filter-btn graph-filter-btn--mode ${graphLayout() === "force" ? "is-active" : ""}`}
                onClick={() => setGraphLayout("force")}
                title="Force view"
              >
                ◎
              </button>
              <span class="graph-filter-divider" />
              <span class="graph-filter-label">Depth</span>
              <For each={[1, 2, 3, 4]}>
                {(depth) => (
                  <button
                    type="button"
                    class={`graph-filter-btn ${visibleDepth() >= depth ? "is-active" : ""}`}
                    onClick={() => setVisibleDepth(depth)}
                  >
                    {depth}
                  </button>
                )}
              </For>
            </div>
            <ComputeGraph
              store={graphStore}
              onNodeSelect={handleNodeSelect}
              onNodeAction={handleNodeAction}
              selectedNodeId={selectedNodeId()}
              layoutMode={graphLayout()}
            />
            <NodeInspector
              node={selectedGraphNode()}
              onClose={() => setSelectedNodeId(null)}
              onOpenDialog={() => {
                /* If NOTA dialog is pending, it's already shown; otherwise no-op */
              }}
            />
          </div>

          <div class="board-map__footer">
            <p>{frontDoor()?.next_action_detail ?? "Waiting for runtime guidance..."}</p>
            <div class="board-meta-list">
              <Show when={currentCheckpoint()?.cadence_object.updated_at}>
                {(updatedAt) => <span>Updated {formatTimestamp(updatedAt())}</span>}
              </Show>
              <span>Transactions {status()?.transaction_count ?? 0}</span>
              <span>Allocations {status()?.allocation_count ?? 0}</span>
            </div>
          </div>
        </article>

        <aside class="board-rail" aria-label="Current state rail">
          <article class="dashboard-card board-state-card">
            <div class="dashboard-card__topline">
              <p class="dashboard-card__caption">Current state</p>
              <span class="dashboard-live-indicator">
                <span class="dashboard-live-indicator__dot" aria-hidden="true" />
                Runtime
              </span>
            </div>
            <h3>{frontDoor()?.posture ?? "Loading current state"}</h3>
            <p>{frontDoor()?.summary ?? "Waiting for runtime truth..."}</p>
            <dl class="dashboard-stat-list">
              <div class="dashboard-stat">
                <dt>Next honest move</dt>
                <dd>{nextStep() ? humanizeState(nextStep()?.step) : "No next step exposed"}</dd>
              </div>
              <div class="dashboard-stat">
                <dt>Review</dt>
                <dd>{review() ? humanizeState(review()?.verdict ?? review()?.state) : "Pending"}</dd>
              </div>
              <div class="dashboard-stat">
                <dt>Integrate</dt>
                <dd>{integrate() ? humanizeState(integrate()?.outcome ?? integrate()?.state) : "Waiting"}</dd>
              </div>
              <div class="dashboard-stat">
                <dt>Finalize</dt>
                <dd>{finalize() ? humanizeState(finalize()?.state) : "Open"}</dd>
              </div>
            </dl>
            <div class="board-action-row">
              <A class="board-action" href="/">
                Open Chat
              </A>
              <A class="board-action" href="/do">
                Open Do
              </A>
            </div>
          </article>

          <article class="dashboard-card board-relay-card">
            <div class="dashboard-card__topline">
              <p class="dashboard-card__caption">Human relay</p>
              <span class="dashboard-card__badge">Body-feel</span>
            </div>
            <h3>Relay pressure is visible instead of implied.</h3>
            <p class="board-relay-copy">{relaySummary()}</p>
            <Show
              when={(currentCheckpoint()?.payload.next_start_hints ?? []).length > 0}
              fallback={
                <p class="board-card-note">
                  {isLoading()
                    ? "Loading next-start hints..."
                    : "No next-start hints are recorded on the active checkpoint."}
                </p>
              }
            >
              <div class="board-hint-strip">
                <For each={currentCheckpoint()?.payload.next_start_hints ?? []}>
                  {(hint) => <span class="board-hint-chip">{hint}</span>}
                </For>
              </div>
            </Show>
          </article>
        </aside>
      </section>

      <section class="dashboard-grid board-grid" aria-label="Dashboard detail cards">
        <article class="dashboard-card dashboard-card--wide board-progress-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Layered progress</p>
            <span class="dashboard-card__badge">{progressTracks().length} tracks</span>
          </div>
          <h3>Progress is explicit, not hidden inside monitor prose.</h3>
          <p>
            These bars come from the runtime front door, plus a derived checkpoint-closure bar
            computed directly from landed and remaining checkpoint truth.
          </p>
          <div class="board-progress-list" aria-label="Mission progress tracks">
            <For each={progressTracks()}>
              {(track) => (
                <section class="board-progress-track">
                  <div class="board-progress-track__meta">
                    <strong>{track.label}</strong>
                    <span>{track.value}%</span>
                  </div>
                  <div class="board-progress-bar" aria-hidden="true">
                    <span class={`is-${track.tone}`} style={{ width: `${track.value}%` }} />
                  </div>
                  <p>{track.summary}</p>
                </section>
              )}
            </For>
          </div>
        </article>

        <article class="dashboard-card board-checkpoint-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Checkpoint detail</p>
            <span class="dashboard-card__badge">
              {currentCheckpoint()?.payload.selected_trunk ?? "No trunk"}
            </span>
          </div>
          <h3>What is already landed versus what still remains.</h3>
          <Show
            when={currentCheckpoint()}
            fallback={
              <p class="board-card-note">
                {isLoading()
                  ? "Loading checkpoint detail..."
                  : "The dashboard will expand here once an active checkpoint exists."}
              </p>
            }
          >
            {(checkpoint) => (
              <div class="board-checkpoint-detail">
                <section class="board-detail-panel">
                  <span class="board-detail-panel__label">Landed</span>
                  <Show
                    when={checkpoint().payload.landed.length > 0}
                    fallback={<p class="board-card-note">No landed items recorded yet.</p>}
                  >
                    <ul class="board-bullet-list">
                      <For each={checkpoint().payload.landed}>{(item) => <li>{item}</li>}</For>
                    </ul>
                  </Show>
                </section>
                <section class="board-detail-panel">
                  <span class="board-detail-panel__label">Remaining</span>
                  <Show
                    when={checkpoint().payload.remaining.length > 0}
                    fallback={<p class="board-card-note">No remaining items recorded.</p>}
                  >
                    <ul class="board-bullet-list">
                      <For each={checkpoint().payload.remaining}>{(item) => <li>{item}</li>}</For>
                    </ul>
                  </Show>
                </section>
              </div>
            )}
          </Show>
        </article>

        <article class="dashboard-card board-activity-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Recent runtime movement</p>
            <span class="dashboard-card__badge">
              {overview()?.transactions.transaction_count ?? 0} transactions
            </span>
          </div>
          <h3>The board stays close to live transaction evidence.</h3>
          <Show
            when={latestTransactions().length > 0}
            fallback={<p class="board-card-note">No NOTA transactions recorded yet.</p>}
          >
            <ul class="board-activity-list">
              <For each={latestTransactions()}>
                {(transaction: StoredNotaRuntimeTransaction) => {
                  const payload = parseTransactionPayload(transaction.payload_json);

                  return (
                    <li class="board-activity-item">
                      <div class="board-activity-item__topline">
                        <strong>{transaction.title}</strong>
                        <span class={`dashboard-status dashboard-status--${transaction.status.toLowerCase()}`}>
                          {transaction.status}
                        </span>
                      </div>
                      <p>
                        {payload.issue_id ?? transaction.transaction_kind}
                        {payload.issue_title ? ` - ${payload.issue_title}` : ""}
                      </p>
                      <div class="board-meta-list">
                        <span>{formatTimestamp(transaction.updated_at)}</span>
                        <span>Forge task {transaction.forge_task_id ?? "pending"}</span>
                        <Show when={payload.worktree_path}>
                          <span>{payload.worktree_path}</span>
                        </Show>
                      </div>
                    </li>
                  );
                }}
              </For>
            </ul>
          </Show>
        </article>

        <article class="dashboard-card board-decision-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Decision anchors</p>
            <span class="dashboard-card__badge">{overview()?.decisions.link_count ?? 0} links</span>
          </div>
          <h3>Canonical decisions still bound what this dashboard is allowed to become.</h3>
          <Show
            when={latestDecisions().length > 0}
            fallback={<p class="board-card-note">No canonical decisions recorded yet.</p>}
          >
            <ul class="board-activity-list">
              <For each={latestDecisions()}>
                {(decision: StoredDecisionRecord) => (
                  <li class="board-activity-item">
                    <div class="board-activity-item__topline">
                      <strong>{decision.title}</strong>
                      <span>{decision.decision_status}</span>
                    </div>
                    <p>{decision.statement}</p>
                    <div class="board-meta-list">
                      <span>{decision.decision_type || "decision"}</span>
                      <span>{decision.enforcement_level}</span>
                      <span>{formatRelativeTimestamp(decision.updated_at)}</span>
                    </div>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </article>
      </section>

      <NotaDialog store={dialogStore} />
    </section>
  );
};

export default Dashboard;
