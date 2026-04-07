import { createSignal, onMount, For, Show, type Component } from "solid-js";
import {
  fetchIssues,
  createIssue,
  updateIssueStatus,
  fetchIssueComments,
  STATUS_COLUMNS,
  PRIORITY_OPTIONS,
  type Issue,
  type IssueStatus,
  type IssueComment,
  type IssuePriority,
} from "../features/issues/client";
import "./Issues.css";

const Issues: Component = () => {
  const [issues, setIssues] = createSignal<Issue[]>([]);
  const [selected, setSelected] = createSignal<Issue | null>(null);
  const [comments, setComments] = createSignal<IssueComment[]>([]);
  const [showCreate, setShowCreate] = createSignal(false);
  const [newTitle, setNewTitle] = createSignal("");
  const [newDesc, setNewDesc] = createSignal("");
  const [newPriority, setNewPriority] = createSignal<IssuePriority>("none");
  const [loading, setLoading] = createSignal(false);

  const load = async () => {
    try {
      const data = await fetchIssues();
      setIssues(data);
    } catch (e) {
      console.error("Failed to load issues", e);
    }
  };

  onMount(load);

  const issuesByStatus = (status: IssueStatus) =>
    issues().filter((i) => i.status === status);

  const onSelectIssue = async (issue: Issue) => {
    setSelected(issue);
    try {
      const c = await fetchIssueComments(issue.issue_key);
      setComments(c);
    } catch {
      setComments([]);
    }
  };

  const onStatusChange = async (issueKey: string, newStatus: IssueStatus) => {
    try {
      const updated = await updateIssueStatus(issueKey, newStatus);
      setIssues((prev) =>
        prev.map((i) => (i.issue_key === updated.issue_key ? updated : i)),
      );
      if (selected()?.issue_key === updated.issue_key) {
        setSelected(updated);
      }
    } catch (e) {
      console.error("Failed to update issue status", e);
    }
  };

  const onCreateIssue = async () => {
    const title = newTitle().trim();
    if (!title) return;
    setLoading(true);
    try {
      const issue = await createIssue(title, newDesc() || undefined, newPriority());
      setIssues((prev) => [issue, ...prev]);
      setNewTitle("");
      setNewDesc("");
      setNewPriority("none");
      setShowCreate(false);
    } catch (e) {
      console.error("Failed to create issue", e);
    }
    setLoading(false);
  };

  const formatTime = (iso: string) => {
    try {
      return new Date(iso).toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
      });
    } catch {
      return iso;
    }
  };

  return (
    <div class="issues-page">
      <div class="issues-header">
        <div class="issues-header-left">
          <h1>Issues</h1>
          <p>Built-in issue tracker — replaces Linear</p>
        </div>
        <button class="issue-create-btn" onClick={() => setShowCreate(true)}>
          + New Issue
        </button>
      </div>

      {/* Board Strip */}
      <div class="board-strip">
        <For each={STATUS_COLUMNS}>
          {(col) => {
            const colIssues = () => issuesByStatus(col.key);
            return (
              <div class="board-column">
                <div class="board-column-header">
                  <span class="board-column-title">{col.label}</span>
                  <span class="board-column-count">{colIssues().length}</span>
                </div>
                <div class="board-column-body">
                  <Show
                    when={colIssues().length > 0}
                    fallback={<div class="board-empty-hint">No issues</div>}
                  >
                    <For each={colIssues()}>
                      {(issue) => (
                        <div
                          class={`issue-card ${selected()?.id === issue.id ? "selected" : ""}`}
                          onClick={() => onSelectIssue(issue)}
                        >
                          <span class="issue-card-key">{issue.issue_key}</span>
                          <span class="issue-card-title">{issue.title}</span>
                          <div class="issue-card-meta">
                            <Show when={issue.priority !== "none"}>
                              <span
                                class={`issue-priority-badge priority-${issue.priority}`}
                              >
                                {PRIORITY_OPTIONS.find((p) => p.key === issue.priority)
                                  ?.icon}{" "}
                                {issue.priority}
                              </span>
                            </Show>
                            <Show when={issue.assignee}>
                              <span class="issue-assignee-badge">
                                → {issue.assignee}
                              </span>
                            </Show>
                          </div>
                        </div>
                      )}
                    </For>
                  </Show>
                </div>
              </div>
            );
          }}
        </For>
      </div>

      {/* Detail Slide-over */}
      <Show when={selected()}>
        {(sel) => (
          <div class="issue-detail-overlay">
            <button
              class="issue-detail-close"
              onClick={() => setSelected(null)}
            >
              ✕
            </button>
            <div class="issue-detail-header">
              <span class="issue-detail-key">{sel().issue_key}</span>
              <h2 class="issue-detail-title">{sel().title}</h2>
              <div class="issue-detail-status-row">
                <select
                  class="issue-status-select"
                  value={sel().status}
                  onChange={(e) =>
                    onStatusChange(
                      sel().issue_key,
                      e.currentTarget.value as IssueStatus,
                    )
                  }
                >
                  <For each={STATUS_COLUMNS}>
                    {(col) => <option value={col.key}>{col.label}</option>}
                  </For>
                </select>
                <select
                  class="issue-status-select"
                  value={sel().priority}
                  onChange={() => {}}
                >
                  <For each={PRIORITY_OPTIONS}>
                    {(p) => (
                      <option value={p.key}>
                        {p.icon} {p.label}
                      </option>
                    )}
                  </For>
                </select>
              </div>
            </div>
            <div class="issue-detail-body">
              <div
                class={`issue-description ${sel().description ? "" : "empty"}`}
              >
                {sel().description || "No description provided."}
              </div>
              <div class="issue-detail-props">
                <span class="issue-prop-label">Created</span>
                <span class="issue-prop-value">
                  {formatTime(sel().created_at)}
                </span>
                <span class="issue-prop-label">Updated</span>
                <span class="issue-prop-value">
                  {formatTime(sel().updated_at)}
                </span>
                <Show when={sel().closed_at}>
                  <span class="issue-prop-label">Closed</span>
                  <span class="issue-prop-value">
                    {formatTime(sel().closed_at!)}
                  </span>
                </Show>
                <Show when={sel().assignee}>
                  <span class="issue-prop-label">Assignee</span>
                  <span class="issue-prop-value">{sel().assignee}</span>
                </Show>
              </div>

              {/* Comments */}
              <div class="issue-comments-section">
                <h3>Comments ({comments().length})</h3>
                <For each={comments()}>
                  {(c) => (
                    <div class="issue-comment">
                      <div class="issue-comment-header">
                        <span class="issue-comment-author">{c.author}</span>
                        <span class="issue-comment-time">
                          {formatTime(c.created_at)}
                        </span>
                      </div>
                      <div class="issue-comment-body">{c.body}</div>
                    </div>
                  )}
                </For>
                <Show when={comments().length === 0}>
                  <div class="board-empty-hint">No comments yet</div>
                </Show>
              </div>
            </div>
          </div>
        )}
      </Show>

      {/* Create Modal */}
      <Show when={showCreate()}>
        <div class="modal-backdrop" onClick={() => setShowCreate(false)}>
          <div class="modal" onClick={(e) => e.stopPropagation()}>
            <h2 style={{ color: "var(--text-primary)", margin: "0 0 var(--space-4)" }}>
              New Issue
            </h2>
            <div class="form-group">
              <label class="form-label">Title</label>
              <input
                class="form-input"
                placeholder="Issue title..."
                value={newTitle()}
                onInput={(e) => setNewTitle(e.currentTarget.value)}
                autofocus
              />
            </div>
            <div class="form-group">
              <label class="form-label">Description</label>
              <textarea
                class="form-input"
                placeholder="Optional description..."
                rows={3}
                value={newDesc()}
                onInput={(e) => setNewDesc(e.currentTarget.value)}
                style={{ resize: "vertical" }}
              />
            </div>
            <div class="form-group">
              <label class="form-label">Priority</label>
              <select
                class="form-input"
                value={newPriority()}
                onChange={(e) =>
                  setNewPriority(e.currentTarget.value as IssuePriority)
                }
              >
                <For each={PRIORITY_OPTIONS}>
                  {(p) => (
                    <option value={p.key}>
                      {p.icon} {p.label}
                    </option>
                  )}
                </For>
              </select>
            </div>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowCreate(false)}>
                Cancel
              </button>
              <button
                class="btn btn-primary"
                onClick={onCreateIssue}
                disabled={loading() || !newTitle().trim()}
              >
                {loading() ? "Creating..." : "Create"}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default Issues;
