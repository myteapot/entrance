import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { invoke } from "@desktop/core";
import { open } from "@desktop/dialog";
import "./Forge.css";
import {
  applyForgeTaskStatusEvent,
  dispatchForgeAgent,
  fetchForgeTaskDetails,
  fetchForgeTasks,
  listenToForgeTaskOutput,
  listenToForgeTaskStatus,
  mergeForgeTask,
  parseForgeTaskMetadata,
  prepareForgeAgentDispatch,
  type ForgeTask,
  type LogLine,
  type PreparedAgentDispatch,
} from "../features/forge/taskFeed";

const AUTO_DISPATCH_MODEL = "codex";
type DispatchMode = "auto" | "project";

function formatDispatchContextError(error: unknown, mode: DispatchMode) {
  const message = String(error ?? "");
  if (message.includes("forge plugin is not enabled")) {
    return "Do runtime is currently turned off for this install. Reopen Entrance after enabling the Do runtime.";
  }

  if (message.includes("git rev-parse --show-toplevel failed")) {
    if (mode === "auto") {
      return "Current worktree could not be detected. If you launched Entrance from the desktop, choose a Git project directory and refresh context.";
    }
    return "The selected folder is not a Git repository root. Choose an existing local repository and refresh context.";
  }

  if (message.includes("Project directory `") && message.includes("does not exist")) {
    return "The selected folder does not exist. Choose an existing local repository path.";
  }

  if (message.includes("does not match worktree repository")) {
    return "The selected project directory does not match the detected worktree repository. Re-select the repository root.";
  }

  return message;
}

