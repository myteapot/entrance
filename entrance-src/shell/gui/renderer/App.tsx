import { Match, Show, Switch, createMemo, createResource, createSignal } from "solid-js";
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
    policy_schema: string | null;
    admission_schema: string | null;
    verdict_schema: string | null;
    last_admission_gate: string | null;
    last_gate_description: string | null;
    last_gate_expected_object_kind: string | null;
    last_admission_passed: boolean | null;
    last_decision: string | null;
    reason_code: string | null;
    score_vector: Array<{
      name: string;
      value: number | null;
    }>;
    human_options: string[];
    worker_kind: string | null;
    worker_mode: string | null;
    worker_ok: boolean | null;
    evidence: Array<{
      id: number;
      round: number;
      stage_role: string | null;
      kind: string;
      summary: string;
      schema_version: string | null;
      admission_result: string | null;
      blocked_phase: string | null;
      missing_receipts: string[];
      operator_options: string[];
      operator_author: string | null;
      operator_action: string | null;
      worker_kind: string | null;
      worker_mode: string | null;
      worker_ok: boolean | null;
      transcript_excerpt: string | null;
    }>;
    stages: Array<{
      role: string;
      status: string;
      summary: string | null;
      evidence_kind: string | null;
      evidence_summary: string | null;
      admission_result: string | null;
      worker_kind: string | null;
      worker_mode: string | null;
      worker_ok: boolean | null;
    }>;
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
  const [selectedIssueId, setSelectedIssueId] = createSignal<number | null>(null);
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

  const selectedIssueCard = createMemo(() => {
    const cards = issueCards() ?? [];
    if (cards.length === 0) return null;
    const issueId = selectedIssueId();
    return cards.find((card) => card.issue.id === issueId) ?? cards[0];
  });

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
    setSelectedIssueId(card.issue.id);
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
    setSelectedIssueId(issueId);
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
    setSelectedIssueId(card.issue.id);
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

  const openIssueComment = (issueId: number) => {
    setSelectedIssueId(issueId);
    setActiveCommentIssue(issueId);
  };

  const addIssueComment = async (issueId: number) => {
    if (!commentBody().trim() || issuePendingLabel(issueId)) return;
    setSelectedIssueId(issueId);
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

  const issueOptionLabel = (option: string) =>
    ({
      comment: "Comment",
      retry: "Retry",
      "request-review": "Review",
      cancel: "Cancel",
    })[option] ?? option;

  const issueOptionDisabled = (card: IssueCard, option: string) =>
    Boolean(issuePendingLabel(card.issue.id)) || (option === "retry" && !card.issue.loop_id);

  const runIssueOption = (card: IssueCard, option: string) => {
    setSelectedIssueId(card.issue.id);
    if (option === "comment") {
      openIssueComment(card.issue.id);
      return;
    }
    if (option === "retry") {
      void retryIssueLoop(card);
      return;
    }
    if (option === "request-review" || option === "cancel") {
      void decideIssue(card.issue.id, option);
      return;
    }
    setBanner(`Unsupported issue option ${option}.`);
  };

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

  const stageWorkerLabel = (stage: NonNullable<IssueCard["trace"]>["stages"][number]) => {
    if (!stage.worker_kind) return "worker pending";
    const state = stage.worker_ok === true ? "ok" : "blocked";
    return `${stage.worker_kind}/${state}`;
  };

  const evidenceWorkerLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (!evidence.worker_kind) return "worker pending";
    const state = evidence.worker_ok === true ? "ok" : "blocked";
    return `${evidence.worker_kind}/${state}`;
  };

  const scoreMetricLabel = (name: string) =>
    ({
      stage_completeness: "stage",
      runtime_readiness: "runtime",
      evidence_presence: "evidence",
      admission_integrity: "admission",
    })[name] ?? name;

  const scoreValueLabel = (value: number | null) => (value === null ? "n/a" : value.toFixed(2));

  const scoreVectorLabel = (trace: NonNullable<IssueCard["trace"]>) =>
    trace.score_vector.length
      ? trace.score_vector
          .map((metric) => `${scoreMetricLabel(metric.name)} ${scoreValueLabel(metric.value)}`)
          .join(" / ")
      : null;

  const scoreSummaryLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    if (!trace.score_vector.length) return null;
    const healthy = trace.score_vector.filter((metric) => metric.value !== null && metric.value >= 1).length;
    return `score ${healthy}/${trace.score_vector.length}`;
  };

  const issueDetailRows = (card: IssueCard) => {
    const trace = card.trace;
    return [
      ["Status", card.issue.status],
      ["Loop", card.issue.loop_id ? `#${card.issue.loop_id}` : "unlinked"],
      ["Round", trace ? String(trace.current_round) : "pending"],
      [
        "Packets",
        trace ? `${trace.round_packet_count}/${trace.packet_count}` : "pending",
      ],
      [
        "Admissions",
        trace ? `${trace.round_admission_count}/${trace.admission_count}` : "pending",
      ],
      ["Evidence", trace ? `${trace.round_evidence_count}/${trace.evidence_count}` : "pending"],
      ["Verdicts", trace ? `${trace.round_verdict_count}/${trace.verdict_count}` : "pending"],
      [
        "Receipts",
        trace
          ? `${trace.round_receipt_required_count - trace.round_receipt_missing_count}/${trace.round_receipt_required_count}`
          : "pending",
      ],
      [
        "Workers",
        trace ? `${trace.round_role_worker_ok_count}/${trace.round_role_worker_count}` : "pending",
      ],
      ["Gate", trace?.last_admission_gate ?? "pending"],
      ["Gate Rule", trace?.last_gate_description ?? "pending"],
      ["Gate Object", trace?.last_gate_expected_object_kind ?? "pending"],
      ["Decision", trace?.last_decision ?? "pending"],
      ["Reason", trace?.reason_code ?? "pending"],
      ["Scores", trace ? scoreVectorLabel(trace) ?? "pending" : "pending"],
      [
        "Options",
        trace?.human_options.length ? trace.human_options.join(", ") : "pending",
      ],
      ["Packet", schemaLabel(trace?.packet_schema)],
      ["Policy", schemaLabel(trace?.policy_schema)],
      ["Admission", schemaLabel(trace?.admission_schema)],
      ["Verdict", schemaLabel(trace?.verdict_schema)],
      ["Worker", workerLabel(card) ?? "pending"],
    ];
  };

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
              <div class="panel-stack">
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

                <article class="panel panel--detail">
                  <p class="panel-kicker">Issue</p>
                  <Show when={selectedIssueCard()} keyed fallback={<p class="muted">No issues</p>}>
                    {(card) => (
                      <>
                        <h3>{card.issue.title}</h3>
                        <p class="muted">{card.issue.summary ?? "No summary"}</p>
                        <dl class="detail-grid">
                          {issueDetailRows(card).map(([label, value]) => (
                            <div>
                              <dt>{label}</dt>
                              <dd>{value}</dd>
                            </div>
                          ))}
                        </dl>
                        {card.trace?.human_options.length ? (
                          <div class="decision-options">
                            {card.trace.human_options.map((option) => (
                              <button
                                type="button"
                                disabled={issueOptionDisabled(card, option)}
                                onClick={() => runIssueOption(card, option)}
                              >
                                {issuePendingLabel(card.issue.id) ?? issueOptionLabel(option)}
                              </button>
                            ))}
                          </div>
                        ) : null}
                        {activeCommentIssue() === card.issue.id ? (
                          <div class="comment-box comment-box--detail">
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
                        ) : null}
                        {card.trace?.stages.length ? (
                          <div class="stage-timeline">
                            {card.trace.stages.map((stage) => (
                              <div class="stage-row">
                                <div class="stage-row-head">
                                  <strong>{stage.role}</strong>
                                  <span>{stage.status}</span>
                                </div>
                                <p>{stage.summary ?? "No stage summary"}</p>
                                <div class="trace-strip">
                                  <span class="trace-pill">{stage.evidence_kind ?? "evidence pending"}</span>
                                  <span class="trace-pill">{stage.admission_result ?? "admission pending"}</span>
                                  <span
                                    class={
                                      stage.worker_ok === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {stageWorkerLabel(stage)}
                                  </span>
                                </div>
                                <p class="muted">{stage.evidence_summary ?? "No evidence summary"}</p>
                              </div>
                            ))}
                          </div>
                        ) : null}
                        {card.trace?.evidence.length ? (
                          <div class="evidence-ledger">
                            <h4>Evidence</h4>
                            {card.trace.evidence.map((evidence) => (
                              <div class="evidence-row">
                                <div class="stage-row-head">
                                  <strong>{evidence.stage_role ?? evidence.kind}</strong>
                                  <span>#{evidence.id}</span>
                                </div>
                                <p>{evidence.summary}</p>
                                <div class="trace-strip">
                                  <span class="trace-pill">{evidence.kind}</span>
                                  <span class="trace-pill">{evidence.admission_result ?? "admission pending"}</span>
                                  <span
                                    class={
                                      evidence.worker_ok === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {evidenceWorkerLabel(evidence)}
                                  </span>
                                  {evidence.schema_version ? (
                                    <span class="trace-pill">{schemaLabel(evidence.schema_version)}</span>
                                  ) : null}
                                  {evidence.blocked_phase ? (
                                    <span class="trace-pill trace-pill--warn">blocked {evidence.blocked_phase}</span>
                                  ) : null}
                                  {evidence.missing_receipts.map((receipt) => (
                                    <span class="trace-pill trace-pill--warn">missing {receipt}</span>
                                  ))}
                                  {evidence.operator_options.map((option) => (
                                    <span class="trace-pill">{option}</span>
                                  ))}
                                  {evidence.operator_author ? (
                                    <span class="trace-pill">operator {evidence.operator_author}</span>
                                  ) : null}
                                  {evidence.operator_action ? (
                                    <span class="trace-pill">{evidence.operator_action}</span>
                                  ) : null}
                                </div>
                                {evidence.transcript_excerpt ? (
                                  <p class="muted">{evidence.transcript_excerpt}</p>
                                ) : null}
                              </div>
                            ))}
                          </div>
                        ) : null}
                        <div class="comment-stack comment-stack--detail">
                          {card.comments.map((comment) => (
                            <div class="comment-line">
                              <strong>{comment.author}</strong>
                              <span>{comment.body}</span>
                            </div>
                          ))}
                        </div>
                      </>
                    )}
                  </Show>
                </article>
              </div>

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
                            <li
                              class={
                                selectedIssueCard()?.issue.id === card.issue.id
                                  ? "record-card issue-card issue-card--selected"
                                  : "record-card issue-card"
                              }
                            >
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
                                  {scoreSummaryLabel(card.trace) ? (
                                    <span class="trace-pill">{scoreSummaryLabel(card.trace)}</span>
                                  ) : null}
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
                                  <button type="button" onClick={() => setSelectedIssueId(card.issue.id)}>
                                    Details
                                  </button>
                                  <button
                                    type="button"
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => openIssueComment(card.issue.id)}
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
