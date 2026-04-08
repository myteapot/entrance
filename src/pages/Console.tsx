import { invoke } from "../platform/core";
import {
  For,
  Show,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type JSX,
} from "solid-js";
import "./Console.css";

const CONSOLE_REFRESH_MS = 10_000;

interface AgentInstance {
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
}

interface SystemPulse {
  timestamp: string;
  agent_tier: string;
  health: "Green" | "Yellow" | "Red";
  active_tasks: number;
  stale_tasks: number;
  pending_approvals: number;
  pending_work: number;
  total_instances: number;
  active_instances: number;
  stale_instances: number;
  stopped_instances: number;
  tick_interval_secs: number;
  stale_threshold_multiplier: number;
}

interface ParallelBudgetConfig {
  max_concurrent_agents: number;
  capacity_mode: "Reject" | "Queue";
}

type FeedbackTone = "success" | "error";

interface FeedbackMessage {
  tone: FeedbackTone;
  text: string;
}

type InstanceBranchProps = {
  instance: AgentInstance;
  depth: number;
  childrenOf: (id: number) => AgentInstance[];
  onSpawn: (parentId: number, displayName: string) => Promise<void>;
  onStop: (id: number, displayName: string) => Promise<void>;
};

const roleOptions = [
  { value: "nota", label: "NOTA" },
  { value: "arch", label: "Arch" },
  { value: "dev", label: "Dev" },
  { value: "agent", label: "Agent" },
];

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

const humanizeToken = (value?: string | null) => {
  if (!value) {
    return "Not recorded";
  }

  return value
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .split("_")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
};

const statusClass = (status: string) => {
  const normalized = status.trim().toLowerCase();
  if (normalized === "busy") {
    return "is-busy";
  }
  if (normalized === "stale") {
    return "is-stale";
  }
  if (normalized === "stopped") {
    return "is-stopped";
  }
  return "is-idle";
};

const healthClass = (health?: string | null) => {
  if (health === "Red") {
    return "is-red";
  }
  if (health === "Yellow") {
    return "is-yellow";
  }
  return "is-green";
};

const canSpawnChildren = (instance: AgentInstance) =>
  instance.role.trim().toLowerCase() !== "agent";

const roleBadgeLabel = (role: string) => role.toUpperCase();

const heartbeatLineFor = (instance: AgentInstance) => {
  if (instance.last_heartbeat_at) {
    return `Heartbeat ${formatRelativeTimestamp(instance.last_heartbeat_at)}`;
  }

  if (instance.status.trim().toLowerCase() === "stopped") {
    return "Stopped";
  }

  return "No heartbeat yet";
};

const budgetUsagePercent = (activeInstances: number, limit: number) => {
  if (limit <= 0) {
    return 0;
  }

  return Math.min((activeInstances / limit) * 100, 100);
};

const metricValue = (value?: number | null) =>
  typeof value === "number" ? value.toLocaleString() : "—";

