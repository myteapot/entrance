import { Match, Switch, createResource, createSignal } from "solid-js";
import Nav from "./components/Nav";
import { bridge } from "./lib/bridge";

type View = "status" | "drawer" | "hive" | "panel" | "launcher";

type AppStatus = {
  app_root: string;
  db_path: string;
  drawer_entries: number;
  hive_runs: number;
  hive_loops: number;
  launcher_entries: number;
  generated_at: string;
};

type DrawerSummary = {
  mode: string;
  root: string;
  items: number;
};

type DrawerHistory = {
  commits: Array<{
    id: string;
    summary: string;
  }>;
};

type DrawerItem = {
  id: number;
  title: string;
  kind: string;
  storage_path: string | null;
  tags: string[];
  updated_at: string;
};

type HiveRun = {
  id: number;
  title: string;
  status: string;
  project_dir: string | null;
  summary: string | null;
  updated_at: string;
};

type HiveSummary = {
  total_runs: number;
  ready_runs: number;
  returned_runs: number;
};

type HiveLoop = {
  id: number;
  title: string;
  goal: string;
  status: string;
  active_phase: string;
  current_round: number;
  runtime: string;
};

type IssueCard = {
  issue: {
    id: number;
    loop_id: number | null;
    title: string;
    status: string;
    summary: string | null;
    updated_at: string;
  };
  comments: Array<{
    id: number;
    author: string;
    body: string;
    created_at: string;
  }>;
  trace: {
    current_round: number;
    packet_count: number;
    admission_count: number;
    evidence_count: number;
    verdict_count: number;
    round_packet_count: number;
    round_admission_count: number;
    round_evidence_count: number;
    round_verdict_count: number;
    receipt_required_count: number;
    receipt_missing_count: number;
    round_receipt_required_count: number;
    round_receipt_missing_count: number;
    role_worker_count: number;
    role_worker_ok_count: number;
    round_role_worker_count: number;
    round_role_worker_ok_count: number;
    packet_schema: string | null;
    admission_schema: string | null;
    verdict_schema: string | null;
    last_admission_gate: string | null;
    last_admission_passed: boolean | null;
    last_decision: string | null;
    reason_code: string | null;
    worker_kind: string | null;
    worker_mode: string | null;
    worker_ok: boolean | null;
  } | null;
};

type LauncherResult = {
  id: number;
  name: string;
  command: string;
  source: string;
  launch_count: number;
  pinned: boolean;
  score: number;
  arguments: string | null;
  working_dir: string | null;
};

