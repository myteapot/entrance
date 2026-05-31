import { Match, Show, Switch, createMemo, createResource, createSignal } from "solid-js";
import Nav from "./components/Nav";
import { bridge } from "./lib/bridge";

type View = "status" | "drawer" | "hive" | "panel" | "launcher";
type CommentSurface = "detail" | "board";
type ActiveCommentComposer = {
  issueId: number;
  surface: CommentSurface;
};

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
    payload?: Record<string, unknown>;
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
    round_worker_duration_ms: number;
    round_worker_timeout_count: number;
    round_worker_retry_exhausted_count: number;
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
    operator_event_count: number;
    round_operator_event_count: number;
    last_operator_event: OperatorEvent | null;
    operator_events: OperatorEvent[];
    worker_kind: string | null;
    worker_mode: string | null;
    worker_ok: boolean | null;
    audit_schema: string | null;
    audit_passed: boolean | null;
    audit_failed_count: number;
    audit_failed_checks: string[];
    audit_failure_details: string[];
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
      packet_envelope_errors: string[];
      operator_options: string[];
      operator_author: string | null;
      operator_action: string | null;
      worker_kind: string | null;
      worker_mode: string | null;
      worker_ok: boolean | null;
      worker_receipt_ok: boolean | null;
      worker_timed_out: boolean | null;
      worker_status: number | null;
      worker_duration_ms: number | null;
      worker_timeout_secs: number | null;
      worker_attempt_count: number | null;
      worker_max_attempts: number | null;
      worker_retry_exhausted: boolean | null;
      worker_command: string | null;
      worker_cwd: string | null;
      worker_action: string | null;
      worker_evidence_summary: string | null;
      worker_gate_count: number | null;
      worker_receipt_errors: string[];
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
  doctor: IssueDoctorSummary | null;
};

type OperatorEvent = {
  id: number;
  round: number;
  kind: string;
  author: string | null;
  action: string | null;
  issue_status: string | null;
  loop_status: string | null;
  note: string | null;
  summary: string;
};

type IssueDoctorSummary = {
  schema_version: string;
  health: string;
  summary: string;
  next_actions: string[];
  runtime: string;
  current_round: number;
  counts: {
    round_receipt_required_count: number;
    round_receipt_missing_count: number;
    round_role_worker_count: number;
    round_role_worker_ok_count: number;
    round_worker_duration_ms: number;
    round_worker_timeout_count: number;
    round_worker_retry_exhausted_count: number;
    audit_failed_count: number;
  };
  failed_checks: string[];
  audit_failure_details: string[];
  missing_receipts: string[];
  worker_failures: string[];
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

type IssueComment = IssueCard["comments"][number];
type LoopRunArgs = {
  runtime?: string;
  workerTimeoutSecs?: number;
  workerAttempts?: number;
};

const COMMENT_CARD_PREVIEW_LIMIT = 132;
const COMMENT_DETAIL_PREVIEW_LIMIT = 360;

const COMMENT_ACTION_LABELS: Record<string, string> = {
  retry: "retry",
  "request-review": "review",
  cancel: "cancel",
};

const compactText = (value: string, limit: number) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= limit) return normalized;
  return `${normalized.slice(0, Math.max(0, limit - 1)).trimEnd()}...`;
};

const commentPayloadString = (comment: IssueComment, field: string) => {
  const value = comment.payload?.[field];
  return typeof value === "string" && value.trim() ? value : null;
};

const commentSchemaLabel = (comment: IssueComment) => {
  const schema = commentPayloadString(comment, "schema_version");
  return schema ? schema.split(".").slice(-2).join(".") : null;
};

const commentPills = (comment: IssueComment) => {
  const source = commentPayloadString(comment, "source") ?? comment.author;
  const action = commentPayloadString(comment, "action");
  const decision = commentPayloadString(comment, "decision");
  const phase =
    commentPayloadString(comment, "phase") ?? commentPayloadString(comment, "next_phase");
  return [
    source,
    action ? COMMENT_ACTION_LABELS[action] ?? action : null,
    decision && decision !== action ? decision : null,
    phase,
    commentSchemaLabel(comment),
  ].filter((value): value is string => Boolean(value));
};

const commentPreview = (comment: IssueComment, limit: number) => {
  if (limit === COMMENT_DETAIL_PREVIEW_LIMIT && commentPayloadString(comment, "source") === "operator") {
    return comment.body;
  }
  return compactText(comment.body, limit);
};

const operatorActionLabel = (action: string | null) =>
  action ? COMMENT_ACTION_LABELS[action] ?? action : "comment";

const operatorEventLabel = (event: OperatorEvent | null) => {
  if (!event) return null;
  const author = event.author ?? "operator";
  return `${author} ${operatorActionLabel(event.action)}`;
};

