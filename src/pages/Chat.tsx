import { A } from "@solidjs/router";
import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import "./Chat.css";
import { listenToForgeTaskStatus } from "../features/forge/taskFeed";
import {
  fetchNotaRuntimeOverview,
  type NotaRuntimeOverview,
  type StoredDecisionRecord,
  type StoredNotaRuntimeTransaction,
} from "../features/nota/overview";

const CHAT_REFRESH_MS = 15_000;

const formatTimestamp = (value: string) =>
  new Date(value).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
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

const parseTransactionPayload = (payloadJson: string) => {
  try {
    const parsed = JSON.parse(payloadJson) as {
      issue_id?: string;
      issue_title?: string | null;
      worktree_path?: string;
      prompt_source?: string;
    };
    if (parsed && typeof parsed === "object") {
      return parsed;
    }
  } catch (error) {
    console.error("Failed to parse NOTA transaction payload", error);
  }

  return {};
};

const Chat = () => {
  const [overview, setOverview] = createSignal<NotaRuntimeOverview | null>(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [lastRefreshedAt, setLastRefreshedAt] = createSignal<string | null>(null);

  const loadOverview = async () => {
    try {
      setError(null);
      setOverview(await fetchNotaRuntimeOverview());
      setLastRefreshedAt(new Date().toISOString());
    } catch (loadError) {
      console.error("Failed to fetch NOTA runtime overview", loadError);
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setIsLoading(false);
    }
  };

  onMount(() => {
    void loadOverview();

    const timer = window.setInterval(() => {
      void loadOverview();
    }, CHAT_REFRESH_MS);

    onCleanup(() => window.clearInterval(timer));

    void (async () => {
      const unlistenStatus = await listenToForgeTaskStatus(() => {
        void loadOverview();
      });

      onCleanup(() => {
        unlistenStatus();
      });
    })();
  });

  const currentCheckpoint = createMemo(
    () => overview()?.checkpoints.checkpoints.find((checkpoint) => checkpoint.cadence_object.is_current) ?? null,
  );
  const recentTransactions = createMemo(() => overview()?.transactions.transactions.slice(0, 4) ?? []);
  const latestDecisions = createMemo(() => overview()?.decisions.decisions.slice(0, 4) ?? []);
  const recentCaptures = createMemo(() => overview()?.chat_captures.captures.slice(0, 4) ?? []);
  const refreshLine = createMemo(() => {
    const value = lastRefreshedAt();
    if (!value) {
      return "Syncing runtime DB...";
    }

    return `Refreshed ${formatRelativeTimestamp(value)}`;
  });

  const decisionLinkCountFor = (decision: StoredDecisionRecord) =>
    overview()?.decisions.links.filter((link) => link.src_decision_id === decision.id).length ?? 0;

  const checkpoint = () => currentCheckpoint();

  return (
    <section class="page page--chat">
      <header class="page__hero page__hero--dashboard page__hero--chat">
        <p class="page__eyebrow">Chat</p>
        <h2>Resume from runtime DB, not from human memory.</h2>
        <p class="page__summary">
          Chat now reads checkpoints, design decisions, Do transactions, and chat archive policy by
          default. This is a continuity surface, not a raw transcript replay.
        </p>
        <div class="dashboard-hero__meta" aria-label="Chat runtime status">
          <span class="dashboard-pill">
            Archive {overview()?.chat_policy.setting.archive_policy ?? "off"}
          </span>
          <span class="dashboard-pill">
            Checkpoints {overview()?.checkpoints.checkpoint_count ?? 0}
          </span>
          <span class="dashboard-pill">
            Decisions {overview()?.decisions.decision_count ?? 0}
          </span>
          <span class="dashboard-pill">
            Transactions {overview()?.transactions.transaction_count ?? 0}
          </span>
          <span class="dashboard-pill">{refreshLine()}</span>
        </div>
      </header>

      <Show when={error()}>
        {(message) => <div class="chat-callout chat-callout--error">{message()}</div>}
      </Show>

      <section class="dashboard-grid chat-grid" aria-label="NOTA runtime overview">
        <article class="dashboard-card dashboard-card--forge-widget dashboard-card--wide chat-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Current checkpoint</p>
            <A class="dashboard-card__link" href="/do">
              Open Do
            </A>
          </div>
          <Show
            when={checkpoint()}
            fallback={
              <p class="dashboard-card__empty">
                {isLoading() ? "Loading NOTA runtime checkpoint..." : "No checkpoint written yet."}
              </p>
            }
          >
            {(record) => (
              <>
                <div class="chat-card__headline">
                  <div>
                    <h3>{record().payload.stable_level}</h3>
                    <p>{record().cadence_object.summary}</p>
                  </div>
                  <span class="chat-status-pill">
                    {record().payload.human_continuity_bus}
                  </span>
                </div>

                <div class="chat-checkpoint-grid">
                  <section class="chat-detail-panel">
                    <span class="chat-detail-panel__label">Landed</span>
                    <ul class="chat-list">
                      <For each={record().payload.landed}>
                        {(item) => <li>{item}</li>}
                      </For>
                    </ul>
                  </section>

                  <section class="chat-detail-panel">
                    <span class="chat-detail-panel__label">Remaining</span>
                    <Show
                      when={record().payload.remaining.length > 0}
                      fallback={<p class="chat-detail-panel__empty">No remaining items recorded.</p>}
                    >
                      <ul class="chat-list">
                        <For each={record().payload.remaining}>
                          {(item) => <li>{item}</li>}
                        </For>
                      </ul>
                    </Show>
                  </section>
                </div>

                <div class="chat-meta-list">
                  <span>
                    Selected trunk: {record().payload.selected_trunk ?? "not pinned"}
                  </span>
                  <span>
                    Updated {formatTimestamp(record().cadence_object.updated_at)}
                  </span>
                  <Show when={record().payload.repo_context?.git_branch}>
                    <span>Branch {record().payload.repo_context?.git_branch}</span>
                  </Show>
                </div>

                <Show when={record().payload.next_start_hints.length > 0}>
                  <div class="chat-hint-strip">
                    <For each={record().payload.next_start_hints}>
                      {(hint) => <span class="chat-hint-chip">{hint}</span>}
                    </For>
                  </div>
                </Show>
              </>
            )}
          </Show>
        </article>

        <article class="dashboard-card dashboard-card--status chat-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Runtime posture</p>
            <span class="dashboard-live-indicator">
              <span class="dashboard-live-indicator__dot" aria-hidden="true" />
              DB-first
            </span>
          </div>
          <h3>Continuity is reconstructable</h3>
          <p>
            The runtime DB now carries checkpoint, decision, transaction, and archive policy cuts
            as distinct records.
          </p>
          <dl class="dashboard-stat-list">
            <div class="dashboard-stat">
              <dt>Current checkpoint</dt>
              <dd>{overview()?.checkpoints.current_checkpoint_id ?? "None"}</dd>
            </div>
            <div class="dashboard-stat">
              <dt>Current archive policy</dt>
              <dd>{overview()?.chat_policy.setting.archive_policy ?? "off"}</dd>
            </div>
            <div class="dashboard-stat">
              <dt>Captured chats</dt>
              <dd>{overview()?.chat_captures.capture_count ?? 0}</dd>
            </div>
          </dl>
        </article>

        <article class="dashboard-card dashboard-card--vault chat-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Chat archive</p>
            <A class="dashboard-card__link" href="/settings">
              Open Settings
            </A>
          </div>
          <h3>Raw chat is stored separately from design truth</h3>
          <p>
            Policy can stay `off`, keep only summaries, or keep full captures. None of these
            records become design decisions unless they are explicitly promoted.
          </p>
          <div class="chat-hint-strip">
            <span class="chat-hint-chip">
              Policy {overview()?.chat_policy.setting.archive_policy ?? "off"}
            </span>
            <span class="chat-hint-chip">
              Capture count {overview()?.chat_captures.capture_count ?? 0}
            </span>
          </div>
          <Show
            when={recentCaptures().length > 0}
            fallback={<p class="chat-detail-panel__empty">No chat captures archived yet.</p>}
          >
            <ul class="chat-feed">
              <For each={recentCaptures()}>
                {(capture) => (
                  <li class="chat-feed__item">
                    <div class="chat-feed__topline">
                      <strong>{capture.role}</strong>
                      <span>{capture.capture_mode}</span>
                    </div>
                    <p>{capture.summary || "No summary stored."}</p>
                    <div class="chat-meta-list">
                      <span>{formatTimestamp(capture.created_at)}</span>
                      <Show when={capture.linked_decision_id}>
                        <span>Decision {capture.linked_decision_id}</span>
                      </Show>
                    </div>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </article>

        <article class="dashboard-card dashboard-card--actions dashboard-card--wide chat-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Recent Do transactions</p>
            <A class="dashboard-card__link" href="/do">
              Continue in Do
            </A>
          </div>
          <Show
            when={recentTransactions().length > 0}
            fallback={<p class="dashboard-card__empty">No NOTA transactions recorded yet.</p>}
          >
            <ul class="chat-feed">
              <For each={recentTransactions()}>
                {(transaction: StoredNotaRuntimeTransaction) => {
                  const payload = parseTransactionPayload(transaction.payload_json);

                  return (
                    <li class="chat-feed__item">
                      <div class="chat-feed__topline">
                        <strong>{transaction.title}</strong>
                        <span class="chat-status-pill">{transaction.status}</span>
                      </div>
                      <p>
                        {payload.issue_id ?? transaction.transaction_kind}
                        {payload.issue_title ? ` - ${payload.issue_title}` : ""}
                      </p>
                      <div class="chat-meta-list">
                        <span>Task {transaction.forge_task_id ?? "pending"}</span>
                        <span>{formatTimestamp(transaction.updated_at)}</span>
                        <Show when={payload.worktree_path}>
                          <span>{payload.worktree_path}</span>
                        </Show>
                      </div>
                    </li>
                  );
                }}
              </For>
            </ul>
          </Show>
        </article>

        <article class="dashboard-card chat-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Design governance</p>
            <span class="dashboard-card__badge">
              {overview()?.decisions.link_count ?? 0} links
            </span>
          </div>
          <h3>Canonical decisions stay distinct</h3>
          <p>
            Supersession and conflict links stay in runtime DB, so future windows can reconstruct
            the current truth without replaying every conversation.
          </p>
          <Show
            when={latestDecisions().length > 0}
            fallback={<p class="chat-detail-panel__empty">No canonical design decisions yet.</p>}
          >
            <ul class="chat-feed">
              <For each={latestDecisions()}>
                {(decision: StoredDecisionRecord) => (
                  <li class="chat-feed__item">
                    <div class="chat-feed__topline">
                      <strong>{decision.title}</strong>
                      <span>{decision.decision_status}</span>
                    </div>
                    <p>{decision.statement}</p>
                    <div class="chat-meta-list">
                      <span>{decision.decision_type || "decision"}</span>
                      <span>{decisionLinkCountFor(decision)} outgoing links</span>
                      <span>{formatTimestamp(decision.updated_at)}</span>
                    </div>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </article>
      </section>
    </section>
  );
};

export default Chat;
