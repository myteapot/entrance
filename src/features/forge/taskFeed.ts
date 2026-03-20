import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type TaskStatus = "Pending" | "Running" | "Done" | "Failed" | "Cancelled" | "Blocked";
export type LogStream = "stdout" | "stderr" | "system";

export interface ForgeTask {
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

export const fetchForgeTasks = () => invoke<ForgeTask[]>("forge_list_tasks");

export const fetchForgeTaskDetails = (id: number) =>
  invoke<ForgeTaskDetails | null>("forge_get_task_details", { id });

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
