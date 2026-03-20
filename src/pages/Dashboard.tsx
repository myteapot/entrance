import { A } from "@solidjs/router";
import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { fetchDashboardSummary, type DashboardSummary } from "../features/dashboard/summary";
import {
  applyForgeTaskStatusEvent,
  fetchForgeTasks,
  listenToForgeTaskStatus,
  type ForgeTask,
} from "../features/forge/taskFeed";

const DASHBOARD_REFRESH_MS = 30_000;

const formatTaskTimestamp = (value: string) =>
  new Date(value).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

const formatDetailTimestamp = (value: string) =>
  new Date(value).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
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

const Dashboard = () => {
  const [dashboardSummary, setDashboardSummary] = createSignal<DashboardSummary | null>(null);
  const [forgeTasks, setForgeTasks] = createSignal<ForgeTask[]>([]);
  const [isLoadingSummary, setIsLoadingSummary] = createSignal(true);
  const [isLoadingForgeTasks, setIsLoadingForgeTasks] = createSignal(true);
  const [lastRefreshedAt, setLastRefreshedAt] = createSignal<string | null>(null);

  const loadDashboardSummary = async () => {
    try {
      setDashboardSummary(await fetchDashboardSummary());
    } catch (error) {
      console.error("Failed to fetch dashboard summary", error);
    } finally {
      setIsLoadingSummary(false);
    }
  };

  const loadForgeTasks = async () => {
    try {
      setForgeTasks(await fetchForgeTasks());
    } catch (error) {
      console.error("Failed to fetch dashboard forge tasks", error);
    } finally {
      setIsLoadingForgeTasks(false);
    }
  };

  const refreshDashboard = async () => {
    await Promise.allSettled([loadDashboardSummary(), loadForgeTasks()]);
    setLastRefreshedAt(new Date().toISOString());
  };

  onMount(() => {
    void refreshDashboard();

    const timer = window.setInterval(() => {
      void refreshDashboard();
    }, DASHBOARD_REFRESH_MS);

    onCleanup(() => window.clearInterval(timer));

    void (async () => {
      const unlistenStatus = await listenToForgeTaskStatus((payload) => {
        const nextTasks = applyForgeTaskStatusEvent(forgeTasks(), payload);
        if (nextTasks) {
          setForgeTasks(nextTasks);
        } else {
          void loadForgeTasks();
        }

        void loadDashboardSummary();
        setLastRefreshedAt(new Date().toISOString());
      });

      onCleanup(() => {
        unlistenStatus();
      });
    })();
  });

  const recentForgeTasks = createMemo(() => forgeTasks().slice(0, 5));
  const totalTaskCount = createMemo(() => forgeTasks().length);
  const runningTaskCount = createMemo(
    () =>
      dashboardSummary()?.running_task_count ??
      forgeTasks().filter((task) => task.status === "Running").length,
  );
  const activityLine = createMemo(() => {
    const value = dashboardSummary()?.last_activity_at;

    if (!value) {
      return isLoadingSummary() ? "Loading activity..." : "No activity recorded yet.";
    }

    return `${formatRelativeTimestamp(value)} · ${formatDetailTimestamp(value)}`;
  });

  const refreshLine = createMemo(() => {
    const value = lastRefreshedAt();
    if (!value) {
      return "Syncing dashboard...";
    }

    return `Refreshed ${formatRelativeTimestamp(value)}`;
  });

  return (
    <section class="page page--dashboard">
      <header class="page__hero page__hero--dashboard">
        <p class="page__eyebrow">Dashboard</p>
        <h2>Welcome back to Entrance</h2>
        <p class="page__summary">
          Live shell telemetry for Forge, Vault, and the plugin runtime, with quick jumps into the surfaces you are
          most likely to use next.
        </p>
        <div class="dashboard-hero__meta" aria-label="Dashboard highlights">
          <span class="dashboard-pill">Version v{dashboardSummary()?.app_version ?? "0.2.0"}</span>
          <span class="dashboard-pill">
            Launch {dashboardSummary()?.launcher_hotkey ?? "Alt+Space"}
          </span>
          <span class="dashboard-pill">Switch Ctrl+1 to Ctrl+6</span>
        </div>
      </header>

      <section class="dashboard-grid" aria-label="Dashboard widgets">
        <A
          class="dashboard-card dashboard-card--forge-widget dashboard-card--wide"
          href="/forge"
          aria-label="Open Forge dashboard"
        >
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Forge dashboard widget</p>
            <span class="dashboard-card__link">Open Forge</span>
          </div>

          <div class="dashboard-card__headline">
            <div>
              <h3>Recent Forge tasks</h3>
              <p>Latest 5 tasks with live queue updates and status feedback from the execution engine.</p>
            </div>
            <div class="dashboard-card__badges">
              <span class="dashboard-card__badge">{totalTaskCount()} total</span>
              <span class="dashboard-card__badge dashboard-card__badge--running">
                {runningTaskCount()} running
              </span>
            </div>
          </div>

          <Show
            when={recentForgeTasks().length > 0}
            fallback={
              <p class="dashboard-card__empty">
                {isLoadingForgeTasks() ? "Loading Forge tasks..." : "No Forge tasks yet. Open Forge to start one."}
              </p>
            }
          >
            <ul class="forge-widget-list">
              <For each={recentForgeTasks()}>
                {(task) => (
                  <li class={`forge-widget-task forge-widget-task--${task.status.toLowerCase()}`}>
                    <div class="forge-widget-task__row">
                      <span class="forge-widget-task__name">{task.name}</span>
                      <span class={`dashboard-status dashboard-status--${task.status.toLowerCase()}`}>
                        {task.status}
                      </span>
                    </div>
                    <div class="forge-widget-task__meta">
                      <span>{formatTaskTimestamp(task.created_at)}</span>
                      <Show when={task.status_message}>
                        <span class="forge-widget-task__message">{task.status_message}</span>
                      </Show>
                    </div>
                    <Show when={task.status === "Running"}>
                      <div class="forge-widget-task__progress" aria-hidden="true">
                        <span />
                      </div>
                    </Show>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </A>

        <article class="dashboard-card dashboard-card--status">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">System status</p>
            <span class="dashboard-live-indicator">
              <span class="dashboard-live-indicator__dot" aria-hidden="true" />
              Live
            </span>
          </div>
          <h3>Runtime health</h3>
          <p>Plugin runtime, background activity, and the freshest event seen by the shell.</p>
          <dl class="dashboard-stat-list">
            <div class="dashboard-stat">
              <dt>Enabled plugins</dt>
              <dd>{dashboardSummary()?.enabled_plugin_count ?? "—"}</dd>
            </div>
            <div class="dashboard-stat">
              <dt>Running tasks</dt>
              <dd>{runningTaskCount()}</dd>
            </div>
            <div class="dashboard-stat">
              <dt>Last activity</dt>
              <dd>{activityLine()}</dd>
            </div>
          </dl>
        </article>

        <article class="dashboard-card dashboard-card--vault">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Vault overview</p>
            <A class="dashboard-card__link" href="/vault">
              Open Vault
            </A>
          </div>
          <h3>Credentials and MCP endpoints</h3>
          <p>Persistent secret storage and connector configuration counts from the local Vault store.</p>
          <dl class="dashboard-stat-list">
            <div class="dashboard-stat">
              <dt>Stored tokens</dt>
              <dd>{dashboardSummary()?.token_count ?? "—"}</dd>
            </div>
            <div class="dashboard-stat">
              <dt>MCP configs</dt>
              <dd>{dashboardSummary()?.mcp_config_count ?? "—"}</dd>
            </div>
            <div class="dashboard-stat">
              <dt>Enabled MCP</dt>
              <dd>{dashboardSummary()?.enabled_mcp_count ?? "—"}</dd>
            </div>
          </dl>
        </article>

        <article class="dashboard-card dashboard-card--actions">
          <p class="dashboard-card__caption">Quick actions</p>
          <h3>Jump into the next move</h3>
          <p>Create a task in Forge or tighten credentials in Vault without hunting through the shell.</p>
          <div class="dashboard-action-list">
            <A class="dashboard-action" href="/forge">
              <span class="dashboard-action__title">New task</span>
              <span class="dashboard-action__detail">Open Forge and start a fresh run.</span>
            </A>
            <A class="dashboard-action" href="/vault">
              <span class="dashboard-action__title">Manage tokens</span>
              <span class="dashboard-action__detail">Review API keys and MCP endpoint settings.</span>
            </A>
          </div>
        </article>
      </section>

      <section class="dashboard-panel">
        <div>
          <p class="dashboard-panel__eyebrow">Workspace pulse</p>
          <h3>Fresh data, subtle motion, no dead air</h3>
        </div>
        <div class="dashboard-panel__body">
          <p>
            {refreshLine()} with automatic polling every {Math.round(DASHBOARD_REFRESH_MS / 1000)} seconds and instant
            updates when Forge task status changes.
          </p>
          <div class="dashboard-inline-stats" aria-label="Dashboard summary">
            <span class="dashboard-inline-stat">
              <strong>{runningTaskCount()}</strong> active Forge job{runningTaskCount() === 1 ? "" : "s"}
            </span>
            <span class="dashboard-inline-stat">
              <strong>{dashboardSummary()?.token_count ?? 0}</strong> stored token
              {(dashboardSummary()?.token_count ?? 0) === 1 ? "" : "s"}
            </span>
            <span class="dashboard-inline-stat">
              <strong>{dashboardSummary()?.mcp_config_count ?? 0}</strong> MCP endpoint
              {(dashboardSummary()?.mcp_config_count ?? 0) === 1 ? "" : "s"}
            </span>
          </div>
        </div>
      </section>
    </section>
  );
};

export default Dashboard;
