import { A } from "@solidjs/router";
import { open } from "@tauri-apps/plugin-dialog";
import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import "./Chat.css";
import {
  importLandingSnapshot,
  type LandingImportReport,
} from "../features/landing/client";
import { listenToForgeTaskStatus } from "../features/forge/taskFeed";
import {
  fetchNotaRuntimeOverview,
  fetchNotaRuntimeStatus,
  type NotaCheckpointRecord,
  type NotaRuntimeOverview,
  type NotaRuntimeStatus,
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

const renderCheckpointTitle = (checkpoint: NotaCheckpointRecord | null) => {
  if (!checkpoint) {
    return "No active checkpoint yet";
  }

  return `Checkpoint ${checkpoint.cadence_object.id}: ${checkpoint.cadence_object.title}`;
};

const Chat = () => {
  const [overview, setOverview] = createSignal<NotaRuntimeOverview | null>(null);
  const [status, setStatus] = createSignal<NotaRuntimeStatus | null>(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [lastRefreshedAt, setLastRefreshedAt] = createSignal<string | null>(null);
  const [isImporting, setIsImporting] = createSignal(false);
  const [importTone, setImportTone] = createSignal<"success" | "error" | null>(null);
  const [importFeedback, setImportFeedback] = createSignal<string | null>(null);
  const [lastImportReport, setLastImportReport] = createSignal<LandingImportReport | null>(null);

  const loadFrontDoor = async () => {
    try {
      setError(null);
      const [nextOverview, nextStatus] = await Promise.all([
        fetchNotaRuntimeOverview(),
        fetchNotaRuntimeStatus(),
      ]);
      setOverview(nextOverview);
      setStatus(nextStatus);
      setLastRefreshedAt(new Date().toISOString());
    } catch (loadError) {
      console.error("Failed to fetch NOTA runtime front door", loadError);
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setIsLoading(false);
    }
  };

  const chooseImportSnapshot = async () => {
    const selected = await open({
      title: "Import Entrance snapshot",
      multiple: false,
      directory: false,
      filters: [{ name: "JSON snapshot", extensions: ["json"] }],
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    setIsImporting(true);
    setImportTone(null);
    setImportFeedback(null);

    try {
      const report = await importLandingSnapshot(selected);
      setLastImportReport(report);
      setImportTone("success");
      setImportFeedback(
        `Imported ${report.imported_planning_item_count} planning items from ${report.source_workspace}.`,
      );
      await loadFrontDoor();
    } catch (importError) {
      console.error("Failed to import snapshot from Chat front door", importError);
      setImportTone("error");
      setImportFeedback(
        importError instanceof Error ? importError.message : String(importError),
      );
    } finally {
      setIsImporting(false);
    }
  };

  onMount(() => {
    void loadFrontDoor();

    const timer = window.setInterval(() => {
      void loadFrontDoor();
    }, CHAT_REFRESH_MS);

    onCleanup(() => window.clearInterval(timer));

    void (async () => {
      const unlistenStatus = await listenToForgeTaskStatus(() => {
        void loadFrontDoor();
      });

      onCleanup(() => {
        unlistenStatus();
      });
    })();
  });

  const currentCheckpoint = createMemo(
    () =>
      status()?.current_checkpoint ??
      overview()?.checkpoints.checkpoints.find((checkpoint) => checkpoint.cadence_object.is_current) ??
      null,
  );
  const frontDoor = createMemo(() => status()?.front_door ?? overview()?.front_door ?? null);
  const recentTransactions = createMemo(
    () => overview()?.transactions.transactions.slice(0, 3) ?? [],
  );
  const latestDecisions = createMemo(() => overview()?.decisions.decisions.slice(0, 3) ?? []);
  const recentCaptures = createMemo(() => overview()?.chat_captures.captures.slice(0, 2) ?? []);
  const latestDecision = createMemo(() => latestDecisions()[0] ?? null);
  const refreshLine = createMemo(() => {
    const value = lastRefreshedAt();
    if (!value) {
      return "Syncing runtime truth...";
    }

    return `Refreshed ${formatRelativeTimestamp(value)}`;
  });

  const decisionLinkCountFor = (decision: StoredDecisionRecord) =>
    overview()?.decisions.links.filter((link) => link.src_decision_id === decision.id).length ?? 0;

  return (
    <section class="page page--chat">
      <header class="page__hero page__hero--dashboard page__hero--chat">
        <p class="page__eyebrow">Native Front Door</p>
        <h2>Chat-first Entrance, grounded in live NOTA runtime truth.</h2>
        <p class="page__summary">
          This front door opens with the active checkpoint, the next honest runtime move, and a
          compact mission rail. It is reading the same NOTA truth plane as `status` and
          `overview`, not a second GUI scheduler.
        </p>
        <div class="dashboard-hero__meta" aria-label="Chat runtime status">
          <span class="dashboard-pill">
            Archive {status()?.chat_policy.setting.archive_policy ?? "off"}
          </span>
          <span class="dashboard-pill">
            Checkpoint {status()?.current_checkpoint_id ?? "none"}
          </span>
          <span class="dashboard-pill">
            Decisions {status()?.decision_count ?? 0}
          </span>
          <span class="dashboard-pill">
            Transactions {status()?.transaction_count ?? 0}
          </span>
          <span class="dashboard-pill">{refreshLine()}</span>
        </div>
      </header>

      <Show when={error()}>
        {(message) => <div class="chat-callout chat-callout--error">{message()}</div>}
      </Show>

      <Show when={importFeedback()}>
        {(message) => (
          <div
            class={`chat-callout ${
              importTone() === "success" ? "chat-callout--success" : "chat-callout--error"
            }`}
          >
            {message()}
          </div>
        )}
      </Show>

      <section class="front-door-layout" aria-label="Native front door">
        <article class="dashboard-card dashboard-card--forge-widget chat-card front-door-shell">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Chat shell</p>
            <span class="chat-status-pill">{frontDoor()?.posture ?? "Loading shell"}</span>
          </div>

          <div class="front-door-shell__headline">
            <div>
              <h3>{renderCheckpointTitle(currentCheckpoint())}</h3>
              <p>{frontDoor()?.summary ?? "Loading front-door continuity from the runtime DB..."}</p>
            </div>
          </div>

          <ul class="front-door-thread" aria-label="Front door runtime summary">
            <li class="front-door-bubble front-door-bubble--runtime">
              <span class="front-door-bubble__label">Runtime</span>
              <strong>{renderCheckpointTitle(currentCheckpoint())}</strong>
              <p>
                {currentCheckpoint()?.cadence_object.summary ??
                  "The current checkpoint will appear here once runtime continuity is available."}
              </p>
              <div class="chat-meta-list">
                <Show when={currentCheckpoint()?.payload.selected_trunk}>
                  <span>Trunk {currentCheckpoint()?.payload.selected_trunk}</span>
                </Show>
                <Show when={currentCheckpoint()?.cadence_object.updated_at}>
                  {(updatedAt) => <span>Updated {formatTimestamp(updatedAt())}</span>}
                </Show>
                <Show when={currentCheckpoint()?.payload.repo_context?.git_branch}>
                  <span>Branch {currentCheckpoint()?.payload.repo_context?.git_branch}</span>
                </Show>
              </div>
            </li>

            <li class="front-door-bubble front-door-bubble--assistant">
              <span class="front-door-bubble__label">NOTA</span>
              <strong>{frontDoor()?.next_action_label ?? "Current slice"}</strong>
              <p>{frontDoor()?.next_action_detail ?? "Waiting for runtime guidance..."}</p>
              <Show when={status()?.next_step}>
                <div class="chat-meta-list">
                  <span>Lineage {status()?.next_step?.lineage_ref}</span>
                  <span>Target {status()?.next_step?.target_ref}</span>
                </div>
              </Show>
            </li>

            <Show when={latestDecision()}>
              {(decision) => (
                <li class="front-door-bubble front-door-bubble--decision">
                  <span class="front-door-bubble__label">Contract</span>
                  <strong>{decision().title}</strong>
                  <p>{decision().statement}</p>
                  <div class="chat-meta-list">
                    <span>{decision().decision_type}</span>
                    <span>{decision().enforcement_level}</span>
                    <span>{formatTimestamp(decision().updated_at)}</span>
                  </div>
                </li>
              )}
            </Show>

            <li class="front-door-bubble front-door-bubble--surface">
              <span class="front-door-bubble__label">Front door</span>
              <strong>Import stays explicit here, dashboard stays bounded.</strong>
              <p>
                Use the import entry when you want new external context in Entrance. The dashboard
                remains a separate future surface, so this round only leaves the hook.
              </p>
            </li>
          </ul>

          <div class="front-door-actions" aria-label="Front door actions">
            <button
              class="front-door-action front-door-action--primary"
              type="button"
              onClick={() => void chooseImportSnapshot()}
              disabled={isImporting()}
            >
              {isImporting() ? "Importing..." : "Import snapshot"}
            </button>
            <A class="front-door-action" href="/do">
              Open Do
            </A>
            <A class="front-door-action" href="/board">
              Dashboard hook
            </A>
          </div>

          <Show when={lastImportReport()}>
            {(report) => (
              <div class="front-door-import-receipt">
                <span class="front-door-bubble__label">Latest import</span>
                <strong>{report().source_project}</strong>
                <p>
                  {report().imported_issue_count} issues, {report().imported_document_count} documents,
                  and {report().imported_planning_item_count} planning items entered through the GUI.
                </p>
              </div>
            )}
          </Show>
        </article>

        <aside class="front-door-rail" aria-label="Current state rail">
          <article class="dashboard-card dashboard-card--status chat-card front-door-state">
            <div class="dashboard-card__topline">
              <p class="dashboard-card__caption">Current state</p>
              <span class="dashboard-live-indicator">
                <span class="dashboard-live-indicator__dot" aria-hidden="true" />
                Live
              </span>
            </div>
            <h3>{frontDoor()?.posture ?? "Loading current state"}</h3>
            <p>{frontDoor()?.summary ?? "Waiting for runtime truth..."}</p>

            <div class="front-door-progress-list" aria-label="Mission progress">
              <For each={frontDoor()?.progress_tracks ?? []}>
                {(track) => (
                  <section class="front-door-progress">
                    <div class="front-door-progress__meta">
                      <strong>{track.label}</strong>
                      <span>{track.value}%</span>
                    </div>
                    <div class="front-door-progress__bar" aria-hidden="true">
                      <span
                        class={`is-${track.tone}`}
                        style={{ width: `${track.value}%` }}
                      />
                    </div>
                    <p>{track.summary}</p>
                  </section>
                )}
              </For>
            </div>
          </article>

          <article class="dashboard-card chat-card front-door-mini">
            <div class="dashboard-card__topline">
              <p class="dashboard-card__caption">Tiny panel</p>
              <span class="dashboard-card__badge">Round 1</span>
            </div>
            <dl class="dashboard-stat-list">
              <div class="dashboard-stat">
                <dt>Checkpoint</dt>
                <dd>{status()?.current_checkpoint_id ?? "None"}</dd>
              </div>
              <div class="dashboard-stat">
                <dt>Human relay</dt>
                <dd>
                  {currentCheckpoint()?.payload.human_continuity_bus ??
                    status()?.recommended_checkpoint?.human_continuity_bus ??
                    "Not recorded yet"}
                </dd>
              </div>
              <div class="dashboard-stat">
                <dt>Next honest move</dt>
                <dd>{frontDoor()?.next_action_label ?? "Loading"}</dd>
              </div>
            </dl>
          </article>

          <details class="front-door-detail" open>
            <summary>Open checkpoint detail</summary>
            <div class="front-door-detail__body">
              <Show
                when={currentCheckpoint()}
                fallback={
                  <p class="chat-detail-panel__empty">
                    {isLoading()
                      ? "Loading checkpoint detail..."
                      : "No checkpoint has been written yet."}
                  </p>
                }
              >
                {(checkpoint) => (
                  <>
                    <section class="chat-detail-panel">
                      <span class="chat-detail-panel__label">Landed</span>
                      <ul class="chat-list">
                        <For each={checkpoint().payload.landed}>
                          {(item) => <li>{item}</li>}
                        </For>
                      </ul>
                    </section>
                    <section class="chat-detail-panel">
                      <span class="chat-detail-panel__label">Remaining</span>
                      <Show
                        when={checkpoint().payload.remaining.length > 0}
                        fallback={
                          <p class="chat-detail-panel__empty">No remaining items recorded.</p>
                        }
                      >
                        <ul class="chat-list">
                          <For each={checkpoint().payload.remaining}>
                            {(item) => <li>{item}</li>}
                          </For>
                        </ul>
                      </Show>
                    </section>
                    <Show when={checkpoint().payload.next_start_hints.length > 0}>
                      <div class="chat-hint-strip">
                        <For each={checkpoint().payload.next_start_hints}>
                          {(hint) => <span class="chat-hint-chip">{hint}</span>}
                        </For>
                      </div>
                    </Show>
                  </>
                )}
              </Show>
            </div>
          </details>

          <details class="front-door-detail">
            <summary>Open deeper runtime detail</summary>
            <div class="front-door-detail__body">
              <dl class="dashboard-stat-list">
                <div class="dashboard-stat">
                  <dt>Chat captures</dt>
                  <dd>{status()?.chat_capture_count ?? 0}</dd>
                </div>
                <div class="dashboard-stat">
                  <dt>Decisions</dt>
                  <dd>{status()?.decision_count ?? 0}</dd>
                </div>
                <div class="dashboard-stat">
                  <dt>Receipts</dt>
                  <dd>{status()?.receipt_count ?? 0}</dd>
                </div>
              </dl>
              <Show when={recentCaptures().length > 0}>
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
            </div>
          </details>
        </aside>
      </section>

      <section class="dashboard-grid chat-grid" aria-label="Front door detail cards">
        <article class="dashboard-card dashboard-card--actions dashboard-card--wide chat-card">
          <div class="dashboard-card__topline">
            <p class="dashboard-card__caption">Recent runtime movement</p>
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
            <p class="dashboard-card__caption">Contract edges</p>
            <span class="dashboard-card__badge">
              {overview()?.decisions.link_count ?? 0} links
            </span>
          </div>
          <h3>Accepted decisions keep the front door honest</h3>
          <p>
            The shell reads the same decision plane that governs ChatUI, dashboard separation, and
            anti-Zeno progress expectations.
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
