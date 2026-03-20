import { createSignal, createEffect, For, Show, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./Forge.css";

type TaskStatus = "Pending" | "Running" | "Done" | "Failed" | "Cancelled";

interface ForgeTask {
  id: number;
  name: string;
  command: string;
  args: string;
  status: TaskStatus;
  exit_code: number | null;
  created_at: string;
  finished_at: string | null;
}

interface LogLine {
  id: number;
  task_id: number;
  stream: "stdout" | "stderr";
  line: string;
  created_at: string | null;
}

interface ForgeTaskDetails {
  id: number;
  name: string;
  command: string;
  args: string;
  status: TaskStatus;
  exit_code: number | null;
  created_at: string;
  finished_at: string | null;
  logs: LogLine[];
}

export default function Forge() {
  const [tasks, setTasks] = createSignal<ForgeTask[]>([]);
  const [selectedTaskId, setSelectedTaskId] = createSignal<number | null>(null);
  const [logs, setLogs] = createSignal<Record<number, LogLine[]>>({});
  const [isLoadingTaskDetails, setIsLoadingTaskDetails] = createSignal(false);
  const [taskDetailsError, setTaskDetailsError] = createSignal<string | null>(null);
  const [restartingTaskId, setRestartingTaskId] = createSignal<number | null>(null);
  
  // New task form
  const [showNewTaskModal, setShowNewTaskModal] = createSignal(false);
  const [newTaskName, setNewTaskName] = createSignal("");
  const [newTaskCommand, setNewTaskCommand] = createSignal("");
  const [newTaskArgs, setNewTaskArgs] = createSignal("");
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

      return prev.map((entry) => (entry.id === task.id ? task : entry));
    });
  };
  
  const fetchTasks = async () => {
    try {
      const result = await invoke<ForgeTask[]>("forge_list_tasks");
      setTasks(result);
    } catch (e) {
      console.error("Failed to fetch tasks", e);
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

  const createTask = async (name: string, command: string, rawArgs: string) => {
    const argsArray = parseArgsInput(rawArgs);
    const id = await invoke<number>("forge_create_task", {
      name,
      command,
      args: JSON.stringify(argsArray),
    });

    await fetchTasks();
    setSelectedTaskId(id);
    await loadTaskDetails(id);
    return id;
  };

  onMount(async () => {
    await fetchTasks();
    
    // Listen to status updates
    const unlistenStatus = await listen<string>("forge:task_status", (event) => {
      try {
        const payload = JSON.parse(event.payload);
        setTasks((prev) => prev.map((t) => {
          if (t.id === payload.id) {
            return { ...t, status: payload.status, exit_code: payload.exit_code ?? t.exit_code, finished_at: payload.finished_at ?? t.finished_at };
          }
          return t;
        }));
      } catch (e) {}
    });
    
    // Listen to output logs
    const unlistenOutput = await listen<string>("forge:task_output", (event) => {
      try {
        const payload = JSON.parse(event.payload) as LogLine;
        setLogs((prev) => {
          const taskId = payload.task_id;
          const currentLogs = prev[taskId] ?? [];
          return { ...prev, [taskId]: mergeLogLines(currentLogs, [payload]) };
        });
      } catch(e) {}
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
    if (!newTaskName() || !newTaskCommand()) return;

    try {
      await createTask(newTaskName(), newTaskCommand(), newTaskArgs());
      setShowNewTaskModal(false);
      setNewTaskName("");
      setNewTaskCommand("");
      setNewTaskArgs("");
    } catch (e) {
      console.error(e);
      alert("Error: " + e);
    }
  };

  const handleCancelTask = async (id: number) => {
    try {
      await invoke("forge_cancel_task", { id });
    } catch (e) {
      console.error(e);
      alert("Error cancelling: " + e);
    }
  };
  
  const handleRestartTask = async (task: ForgeTask) => {
      setRestartingTaskId(task.id);
      try {
        await createTask(task.name, task.command, task.args);
      } catch (e) {
        console.error("Failed to restart task", e);
        alert("Error restarting: " + e);
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
        behavior: "smooth"
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
                  <div class="task-item-actions">
                     <Show when={task.status === "Running" || task.status === "Pending"}>
                        <button class="btn-icon" onClick={(e) => { e.stopPropagation(); handleCancelTask(task.id); }}>Stop</button>
                     </Show>
                     <Show when={task.status === "Failed" || task.status === "Cancelled" || task.status === "Done"}>
                        <button
                          class="btn-icon"
                          disabled={restartingTaskId() === task.id}
                          onClick={(e) => { e.stopPropagation(); void handleRestartTask(task); }}
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
          <Show when={selectedTaskId()} fallback={<div class="empty-selection">Select a task to view details and logs.</div>}>
            <div class="log-panel">
              <div class="log-header">
                <h3>Execution Logs (Task ID: {selectedTaskId()})</h3>
              </div>
              <div class="log-stream" ref={logContainerRef}>
                <Show when={taskDetailsError()}>
                  <div class="log-empty log-error">{taskDetailsError()}</div>
                </Show>
                <Show when={isLoadingTaskDetails() && (logs()[selectedTaskId() as number] || []).length === 0}>
                  <div class="log-empty">Loading stored logs...</div>
                </Show>
                <For each={logs()[selectedTaskId() as number] || []}>
                  {(entry) => {
                     const isErr = entry.stream === "stderr";
                     return <div class={`log-line ${isErr ? "log-err" : ""}`}>[{entry.stream}] {normalizeLogLine(entry.line)}</div>
                  }}
                </For>
                <Show when={!isLoadingTaskDetails() && !taskDetailsError() && (logs()[selectedTaskId() as number] || []).length === 0}>
                   <div class="log-empty">No logs captured for this task yet.</div>
                </Show>
              </div>
            </div>
          </Show>
        </div>
      </div>

      {/* New Task Modal */}
      <Show when={showNewTaskModal()}>
        <div class="modal-backdrop">
          <div class="modal">
            <h2 style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>New Task</h2>
            <div class="form-group">
              <label class="form-label">Task Name</label>
              <input class="form-input" type="text" value={newTaskName()} onInput={(e) => setNewTaskName(e.currentTarget.value)} placeholder="e.g. Echo Server" />
            </div>
            <div class="form-group">
              <label class="form-label">Command</label>
              <input class="form-input" type="text" value={newTaskCommand()} onInput={(e) => setNewTaskCommand(e.currentTarget.value)} placeholder="e.g. node" />
            </div>
            <div class="form-group">
              <label class="form-label">Arguments (JSON Array)</label>
              <input class="form-input" type="text" value={newTaskArgs()} onInput={(e) => setNewTaskArgs(e.currentTarget.value)} placeholder='e.g. ["server.js", "--port", "8080"]' />
            </div>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowNewTaskModal(false)}>Cancel</button>
              <button class="btn btn-primary" onClick={handleCreateTask}>Spawn Task</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