const operatorEventStatusLabel = (event: OperatorEvent) => {
  const status = event.issue_status ?? event.loop_status;
  return status ? `-> ${status}` : `round ${event.round}`;
};

export default function App() {
  const [view, setView] = createSignal<View>("status");
  const [launcherQuery, setLauncherQuery] = createSignal("");
  const [hiveTitle, setHiveTitle] = createSignal("");
  const [hiveProject, setHiveProject] = createSignal("");
  const [loopTitle, setLoopTitle] = createSignal("");
  const [loopGoal, setLoopGoal] = createSignal("");
  const [loopRuntime, setLoopRuntime] = createSignal("codex");
  const [loopWorkerTimeoutSecs, setLoopWorkerTimeoutSecs] = createSignal("");
  const [loopWorkerAttempts, setLoopWorkerAttempts] = createSignal("");
  const [selectedIssueId, setSelectedIssueId] = createSignal<number | null>(null);
  const [activeCommentComposer, setActiveCommentComposer] =
    createSignal<ActiveCommentComposer | null>(null);
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
  const selectedIssueDoctor = createMemo(() => selectedIssueCard()?.doctor ?? null);

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
  const issueDecisionNote = (issueId: number) =>
    activeCommentComposer()?.issueId === issueId ? commentBody().trim() : "";
  const commentSubmitDisabled = (issueId: number) =>
    Boolean(issuePendingLabel(issueId)) || !commentBody().trim();
  const clearIssueComposer = (issueId: number) => {
    if (activeCommentComposer()?.issueId === issueId) {
      setCommentBody("");
      setActiveCommentComposer(null);
    }
  };
  const commentComposerActive = (issueId: number, surface: CommentSurface) => {
    const active = activeCommentComposer();
    return active?.issueId === issueId && active.surface === surface;
  };
  const actionErrorMessage = (error: unknown) =>
    error instanceof Error ? error.message : String(error);
  const workerLimitRunArgs = (): LoopRunArgs => {
    const timeoutText = loopWorkerTimeoutSecs().trim();
    const workerTimeoutSecs = timeoutText ? Number.parseInt(timeoutText, 10) : undefined;
    const attemptsText = loopWorkerAttempts().trim();
    const workerAttempts = attemptsText ? Number.parseInt(attemptsText, 10) : undefined;
    return {
      workerTimeoutSecs: workerTimeoutSecs && workerTimeoutSecs > 0 ? workerTimeoutSecs : undefined,
      workerAttempts: workerAttempts && workerAttempts > 0 ? workerAttempts : undefined,
    };
  };
  const loopRunArgs = (): LoopRunArgs => ({
    runtime: loopRuntime(),
    ...workerLimitRunArgs(),
  });
  const commandRunArgs = (command: string | undefined): LoopRunArgs => {
    if (!command) return {};
    const tokens = command.split(/\s+/);
    const valueAfter = (flag: string) => {
      const index = tokens.indexOf(flag);
      return index >= 0 ? tokens[index + 1] : undefined;
    };
    const parsePositive = (value: string | undefined) => {
      if (!value) return undefined;
      const parsed = Number.parseInt(value, 10);
      return parsed > 0 ? parsed : undefined;
    };
    return {
      runtime: valueAfter("--runtime"),
      workerTimeoutSecs: parsePositive(valueAfter("--worker-timeout-secs")),
      workerAttempts: parsePositive(valueAfter("--worker-attempts")),
    };
  };
  const doctorRunArgs = (card: IssueCard, commandNeedle: string) =>
    commandRunArgs(card.doctor?.next_actions.find((action) => action.includes(commandNeedle)));
  const mergeRunArgs = (...argsList: LoopRunArgs[]) => {
    const merged: LoopRunArgs = {};
    argsList.forEach((args) => {
      if (args.runtime) merged.runtime = args.runtime;
      if (args.workerTimeoutSecs) merged.workerTimeoutSecs = args.workerTimeoutSecs;
      if (args.workerAttempts) merged.workerAttempts = args.workerAttempts;
    });
    return merged;
  };
  const hasRunArgs = (args: LoopRunArgs) =>
    Boolean(args.runtime || args.workerTimeoutSecs || args.workerAttempts);
  const issueRunArgs = (card: IssueCard) => {
    const doctorArgs = doctorRunArgs(card, "entrance hive loop run");
    return hasRunArgs(doctorArgs)
      ? mergeRunArgs(doctorArgs, workerLimitRunArgs())
      : loopRunArgs();
  };
  const issueRetryRunArgs = (card: IssueCard) =>
    mergeRunArgs(doctorRunArgs(card, "entrance hive issue retry-run"), workerLimitRunArgs());
  const runArgsSummary = (args: LoopRunArgs, fallbackRuntime?: string) => {
    const runtime = args.runtime ?? fallbackRuntime;
    const parts = runtime ? [runtime] : [];
    if (args.workerAttempts && args.workerAttempts > 1) parts.push(`${args.workerAttempts}x`);
    if (args.workerTimeoutSecs) parts.push(`${args.workerTimeoutSecs}s`);
    return parts.join(" ");
  };
  const issueRuntimeSummary = (card: IssueCard, retry: boolean) =>
    runArgsSummary(
      retry ? issueRetryRunArgs(card) : issueRunArgs(card),
      card.doctor?.runtime || card.trace?.worker_kind || undefined,
    );
  const issueRuntimeActionLabel = (card: IssueCard, retry: boolean) => {
    const pending = issuePendingLabel(card.issue.id);
    if (pending) return pending;
    const verb = retry ? "Retry" : "Run";
    const summary = issueRuntimeSummary(card, retry);
    return summary ? `${verb} ${summary}` : verb;
  };
  const issueRuntimeActionAriaLabel = (card: IssueCard, retry: boolean, surface: string) => {
    const verb = retry ? "Retry" : "Run";
    const summary = issueRuntimeSummary(card, retry);
    return summary
      ? `${verb} issue #${card.issue.id} from ${surface} with ${summary}`
      : `${verb} issue #${card.issue.id} from ${surface}`;
  };

  const runHiveLoop = async (loop: HiveLoop) => {
    if (loopPendingLabel(loop.id)) return;
    setPendingLoop(loop.id, "Running");
    try {
      const runArgs = loopRunArgs();
      await bridge.invoke("hive_loop_run", {
        id: loop.id,
        ...runArgs,
        runtime: loop.runtime || runArgs.runtime,
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
      await bridge.invoke("hive_issue_run", {
        issueId: card.issue.id,
        ...issueRunArgs(card),
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
        body: issueDecisionNote(issueId) || undefined,
      });
      clearIssueComposer(issueId);
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
      await bridge.invoke("hive_issue_run", {
        issueId: card.issue.id,
        retry: true,
        author: "human",
        body: issueDecisionNote(card.issue.id) || undefined,
        ...issueRetryRunArgs(card),
      });
      clearIssueComposer(card.issue.id);
      setBanner(`Issue #${card.issue.id} retried.`);
      await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
    } catch (error) {
      setBanner(`Issue #${card.issue.id} retry failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
    }
  };

  const openIssueComment = (issueId: number, surface: CommentSurface) => {
    if (activeCommentComposer()?.issueId !== issueId) {
      setCommentBody("");
    }
    setSelectedIssueId(issueId);
    setActiveCommentComposer({ issueId, surface });
    focusIssueComment(issueId, surface);
  };

  const closeIssueComment = (issueId: number) => {
    clearIssueComposer(issueId);
  };
  const focusIssueComment = (issueId: number, surface: CommentSurface) => {
    window.setTimeout(() => {
      document
        .querySelector<HTMLTextAreaElement>(`[data-testid="issue-comment-${surface}-${issueId}"]`)
        ?.focus();
    }, 0);
  };
  const handleCommentKeyDown = (event: KeyboardEvent, issueId: number) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void addIssueComment(issueId);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      closeIssueComment(issueId);
    }
  };

  const addIssueComment = async (issueId: number) => {
    const body = commentBody().trim();
    if (!body || issuePendingLabel(issueId)) return;
    setSelectedIssueId(issueId);
    setPendingIssue(issueId, "Sending");
    try {
      await bridge.invoke("hive_issue_comment", {
        issueId,
        author: "human",
        body,
      });
      setCommentBody("");
      setActiveCommentComposer(null);
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
  const issueDecisionButtonLabel = (card: IssueCard, option: string) =>
    option === "retry"
      ? issueRuntimeActionLabel(card, true)
      : issuePendingLabel(card.issue.id) ?? issueOptionLabel(option);

  const issueOptionDisabled = (card: IssueCard, option: string) =>
    Boolean(issuePendingLabel(card.issue.id)) || (option === "retry" && !card.issue.loop_id);
  const issueDecisionOptions = (card: IssueCard) =>
    card.trace?.human_options.filter((option) => option !== "comment") ?? [];

  const runIssueOption = (card: IssueCard, option: string) => {
    setSelectedIssueId(card.issue.id);
    if (option === "comment") {
      openIssueComment(card.issue.id, "detail");
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

  const auditLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    if (trace.audit_passed === null) return "audit pending";
    return trace.audit_passed ? "audit ok" : `audit fail ${trace.audit_failed_count}`;
  };

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

  const workerReceiptLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_receipt_ok === null) return null;
    return evidence.worker_receipt_ok ? "receipt ok" : "receipt fail";
  };

  const workerStatusLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_status === null) return null;
    return `exit ${evidence.worker_status}`;
  };

  const workerDurationLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_duration_ms === null) return null;
    if (evidence.worker_duration_ms >= 1000) {
      return `${(evidence.worker_duration_ms / 1000).toFixed(1)}s`;
    }
    return `${evidence.worker_duration_ms}ms`;
  };

  const runtimeDurationLabel = (durationMs: number) =>
    durationMs >= 1000 ? `${(durationMs / 1000).toFixed(1)}s` : `${durationMs}ms`;

  const traceRuntimeLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    if (trace.round_role_worker_count === 0) return null;
    return `runtime ${runtimeDurationLabel(trace.round_worker_duration_ms)}`;
  };

  const traceRuntimeWarnLabel = (trace: NonNullable<IssueCard["trace"]>) => {
    const warnings = [];
    if (trace.round_worker_timeout_count) warnings.push(`timeouts ${trace.round_worker_timeout_count}`);
    if (trace.round_worker_retry_exhausted_count) {
      warnings.push(`retry exhausted ${trace.round_worker_retry_exhausted_count}`);
    }
    return warnings.length ? warnings.join(" / ") : null;
  };

  const workerTimeoutLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) =>
    evidence.worker_timeout_secs === null ? null : `limit ${evidence.worker_timeout_secs}s`;

  const workerAttemptLabel = (evidence: NonNullable<IssueCard["trace"]>["evidence"][number]) => {
    if (evidence.worker_attempt_count === null) return null;
    return evidence.worker_max_attempts === null
      ? `attempts ${evidence.worker_attempt_count}`
      : `attempts ${evidence.worker_attempt_count}/${evidence.worker_max_attempts}`;
  };
  const workerCommandLabel = (command: string | null) => {
    if (!command) return null;
    return command.startsWith("codex ")
      ? "cmd codex exec"
      : `cmd ${command.split(/\s+/).slice(0, 2).join(" ")}`;
  };

  const shouldShowTranscriptExcerpt = (
    evidence: NonNullable<IssueCard["trace"]>["evidence"][number],
  ) =>
    Boolean(
      evidence.transcript_excerpt &&
        (evidence.worker_ok === false ||
          evidence.worker_receipt_ok === false ||
          evidence.worker_timed_out === true ||
          evidence.worker_retry_exhausted === true ||
          evidence.worker_receipt_errors.length > 0),
    );

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

  const doctorHealthLabel = (health: string) =>
    ({
      ok: "ok",
      blocked: "blocked",
      needs_review: "needs review",
      rejected: "rejected",
      audit_failed: "audit failed",
      worker_failed: "worker failed",
      pending: "pending",
      unknown: "unknown",
    })[health] ?? health;

  const doctorHealthTone = (health: string) =>
    health === "ok" ? "ok" : health === "pending" ? "pending" : "warn";

  const doctorReceiptLabel = (doctor: IssueDoctorSummary) => {
    const required = doctor.counts.round_receipt_required_count;
    if (required === 0) return "receipts pending";
    const present = required - doctor.counts.round_receipt_missing_count;
    return `receipts ${present}/${required}`;
  };

  const doctorWorkerLabel = (doctor: IssueDoctorSummary) => {
    const total = doctor.counts.round_role_worker_count;
    if (total === 0) return "workers pending";
    return `workers ${doctor.counts.round_role_worker_ok_count}/${total}`;
  };

  const doctorRuntimeLabel = (doctor: IssueDoctorSummary) => {
    if (doctor.counts.round_role_worker_count === 0) return "runtime pending";
    return `runtime ${runtimeDurationLabel(doctor.counts.round_worker_duration_ms)}`;
  };

  const cardDoctor = (card: IssueCard) => card.doctor;

  const cardAuditFailureDetails = (card: IssueCard) =>
    card.doctor?.audit_failure_details.length
      ? card.doctor.audit_failure_details
      : card.trace?.audit_failure_details ?? [];

  const compactAuditFailureDetail = (detail: string) => {
    const parts = detail.split(":").filter(Boolean);
    if (parts.length < 2) return detail;
    return `${parts[0]} / ${parts[parts.length - 1]}`;
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
      ["Runtime", trace ? runtimeDurationLabel(trace.round_worker_duration_ms) : "pending"],
      [
        "Timeouts",
        trace
          ? `${trace.round_worker_timeout_count} timeout / ${trace.round_worker_retry_exhausted_count} exhausted`
          : "pending",
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
      [
        "Operator",
        operatorEventLabel(trace?.last_operator_event ?? null) ?? "none",
      ],
      [
        "Operator Events",
        trace
          ? `${trace.round_operator_event_count}/${trace.operator_event_count}`
          : "pending",
      ],
      ["Packet", schemaLabel(trace?.packet_schema)],
      ["Policy", schemaLabel(trace?.policy_schema)],
      ["Admission", schemaLabel(trace?.admission_schema)],
      ["Verdict", schemaLabel(trace?.verdict_schema)],
      ["Audit", trace ? auditLabel(trace) : "pending"],
      ["Audit Schema", schemaLabel(trace?.audit_schema)],
      [
        "Audit Fails",
        trace?.audit_failed_checks.length ? trace.audit_failed_checks.join(", ") : "none",
      ],
      [
        "Audit Details",
        trace?.audit_failure_details.length ? trace.audit_failure_details.join(", ") : "none",
      ],
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
                    aria-label="Loop title"
                    value={loopTitle()}
                    onInput={(event) => setLoopTitle(event.currentTarget.value)}
                    placeholder="Title"
                  />
                  <textarea
                    aria-label="Loop goal"
                    value={loopGoal()}
                    onInput={(event) => setLoopGoal(event.currentTarget.value)}
                    placeholder="Goal"
                  />
                  <select
                    aria-label="Loop runtime"
                    value={loopRuntime()}
                    onChange={(event) => setLoopRuntime(event.currentTarget.value)}
                  >
                    <option value="codex">codex</option>
                    <option value="local">local</option>
                  </select>
                  <input
                    aria-label="Worker timeout seconds"
                    type="number"
                    min="1"
                    value={loopWorkerTimeoutSecs()}
                    onInput={(event) => setLoopWorkerTimeoutSecs(event.currentTarget.value)}
                    placeholder="Worker timeout seconds"
                  />
                  <input
                    aria-label="Worker attempts"
                    type="number"
                    min="1"
                    max="3"
                    value={loopWorkerAttempts()}
                    onInput={(event) => setLoopWorkerAttempts(event.currentTarget.value)}
                    placeholder="Worker attempts"
                  />
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
                        <Show when={selectedIssueDoctor()} keyed>
                          {(doctor) => (
                            <div class={`doctor-summary doctor-summary--${doctorHealthTone(doctor.health)}`}>
                              <div class="stage-row-head">
                                <strong>Doctor</strong>
                                <span>{doctorHealthLabel(doctor.health)}</span>
                              </div>
                              <p>{doctor.summary}</p>
                              <div class="trace-strip">
                                <span class="trace-pill">{schemaLabel(doctor.schema_version)}</span>
                                <span class="trace-pill">round {doctor.current_round}</span>
                                <span class="trace-pill">{doctorWorkerLabel(doctor)}</span>
                                <span class="trace-pill">{doctorRuntimeLabel(doctor)}</span>
                                {doctor.counts.round_worker_timeout_count ||
                                doctor.counts.round_worker_retry_exhausted_count ? (
                                  <span class="trace-pill trace-pill--warn">
                                    {doctor.counts.round_worker_timeout_count} timeout /{" "}
                                    {doctor.counts.round_worker_retry_exhausted_count} exhausted
                                  </span>
                                ) : null}
                                <span
                                  class={
                                    doctor.counts.round_receipt_missing_count === 0
                                      ? "trace-pill"
                                      : "trace-pill trace-pill--warn"
                                  }
                                >
                                  {doctorReceiptLabel(doctor)}
                                </span>
                                <span
                                  class={
                                    doctor.counts.audit_failed_count === 0
                                      ? "trace-pill"
                                      : "trace-pill trace-pill--warn"
                                  }
                                >
                                  audit {doctor.counts.audit_failed_count}
                                </span>
                              </div>
                              {doctor.failed_checks.length ||
                              doctor.audit_failure_details.length ||
                              doctor.missing_receipts.length ||
                              doctor.worker_failures.length ? (
                                <div class="doctor-lines">
                                  {doctor.failed_checks.map((check) => (
                                    <span>check {check}</span>
                                  ))}
                                  {doctor.audit_failure_details.map((detail) => (
                                    <span>detail {detail}</span>
                                  ))}
                                  {doctor.missing_receipts.map((receipt) => (
                                    <span>missing {receipt}</span>
                                  ))}
                                  {doctor.worker_failures.map((failure) => (
                                    <span>{failure}</span>
                                  ))}
                                </div>
                              ) : null}
                              {doctor.next_actions.length ? (
                                <div class="doctor-actions">
                                  {doctor.next_actions.slice(0, 3).map((action) => (
                                    <code>{action}</code>
                                  ))}
                                </div>
                              ) : null}
                            </div>
                          )}
                        </Show>
                        <dl class="detail-grid">
                          {issueDetailRows(card).map(([label, value]) => (
                            <div>
                              <dt>{label}</dt>
                              <dd>{value}</dd>
                            </div>
                          ))}
                        </dl>
                        {card.trace?.operator_events.length ? (
                          <div class="operator-trail">
                            <h4>Operator Trail</h4>
                            {card.trace.operator_events.map((event) => (
                              <div class="operator-event">
                                <strong>{operatorEventLabel(event)}</strong>
                                <span>{operatorEventStatusLabel(event)}</span>
                                <p>{event.note ?? event.summary}</p>
                              </div>
                            ))}
                          </div>
                        ) : null}
                        {card.trace?.human_options.length ? (
                          <div class="decision-options">
                            {card.issue.loop_id && card.issue.status === "Todo" ? (
                              <button
                                type="button"
                                aria-label={issueRuntimeActionAriaLabel(card, false, "detail")}
                                data-testid={`issue-action-detail-run-${card.issue.id}`}
                                disabled={Boolean(issuePendingLabel(card.issue.id))}
                                onClick={() => void runIssueLoop(card)}
                              >
                                {issueRuntimeActionLabel(card, false)}
                              </button>
                            ) : null}
                            {card.trace.human_options.map((option) => (
                              <button
                                type="button"
                                aria-label={
                                  option === "retry"
                                    ? issueRuntimeActionAriaLabel(card, true, "detail")
                                    : `${issueOptionLabel(option)} issue #${card.issue.id} from detail`
                                }
                                data-testid={`issue-action-detail-${option}-${card.issue.id}`}
                                disabled={issueOptionDisabled(card, option)}
                                onClick={() => runIssueOption(card, option)}
                              >
                                {issueDecisionButtonLabel(card, option)}
                              </button>
                            ))}
                          </div>
                        ) : null}
                        {commentComposerActive(card.issue.id, "detail") ? (
                          <div class="comment-box comment-box--detail">
                            <textarea
                              aria-label={`Detail issue #${card.issue.id} comment`}
                              data-testid={`issue-comment-detail-${card.issue.id}`}
                              value={commentBody()}
                              onInput={(event) => setCommentBody(event.currentTarget.value)}
                              onKeyDown={(event) => handleCommentKeyDown(event, card.issue.id)}
                              placeholder="Comment"
                            />
                            <button
                              type="button"
                              aria-label={`Send detail issue #${card.issue.id} comment`}
                              data-testid={`issue-comment-detail-send-${card.issue.id}`}
                              disabled={commentSubmitDisabled(card.issue.id)}
                              onClick={() => void addIssueComment(card.issue.id)}
                            >
                              {issuePendingLabel(card.issue.id) ?? "Send"}
                            </button>
                            {issueDecisionOptions(card).map((option) => (
                              <button
                                type="button"
                                aria-label={
                                  option === "retry"
                                    ? issueRuntimeActionAriaLabel(card, true, "detail composer")
                                    : `${issueOptionLabel(option)} issue #${card.issue.id} from detail composer`
                                }
                                data-testid={`issue-action-detail-composer-${option}-${card.issue.id}`}
                                disabled={issueOptionDisabled(card, option)}
                                onClick={() => runIssueOption(card, option)}
                              >
                                {issueDecisionButtonLabel(card, option)}
                              </button>
                            ))}
                            <button
                              type="button"
                              aria-label={`Close detail issue #${card.issue.id} comment`}
                              data-testid={`issue-comment-detail-close-${card.issue.id}`}
                              disabled={Boolean(issuePendingLabel(card.issue.id))}
                              onClick={() => closeIssueComment(card.issue.id)}
                            >
                              Close
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
                                  {workerReceiptLabel(evidence) ? (
                                    <span
                                      class={
                                        evidence.worker_receipt_ok === false
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      {workerReceiptLabel(evidence)}
                                    </span>
                                  ) : null}
                                  {evidence.worker_timed_out === true ? (
                                    <span class="trace-pill trace-pill--warn">timeout</span>
                                  ) : null}
                                  {workerStatusLabel(evidence) ? (
                                    <span class="trace-pill">{workerStatusLabel(evidence)}</span>
                                  ) : null}
                                  {workerDurationLabel(evidence) ? (
                                    <span class="trace-pill">{workerDurationLabel(evidence)}</span>
                                  ) : null}
                                  {workerTimeoutLabel(evidence) ? (
                                    <span class="trace-pill">{workerTimeoutLabel(evidence)}</span>
                                  ) : null}
                                  {workerAttemptLabel(evidence) ? (
                                    <span class="trace-pill">{workerAttemptLabel(evidence)}</span>
                                  ) : null}
                                  {workerCommandLabel(evidence.worker_command) ? (
                                    <span class="trace-pill">{workerCommandLabel(evidence.worker_command)}</span>
                                  ) : null}
                                  {evidence.worker_action ? (
                                    <span class="trace-pill">{evidence.worker_action}</span>
                                  ) : null}
                                  {evidence.worker_gate_count !== null ? (
                                    <span class="trace-pill">gates {evidence.worker_gate_count}</span>
                                  ) : null}
                                  {evidence.worker_retry_exhausted === true ? (
                                    <span class="trace-pill trace-pill--warn">retry exhausted</span>
                                  ) : null}
                                  {evidence.blocked_phase ? (
                                    <span class="trace-pill trace-pill--warn">blocked {evidence.blocked_phase}</span>
                                  ) : null}
                                  {evidence.missing_receipts.map((receipt) => (
                                    <span class="trace-pill trace-pill--warn">missing {receipt}</span>
                                  ))}
                                  {evidence.packet_envelope_errors.map((field) => (
                                    <span class="trace-pill trace-pill--warn">invalid {field}</span>
                                  ))}
                                  {evidence.worker_receipt_errors.map((field) => (
                                    <span class="trace-pill trace-pill--warn">receipt {field}</span>
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
                                {evidence.worker_evidence_summary ? (
                                  <p class="muted">{evidence.worker_evidence_summary}</p>
                                ) : null}
                                {evidence.worker_cwd ? (
                                  <p class="muted">cwd {evidence.worker_cwd}</p>
                                ) : null}
                                {shouldShowTranscriptExcerpt(evidence) ? (
                                  <p class="muted">{evidence.transcript_excerpt}</p>
                                ) : null}
                              </div>
                            ))}
                          </div>
                        ) : null}
                        <div class="comment-stack comment-stack--detail">
                          {card.comments.map((comment) => (
                            <div class="comment-line comment-line--detail">
                              <div class="comment-line-head">
                                <strong>{comment.author}</strong>
                                <div class="comment-tags">
                                  {commentPills(comment).map((pill) => (
                                    <span>{pill}</span>
                                  ))}
                                </div>
                              </div>
                              <span>{commentPreview(comment, COMMENT_DETAIL_PREVIEW_LIMIT)}</span>
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
                  {["Todo", "Doing", "Blocked", "Needs Review", "Done", "Canceled"].map((statusName) => (
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
                              <Show when={cardDoctor(card)} keyed>
                                {(doctor) => (
                                  <div class={`doctor-strip doctor-strip--${doctorHealthTone(doctor.health)}`}>
                                    <strong>Doctor</strong>
                                    <span>{doctorHealthLabel(doctor.health)}</span>
                                    <span>{doctorWorkerLabel(doctor)}</span>
                                    <span>{doctorReceiptLabel(doctor)}</span>
                                  </div>
                                )}
                              </Show>
                              {cardAuditFailureDetails(card).length ? (
                                <div class="audit-preview">
                                  {cardAuditFailureDetails(card)
                                    .slice(0, 2)
                                    .map((detail) => (
                                      <span title={detail}>{compactAuditFailureDetail(detail)}</span>
                                    ))}
                                  {cardAuditFailureDetails(card).length > 2 ? (
                                    <span>+{cardAuditFailureDetails(card).length - 2} more</span>
                                  ) : null}
                                </div>
                              ) : null}
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
                                  <span
                                    class={
                                      card.trace.audit_passed === false
                                        ? "trace-pill trace-pill--warn"
                                        : "trace-pill"
                                    }
                                  >
                                    {auditLabel(card.trace)}
                                  </span>
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
                                  {operatorEventLabel(card.trace.last_operator_event) ? (
                                    <span class="trace-pill">
                                      {operatorEventLabel(card.trace.last_operator_event)}
                                    </span>
                                  ) : null}
                                  {workerLabel(card) ? <span class="trace-pill">{workerLabel(card)}</span> : null}
                                  {traceRuntimeLabel(card.trace) ? (
                                    <span class="trace-pill">{traceRuntimeLabel(card.trace)}</span>
                                  ) : null}
                                  {traceRuntimeWarnLabel(card.trace) ? (
                                    <span class="trace-pill trace-pill--warn">
                                      {traceRuntimeWarnLabel(card.trace)}
                                    </span>
                                  ) : null}
                                </div>
                              ) : null}
                              <div class="comment-stack">
                                {card.comments.length > 2 ? (
                                  <div class="comment-more">+{card.comments.length - 2} earlier comments</div>
                                ) : null}
                                {card.comments.slice(-2).map((comment) => (
                                  <div class="comment-line comment-line--compact">
                                    <div class="comment-line-head">
                                      <strong>{comment.author}</strong>
                                      <div class="comment-tags">
                                        {commentPills(comment).slice(0, 3).map((pill) => (
                                          <span>{pill}</span>
                                        ))}
                                      </div>
                                    </div>
                                    <span>{commentPreview(comment, COMMENT_CARD_PREVIEW_LIMIT)}</span>
                                  </div>
                                ))}
                              </div>
                              {commentComposerActive(card.issue.id, "board") ? (
                                <div class="comment-box">
                                  <textarea
                                    aria-label={`Board issue #${card.issue.id} comment`}
                                    data-testid={`issue-comment-board-${card.issue.id}`}
                                    value={commentBody()}
                                    onInput={(event) => setCommentBody(event.currentTarget.value)}
                                    onKeyDown={(event) => handleCommentKeyDown(event, card.issue.id)}
                                    placeholder="Comment"
                                  />
                                  <button
                                    type="button"
                                    aria-label={`Send board issue #${card.issue.id} comment`}
                                    data-testid={`issue-comment-board-send-${card.issue.id}`}
                                    disabled={commentSubmitDisabled(card.issue.id)}
                                    onClick={() => void addIssueComment(card.issue.id)}
                                  >
                                    {issuePendingLabel(card.issue.id) ?? "Send"}
                                  </button>
                                  {issueDecisionOptions(card).map((option) => (
                                    <button
                                      type="button"
                                      aria-label={
                                        option === "retry"
                                          ? issueRuntimeActionAriaLabel(card, true, "board composer")
                                          : `${issueOptionLabel(option)} issue #${card.issue.id} from board composer`
                                      }
                                      data-testid={`issue-action-board-composer-${option}-${card.issue.id}`}
                                      disabled={issueOptionDisabled(card, option)}
                                      onClick={() => runIssueOption(card, option)}
                                    >
                                      {issueDecisionButtonLabel(card, option)}
                                    </button>
                                  ))}
                                  <button
                                    type="button"
                                    aria-label={`Close board issue #${card.issue.id} comment`}
                                    data-testid={`issue-comment-board-close-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => closeIssueComment(card.issue.id)}
                                  >
                                    Close
                                  </button>
                                </div>
                              ) : (
                                <div class="record-actions">
                                  {card.issue.loop_id && ["Todo", "Blocked"].includes(card.issue.status) ? (
                                    <button
                                      type="button"
                                      aria-label={issueRuntimeActionAriaLabel(
                                        card,
                                        card.issue.status === "Blocked",
                                        "board",
                                      )}
                                      data-testid={`issue-action-board-${
                                        card.issue.status === "Blocked" ? "retry" : "run"
                                      }-${card.issue.id}`}
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() =>
                                        card.issue.status === "Blocked"
                                          ? void retryIssueLoop(card)
                                          : void runIssueLoop(card)
                                      }
                                    >
                                      {issueRuntimeActionLabel(card, card.issue.status === "Blocked")}
                                    </button>
                                  ) : null}
                                  {card.issue.status === "Blocked" ? (
                                    <button
                                      type="button"
                                      aria-label={`Review issue #${card.issue.id} from board`}
                                      data-testid={`issue-action-board-review-${card.issue.id}`}
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void decideIssue(card.issue.id, "request-review")}
                                    >
                                      {issuePendingLabel(card.issue.id) ?? "Review"}
                                    </button>
                                  ) : null}
                                  {card.issue.loop_id && card.issue.status === "Needs Review" ? (
                                    <button
                                      type="button"
                                      aria-label={issueRuntimeActionAriaLabel(card, true, "board")}
                                      data-testid={`issue-action-board-retry-${card.issue.id}`}
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void retryIssueLoop(card)}
                                    >
                                      {issueRuntimeActionLabel(card, true)}
                                    </button>
                                  ) : null}
                                  {["Todo", "Blocked", "Needs Review"].includes(card.issue.status) ? (
                                    <button
                                      type="button"
                                      aria-label={`Cancel issue #${card.issue.id} from board`}
                                      data-testid={`issue-action-board-cancel-${card.issue.id}`}
                                      disabled={Boolean(issuePendingLabel(card.issue.id))}
                                      onClick={() => void decideIssue(card.issue.id, "cancel")}
                                    >
                                      {issuePendingLabel(card.issue.id) ?? "Cancel"}
                                    </button>
                                  ) : null}
                                  <button
                                    type="button"
                                    aria-label={`Show issue #${card.issue.id} details`}
                                    data-testid={`issue-action-board-details-${card.issue.id}`}
                                    onClick={() => setSelectedIssueId(card.issue.id)}
                                  >
                                    Details
                                  </button>
                                  <button
                                    type="button"
                                    aria-label={`Comment on issue #${card.issue.id} from board`}
                                    data-testid={`issue-action-board-comment-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => openIssueComment(card.issue.id, "board")}
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
