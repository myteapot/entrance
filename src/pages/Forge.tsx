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
  stream: "stdout" | "stderr";
  line: string;
}

export default function Forge() {
  const [tasks, setTasks] = createSignal<ForgeTask[]>([]);
  const [selectedTaskId, setSelectedTaskId] = createSignal<number | null>(null);
  const [logs, setLogs] = createSignal<{ [taskId: number]: string[] }>({});
  
  // New task form
  const [showNewTaskModal, setShowNewTaskModal] = createSignal(false);
  const [newTaskName, setNewTaskName] = createSignal("");
  const [newTaskCommand, setNewTaskCommand] = createSignal("");
  const [newTaskArgs, setNewTaskArgs] = createSignal("");
  
  const fetchTasks = async () => {
    try {
      const result = await invoke<ForgeTask[]>("forge_list_tasks");
      setTasks(result);
    } catch (e) {
      console.error("Failed to fetch tasks", e);
    }
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
          const currentLogs = prev[payload.id] || [];
          // Some backend lines are stringified strings, let's parse if it's JSON string
          let rawLine = payload.line;
          try {
             let parsed = JSON.parse(rawLine);
             if (typeof parsed === 'string') rawLine = parsed;
          } catch(e){}
          return { ...prev, [payload.id]: [...currentLogs, `[${payload.stream}] ${rawLine}`] };
        });
      } catch(e) {}
    });
    
    onCleanup(() => {
      unlistenStatus();
      unlistenOutput();
    });
  });

  const handleCreateTask = async () => {
    if (!newTaskName() || !newTaskCommand()) return;
    
    let argsArray: string[] = [];
    try {
        if (newTaskArgs().trim()) {
            argsArray = JSON.parse(newTaskArgs());
            if (!Array.isArray(argsArray)) throw new Error("Args must be an array");
        }
    } catch (e) {
       // fallback to split by space if json parse fails
       argsArray = newTaskArgs().split(' ').filter(Boolean);
    }

    try {
      await invoke("forge_create_task", {
        name: newTaskName(),
        command: newTaskCommand(),
        args: JSON.stringify(argsArray),
      });
      setShowNewTaskModal(false);
      setNewTaskName("");
      setNewTaskCommand("");
      setNewTaskArgs("");
      await fetchTasks(); // Refresh
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
      setNewTaskName(task.name + " (Restart)");
      setNewTaskCommand(task.command);
      setNewTaskArgs(task.args);
      setShowNewTaskModal(true);
  };

  let logContainerRef: HTMLDivElement | undefined;
  createEffect(() => {
    // Read the evaluated property to trigger tracking
    void logs()[selectedTaskId() as number];
    if (logContainerRef) {
      // scroll to bottom smoothly
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
                        <button class="btn-icon" onClick={(e) => { e.stopPropagation(); handleRestartTask(task); }}>Restart</button>
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
                <For each={logs()[selectedTaskId() as number] || []}>
                  {(line) => {
                     const isErr = line.startsWith("[stderr]");
                     return <div class={`log-line ${isErr ? "log-err" : ""}`}>{line}</div>
                  }}
                </For>
                <Show when={(logs()[selectedTaskId() as number] || []).length === 0}>
                   <div class="log-empty">Waiting for logs...</div>
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
