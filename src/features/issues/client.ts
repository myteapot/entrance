import { invoke } from "@tauri-apps/api/core";

export type IssueStatus = "todo" | "in_progress" | "in_review" | "done" | "cancelled";
export type IssuePriority = "none" | "urgent" | "high" | "medium" | "low";

export interface Issue {
  id: number;
  issue_key: string;
  title: string;
  description: string;
  status: IssueStatus;
  priority: IssuePriority;
  labels: string;
  assignee: string;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
}

export interface IssueComment {
  id: number;
  issue_id: number;
  author: string;
  body: string;
  created_at: string;
}

export const fetchIssues = (status?: IssueStatus) =>
  invoke<Issue[]>("issue_list", status ? { status } : {});

export const fetchIssue = (issueKey: string) =>
  invoke<Issue | null>("issue_get", { issueKey });

export const createIssue = (
  title: string,
  description?: string,
  priority?: IssuePriority,
  assignee?: string,
) =>
  invoke<Issue>("issue_create", { title, description, priority, assignee });

export const updateIssueStatus = (issueKey: string, status: IssueStatus) =>
  invoke<Issue>("issue_update_status", { issueKey, status });

export const updateIssue = (
  issueKey: string,
  updates: {
    title?: string;
    description?: string;
    priority?: string;
    labels?: string;
    assignee?: string;
  },
) => invoke<Issue>("issue_update", { issueKey, ...updates });

export const deleteIssue = (issueKey: string) =>
  invoke<void>("issue_delete", { issueKey });

export const addIssueComment = (issueKey: string, author: string, body: string) =>
  invoke<IssueComment>("issue_add_comment", { issueKey, author, body });

export const fetchIssueComments = (issueKey: string) =>
  invoke<IssueComment[]>("issue_list_comments", { issueKey });

export const STATUS_COLUMNS: { key: IssueStatus; label: string }[] = [
  { key: "todo", label: "Todo" },
  { key: "in_progress", label: "In Progress" },
  { key: "in_review", label: "In Review" },
  { key: "done", label: "Done" },
  { key: "cancelled", label: "Cancelled" },
];

export const PRIORITY_OPTIONS: { key: IssuePriority; label: string; icon: string }[] = [
  { key: "urgent", label: "Urgent", icon: "🔴" },
  { key: "high", label: "High", icon: "🟠" },
  { key: "medium", label: "Medium", icon: "🟡" },
  { key: "low", label: "Low", icon: "🟢" },
  { key: "none", label: "None", icon: "⚪" },
];