const InstanceBranch = (props: InstanceBranchProps) => {
  const children = createMemo(() => props.childrenOf(props.instance.id));

  return (
    <div class="instance-branch">
      <div
        class="instance-node"
        style={{ "--depth": String(props.depth) } as JSX.CSSProperties}
      >
        <div class="instance-node__main">
          <span class={`instance-status-dot ${statusClass(props.instance.status)}`} aria-hidden="true" />
          <div class="instance-node__identity">
            <div class="instance-node__title-row">
              <span class="instance-role-badge">{roleBadgeLabel(props.instance.role)}</span>
              <strong>{props.instance.display_name}</strong>
              <span class="console-status-chip">{humanizeToken(props.instance.status)}</span>
            </div>
            <div class="instance-node__meta">
              <span>{heartbeatLineFor(props.instance)}</span>
              <span>Tier {humanizeToken(props.instance.agent_tier)}</span>
              <Show when={props.instance.workspace_path}>
                {(workspacePath) => <span class="instance-node__workspace">{workspacePath()}</span>}
              </Show>
            </div>
          </div>
        </div>
        <div class="instance-node__actions">
          <button
            type="button"
            class="console-button console-button--danger"
            onClick={() => void props.onStop(props.instance.id, props.instance.display_name)}
          >
            Stop
          </button>
          <Show when={canSpawnChildren(props.instance)}>
            <button
              type="button"
              class="console-button console-button--secondary"
              onClick={() => void props.onSpawn(props.instance.id, props.instance.display_name)}
            >
              Spawn Child
            </button>
          </Show>
        </div>
      </div>

      <Show when={children().length > 0}>
        <div class="instance-children">
          <For each={children()}>
            {(child) => (
              <InstanceBranch
                instance={child}
                depth={props.depth + 1}
                childrenOf={props.childrenOf}
                onSpawn={props.onSpawn}
                onStop={props.onStop}
              />
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

export default function Console() {
  const [instances, setInstances] = createSignal<AgentInstance[]>([]);
  const [pulse, setPulse] = createSignal<SystemPulse | null>(null);
  const [budgetConfig, setBudgetConfig] = createSignal<ParallelBudgetConfig | null>(null);
  const [isCreating, setIsCreating] = createSignal(false);
  const [newRole, setNewRole] = createSignal("arch");
  const [newName, setNewName] = createSignal("");
  const [newParentId, setNewParentId] = createSignal<number | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [feedback, setFeedback] = createSignal<FeedbackMessage | null>(null);
  const [lastRefreshedAt, setLastRefreshedAt] = createSignal<string | null>(null);

  const refresh = async () => {
    try {
      setError(null);
      const [instanceData, pulseData, budgetData] = await Promise.all([
        invoke<AgentInstance[]>("list_agent_instances"),
        invoke<SystemPulse>("get_system_pulse"),
        invoke<ParallelBudgetConfig>("get_parallel_budget_config"),
      ]);
      setInstances(instanceData ?? []);
      setPulse(pulseData ?? null);
      setBudgetConfig(budgetData ?? null);
      setLastRefreshedAt(new Date().toISOString());
    } catch (loadError) {
      console.warn("Console refresh failed", loadError);
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    }
  };

  const handleCreate = async () => {
    const displayName = newName().trim();
    if (!displayName) {
      setFeedback({
        tone: "error",
        text: "Display name is required before creating an instance.",
      });
      return;
    }

    setIsCreating(true);
    setFeedback(null);

    try {
      await invoke<AgentInstance>("create_agent_instance", {
        role: newRole(),
        parentInstanceId: newParentId(),
        displayName,
        configJson: "{}",
      });
      setNewName("");
      setNewParentId(null);
      setFeedback({
        tone: "success",
        text: `Created instance ${displayName}.`,
      });
      await refresh();
    } catch (createError) {
      setFeedback({
        tone: "error",
        text: createError instanceof Error ? createError.message : String(createError),
      });
    } finally {
      setIsCreating(false);
    }
  };

  const handleStop = async (id: number, displayName: string) => {
    setFeedback(null);

    try {
      await invoke("stop_agent_instance", { id });
      setFeedback({
        tone: "success",
        text: `Stopped ${displayName} and any running descendants.`,
      });
      await refresh();
    } catch (stopError) {
      setFeedback({
        tone: "error",
        text: stopError instanceof Error ? stopError.message : String(stopError),
      });
    }
  };

  const handleSpawn = async (parentId: number, displayName: string) => {
    setFeedback(null);

    try {
      await invoke("spawn_child_instances", { parentId, count: 1 });
      setFeedback({
        tone: "success",
        text: `Spawned a child instance under ${displayName}.`,
      });
      await refresh();
    } catch (spawnError) {
      setFeedback({
        tone: "error",
        text: spawnError instanceof Error ? spawnError.message : String(spawnError),
      });
    }
  };

  onMount(() => {
    void refresh();
    const timer = window.setInterval(() => {
      void refresh();
    }, CONSOLE_REFRESH_MS);

    onCleanup(() => window.clearInterval(timer));
  });

  const sortedInstances = createMemo(() =>
    [...instances()].sort((left, right) => left.id - right.id),
  );
  const rootInstances = createMemo(() =>
    sortedInstances().filter((instance) => instance.parent_instance_id === null),
  );
  const childrenOf = (id: number) =>
    sortedInstances().filter((instance) => instance.parent_instance_id === id);
  const activeInstances = createMemo(() => pulse()?.active_instances ?? 0);
  const budgetLimit = createMemo(() => budgetConfig()?.max_concurrent_agents ?? 0);
  const budgetPercent = createMemo(() =>
    budgetUsagePercent(activeInstances(), budgetLimit()),
  );
  const refreshLine = createMemo(() => {
    const value = lastRefreshedAt();
    if (!value) {
      return "Syncing runtime truth...";
    }

    return `Refreshed ${formatRelativeTimestamp(value)}`;
  });

  return (
    <section class="page page--dashboard page--console console-page">
      <header class="page__hero page__hero--dashboard page__hero--console">
        <p class="page__eyebrow">Console</p>
        <div class="dashboard-hero__meta" aria-label="Console runtime summary">
          <span class="dashboard-pill">Tier {humanizeToken(pulse()?.agent_tier)}</span>
          <span class="dashboard-pill">Health {pulse()?.health ?? "Syncing"}</span>
          <span class="dashboard-pill">
            Active instances {metricValue(pulse()?.active_instances)}
          </span>
          <span class="dashboard-pill">
            Budget {metricValue(activeInstances())}/{budgetLimit() || "?"}
          </span>
          <span class="dashboard-pill">{refreshLine()}</span>
        </div>
      </header>

      <Show when={error()}>
        {(message) => <div class="console-callout console-callout--error">{message()}</div>}
      </Show>

      <Show when={feedback()}>
        {(message) => (
          <div class={`console-callout console-callout--${message().tone}`}>{message().text}</div>
        )}
      </Show>

      <section class="console-grid" aria-label="Operations console">
        <article class="dashboard-card console-tree-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Instance tree</p>
            <span class="dashboard-live-indicator">
              <span class="dashboard-live-indicator__dot" aria-hidden="true" />
              Live
            </span>
          </div>

          <div class="dashboard-card__headline">
            <div>
              <h3>Manage the full instance lineage from root to leaf.</h3>
              <p>
                Create new instances, stop a subtree, or spawn children without leaving the
                runtime shell.
              </p>
            </div>
            <div class="dashboard-card__badges">
              <span class="dashboard-card__badge">
                {metricValue(pulse()?.total_instances)} total
              </span>
              <span class="dashboard-card__badge dashboard-card__badge--running">
                {metricValue(pulse()?.active_instances)} active
              </span>
            </div>
          </div>

          <form
            class="console-create-form"
            onSubmit={(event) => {
              event.preventDefault();
              void handleCreate();
            }}
          >
            <label class="console-field">
              <span>Role</span>
              <select value={newRole()} onChange={(event) => setNewRole(event.currentTarget.value)}>
                <For each={roleOptions}>
                  {(role) => <option value={role.value}>{role.label}</option>}
                </For>
              </select>
            </label>

            <label class="console-field console-field--wide">
              <span>Display name</span>
              <input
                type="text"
                placeholder="Arch Lead"
                value={newName()}
                onInput={(event) => setNewName(event.currentTarget.value)}
              />
            </label>

            <label class="console-field">
              <span>Parent</span>
              <select
                value={newParentId() ?? ""}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  setNewParentId(value ? Number(value) : null);
                }}
              >
                <option value="">Root instance</option>
                <For each={sortedInstances()}>
                  {(instance) => (
                    <option value={instance.id}>
                      {instance.display_name} ({roleBadgeLabel(instance.role)} #{instance.id})
                    </option>
                  )}
                </For>
              </select>
            </label>

            <button type="submit" class="console-button console-button--primary" disabled={isCreating()}>
              {isCreating() ? "Creating..." : "Create Instance"}
            </button>
          </form>

          <Show
            when={rootInstances().length > 0}
            fallback={
              <p class="dashboard-card__empty console-empty">
                No instances recorded yet. Create a root instance to start the tree.
              </p>
            }
          >
            <div class="console-tree" aria-label="Agent instance tree">
              <For each={rootInstances()}>
                {(instance) => (
                  <InstanceBranch
                    instance={instance}
                    depth={0}
                    childrenOf={childrenOf}
                    onSpawn={handleSpawn}
                    onStop={handleStop}
                  />
                )}
              </For>
            </div>
          </Show>
        </article>

        <aside class="console-rail" aria-label="System health and budget">
          <article class="dashboard-card console-health-card">
            <div class="dashboard-card__topline">
              <p class="dashboard-card__caption">System health</p>
              <span class="dashboard-live-indicator">
                <span class="dashboard-live-indicator__dot" aria-hidden="true" />
                Pulse
              </span>
            </div>

            <div class="console-health-header">
              <div class={`health-indicator ${healthClass(pulse()?.health)}`}>
                {pulse()?.health?.charAt(0) ?? "?"}
              </div>
              <div class="console-health-copy">
                <h3>{pulse()?.health ?? "Syncing pulse"}</h3>
                <p>
                  Task freshness, approval pressure, and instance liveness are collapsed into one
                  runtime signal instead of hidden inside logs.
                </p>
              </div>
            </div>

            <div class="console-metric-grid">
              <div class="console-metric">
                <span>Active tasks</span>
                <strong>{metricValue(pulse()?.active_tasks)}</strong>
              </div>
              <div class="console-metric">
                <span>Stale tasks</span>
                <strong>{metricValue(pulse()?.stale_tasks)}</strong>
              </div>
              <div class="console-metric">
                <span>Pending approvals</span>
                <strong>{metricValue(pulse()?.pending_approvals)}</strong>
              </div>
              <div class="console-metric">
                <span>Pending work</span>
                <strong>{metricValue(pulse()?.pending_work)}</strong>
              </div>
              <div class="console-metric">
                <span>Total instances</span>
                <strong>{metricValue(pulse()?.total_instances)}</strong>
              </div>
              <div class="console-metric">
                <span>Active instances</span>
                <strong>{metricValue(pulse()?.active_instances)}</strong>
              </div>
              <div class="console-metric">
                <span>Stale instances</span>
                <strong>{metricValue(pulse()?.stale_instances)}</strong>
              </div>
              <div class="console-metric">
                <span>Stopped instances</span>
                <strong>{metricValue(pulse()?.stopped_instances)}</strong>
              </div>
            </div>

            <div class="console-footer-note">
              <span>
                Last pulse{" "}
                {pulse()?.timestamp ? formatTimestamp(pulse()!.timestamp) : "not recorded yet"}
              </span>
              <span>
                Tick {pulse()?.tick_interval_secs ?? "—"}s / stale after{" "}
                {pulse()?.stale_threshold_multiplier ?? "—"} ticks
              </span>
            </div>
          </article>

          <article class="dashboard-card console-budget-card">
            <div class="dashboard-card__topline">
              <p class="dashboard-card__caption">Budget overview</p>
              <span class="dashboard-card__badge">
                {humanizeToken(budgetConfig()?.capacity_mode)}
              </span>
            </div>

            <h3>Parallel capacity is visible before the queue turns opaque.</h3>
            <p>
              The budget panel compares active instances against the current concurrency cap used
              by Forge-backed agent execution.
            </p>

            <div class="console-budget-meter">
              <div class="console-budget-meter__meta">
                <strong>
                  {metricValue(activeInstances())} / {budgetLimit() || "?"}
                </strong>
                <span>slots in use</span>
              </div>
              <div class="console-budget-meter__track" aria-hidden="true">
                <span style={{ width: `${budgetPercent()}%` }} />
              </div>
            </div>

            <dl class="dashboard-stat-list">
              <div class="dashboard-stat">
                <dt>Max concurrent agents</dt>
                <dd>{budgetLimit() || "Loading"}</dd>
              </div>
              <div class="dashboard-stat">
                <dt>Capacity mode</dt>
                <dd>{humanizeToken(budgetConfig()?.capacity_mode)}</dd>
              </div>
              <div class="dashboard-stat">
                <dt>Active / total instances</dt>
                <dd>
                  {metricValue(pulse()?.active_instances)} / {metricValue(pulse()?.total_instances)}
                </dd>
              </div>
              <div class="dashboard-stat">
                <dt>Remaining headroom</dt>
                <dd>{Math.max(budgetLimit() - activeInstances(), 0)}</dd>
              </div>
            </dl>
          </article>
        </aside>
      </section>
    </section>
  );
}
