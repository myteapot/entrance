import { A } from "@solidjs/router";
import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import {
  applyForgeTaskStatusEvent,
  fetchForgeTasks,
  listenToForgeTaskStatus,
  type ForgeTask,
} from "../features/forge/taskFeed";

const placeholderWidgets = [
  {
    title: "Forge pulse",
    caption: "Draft workspace widgets",
    detail: "Forge modules can register cards into this strip once plugin discovery is wired in.",
  },
  {
    title: "Connector stream",
    caption: "Comm replacement slot",
    detail: "The renamed connector route keeps a dedicated space for sync health and external bridges.",
  },
] as const;

const formatTaskTimestamp = (value: string) =>
  new Date(value).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

const Dashboard = () => {
  const [forgeTasks, setForgeTasks] = createSignal<ForgeTask[]>([]);
  const [isLoadingForgeTasks, setIsLoadingForgeTasks] = createSignal(true);

  const loadForgeTasks = async () => {
    try {
      setForgeTasks(await fetchForgeTasks());
    } catch (error) {
      console.error("Failed to fetch dashboard forge tasks", error);
    } finally {
      setIsLoadingForgeTasks(false);
    }
  };

  onMount(() => {
    void loadForgeTasks();

    void (async () => {
      const unlistenStatus = await listenToForgeTaskStatus((payload) => {
        const nextTasks = applyForgeTaskStatusEvent(forgeTasks(), payload);
        if (nextTasks) {
          setForgeTasks(nextTasks);
          return;
        }

        void loadForgeTasks();
      });

      onCleanup(() => unlistenStatus());
    })();
  });

  const recentForgeTasks = createMemo(() => forgeTasks().slice(0, 5));
  const runningTaskCount = createMemo(
    () => recentForgeTasks().filter((task) => task.status === "Running").length,
  );

  return (
    <section class="page page--dashboard">
      <header class="page__hero">
        <p class="page__eyebrow">Dashboard</p>
        <h2>Welcome to Entrance</h2>
        <p class="page__summary">
          The desktop shell is now split into a persistent sidebar and a routed main panel, ready for plugin pages and
          Tauri IPC wiring.
        </p>
      </header>

      <section class="dashboard-grid" aria-label="Dashboard widgets">
        <A class="dashboard-card dashboard-card--forge-widget" href="/forge" aria-label="Open Forge dashboard">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Forge dashboard widget</p>
            <span class="dashboard-card__link">Open Forge</span>
          </div>

          <div class="dashboard-card__headline">
            <div>
              <h3>Recent Forge tasks</h3>
              <p>Latest 5 tasks with live status updates from the Forge queue.</p>
            </div>
            <div class="dashboard-card__badges">
              <span class="dashboard-card__badge">{forgeTasks().length} total</span>
              <Show when={runningTaskCount() > 0}>
                <span class="dashboard-card__badge dashboard-card__badge--running">
                  {runningTaskCount()} running
                </span>
              </Show>
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
                      <span class={`task-status status-${task.status.toLowerCase()}`}>{task.status}</span>
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

        <For each={placeholderWidgets}>
          {(widget) => (
            <article class="dashboard-card">
              <p class="dashboard-card__caption">{widget.caption}</p>
              <h3>{widget.title}</h3>
              <p>{widget.detail}</p>
            </article>
          )}
        </For>
      </section>

      <section class="dashboard-panel">
        <div>
          <p class="dashboard-panel__eyebrow">Next integration</p>
          <h3>Plugin widget host placeholder</h3>
        </div>
        <p>
          This surface intentionally stays empty for now. Later slices can hydrate it from the Rust-side plugin manager
          without revisiting the overall layout contract.
        </p>
      </section>
    </section>
  );
};

export default Dashboard;
