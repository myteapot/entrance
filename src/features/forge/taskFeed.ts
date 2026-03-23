import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type TaskStatus = "Pending" | "Running" | "Done" | "Failed" | "Cancelled" | "Blocked";
export type LogStream = "stdout" | "stderr" | "system";

export interface ForgeTask {
  id: number;
  name: string;
  command: string;
  args: string;
  working_dir: string | null;
  stdin_text: string | null;
  required_tokens: string;
  metadata: string;
  status: TaskStatus;
  status_message: string | null;
  exit_code: number | null;
  created_at: string;
  finished_at: string | null;
}

export interface LogLine {
  id: number;
  task_id: number;
  stream: LogStream;
  line: string;
  created_at: string | null;
}

export interface ForgeTaskDetails extends ForgeTask {
  logs: LogLine[];
}

export interface ForgeTaskStatusEvent {
  id: number;
  status: TaskStatus;
  status_message: string | null;
  exit_code: number | null;
  finished_at: string | null;
}

export interface ForgeTaskMetadata {
  kind?: string | null;
  issue_id?: string | null;
  worktree_path?: string | null;
  model?: string | null;
}

export interface PreparedAgentDispatch {
  issue_id: string;
  issue_status: string;
  issue_status_source: string;
  issue_title: string | null;
  project_root: string;
  worktree_path: string;
  prompt_source: string;
  prompt: string;
}

export const fetchForgeTasks = () => invoke<ForgeTask[]>("forge_list_tasks");

export const fetchForgeTaskDetails = (id: number) =>
  invoke<ForgeTaskDetails | null>("forge_get_task_details", { id });

export const prepareForgeAgentDispatch = (projectDir?: string) =>
  invoke<PreparedAgentDispatch>("forge_prepare_agent_dispatch", { projectDir });

export const dispatchForgeAgent = (
  issueId: string,
  worktreePath: string,
  model: string,
  prompt: string,
  requiredTokens?: string[],
  agentCommand?: string,
) =>
  invoke<number>("forge_dispatch_agent", {
    issueId,
    worktreePath,
    model,
    prompt,
    requiredTokens,
    agentCommand,
  });

export const listenToForgeTaskStatus = (
  handler: (payload: ForgeTaskStatusEvent) => void,
) =>
  listen<string>("forge:task_status", (event) => {
    try {
      handler(JSON.parse(event.payload) as ForgeTaskStatusEvent);
    } catch (error) {
      console.error("Failed to process forge status event", error);
    }
  });

export const listenToForgeTaskOutput = (
  handler: (payload: LogLine) => void,
) =>
  listen<string>("forge:task_output", (event) => {
    try {
      handler(JSON.parse(event.payload) as LogLine);
    } catch (error) {
      console.error("Failed to process forge output event", error);
    }
  });

export const mergeForgeTask = (tasks: ForgeTask[], nextTask: ForgeTask) => {
  const existingIndex = tasks.findIndex((task) => task.id === nextTask.id);
  if (existingIndex === -1) {
    return [nextTask, ...tasks];
  }

  return tasks.map((task) => (task.id === nextTask.id ? { ...task, ...nextTask } : task));
};

export const applyForgeTaskStatusEvent = (
  tasks: ForgeTask[],
  payload: ForgeTaskStatusEvent,
) => {
  let seen = false;
  const nextTasks = tasks.map((task) => {
    if (task.id !== payload.id) {
      return task;
    }

    seen = true;
    return {
      ...task,
      status: payload.status,
      status_message: payload.status_message,
      exit_code: payload.exit_code,
      finished_at: payload.finished_at,
    };
  });

  return seen ? nextTasks : null;
};

export const parseForgeTaskMetadata = (metadata: string): ForgeTaskMetadata => {
  try {
    const parsed = JSON.parse(metadata) as ForgeTaskMetadata;
    if (parsed && typeof parsed === "object") {
      return parsed;
    }
  } catch (error) {
    console.error("Failed to parse forge task metadata", error);
  }

  return {};
};