export default function App() {
  const [view, setView] = createSignal<View>("status");
  const [launcherQuery, setLauncherQuery] = createSignal("");
  const [hiveTitle, setHiveTitle] = createSignal("");
  const [hiveProject, setHiveProject] = createSignal("");
  const [loopTitle, setLoopTitle] = createSignal("");
  const [loopGoal, setLoopGoal] = createSignal("");
  const [loopRuntime, setLoopRuntime] = createSignal("codex");
  const [activeCommentIssue, setActiveCommentIssue] = createSignal<number | null>(null);
  const [commentBody, setCommentBody] = createSignal("");
  const [pendingLoopActions, setPendingLoopActions] = createSignal<Record<number, string>>({});
  const [pendingIssueActions, setPendingIssueActions] = createSignal<Record<number, string>>({});
  const [drawerTitle, setDrawerTitle] = createSignal("");
  const [drawerBody, setDrawerBody] = createSignal("");
  const [banner, setBanner] = createSignal<string>("");

  const [status, { refetch: refetchStatus }] = createResource(async () =>
    bridge.invoke<AppStatus>("status"),
  );
  const [drawerSummary, { refetch: refetchDrawerSummary }] = createResource(async () =>
    bridge.invoke<DrawerSummary>("drawer_summary"),
  );
  const [drawerItems, { refetch: refetchDrawerItems }] = createResource(async () =>
    bridge.invoke<DrawerItem[]>("drawer_list", {}),
  );
  const [drawerHistory, { refetch: refetchDrawerHistory }] = createResource(async () =>
    bridge.invoke<DrawerHistory>("drawer_history"),
  );
  const [hiveRuns, { refetch: refetchHiveRuns }] = createResource(async () =>
    bridge.invoke<HiveRun[]>("hive_list"),
  );
  const [hiveSummary, { refetch: refetchHiveSummary }] = createResource(async () =>
    bridge.invoke<HiveSummary>("hive_summary"),
  );
  const [hiveLoops, { refetch: refetchHiveLoops }] = createResource(async () =>
    bridge.invoke<HiveLoop[]>("hive_loop_list"),
  );
  const [issueCards, { refetch: refetchIssueCards }] = createResource(async () =>
    bridge.invoke<IssueCard[]>("hive_panel"),
  );
  const [launcherItems, { refetch: refetchLauncher }] = createResource(launcherQuery, async (query) =>
    bridge.invoke<LauncherResult[]>("launcher_search", { query, limit: 12 }),
  );

  const refreshAll = async () => {
    await Promise.all([
      refetchStatus(),
      refetchDrawerSummary(),
      refetchDrawerItems(),
      refetchDrawerHistory(),
      refetchHiveRuns(),
      refetchHiveSummary(),
      refetchHiveLoops(),
      refetchIssueCards(),
      refetchLauncher(),
    ]);
  };

  const addDrawerNote = async () => {
    await bridge.invoke("drawer_add_note", {
      title: drawerTitle() || "Untitled Note",
      body: drawerBody(),
    });
    setDrawerTitle("");
    setDrawerBody("");
    setBanner("Drawer note created.");
    await Promise.all([refetchDrawerSummary(), refetchDrawerItems(), refetchDrawerHistory(), refetchStatus()]);
  };

  const dispatchHive = async () => {
    await bridge.invoke("hive_dispatch", {
      title: hiveTitle() || "Untitled dispatch",
      projectDir: hiveProject() || undefined,
    });
    setHiveTitle("");
    setHiveProject("");
    setBanner("Hive dispatch persisted.");
    await Promise.all([refetchHiveRuns(), refetchHiveSummary(), refetchStatus()]);
  };

  const createHiveLoop = async () => {
    await bridge.invoke("hive_loop_create", {
      title: loopTitle() || "Untitled loop",
      goal: loopGoal() || loopTitle() || "Run an Entrance loop",
      runtime: loopRuntime(),
      approachSpace: ["Explore the smallest runnable MVP"],
      evalSpace: ["CLI loop run produces a keep/reject/block verdict"],
    });
    setLoopTitle("");
    setLoopGoal("");
    setBanner("Loop contract created.");
    await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
  };

  const setPendingLoop = (loopId: number, label: string | null) => {
    setPendingLoopActions((current) => {
      const next = { ...current };
      if (label) {
        next[loopId] = label;
      } else {
        delete next[loopId];
      }
      return next;
    });
  };

  const setPendingIssue = (issueId: number, label: string | null) => {
    setPendingIssueActions((current) => {
      const next = { ...current };
      if (label) {
        next[issueId] = label;
      } else {
        delete next[issueId];
      }
      return next;
    });
  };

  const loopPendingLabel = (loopId: number) => pendingLoopActions()[loopId] ?? null;
  const issuePendingLabel = (issueId: number) => pendingIssueActions()[issueId] ?? null;
  const actionErrorMessage = (error: unknown) =>
    error instanceof Error ? error.message : String(error);

  const runHiveLoop = async (loop: HiveLoop) => {
    if (loopPendingLabel(loop.id)) return;
    setPendingLoop(loop.id, "Running");
    try {
      await bridge.invoke("hive_loop_run", {
        id: loop.id,
        runtime: loop.runtime || loopRuntime(),
      });
      setBanner(`Loop #${loop.id} finished.`);
      await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
    } catch (error) {
      setBanner(`Loop #${loop.id} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingLoop(loop.id, null);
    }
  };

  const runIssueLoop = async (card: IssueCard) => {
    if (!card.issue.loop_id || issuePendingLabel(card.issue.id)) return;
    setPendingIssue(card.issue.id, "Running");
    try {
      await bridge.invoke("hive_loop_run", {
        id: card.issue.loop_id,
        runtime: loopRuntime(),
      });
      setBanner(`Loop #${card.issue.loop_id} finished.`);
      await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
    } catch (error) {
      setBanner(`Loop #${card.issue.loop_id} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
    }
  };

  const decideIssue = async (issueId: number, action: string) => {
    if (issuePendingLabel(issueId)) return;
    setPendingIssue(issueId, issuePendingActionLabel(action));
    try {
      await bridge.invoke("hive_issue_decide", {
        issueId,
        action,
        author: "human",
      });
      setBanner(`Issue #${issueId} ${issueActionLabel(action)}.`);
      await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
    } catch (error) {
      setBanner(`Issue #${issueId} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(issueId, null);
    }
  };

  const retryIssueLoop = async (card: IssueCard) => {
    if (issuePendingLabel(card.issue.id)) return;
    setPendingIssue(card.issue.id, "Retrying");
    try {
      await bridge.invoke("hive_issue_decide", {
        issueId: card.issue.id,
        action: "retry",
        author: "human",
      });
      if (card.issue.loop_id) {
        await bridge.invoke("hive_loop_run", {
          id: card.issue.loop_id,
          runtime: loopRuntime(),
        });
      }
      setBanner(`Issue #${card.issue.id} retried.`);
      await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
    } catch (error) {
      setBanner(`Issue #${card.issue.id} retry failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
    }
  };

  const addIssueComment = async (issueId: number) => {
    if (!commentBody().trim() || issuePendingLabel(issueId)) return;
    setPendingIssue(issueId, "Sending");
    try {
      await bridge.invoke("hive_issue_comment", {
        issueId,
        author: "human",
        body: commentBody(),
      });
      setCommentBody("");
      setActiveCommentIssue(null);
      setBanner(`Commented on issue #${issueId}.`);
      await refetchIssueCards();
    } catch (error) {
      setBanner(`Comment failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(issueId, null);
    }
  };

  const refreshLauncherIndex = async () => {
    await bridge.invoke("launcher_refresh", {});
    setBanner("Launcher index refreshed.");
    await Promise.all([refetchLauncher(), refetchStatus()]);
  };

  const launchItem = async (item: LauncherResult) => {
    await bridge.invoke("launcher_launch", {
      command: item.command,
      arguments: item.arguments,
      workingDir: item.working_dir,
    });
    setBanner(`Launched ${item.name}.`);
    await refetchLauncher();
  };

  const pinItem = async (item: LauncherResult) => {
    await bridge.invoke("launcher_pin", {
      command: item.command,
      pinned: !item.pinned,
    });
    await refetchLauncher();
  };

  const issueActionLabel = (action: string) =>
    ({
      retry: "retried",
      "request-review": "sent to review",
      cancel: "canceled",
    })[action] ?? action;

  const issuePendingActionLabel = (action: string) =>
    ({
      retry: "Retrying",
      "request-review": "Reviewing",
      cancel: "Canceling",
    })[action] ?? "Working";

  const schemaLabel = (schema: string | null | undefined) =>
    schema ? schema.split(".").slice(-2).join(".") : "pending";

  const workerLabel = (card: IssueCard) => {
    if (!card.trace?.worker_kind) return null;
    const state = card.trace.worker_ok === true ? "ok" : "blocked";
    return `${card.trace.worker_kind}/${state}`;
  };

  const receiptLabel = (card: IssueCard) => {
    if (!card.trace) return null;
    if (card.trace.round_receipt_required_count === 0) return "receipts pending";
    return card.trace.round_receipt_missing_count === 0
      ? "receipts ok"
      : `missing ${card.trace.round_receipt_missing_count}`;
  };

  const gateLabel = (card: IssueCard) => {
    if (!card.trace?.last_admission_gate) return null;
    const state = card.trace.last_admission_passed === true ? "ok" : "blocked";
    return `gate ${state}`;
  };

  const roleWorkerLabel = (card: IssueCard) => {
    if (!card.trace) return null;
    if (card.trace.round_role_worker_count === 0) return "workers pending";
    return `workers ${card.trace.round_role_worker_ok_count}/${card.trace.round_role_worker_count}`;
  };

  const traceCountLabel = (label: string, current: number, total: number) =>
    total > current ? `${label} ${current}/${total}` : `${label} ${current}`;

  return (
    <div class="app-shell">
      <Nav current={view()} onSelect={setView} />

      <main class="main-shell">
        <header class="hero-panel">
          <div>
            <p class="hero-kicker">Refactor target</p>
            <h2>Core / Plugins / Shell</h2>
            <p class="hero-copy">
              This GUI talks only to the unified `entrance daemon` protocol.
            </p>
          </div>
          <button type="button" class="hero-action" onClick={() => void refreshAll()}>
            Refresh
          </button>
        </header>

        {banner() ? <p class="banner">{banner()}</p> : null}

        <Switch>
          <Match when={view() === "status"}>
            <section class="panel-grid panel-grid--status">
              <article class="panel">
                <p class="panel-kicker">Kernel</p>
                <h3>Runtime status</h3>
                <dl class="metric-list">
                  <div><dt>App root</dt><dd>{status()?.app_root ?? "..."}</dd></div>
                  <div><dt>Database</dt><dd>{status()?.db_path ?? "..."}</dd></div>
                  <div><dt>Drawer</dt><dd>{status()?.drawer_entries ?? 0}</dd></div>
                  <div><dt>Hive</dt><dd>{status()?.hive_runs ?? 0}</dd></div>
                  <div><dt>Loops</dt><dd>{status()?.hive_loops ?? 0}</dd></div>
                  <div><dt>Launcher</dt><dd>{status()?.launcher_entries ?? 0}</dd></div>
                </dl>
              </article>

              <article class="panel">
                <p class="panel-kicker">Drawer</p>
                <h3>Storage mode</h3>
                <p class="big-copy">{drawerSummary()?.mode ?? "..."}</p>
                <p class="muted">{drawerSummary()?.root ?? "..."}</p>
              </article>

              <article class="panel">
                <p class="panel-kicker">Identity</p>
                <h3>Microkernel cutover</h3>
                <p class="muted">
                  Runtime, daemon, and GUI now share the same single-binary command contract.
                </p>
              </article>
            </section>
          </Match>

          <Match when={view() === "drawer"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Drawer</p>
                <h3>Add note</h3>
                <input
                  value={drawerTitle()}
                  onInput={(event) => setDrawerTitle(event.currentTarget.value)}
                  placeholder="Title"
                />
                <textarea
                  value={drawerBody()}
                  onInput={(event) => setDrawerBody(event.currentTarget.value)}
                  placeholder="Write a note for the drawer"
                />
                <button type="button" class="primary-button" onClick={() => void addDrawerNote()}>
                  Create Note
                </button>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Items</p>
                <h3>Stored entries</h3>
                <ul class="record-list">
                  {(drawerItems() ?? []).map((item) => (
                    <li class="record-card">
                      <strong>{item.title}</strong>
                      <span>{item.kind}</span>
                      <code>{item.storage_path ?? "db-only"}</code>
                    </li>
                  ))}
                </ul>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Versioning</p>
                <h3>Drawer history</h3>
                <ul class="record-list">
                  {(drawerHistory()?.commits ?? []).map((commit) => (
                    <li class="record-card">
                      <strong>{commit.summary}</strong>
                      <code>{commit.id}</code>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>

          <Match when={view() === "hive"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Hive</p>
                <h3>Dispatch</h3>
                <input
                  value={hiveTitle()}
                  onInput={(event) => setHiveTitle(event.currentTarget.value)}
                  placeholder="Task title"
                />
                <input
                  value={hiveProject()}
                  onInput={(event) => setHiveProject(event.currentTarget.value)}
                  placeholder="Project path (optional)"
                />
                <button type="button" class="primary-button" onClick={() => void dispatchHive()}>
                  Persist Dispatch
                </button>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Runs</p>
                <h3>Dispatch ledger</h3>
                <p class="muted">
                  Ready {hiveSummary()?.ready_runs ?? 0} / Total {hiveSummary()?.total_runs ?? 0}
                </p>
                <ul class="record-list">
                  {(hiveRuns() ?? []).map((run) => (
                    <li class="record-card">
                      <strong>{run.title}</strong>
                      <span>{run.status}</span>
                      <code>{run.project_dir ?? "no project"}</code>
                    </li>
                  ))}
                </ul>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Loops</p>
                <h3>Contracts</h3>
                <ul class="record-list">
                  {(hiveLoops() ?? []).map((loop) => (
                    <li class="record-card">
                      <div class="record-head">
                        <strong>{loop.title}</strong>
                        <span>{loop.status}</span>
                      </div>
                      <span>{loop.active_phase} / round {loop.current_round}</span>
                      <code>{loop.runtime}</code>
                      {loop.status === "todo" ? (
                        <div class="record-actions">
                          <button
                            type="button"
                            disabled={Boolean(loopPendingLabel(loop.id))}
                            onClick={() => void runHiveLoop(loop)}
                          >
                            {loopPendingLabel(loop.id) ?? "Run"}
                          </button>
                        </div>
                      ) : null}
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>

          <Match when={view() === "panel"}>
            <section class="panel-grid panel-grid--board">
              <article class="panel panel--form">
                <p class="panel-kicker">Loop</p>
                <h3>Contract</h3>
                <input
                  value={loopTitle()}
                  onInput={(event) => setLoopTitle(event.currentTarget.value)}
                  placeholder="Title"
                />
                <textarea
                  value={loopGoal()}
                  onInput={(event) => setLoopGoal(event.currentTarget.value)}
                  placeholder="Goal"
                />
                <select value={loopRuntime()} onChange={(event) => setLoopRuntime(event.currentTarget.value)}>
                  <option value="codex">codex</option>
                  <option value="local">local</option>
                </select>
                <button type="button" class="primary-button" onClick={() => void createHiveLoop()}>
                  Create Loop
                </button>
              </article>

              <article class="panel panel--board">
                <p class="panel-kicker">Issues</p>
                <h3>Status board</h3>
                <div class="board-columns">
                  {["Todo", "Doing", "Needs Review", "Blocked", "Done", "Canceled"].map((statusName) => (
                    <section class="board-column">
                      <div class="board-column-head">
                        <strong>{statusName}</strong>
                        <span>{(issueCards() ?? []).filter((card) => card.issue.status === statusName).length}</span>
                      </div>
                      <ul class="record-list">
                        {(issueCards() ?? [])
                          .filter((card) => card.issue.status === statusName)
                          .map((card) => (
                            <li class="record-card issue-card">
                              <div class="record-head">
                                <strong>{card.issue.title}</strong>
                                <span>#{card.issue.id}</span>
                              </div>
                              <p class="muted">{card.issue.summary ?? "No summary"}</p>
                              {card.trace ? (
                                <div class="trace-strip">
                                  <span class="trace-pill">R {card.trace.current_round}</span>
                                  <span class="trace-pill">
                                    {traceCountLabel("P", card.trace.round_packet_count, card.trace.packet_count)}
                                  </span>
                                  <span class="trace-pill">
                                    {traceCountLabel("A", card.trace.round_admission_count, card.trace.admission_count)}
                                  </span>
                                  <span class="trace-pill">
                                    {traceCountLabel("E", card.trace.round_evidence_count, card.trace.evidence_count)}
                                  </span>
                                  <span class="trace-pill">
                                    {traceCountLabel("V", card.trace.round_verdict_count, card.trace.verdict_count)}
                                  </span>
                                  <span class="trace-pill">{schemaLabel(card.trace.verdict_schema)}</span>
                                  {receiptLabel(card) ? (
                                    <span
                                      class={
                                        card.trace.round_receipt_missing_count === 0
                                          ? "trace-pill"
                                          : "trace-pill trace-pill--warn"
                                      }
                                    >
                                      {receiptLabel(card)}
                                    </span>
                                  ) : null}
                                  {gateLabel(card) ? (
                                    <span
                                      class={
                                        card.trace.last_admission_passed === true
                                          ? "trace-pill"
                                          : "trace-pill trace-pill--warn"
                                      }
                                    >
                                      {gateLabel(card)}
                                    </span>
                                  ) : null}
                                  {roleWorkerLabel(card) ? (
                                    <span
                                      class={
                                        card.trace.round_role_worker_count ===
                                        card.trace.round_role_worker_ok_count
                                          ? "trace-pill"
                                          : "trace-pill trace-pill--warn"
                                      }
                                    >
                                      {roleWorkerLabel(card)}
                                    </span>
                                  ) : null}
                                  {card.trace.last_decision ? (
                                    <span class="trace-pill">{card.trace.last_decision}</span>
                                  ) : null}
                                  {workerLabel(card) ? <span class="trace-pill">{workerLabel(card)}</span> : null}
                                </div>
                              ) : null}
                              <div class="comment-stack">
                                {card.comments.slice(-3).map((comment) => (
                                  <div class="comment-line">
                                    <strong>{comment.author}</strong>
                                    <span>{comment.body}</span>
                                  </div>
                                ))}
                              </div>
                              {activeCommentIssue() === card.issue.id ? (
                                <div class="comment-box">
                                  <textarea
                                    value={commentBody()}
                                    onInput={(event) => setCommentBody(event.currentTarget.value)}
                                    placeholder="Comment"
                                  />
                                  <button
                                    type="button"
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => void addIssueComment(card.issue.id)}
                                  >
                                    {issuePendingLabel(card.issue.id) ?? "Send"}
                                  </button>
                                </div>
                              ) : (
                                <div class="record-actions">
                                  {card.issue.loop_id && ["Todo", "Blocked"].includes(card.issue.status) ? (
                                    <button
                                      type="button"
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() =>
                                        card.issue.status === "Blocked"
                                          ? void retryIssueLoop(card)
                                          : void runIssueLoop(card)
                                      }
                                    >
                                      {issuePendingLabel(card.issue.id) ??
                                        (card.issue.status === "Blocked" ? "Retry" : "Run")}
                                    </button>
                                  ) : null}
                                  {card.issue.status === "Blocked" ? (
                                    <button
                                      type="button"
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void decideIssue(card.issue.id, "request-review")}
                                    >
                                      {issuePendingLabel(card.issue.id) ?? "Review"}
                                    </button>
                                  ) : null}
                                  {card.issue.loop_id && card.issue.status === "Needs Review" ? (
                                    <button
                                      type="button"
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void retryIssueLoop(card)}
                                    >
                                      {issuePendingLabel(card.issue.id) ?? "Retry"}
                                    </button>
                                  ) : null}
                                  {["Blocked", "Needs Review"].includes(card.issue.status) ? (
                                    <button
                                      type="button"
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void decideIssue(card.issue.id, "cancel")}
                                    >
                                      {issuePendingLabel(card.issue.id) ?? "Cancel"}
                                    </button>
                                  ) : null}
                                  <button
                                    type="button"
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => setActiveCommentIssue(card.issue.id)}
                                  >
                                    Comment
                                  </button>
                                </div>
                              )}
                            </li>
                          ))}
                      </ul>
                    </section>
                  ))}
                </div>
              </article>
            </section>
          </Match>

          <Match when={view() === "launcher"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Launcher</p>
                <h3>Search index</h3>
                <input
                  value={launcherQuery()}
                  onInput={(event) => setLauncherQuery(event.currentTarget.value)}
                  placeholder="Search indexed apps"
                />
                <button type="button" class="primary-button" onClick={() => void refreshLauncherIndex()}>
                  Refresh Index
                </button>
                <p class="muted">Launcher routes through the unified daemon contract.</p>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Matches</p>
                <h3>Launch surface</h3>
                <ul class="record-list">
                  {(launcherItems() ?? []).map((item) => (
                    <li class="record-card">
                      <div class="record-head">
                        <strong>{item.name}</strong>
                        <span>{item.score.toFixed(2)}</span>
                      </div>
                      <code>{item.command}</code>
                      <div class="record-actions">
                        <button type="button" onClick={() => void launchItem(item)}>
                          Launch
                        </button>
                        <button type="button" onClick={() => void pinItem(item)}>
                          {item.pinned ? "Unpin" : "Pin"}
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>
        </Switch>
      </main>
    </div>
  );
}