export default function Forge() {
  const [tasks, setTasks] = createSignal<ForgeTask[]>([]);
  const [selectedTaskId, setSelectedTaskId] = createSignal<number | null>(null);
  const [logs, setLogs] = createSignal<Record<number, LogLine[]>>({});
  const [isLoadingTaskDetails, setIsLoadingTaskDetails] = createSignal(false);
  const [taskDetailsError, setTaskDetailsError] = createSignal<string | null>(null);
  const [restartingTaskId, setRestartingTaskId] = createSignal<number | null>(null);
  const [dispatchContext, setDispatchContext] = createSignal<PreparedAgentDispatch | null>(null);
  const [dispatchContextError, setDispatchContextError] = createSignal<string | null>(null);
  const [isLoadingDispatchContext, setIsLoadingDispatchContext] = createSignal(false);
  const [isLaunchingAgent, setIsLaunchingAgent] = createSignal(false);
  const [projectDir, setProjectDir] = createSignal(localStorage.getItem("forge_project_dir") || "");
  const [dispatchMode, setDispatchMode] = createSignal<DispatchMode>(
    localStorage.getItem("forge_project_dir")?.trim() ? "project" : "auto",
  );
  const [agentCommand, setAgentCommand] = createSignal(localStorage.getItem("forge_agent_command") || "");

  const updateProjectDir = (dir: string) => {
    setProjectDir(dir);
    localStorage.setItem("forge_project_dir", dir);
    if (dir.trim()) {
      setDispatchMode("project");
    }
    setDispatchContext(null);
    setDispatchContextError(null);
  };

  const updateAgentCommand = (cmd: string) => {
    setAgentCommand(cmd);
    localStorage.setItem("forge_agent_command", cmd);
  };

  const pickProjectDir = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose Repository Folder",
      defaultPath: projectDir().trim() || undefined,
    });
    if (selected && typeof selected === "string") {
      updateProjectDir(selected);
      void loadDispatchContext("project");
    }
  };

  const [showNewTaskModal, setShowNewTaskModal] = createSignal(false);
  const [newTaskName, setNewTaskName] = createSignal("");
  const [newTaskCommand, setNewTaskCommand] = createSignal("");
  const [newTaskArgs, setNewTaskArgs] = createSignal("");
  const [newTaskRequiredTokens, setNewTaskRequiredTokens] = createSignal("");
  let activeTaskDetailsRequest = 0;

  const parseArgsInput = (value: string) => {
    if (!value.trim()) {
      return [] as string[];
    }

    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return parsed.map((item) => String(item));
      }
    } catch (error) {
      // Fall through to shell-style splitting for legacy task rows.
    }

    return value.split(" ").filter(Boolean);
  };

  const parseRequiredTokensInput = (value: string) => {
    if (!value.trim()) {
      return [] as string[];
    }

    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return parsed
          .map((item) => String(item).trim())
          .filter(Boolean);
      }
    } catch (error) {
      // Fall through to comma-separated parsing for manual input.
    }

    return value
      .split(",")
      .map((token) => token.trim())
      .filter(Boolean);
  };

  const parseStoredRequiredTokens = (value: string) => {
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return parsed
          .map((item) => String(item).trim())
          .filter(Boolean);
      }
    } catch (error) {
      // Ignore malformed legacy values and render an empty token list instead.
    }

    return [] as string[];
  };

  const normalizeLogLine = (value: string) => {
    try {
      const parsed = JSON.parse(value);
      if (typeof parsed === "string") {
        return parsed;
      }
    } catch (error) {
      // Stored log lines are usually plain text already.
    }

    return value;
  };

  const mergeLogLines = (existing: LogLine[], incoming: LogLine[]) => {
    const merged = [...existing];
    const seen = new Set(existing.map((entry) => entry.id));

    for (const entry of incoming) {
      if (!seen.has(entry.id)) {
        merged.push(entry);
        seen.add(entry.id);
      }
    }

    merged.sort((left, right) => left.id - right.id);
    return merged;
  };

  const upsertTask = (task: ForgeTask) => {
    setTasks((prev) => mergeForgeTask(prev, task));
  };

  const selectedTask = () =>
    tasks().find((task) => task.id === selectedTaskId()) ?? null;

  const fetchTasks = async () => {
    try {
      setTasks(await fetchForgeTasks());
    } catch (error) {
      console.error("Failed to fetch tasks", error);
    }
  };

  const loadTaskDetails = async (taskId: number) => {
    const requestId = ++activeTaskDetailsRequest;
    setIsLoadingTaskDetails(true);
    setTaskDetailsError(null);

    try {
      const details = await fetchForgeTaskDetails(taskId);
      if (requestId !== activeTaskDetailsRequest || !details) {
        return;
      }

      upsertTask(details);
      setLogs((prev) => ({
        ...prev,
        [taskId]: mergeLogLines(details.logs, prev[taskId] ?? []),
      }));
    } catch (error) {
      if (requestId !== activeTaskDetailsRequest) {
        return;
      }

      console.error("Failed to load task details", error);
      setTaskDetailsError(String(error));
    } finally {
      if (requestId === activeTaskDetailsRequest) {
        setIsLoadingTaskDetails(false);
      }
    }
  };

  const createTask = async (
    name: string,
    command: string,
    rawArgs: string,
    rawRequiredTokens: string,
  ) => {
    const argsArray = parseArgsInput(rawArgs);
    const requiredTokens = parseRequiredTokensInput(rawRequiredTokens);
    const id = await invoke<number>("forge_create_task", {
      name,
      command,
      args: JSON.stringify(argsArray),
      requiredTokens,
    });

    await fetchTasks();
    setSelectedTaskId(id);
    await loadTaskDetails(id);
    return id;
  };

  const dispatchAgent = async (
    issueId: string,
    worktreePath: string,
    model: string,
    prompt: string,
    rawRequiredTokens: string,
  ) => {
    const requiredTokens = parseRequiredTokensInput(rawRequiredTokens);
    const cmdOverride = agentCommand() || undefined;
    const id = await dispatchForgeAgent(issueId, worktreePath, model, prompt, requiredTokens, cmdOverride);

    await fetchTasks();
    setSelectedTaskId(id);
    await loadTaskDetails(id);
    return id;
  };

  const loadDispatchContext = async (mode: DispatchMode) => {
    setDispatchMode(mode);
    setIsLoadingDispatchContext(true);
    setDispatchContextError(null);

    const selectedProjectDir = projectDir().trim();
    if (mode === "project" && !selectedProjectDir) {
      setDispatchContext(null);
      setDispatchContextError("Choose a project directory first, then refresh context.");
      setIsLoadingDispatchContext(false);
      return;
    }

    try {
      setDispatchContext(
        await prepareForgeAgentDispatch(mode === "project" ? selectedProjectDir : undefined),
      );
    } catch (error) {
      console.error("Failed to prepare Agent dispatch", error);
      setDispatchContext(null);
      setDispatchContextError(formatDispatchContextError(error, mode));
    } finally {
      setIsLoadingDispatchContext(false);
    }
  };

  const handleLaunchPreparedAgent = async () => {
    setIsLaunchingAgent(true);
    setDispatchContextError(null);

    const mode = dispatchMode();
    const selectedProjectDir = projectDir().trim();
    if (mode === "project" && !selectedProjectDir) {
      setDispatchContextError("Choose a project directory first, then refresh context.");
      setIsLaunchingAgent(false);
      return;
    }

    try {
      const context = await prepareForgeAgentDispatch(
        mode === "project" ? selectedProjectDir : undefined,
      );
      setDispatchContext(context);
      await dispatchAgent(
        context.issue_id,
        context.worktree_path,
        AUTO_DISPATCH_MODEL,
        context.prompt,
        "",
      );
    } catch (error) {
      console.error("Failed to auto-dispatch Agent", error);
      const formattedError = formatDispatchContextError(error, mode);
      setDispatchContextError(formattedError);
      alert("Error dispatching agent: " + formattedError);
    } finally {
      setIsLaunchingAgent(false);
    }
  };

  onMount(() => {
    void fetchTasks();
    if (projectDir().trim()) {
      void loadDispatchContext("project");
    }

    void (async () => {
      const unlistenStatus = await listenToForgeTaskStatus((payload) => {
        const nextTasks = applyForgeTaskStatusEvent(tasks(), payload);
        if (nextTasks) {
          setTasks(nextTasks);
          return;
        }

        void fetchTasks();
      });

      const unlistenOutput = await listenToForgeTaskOutput((payload) => {
        setLogs((prev) => {
          const taskId = payload.task_id;
          const currentLogs = prev[taskId] ?? [];
          return { ...prev, [taskId]: mergeLogLines(currentLogs, [payload]) };
        });
      });

      onCleanup(() => {
        unlistenStatus();
        unlistenOutput();
      });
    })();
  });

  createEffect(() => {
    const taskId = selectedTaskId();
    if (taskId == null) {
      setIsLoadingTaskDetails(false);
      setTaskDetailsError(null);
      return;
    }

    void loadTaskDetails(taskId);
  });

  const handleCreateTask = async () => {
    if (!newTaskName() || !newTaskCommand()) {
      return;
    }

    try {
      await createTask(
        newTaskName(),
        newTaskCommand(),
        newTaskArgs(),
        newTaskRequiredTokens(),
      );
      setShowNewTaskModal(false);
      setNewTaskName("");
      setNewTaskCommand("");
      setNewTaskArgs("");
      setNewTaskRequiredTokens("");
    } catch (error) {
      console.error(error);
      alert("Error: " + error);
    }
  };

  const handleCancelTask = async (id: number) => {
    try {
      await invoke("forge_cancel_task", { id });
    } catch (error) {
      console.error(error);
      alert("Error cancelling: " + error);
    }
  };

  const handleRestartTask = async (task: ForgeTask) => {
    setRestartingTaskId(task.id);
    try {
      const metadata = parseForgeTaskMetadata(task.metadata);
      if (
        metadata.kind === "agent_dispatch" &&
        metadata.issue_id &&
        metadata.worktree_path &&
        metadata.model &&
        task.stdin_text
      ) {
        await dispatchAgent(
          metadata.issue_id,
          metadata.worktree_path,
          metadata.model,
          task.stdin_text,
          task.required_tokens,
        );
      } else {
        await createTask(task.name, task.command, task.args, task.required_tokens);
      }
    } catch (error) {
      console.error("Failed to restart task", error);
      alert("Error restarting: " + error);
    } finally {
      setRestartingTaskId(null);
    }
  };

  let logContainerRef: HTMLDivElement | undefined;
  createEffect(() => {
    const taskId = selectedTaskId();
    if (taskId == null) {
      return;
    }

    void logs()[taskId];
    if (logContainerRef) {
      logContainerRef.scrollTo({
        top: logContainerRef.scrollHeight,
        behavior: "smooth",
      });
    }
  });

  return (
    <div class="forge-page">
      <div class="forge-header">
        <div>
          <h1 class="forge-title">Do</h1>
          <p class="forge-subtitle">
            Run work from the current worktree, or choose an existing Git repository folder.
          </p>
        </div>
        <div class="forge-header-actions">
          <button
            class="btn btn-primary"
            disabled={isLaunchingAgent() || isLoadingDispatchContext() || !dispatchContext()}
            onClick={() => void handleLaunchPreparedAgent()}
          >
            {isLaunchingAgent()
              ? "Running Do..."
              : isLoadingDispatchContext()
                ? "Preparing Do..."
                : "Run Do"}
          </button>
        </div>
      </div>

      <div class="do-launch-panel">
        <div class="do-entry-grid">
          <div class={`do-entry-card ${dispatchMode() === "auto" ? "active" : ""}`}>
            <p class="do-entry-card__eyebrow">Entry A</p>
            <h2 class="do-entry-card__title">Use current worktree</h2>
            <p class="do-entry-card__desc">
              Use this when Entrance is launched from a repository terminal.
            </p>
            <div class="do-entry-card__actions">
              <button
                class="btn"
                disabled={isLoadingDispatchContext()}
                onClick={() => void loadDispatchContext("auto")}
              >
                {isLoadingDispatchContext() && dispatchMode() === "auto"
                  ? "Detecting..."
                  : "Detect current worktree"}
              </button>
            </div>
            <Show when={dispatchMode() === "auto" && dispatchContext()}>
              <div class="do-entry-state do-entry-state-ok">Current worktree context is ready.</div>
            </Show>
            <Show when={dispatchMode() === "auto" && dispatchContextError()}>
              <div class="do-entry-state do-entry-state-warn">{dispatchContextError()}</div>
            </Show>
          </div>

          <div class={`do-entry-card ${dispatchMode() === "project" ? "active" : ""}`}>
            <p class="do-entry-card__eyebrow">Entry B</p>
            <h2 class="do-entry-card__title">Choose project directory</h2>
            <p class="do-entry-card__desc">
              Recommended when Entrance is opened from an installed desktop app.
            </p>
            <div class="do-input-row">
              <input
                class="form-input"
                type="text"
                value={projectDir()}
                onInput={(e) => updateProjectDir(e.currentTarget.value)}
                placeholder="Existing Git repository root (e.g. /home/user/work/entrance)"
              />
            </div>
            <p class="do-entry-card__hint">
              Choose an existing repository root. Do will not create a new project folder for you.
            </p>
            <div class="do-entry-card__actions">
              <button class="btn" onClick={() => void pickProjectDir()}>Choose Folder</button>
              <button
                class="btn"
                disabled={isLoadingDispatchContext()}
                onClick={() => void loadDispatchContext("project")}
              >
                {isLoadingDispatchContext() && dispatchMode() === "project"
                  ? "Refreshing..."
                  : "Refresh Context"}
              </button>
            </div>
            <Show when={dispatchMode() === "project" && dispatchContext()}>
              <div class="do-entry-state do-entry-state-ok">Project directory context is ready.</div>
            </Show>
            <Show when={dispatchMode() === "project" && dispatchContextError()}>
              <div class="do-entry-state do-entry-state-warn">{dispatchContextError()}</div>
            </Show>
          </div>
        </div>

        <div class="do-settings-row">
          <div class="do-agent-command">
            <label class="form-label">Agent command (optional)</label>
            <input
              class="form-input"
              type="text"
              value={agentCommand()}
              onInput={(e) => updateAgentCommand(e.currentTarget.value)}
              placeholder="Agent command (leave empty for default, e.g. codex)"
            />
            <p class="do-setting-hint">
              {agentCommand() ? `Using: ${agentCommand()}` : "Using default CLI"}
            </p>
          </div>
          <div class="do-secondary-actions">
            <span class="do-secondary-label">More options</span>
            <button class="btn do-advanced-btn" onClick={() => setShowNewTaskModal(true)}>
              Advanced Task
            </button>
          </div>
        </div>

        <Show when={dispatchContext()} fallback={
          <div class={`task-callout ${dispatchContextError() ? "callout-failed" : "callout-neutral"}`}>
            {dispatchContextError() ?? "Choose one entry above and refresh context to prepare a Do run."}
          </div>
        }>
          {(context) => (
            <>
              <h3 class="do-context-title">Prepared context</h3>
              <div class="token-chip-list">
                <span class="token-chip">{context().issue_id}</span>
                <span class="token-chip">{context().issue_status}</span>
                <span class="token-chip">{AUTO_DISPATCH_MODEL}</span>
              </div>
              <div class="do-context-grid">
                <div class="do-context-field">
                  <span class="do-context-field__label">Issue</span>
                  <span class="do-context-field__value">
                    {context().issue_title ?? "Current worktree issue"}
                  </span>
                </div>
                <div class="do-context-field">
                  <span class="do-context-field__label">Repository</span>
                  <span class="do-context-field__value">{context().project_root}</span>
                </div>
                <div class="do-context-field">
                  <span class="do-context-field__label">Worktree</span>
                  <span class="do-context-field__value">{context().worktree_path}</span>
                </div>
                <div class="do-context-field">
                  <span class="do-context-field__label">Prompt source</span>
                  <span class="do-context-field__value">{context().prompt_source}</span>
                </div>
              </div>
              <Show when={context().issue_status_source === "fallback"}>
                <div class="task-callout callout-blocked">
                  No local issue summary was available, so Do used a generic `Todo` prompt
                  fallback. Dispatch still works, but status-specific prompt shaping was skipped.
                </div>
              </Show>
            </>
          )}
        </Show>
      </div>

      <div class="forge-layout">
        <div class="forge-sidebar">
          <ul class="task-list">
            <For each={tasks()}>
              {(task) => (
                <li class={`task-item ${selectedTaskId() === task.id ? "active" : ""}`} onClick={() => setSelectedTaskId(task.id)}>
                  <div class="task-item-main">
                    <span class="task-name">{task.name}</span>
                    <span class={`task-status status-${task.status.toLowerCase()}`}>{task.status}</span>
                  </div>
                  <div class="task-item-meta">
                    <span class="task-time">{new Date(task.created_at).toLocaleString()}</span>
                  </div>
                  <Show when={task.status_message}>
                    <div class="task-status-message">{task.status_message}</div>
                  </Show>
                  <div class="task-item-actions">
                    <Show when={task.status === "Running" || task.status === "Pending"}>
                      <button class="btn-icon" onClick={(event) => { event.stopPropagation(); void handleCancelTask(task.id); }}>Stop</button>
                    </Show>
                    <Show when={task.status === "Failed" || task.status === "Cancelled" || task.status === "Done" || task.status === "Blocked"}>
                      <button
                        class="btn-icon"
                        disabled={restartingTaskId() === task.id}
                        onClick={(event) => { event.stopPropagation(); void handleRestartTask(task); }}
                      >
                        {restartingTaskId() === task.id ? "Restarting..." : "Restart"}
                      </button>
                    </Show>
                  </div>
                </li>
              )}
            </For>
            <Show when={tasks().length === 0}>
              <li class="empty-state">No Do runs or advanced tasks yet.</li>
            </Show>
          </ul>
        </div>

        <div class="forge-main">
          <Show when={selectedTask()} fallback={<div class="empty-selection">Select a run to inspect details and logs.</div>}>
            {(task) => {
              const requiredTokens = () => parseStoredRequiredTokens(task().required_tokens);
              const metadata = () => parseForgeTaskMetadata(task().metadata);

              return (
                <div class="log-panel">
                  <div class="log-header">
                    <div class="log-header-main">
                      <div>
                        <h3 class="log-task-name">{task().name}</h3>
                        <p class="log-task-command">
                          {task().command}
                          <Show when={task().args !== "[]"}>{` ${task().args}`}</Show>
                        </p>
                      </div>
                      <span class={`task-status status-${task().status.toLowerCase()}`}>{task().status}</span>
                    </div>
                    <div class="log-task-meta">
                      <span>Task ID: {task().id}</span>
                      <span>Created: {new Date(task().created_at).toLocaleString()}</span>
                      <Show when={task().working_dir}>
                        <span>Worktree: {task().working_dir as string}</span>
                      </Show>
                      <Show when={task().finished_at}>
                        <span>Finished: {new Date(task().finished_at as string).toLocaleString()}</span>
                      </Show>
                    </div>
                    <Show when={metadata().kind === "agent_dispatch"}>
                      <div class="task-required-tokens">
                        <span class="task-required-label">Agent Dispatch</span>
                        <div class="token-chip-list">
                          <Show when={metadata().issue_id}>
                            <span class="token-chip">{metadata().issue_id}</span>
                          </Show>
                          <Show when={metadata().model}>
                            <span class="token-chip">{metadata().model}</span>
                          </Show>
                        </div>
                      </div>
                    </Show>
                    <Show when={requiredTokens().length > 0}>
                      <div class="task-required-tokens">
                        <span class="task-required-label">Required tokens</span>
                        <div class="token-chip-list">
                          <For each={requiredTokens()}>
                            {(token) => <span class="token-chip">{token}</span>}
                          </For>
                        </div>
                      </div>
                    </Show>
                    <Show when={task().status_message}>
                      <div class={`task-callout callout-${task().status.toLowerCase()}`}>{task().status_message}</div>
                    </Show>
                  </div>
                  <div class="log-stream" ref={logContainerRef}>
                    <Show when={taskDetailsError()}>
                      <div class="log-empty log-error">{taskDetailsError()}</div>
                    </Show>
                    <Show when={isLoadingTaskDetails() && (logs()[task().id] || []).length === 0}>
                      <div class="log-empty">Loading stored logs...</div>
                    </Show>
                    <For each={logs()[task().id] || []}>
                      {(entry) => {
                        const streamClass =
                          entry.stream === "stderr"
                            ? "log-err"
                            : entry.stream === "system"
                              ? "log-system"
                              : "";
                        return <div class={`log-line ${streamClass}`}>[{entry.stream}] {normalizeLogLine(entry.line)}</div>;
                      }}
                    </For>
                    <Show when={!isLoadingTaskDetails() && !taskDetailsError() && (logs()[task().id] || []).length === 0}>
                      <div class="log-empty">No logs captured for this run yet.</div>
                    </Show>
                  </div>
                </div>
              );
            }}
          </Show>
        </div>
      </div>

      <Show when={showNewTaskModal()}>
        <div class="modal-backdrop">
          <div class="modal">
            <h2 style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>Advanced Task</h2>
            <div class="form-group">
              <label class="form-label">Task Name</label>
              <input class="form-input" type="text" value={newTaskName()} onInput={(event) => setNewTaskName(event.currentTarget.value)} placeholder="e.g. Echo Server" />
            </div>
            <div class="form-group">
              <label class="form-label">Command</label>
              <input class="form-input" type="text" value={newTaskCommand()} onInput={(event) => setNewTaskCommand(event.currentTarget.value)} placeholder="e.g. node" />
            </div>
            <div class="form-group">
              <label class="form-label">Arguments (JSON Array)</label>
              <input class="form-input" type="text" value={newTaskArgs()} onInput={(event) => setNewTaskArgs(event.currentTarget.value)} placeholder='e.g. ["server.js", "--port", "8080"]' />
            </div>
            <div class="form-group">
              <label class="form-label">Required Tokens</label>
              <input class="form-input" type="text" value={newTaskRequiredTokens()} onInput={(event) => setNewTaskRequiredTokens(event.currentTarget.value)} placeholder='e.g. openai, minimax or ["openai"]' />
              <p class="form-hint">Optional. Missing tokens will block the task and show a Vault prompt.</p>
            </div>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowNewTaskModal(false)}>Cancel</button>
              <button class="btn btn-primary" onClick={() => void handleCreateTask()}>Create Task</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
