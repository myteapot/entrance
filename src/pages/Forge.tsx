import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./Forge.css";

type TaskStatus = "Pending" | "Running" | "Done" | "Failed" | "Cancelled" | "Blocked";
type LogStream = "stdout" | "stderr" | "system";

interface ForgeTask {
  id: number;
  name: string;
  command: string;
  args: string;
  required_tokens: string;
  status: TaskStatus;
  status_message: string | null;
  exit_code: number | null;
  created_at: string;
  finished_at: string | null;
}

interface LogLine {
  id: number;
  task_id: number;
  stream: LogStream;
  line: string;
  created_at: string | null;
}

interface ForgeTaskDetails extends ForgeTask {
  logs: LogLine[];
}

interface ForgeTaskStatusEvent {
  id: number;
  status: TaskStatus;
  status_message: string | null;
  exit_code: number | null;
  finished_at: string | null;
}

export default function Forge() {
  const [tasks, setTasks] = createSignal<ForgeTask[]>([]);
  const [selectedTaskId, setSelectedTaskId] = createSignal<number | null>(null);
  const [logs, setLogs] = createSignal<Record<number, LogLine[]>>({});
  const [isLoadingTaskDetails, setIsLoadingTaskDetails] = createSignal(false);
  const [taskDetailsError, setTaskDetailsError] = createSignal<string | null>(null);
  const [restartingTaskId, setRestartingTaskId] = createSignal<number | null>(null);

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
    setTasks((prev) => {
      const exists = prev.some((entry) => entry.id === task.id);
      if (!exists) {
        return [task, ...prev];
      }

      return prev.map((entry) => (entry.id === task.id ? { ...entry, ...task } : entry));
    });
  };

  const selectedTask = () =>
    tasks().find((task) => task.id === selectedTaskId()) ?? null;

  const fetchTasks = async () => {
    try {
      const result = await invoke<ForgeTask[]>("forge_list_tasks");
      setTasks(result);
    } catch (error) {
      console.error("Failed to fetch tasks", error);
    }
  };

  const loadTaskDetails = async (taskId: number) => {
    const requestId = ++activeTaskDetailsRequest;
    setIsLoadingTaskDetails(true);
    setTaskDetailsError(null);

    try {
      const details = await invoke<ForgeTaskDetails | null>("forge_get_task_details", { id: taskId });
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

  onMount(async () => {
    await fetchTasks();

    const unlistenStatus = await listen<string>("forge:task_status", (event) => {
      try {
        const payload = JSON.parse(event.payload) as ForgeTaskStatusEvent;
        setTasks((prev) =>
          prev.map((task) =>
            task.id === payload.id
              ? {
                  ...task,
                  status: payload.status,
                  status_message: payload.status_message,
                  exit_code: payload.exit_code,
                  finished_at: payload.finished_at,
                }
              : task,
          ),
        );
      } catch (error) {
        console.error("Failed to process forge status event", error);
      }
    });

    const unlistenOutput = await listen<string>("forge:task_output", (event) => {
      try {
        const payload = JSON.parse(event.payload) as LogLine;
        setLogs((prev) => {
          const taskId = payload.task_id;
          const currentLogs = prev[taskId] ?? [];
          return { ...prev, [taskId]: mergeLogLines(currentLogs, [payload]) };
        });
      } catch (error) {
        console.error("Failed to process forge output event", error);
      }
    });

    onCleanup(() => {
      unlistenStatus();
      unlistenOutput();
    });
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
      await createTask(task.name, task.command, task.args, task.required_tokens);
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
          <h1 class="forge-title">Forge</h1>
          <p class="forge-subtitle">Task runner and real-time execution engine logs.</p>
        </div>
        <button class="btn btn-primary" onClick={() => setShowNewTaskModal(true)}>+ New Task</button>
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
              <li class="empty-state">No tasks created yet.</li>
            </Show>
          </ul>
        </div>

        <div class="forge-main">
          <Show when={selectedTask()} fallback={<div class="empty-selection">Select a task to view details and logs.</div>}>
            {(task) => {
              const requiredTokens = () => parseStoredRequiredTokens(task().required_tokens);

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
                      <Show when={task().finished_at}>
                        <span>Finished: {new Date(task().finished_at as string).toLocaleString()}</span>
                      </Show>
                    </div>
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
                      <div class="log-empty">No logs captured for this task yet.</div>
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
            <h2 style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>New Task</h2>
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
              <button class="btn btn-primary" onClick={() => void handleCreateTask()}>Spawn Task</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
