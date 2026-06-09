import { Match, Switch, createMemo, createResource, createSignal, onCleanup } from "solid-js";
import Nav from "./components/Nav";
import HiveWorkbenchPanel from "./components/HiveWorkbenchPanel";
import { bridge } from "./lib/bridge";

import { compactText, operatorEventLabel } from "./lib/hive";
import type {
  ActiveCommentComposer,
  AppStatus,
  CommentPill,
  CommentSurface,
  DrawerHistory,
  DrawerItem,
  DrawerSummary,
  EvidenceDrilldownItem,
  EvidenceDrilldownReport,
  EvidenceManifestCoverage,
  EvidenceManifestEntry,
  EvidenceManifestReport,
  HiveLoop,
  HiveRun,
  HiveSummary,
  IssueAction,
  IssueCard,
  IssueDoctorSummary,
  IssueTimelineDecisionReceipt,
  IssueTimelineHumanDecision,
  IssueTimelineItem,
  IssueTimelineReport,
  IssueTimelineRoundGroup,
  IssueTransitionPolicyReport,
  LauncherResult,
  LoopControlPacket,
  LoopDashboardAgent,
  LoopDashboardReport,
  LoopDashboardRound,
  LoopDashboardRoundAdmission,
  LoopDashboardRoundEvidence,
  LoopDashboardRoundPacket,
  LoopDashboardRoundVerdict,
  LoopRunArgs,
  RuntimePreflightReport,
  View,
  WorkerLifecycleReport,
  WorkerLifecycleRound,
  WorkerLifecycleWorker,
} from "./lib/hive";

const VIEW_VALUES: View[] = ["status", "drawer", "hive", "panel", "launcher"];

const locationToView = (): View => {
  const urlView = new URLSearchParams(window.location.search).get("view");
  const rawHash = window.location.hash.replace(/^#\/?/, "");
  const rawView = rawHash || urlView || "";
  return VIEW_VALUES.includes(rawView as View) ? (rawView as View) : "status";
};

export default function App() {
  let issueDetailPanel: HTMLElement | undefined;
  const setIssueDetailPanel = (element: HTMLElement) => {
    issueDetailPanel = element;
  };
  const evidenceRows = new Map<number, HTMLElement>();

  const [view, setView] = createSignal<View>(locationToView());
  const [launcherQuery, setLauncherQuery] = createSignal("");
  const [hiveTitle, setHiveTitle] = createSignal("");
  const [hiveProject, setHiveProject] = createSignal("");
  const [loopTitle, setLoopTitle] = createSignal("");
  const [loopGoal, setLoopGoal] = createSignal("");
  const [loopRuntime, setLoopRuntime] = createSignal("codex");
  const [loopWorkerTimeoutSecs, setLoopWorkerTimeoutSecs] = createSignal("");
  const [loopWorkerAttempts, setLoopWorkerAttempts] = createSignal("");
  const [selectedIssueId, setSelectedIssueId] = createSignal<number | null>(null);
  const [selectedIssueRefreshNonce, setSelectedIssueRefreshNonce] = createSignal(0);
  const [selectedEvidenceId, setSelectedEvidenceId] = createSignal<number | null>(null);
  const [activeCommentComposer, setActiveCommentComposer] =
    createSignal<ActiveCommentComposer | null>(null);
  const [commentBody, setCommentBody] = createSignal("");
  const [pendingLoopActions, setPendingLoopActions] = createSignal<Record<number, string>>({});
  const [pendingIssueActions, setPendingIssueActions] = createSignal<Record<number, string>>({});
  const [pendingDemoAction, setPendingDemoAction] = createSignal<string | null>(null);
  const [drawerTitle, setDrawerTitle] = createSignal("");
  const [drawerBody, setDrawerBody] = createSignal("");
  const [banner, setBanner] = createSignal<string>("");

  const selectView = (nextView: View) => {
    setView(nextView);
    const nextHash = `#${nextView}`;
    if (window.location.hash !== nextHash) {
      window.history.replaceState(null, "", nextHash);
    }
  };

  const syncViewFromHash = () => {
    setView(locationToView());
  };

  window.addEventListener("hashchange", syncViewFromHash);
  onCleanup(() => window.removeEventListener("hashchange", syncViewFromHash));

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
  const selectedIssueTimelineKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card) return null;
    return [
      card.issue.id,
      card.issue.updated_at,
      card.comments.length,
      card.trace?.current_round ?? 0,
      card.trace?.evidence_count ?? 0,
      card.trace?.verdict_count ?? 0,
      card.trace?.last_operator_event?.id ?? 0,
      selectedIssueRefreshNonce(),
    ].join(":");
  });
  const [selectedIssueTimeline] = createResource(selectedIssueTimelineKey, async (key) => {
    if (!key) return null;
    const issueId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(issueId)) return null;
    return bridge.invoke<IssueTimelineReport>("hive_issue_timeline", { issueId });
  });
  const selectedIssueActivityTimeline = createMemo(() => {
    const timeline = selectedIssueTimeline();
    const issueId = selectedIssueCard()?.issue.id;
    return timeline && timeline.issue.id === issueId ? timeline : null;
  });
  const selectedIssueTransitionKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card) return null;
    return [
      card.issue.id,
      card.issue.status,
      card.issue.updated_at,
      card.actions.length,
      card.trace?.current_round ?? 0,
      card.trace?.last_decision ?? "pending",
      card.trace?.reason_code ?? "none",
      selectedIssueRefreshNonce(),
    ].join(":");
  });
  const [selectedTransitionPolicy] = createResource(selectedIssueTransitionKey, async (key) => {
    if (!key) return null;
    const issueId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(issueId)) return null;
    return bridge.invoke<IssueTransitionPolicyReport>("hive_issue_transition_policy", { issueId });
  });
  const selectedIssueTransitionPolicy = createMemo(() => {
    const policy = selectedTransitionPolicy();
    const issueId = selectedIssueCard()?.issue.id;
    return policy && policy.issue.id === issueId ? policy : null;
  });
  const selectedIssueDashboardKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card?.issue.loop_id) return null;
    return [
      card.issue.loop_id,
      card.issue.updated_at,
      card.trace?.current_round ?? 0,
      card.trace?.last_decision ?? "pending",
      card.trace?.last_admission_passed ?? "pending",
      card.trace?.round_role_worker_count ?? 0,
      selectedIssueRefreshNonce(),
    ].join(":");
  });
  const [selectedLoopDashboard] = createResource(selectedIssueDashboardKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<LoopDashboardReport>("hive_loop_dashboard", { id: loopId });
  });
  const selectedIssueLoopDashboard = createMemo(() => {
    const dashboard = selectedLoopDashboard();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return dashboard && dashboard.loop_id === loopId ? dashboard : null;
  });
  const [selectedLoopControl] = createResource(selectedIssueDashboardKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<LoopControlPacket>("hive_loop_control", { id: loopId });
  });
  const selectedIssueLoopControl = createMemo(() => {
    const control = selectedLoopControl();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return control && control.loop_id === loopId ? control : null;
  });
  const selectedIssueEvidenceKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card?.issue.loop_id) return null;
    return [
      card.issue.loop_id,
      card.issue.updated_at,
      card.trace?.current_round ?? 0,
      card.trace?.evidence_count ?? 0,
      card.trace?.round_evidence_count ?? 0,
      card.trace?.last_operator_event?.id ?? 0,
      selectedIssueRefreshNonce(),
    ].join(":");
  });
  const [selectedEvidenceDrilldown] = createResource(selectedIssueEvidenceKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<EvidenceDrilldownReport>("hive_loop_evidence_drilldown", { id: loopId });
  });
  const selectedIssueEvidenceDrilldown = createMemo(() => {
    const drilldown = selectedEvidenceDrilldown();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return drilldown && drilldown.loop_id === loopId ? drilldown : null;
  });
  const [selectedEvidenceManifest] = createResource(selectedIssueEvidenceKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<EvidenceManifestReport>("hive_loop_evidence_manifest", { id: loopId });
  });
  const selectedIssueEvidenceManifest = createMemo(() => {
    const manifest = selectedEvidenceManifest();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return manifest && manifest.loop_id === loopId ? manifest : null;
  });
  const selectedIssuePreflightKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card?.issue.loop_id) return null;
    return [
      card.issue.loop_id,
      card.issue.updated_at,
      card.trace?.current_round ?? 0,
      card.trace?.admission_count ?? 0,
      card.trace?.last_admission_passed ?? "pending",
      selectedIssueRefreshNonce(),
    ].join(":");
  });
  const [selectedRuntimePreflight] = createResource(selectedIssuePreflightKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<RuntimePreflightReport>("hive_loop_runtime_preflight", { id: loopId });
  });
  const selectedIssueRuntimePreflight = createMemo(() => {
    const preflight = selectedRuntimePreflight();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return preflight && preflight.loop_id === loopId ? preflight : null;
  });
  const selectedIssueLifecycleKey = createMemo(() => {
    const card = selectedIssueCard();
    if (!card?.issue.loop_id) return null;
    return [
      card.issue.loop_id,
      card.issue.updated_at,
      card.trace?.current_round ?? 0,
      card.trace?.evidence_count ?? 0,
      card.trace?.role_worker_count ?? 0,
      selectedIssueRefreshNonce(),
    ].join(":");
  });
  const [selectedWorkerLifecycle] = createResource(selectedIssueLifecycleKey, async (key) => {
    if (!key) return null;
    const loopId = Number.parseInt(key.split(":")[0], 10);
    if (!Number.isFinite(loopId)) return null;
    return bridge.invoke<WorkerLifecycleReport>("hive_loop_worker_lifecycle", { id: loopId });
  });
  const selectedIssueWorkerLifecycle = createMemo(() => {
    const lifecycle = selectedWorkerLifecycle();
    const loopId = selectedIssueCard()?.issue.loop_id;
    return lifecycle && lifecycle.loop_id === loopId ? lifecycle : null;
  });
  const issueCardsForStatus = (statusName: string) =>
    (issueCards() ?? []).filter((card) => card.issue.status === statusName);
  const reviewQueueCards = createMemo(() =>
    (issueCards() ?? []).filter((card) =>
      card.issue.status === "Blocked" || card.issue.status === "Needs Review",
    ),
  );
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
  const writeClipboardText = async (text: string) => {
    if (navigator.clipboard?.writeText) {
      try {
        await navigator.clipboard.writeText(text);
        return;
      } catch {
        // Browser and Electron focus rules can reject the async clipboard API.
        // Fall back to a selected textarea so the control remains usable.
      }
    }
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "true");
    textarea.style.position = "fixed";
    textarea.style.left = "-9999px";
    document.body.appendChild(textarea);
    window.focus();
    textarea.focus({ preventScroll: true });
    textarea.select();
    const copied = document.execCommand("copy");
    textarea.remove();
    if (!copied) {
      throw new Error("clipboard is unavailable");
    }
  };
  const copyCommandAction = async (label: string, action: string) => {
    try {
      await writeClipboardText(action);
      setBanner(`Copied ${label}: ${compactText(action, 96)}`);
    } catch {
      setBanner(`Copy unavailable; command: ${compactText(action, 120)}`);
    }
  };
  const copyDoctorAction = (action: string) => copyCommandAction("next action", action);
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
  const issueActionByName = (card: IssueCard, actionName: string) =>
    card.actions.find((action) => action.action === actionName);
  const doctorRunArgs = (card: IssueCard, commandNeedles: string | string[]) => {
    const needles = Array.isArray(commandNeedles) ? commandNeedles : [commandNeedles];
    return commandRunArgs(
      card.doctor?.next_actions.find((action) =>
        needles.some((needle) => action.includes(needle)),
      ),
    );
  };
  const mergeRunArgs = (...argsList: LoopRunArgs[]) => {
    const merged: LoopRunArgs = {};
    argsList.forEach((args) => {
      if (args.runtime) merged.runtime = args.runtime;
      if (args.workerTimeoutSecs) merged.workerTimeoutSecs = args.workerTimeoutSecs;
      if (args.workerAttempts) merged.workerAttempts = args.workerAttempts;
    });
    return merged;
  };
  const demoRunArgs = () =>
    mergeRunArgs({ runtime: "codex", workerTimeoutSecs: 90, workerAttempts: 1 }, workerLimitRunArgs());
  const hasRunArgs = (args: LoopRunArgs) =>
    Boolean(args.runtime || args.workerTimeoutSecs || args.workerAttempts);
  const issueRunArgs = (card: IssueCard) => {
    const actionArgs = commandRunArgs(issueActionByName(card, "run")?.command);
    if (hasRunArgs(actionArgs)) return mergeRunArgs(actionArgs, workerLimitRunArgs());
    const doctorArgs = doctorRunArgs(card, ["entrance hive issue run", "entrance hive loop run"]);
    return hasRunArgs(doctorArgs)
      ? mergeRunArgs(doctorArgs, workerLimitRunArgs())
      : loopRunArgs();
  };
  const issueRetryRunArgs = (card: IssueCard) =>
    mergeRunArgs(
      commandRunArgs(issueActionByName(card, "retry")?.command),
      doctorRunArgs(card, "entrance hive issue retry-run"),
      workerLimitRunArgs(),
    );
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

  const revealIssueDetail = () => {
    window.setTimeout(() => {
      issueDetailPanel?.scrollIntoView({ block: "start", behavior: "auto" });
    }, 50);
  };
  const focusEvidence = (issueId: number, evidenceId: number) => {
    setSelectedIssueId(issueId);
    setSelectedEvidenceId(evidenceId);
    window.setTimeout(() => {
      (evidenceRows.get(evidenceId) ?? issueDetailPanel)?.scrollIntoView({
        block: "center",
        behavior: "auto",
      });
    }, 50);
  };
  const commentPillNode = (issueId: number, pill: CommentPill) =>
    pill.evidenceId ? (
      <button
        type="button"
        class="comment-tag-button"
        aria-label={`Show evidence #${pill.evidenceId} for issue #${issueId}`}
        onClick={() => focusEvidence(issueId, pill.evidenceId ?? 0)}
      >
        {pill.label}
      </button>
    ) : (
      <span>{pill.label}</span>
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
  const refreshSelectedIssueSurfaces = (issueId: number | null | undefined = selectedIssueId()) => {
    const selected = selectedIssueId();
    if (selected !== null && (issueId === null || issueId === undefined || selected === issueId)) {
      setSelectedIssueRefreshNonce((current) => current + 1);
    }
  };
  const refetchLoopSurfaces = async () => {
    await Promise.all([refetchHiveLoops(), refetchIssueCards(), refetchStatus()]);
  };
  const refetchIssueControlSurfaces = async (issueId: number | null | undefined = selectedIssueId()) => {
    await refetchLoopSurfaces();
    refreshSelectedIssueSurfaces(issueId);
  };
  const pollLoopSurfaces = () => {
    void refetchLoopSurfaces().catch(() => undefined);
  };
  const withLoopProgressPolling = async <T,>(work: Promise<T>) => {
    pollLoopSurfaces();
    const interval = window.setInterval(pollLoopSurfaces, 2500);
    try {
      return await work;
    } finally {
      window.clearInterval(interval);
    }
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
    const report = await bridge.invoke<{ issues: IssueCard[] }>("hive_loop_create", {
      title: loopTitle() || "Untitled loop",
      goal: loopGoal() || loopTitle() || "Run an Entrance loop",
      runtime: loopRuntime(),
      approachSpace: ["Explore the smallest runnable MVP"],
      evalSpace: ["CLI loop run produces a keep/reject/block verdict"],
    });
    const createdIssueId = report.issues[0]?.issue.id ?? null;
    if (createdIssueId !== null) {
      setSelectedIssueId(createdIssueId);
    }
    setLoopTitle("");
    setLoopGoal("");
    setBanner(
      createdIssueId === null
        ? "Loop contract created."
        : `Loop contract created as issue #${createdIssueId}.`,
    );
    await refetchIssueControlSurfaces(createdIssueId);
    if (createdIssueId !== null) {
      revealIssueDetail();
    }
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

  const startDemoLoop = async () => {
    if (pendingDemoAction()) return;
    selectView("panel");
    setPendingDemoAction("Running Demo");
    setBanner("Running Entrance MVP demo.");
    try {
      const runArgs = demoRunArgs();
      const report = await bridge.invoke<{ contract?: HiveLoop; issues: IssueCard[] }>("hive_loop_create", {
        title: "Entrance MVP demo",
        goal: "Run the Entrance Explorer -> Developer -> Reviewer loop and expose it on the issue/status/comment panel.",
        boundary: "Use the local Hive SQLite ledger, typed receipts, compact CLI output, and the local Panel surface.",
        runtime: "codex",
        approachSpace: [
          "Compile the natural-language goal into a typed candidate",
          "Develop only the admitted candidate",
          "Review the evidence with keep/reject/block gates",
        ],
        evalSpace: [
          "Explorer, Developer, and Reviewer each produce role receipts",
          "Admissions bind packets to policy gates",
          "Panel shows issue status, comments, evidence, verdict, and recovery actions",
        ],
      });
      const issue = report.issues[0];
      if (!issue) throw new Error("Demo loop did not create an issue.");
      setSelectedIssueId(issue.issue.id);
      await refetchIssueControlSurfaces(issue.issue.id);
      await withLoopProgressPolling(bridge.invoke("hive_issue_run", {
        issueId: issue.issue.id,
        ...runArgs,
      }));
      const loopId = issue.issue.loop_id ?? report.contract?.id;
      setBanner(loopId ? `Demo loop #${loopId} finished.` : "Demo loop finished.");
      await refetchIssueControlSurfaces(issue.issue.id);
      revealIssueDetail();
    } catch (error) {
      setBanner(`Demo loop failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingDemoAction(null);
    }
  };


  const runHiveLoop = async (loop: HiveLoop) => {
    if (loopPendingLabel(loop.id)) return;
    setPendingLoop(loop.id, "Running");
    try {
      const runArgs = loopRunArgs();
      await withLoopProgressPolling(bridge.invoke("hive_loop_run", {
        id: loop.id,
        ...runArgs,
        runtime: loop.runtime || runArgs.runtime,
      }));
      setBanner(`Loop #${loop.id} finished.`);
      await refetchIssueControlSurfaces(selectedIssueId());
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
      await withLoopProgressPolling(bridge.invoke("hive_issue_run", {
        issueId: card.issue.id,
        ...issueRunArgs(card),
      }));
      setBanner(`Loop #${card.issue.loop_id} finished.`);
      await refetchIssueControlSurfaces(card.issue.id);
    } catch (error) {
      setBanner(`Loop #${card.issue.loop_id} failed: ${actionErrorMessage(error)}`);
    } finally {
      setPendingIssue(card.issue.id, null);
    }
  };

  const advanceIssue = async (card: IssueCard) => {
    if (!card.issue.loop_id || issuePendingLabel(card.issue.id)) return;
    setSelectedIssueId(card.issue.id);
    setPendingIssue(card.issue.id, "Advancing");
    try {
      await withLoopProgressPolling(bridge.invoke("hive_issue_advance", {
        issueId: card.issue.id,
        mode: "until_wait",
        ...issueRunArgs(card),
      }));
      setBanner(`Issue #${card.issue.id} advanced.`);
      await refetchIssueControlSurfaces(card.issue.id);
    } catch (error) {
      setBanner(`Issue #${card.issue.id} advance failed: ${actionErrorMessage(error)}`);
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
      await refetchIssueControlSurfaces(issueId);
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
      await withLoopProgressPolling(bridge.invoke("hive_issue_run", {
        issueId: card.issue.id,
        retry: true,
        author: "human",
        body: issueDecisionNote(card.issue.id) || undefined,
        ...issueRetryRunArgs(card),
      }));
      clearIssueComposer(card.issue.id);
      setBanner(`Issue #${card.issue.id} retried.`);
      await refetchIssueControlSurfaces(card.issue.id);
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
      await refetchIssueControlSurfaces(issueId);
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

  const issueDecisionButtonLabel = (card: IssueCard, action: IssueAction) =>
    action.action === "retry"
      ? issueRuntimeActionLabel(card, true)
      : issuePendingLabel(card.issue.id) ?? action.label;
  const issueActionContractLabel = (action: IssueAction) =>
    [
      action.action,
      action.source,
      action.input,
      action.runtime ?? "no-runtime",
      action.confirmation_required ? "confirmed" : "no-confirm",
      action.destructive ? "destructive" : "non-destructive",
    ].join(" / ");
  const issueActionTitle = (action: IssueAction) =>
    [
      `schema=${action.schema_version}`,
      `source=${action.source}`,
      `input=${action.input}`,
      `runtime=${action.runtime ?? "none"}`,
      `confirmation_required=${action.confirmation_required ? "true" : "false"}`,
      action.confirmation_arg ? `confirmation_arg=${action.confirmation_arg}` : null,
      action.receipt_schema ? `receipt_schema=${action.receipt_schema}` : null,
      action.policy_schema_version ? `policy=${action.policy_schema_version}` : null,
      `destructive=${action.destructive ? "true" : "false"}`,
      action.command,
    ]
      .filter((value): value is string => Boolean(value))
      .join(" | ");
  const issueActionButtonAttrs = (action: IssueAction | undefined) =>
    action
      ? {
          "data-action-destructive": action.destructive ? "true" : "false",
          "data-action-input": action.input,
          "data-action-runtime": action.runtime ?? "",
          "data-action-schema-version": action.schema_version,
          "data-action-source": action.source,
          "data-action-confirmation-required": action.confirmation_required ? "true" : "false",
          "data-action-confirmation-arg": action.confirmation_arg ?? "",
          "data-action-receipt-schema": action.receipt_schema ?? "",
          "data-action-policy-schema-version": action.policy_schema_version ?? "",
          title: issueActionTitle(action),
        }
      : {};
  const issueActionContractChips = (card: IssueCard, surface: string) => (
    <div
      class="action-contracts"
      data-testid={`issue-action-contracts-${surface}-${card.issue.id}`}
    >
      {card.actions.map((action) => (
        <span
          class={`action-contract${action.destructive ? " action-contract--destructive" : ""}`}
          data-testid={`issue-action-contract-${surface}-${action.action}-${card.issue.id}`}
          title={issueActionTitle(action)}
        >
          {issueActionContractLabel(action)}
        </span>
      ))}
    </div>
  );

  const issueOptionDisabled = (card: IssueCard, action: IssueAction) =>
    Boolean(issuePendingLabel(card.issue.id)) ||
    (action.action === "retry" && !card.issue.loop_id);
  const issueHumanActions = (card: IssueCard) =>
    card.actions.filter((action) => action.source !== "runtime" && action.action !== "run");
  const issueDecisionActions = (card: IssueCard) =>
    issueHumanActions(card).filter((action) => action.action !== "comment");

  const runIssueAction = (card: IssueCard, action: IssueAction) => {
    const option = action.action;
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

  const reviewQueueDecisionLabel = (card: IssueCard) =>
    card.trace?.last_decision
      ? `${card.trace.last_decision}${card.trace.reason_code ? ` / ${card.trace.reason_code}` : ""}`
      : card.issue.status;

  const reviewQueueBlockerLabel = (card: IssueCard) => {
    const doctor = card.doctor;
    if (!doctor) return "doctor pending";
    const blockers = [
      ...doctor.failed_checks,
      ...doctor.audit_failure_details,
      ...doctor.missing_receipts.map((receipt) => `missing ${receipt}`),
      ...doctor.worker_failures,
    ];
    return blockers.length ? compactText(blockers.join(", "), 96) : doctor.health;
  };

  const reviewQueueEvidence = (card: IssueCard) => card.trace?.evidence.slice(-3) ?? [];

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

  const loopDashboardStateLabel = (state: string) =>
    ({
      ok: "ok",
      done: "done",
      ready: "ready",
      running: "running",
      pending: "pending",
      blocked: "blocked",
      needs_review: "needs review",
      worker_failed: "worker failed",
      canceled: "canceled",
      attention: "attention",
    })[state] ?? state;

  const loopDashboardTone = (state: string) =>
    state === "ok" || state === "done" || state === "ready"
      ? "ok"
      : state === "pending" || state === "running"
        ? "pending"
        : "warn";

  const loopDashboardAgentTone = (agent: LoopDashboardAgent) =>
    agent.state === "ok" ? "ok" : agent.state === "pending" || agent.state === "observed" ? "pending" : "warn";

  const loopDashboardAgentLabel = (agent: LoopDashboardAgent) => {
    if (agent.state === "ok") return "ok";
    if (agent.state === "retry_exhausted") return "retry exhausted";
    return agent.state.replace(/_/g, " ");
  };

  const loopDashboardGateLabel = (dashboard: LoopDashboardReport) => {
    const passed = dashboard.kernel.gate_passed;
    if (passed === true) return `${dashboard.kernel.gate} ok`;
    if (passed === false) return `${dashboard.kernel.gate} blocked`;
    return `${dashboard.kernel.gate} pending`;
  };

  const loopDashboardHumanLabel = (dashboard: LoopDashboardReport) =>
    dashboard.human_decision.required
      ? `human decision ${dashboard.human_decision.actions.length}`
      : "human clear";

  const loopControlTone = (control: LoopControlPacket) =>
    control.state.reviewer_invalid_budget_exhausted || control.state.needs_human_decision
      ? "warn"
      : control.state.primary_action === "issue_run"
        ? "pending"
        : "ok";

  const loopControlStateLabel = (control: LoopControlPacket) =>
    control.state.primary_action === "human_decision"
      ? "human decision"
      : control.state.primary_action.replace(/_/g, " ");

  const loopControlBudgetLabel = (control: LoopControlPacket) => {
    const used = control.state.reviewer_invalid_rounds_used;
    const budget = control.state.reviewer_invalid_round_budget;
    if (used === null || budget === null) return "budget pending";
    const exhausted = control.state.reviewer_invalid_budget_exhausted ? " exhausted" : "";
    return `reviewer ${used}/${budget}${exhausted}`;
  };

  const loopControlGateLabel = (
    gate: LoopControlPacket["reviewer_gate_surface"]["gates"]["runtime_preflight"],
  ) => {
    if (!gate) return "runtime pending";
    if (gate.passed === true) return `${gate.gate ?? "runtime"} ok`;
    if (gate.passed === false) return `${gate.gate ?? "runtime"} blocked`;
    return `${gate.gate ?? "runtime"} pending`;
  };

  const loopControlScoreLabel = (metric: LoopControlPacket["reviewer_gate_surface"]["score_vector"][number]) =>
    `${scoreMetricLabel(metric.name)} ${scoreValueLabel(metric.value ?? metric.score ?? null)}`;

  const loopControlOptionTone = (option: LoopControlPacket["operator_decision_surface"]["options"][number]) =>
    option.enabled ? "pending" : "warn";

  const loopControlCallLabel = (option: LoopControlPacket["operator_decision_surface"]["options"][number]) => {
    const call = option.call;
    if (!call?.name) return option.tool ?? "inspect";
    return call.name;
  };

  const loopDashboardRoundTone = (round: LoopDashboardRound) =>
    round.blocker || round.rejected_count || round.receipt_missing_count ? "warn" : "ok";

  const loopDashboardRoundLabel = (round: LoopDashboardRound) => {
    const decision = round.decision ? ` ${round.decision}` : "";
    const current = round.current ? " current" : "";
    return `r${round.round} ${round.status}${decision}${current}`;
  };

  const loopDashboardRoundCounts = (round: LoopDashboardRound) =>
    `packets ${round.packet_count} / admissions ${round.admission_count} / evidence ${round.evidence_count} / verdicts ${round.verdict_count}`;

  const loopDashboardPacketLabel = (packet: LoopDashboardRoundPacket) =>
    `packet ${packet.writer_role} ${packet.route_from}->${packet.route_to} ${packet.object_kind} ${packet.admission_result ?? "pending"}`;

  const loopDashboardAdmissionLabel = (admission: LoopDashboardRoundAdmission) =>
    `gate ${admission.gate ?? "unknown"} ${admission.result}`;

  const loopDashboardEvidenceLabel = (evidence: LoopDashboardRoundEvidence) =>
    `evidence ${evidence.stage_role ?? "kernel"} ${evidence.kind}`;

  const loopDashboardVerdictLabel = (verdict: LoopDashboardRoundVerdict) =>
    `verdict ${verdict.decision}${verdict.reason_code ? ` ${verdict.reason_code}` : ""}`;

  const transitionPolicyTone = (policy: IssueTransitionPolicyReport) =>
    policy.state_class === "terminal"
      ? "ok"
      : policy.human_decision_required || policy.state_class === "needs_human"
        ? "warn"
        : "pending";

  const transitionPolicyStateLabel = (state: string) =>
    ({
      runnable: "runnable",
      running: "running",
      needs_human: "needs human",
      terminal: "terminal",
      unknown: "unknown",
    })[state] ?? state;

  const transitionPolicyActionLabel = (
    action: IssueTransitionPolicyReport["allowed_actions"][number],
  ) =>
    `${action.action.label} -> ${action.to_status ?? "unknown"}${action.requires_human ? " / human" : ""}`;

  const transitionPolicyBudgetLabel = (policy: IssueTransitionPolicyReport) => {
    const budget = policy.reviewer_budget;
    if (!budget) return "reviewer budget none";
    const exhausted = budget.reviewer_invalid_budget_exhausted ? " exhausted" : "";
    return `reviewer ${budget.reviewer_invalid_rounds_used}/${budget.reviewer_invalid_round_budget}${exhausted}`;
  };

  const evidenceDrilldownTone = (state: string) =>
    state === "complete" ? "ok" : state === "observing" ? "pending" : "warn";

  const evidenceDrilldownStateLabel = (state: string) =>
    ({
      complete: "complete",
      observing: "observing",
      blocked: "blocked",
      needs_human: "needs human",
    })[state] ?? state;

  const evidenceManifestTone = (state: string) =>
    state === "ok" ? "ok" : state === "observing" ? "pending" : "warn";

  const evidenceManifestStateLabel = (state: string) =>
    ({
      ok: "ok",
      observing: "observing",
      reviewing: "reviewing",
      blocked: "blocked",
    })[state] ?? state;

  const evidenceManifestCoverageLabel = (coverage: EvidenceManifestCoverage) =>
    `entries ${coverage.entry_count} / payloads ${coverage.payload_count} / receipts ${coverage.receipt_count} / artifacts ${coverage.artifact_count}`;

  const evidenceManifestPathLabel = (coverage: EvidenceManifestCoverage) =>
    `paths ${coverage.path_present_count}/${coverage.path_missing_count}/${coverage.path_unverified_count}`;

  const evidenceManifestEntryTone = (entry: EvidenceManifestEntry) =>
    !entry.verified || entry.path_status === "missing" ? "warn" : "ok";

  const evidenceManifestEntryDigestLabel = (entry: EvidenceManifestEntry) =>
    entry.sha256 ? `sha256 ${entry.sha256.slice(0, 12)}` : "sha256 none";

  const evidenceManifestEntryPathLabel = (entry: EvidenceManifestEntry) =>
    entry.path ? `${entry.path_status} ${entry.path}` : entry.path_status;

  const evidenceManifestEntrySizeLabel = (entry: EvidenceManifestEntry) =>
    entry.size_bytes == null ? null : `${entry.size_bytes} bytes`;

  const issueTimelineTone = (state: string) =>
    state === "closed" ? "ok" : state === "open" || state === "running" ? "pending" : "warn";

  const issueTimelineStateLabel = (state: string) =>
    ({
      closed: "closed",
      open: "open",
      running: "running",
      needs_human: "needs human",
      observing: "observing",
    })[state] ?? state;

  const issueTimelineCountsLabel = (timeline: IssueTimelineReport) =>
    `items ${timeline.counts.item_count} / comments ${timeline.counts.comment_count} / evidence ${timeline.counts.evidence_count} / verdicts ${timeline.counts.verdict_count}`;

  const issueTimelineRoundTone = (round: IssueTimelineRoundGroup) =>
    round.blocker_count || round.state === "blocked" || round.state === "needs_human"
      ? "warn"
      : round.state === "observing"
        ? "pending"
        : "ok";

  const issueTimelineRoundCountsLabel = (round: IssueTimelineRoundGroup) =>
    `comments ${round.comment_count} / evidence ${round.evidence_count} / verdicts ${round.verdict_count}`;

  const issueTimelineRoundMeta = (round: IssueTimelineRoundGroup) =>
    [
      round.label,
      round.state,
      round.phases.length ? round.phases.join("/") : null,
      round.decisions.length ? `decision ${round.decisions.join("/")}` : null,
    ]
      .filter(Boolean)
      .join(" ");

  const issueTimelineDecisionLabel = (decision: IssueTimelineHumanDecision) =>
    `decision ${decision.primary_action ?? "comment"}`;

  const issueTimelineReceiptTone = (receipt: IssueTimelineDecisionReceipt) =>
    receipt.human_confirmed === false || !receipt.receipt_schema_version ? "warn" : "ok";

  const issueTimelineReceiptLabel = (receipt: IssueTimelineDecisionReceipt) =>
    [
      receipt.action ?? "decision",
      receipt.receipt_source ?? receipt.source,
      receipt.author,
      receipt.round == null ? null : `r${receipt.round}`,
    ]
      .filter(Boolean)
      .join(" ");

  const issueTimelineReceiptMeta = (receipt: IssueTimelineDecisionReceipt) =>
    [
      receipt.comment_id ? `comment #${receipt.comment_id}` : null,
      receipt.evidence_id ? `evidence #${receipt.evidence_id}` : null,
      receipt.actor_label ? `actor ${receipt.actor_label}` : null,
      receipt.actor_trust,
      receipt.client_name ? `client ${receipt.client_name}` : null,
      receipt.human_confirmed === true ? "confirmed" : "unconfirmed",
    ]
      .filter(Boolean)
      .join(" ");

  const issueTimelineItemTone = (item: IssueTimelineItem) =>
    item.blocker || item.status === "blocked" || item.status === "failed" ? "warn" : "ok";

  const issueTimelineItemMeta = (item: IssueTimelineItem) =>
    [
      item.source,
      item.round == null ? null : `r${item.round}`,
      item.phase,
      item.status,
      item.decision ? `decision ${item.decision}` : null,
    ]
      .filter(Boolean)
      .join(" ");

  const issueTimelineTimeLabel = (item: IssueTimelineItem) =>
    item.timestamp ? item.timestamp.replace("T", " ").replace("Z", "") : `#${item.sequence}`;

  const evidenceItemTone = (item: EvidenceDrilldownItem) =>
    item.blocker || item.admission_result === "rejected" || item.worker?.ok === false
      ? "warn"
      : "ok";

  const evidenceItemLabel = (item: EvidenceDrilldownItem) =>
    `#${item.id} r${item.round} ${item.stage_role ?? "kernel"} ${item.kind}`;

  const evidenceDrilldownWorkerLabel = (item: EvidenceDrilldownItem) => {
    const worker = item.worker;
    if (!worker) return "worker none";
    const state = worker.ok === true ? "ok" : worker.ok === false ? "fail" : "pending";
    return `${worker.kind ?? "worker"} ${state}${worker.receipt_ok === false ? " receipt fail" : ""}`;
  };

  const evidenceReceiptLabel = (item: EvidenceDrilldownItem) => {
    const receipt = item.receipt;
    if (!receipt) return "receipt none";
    return `receipt ${receipt.role ?? "unknown"} ${receipt.action ?? "unknown"} gates ${receipt.gates.length}`;
  };

  const evidenceArtifactLabel = (item: EvidenceDrilldownItem) =>
    item.artifacts.length
      ? `artifacts ${item.artifacts.length}${item.artifacts[0]?.path ? ` ${item.artifacts[0].path}` : ""}`
      : "artifacts none";

  const evidencePayloadDiffLabel = (item: EvidenceDrilldownItem) => {
    const diff = item.payload.diff_from_previous;
    if (diff.relative_to_evidence_id == null) return "payload baseline";
    return `payload +${diff.added_keys.length} -${diff.removed_keys.length} ~${diff.changed_keys.length}`;
  };

  const runtimePreflightStateLabel = (state: string) =>
    ({
      admitted: "admitted",
      ready: "ready",
      blocked: "blocked",
      pending: "pending",
    })[state] ?? state;

  const runtimePreflightTone = (state: string) =>
    state === "admitted" || state === "ready"
      ? "ok"
      : state === "pending"
        ? "pending"
        : "warn";

  const runtimePreflightBoolLabel = (label: string, value: boolean | null | undefined) => {
    if (value === true) return `${label} ok`;
    if (value === false) return `${label} blocked`;
    return `${label} pending`;
  };

  const runtimePreflightGateLabel = (preflight: RuntimePreflightReport) => {
    const gate = preflight.current?.gate ?? preflight.policy.gate;
    const passed = preflight.current?.gate_passed;
    if (passed === true) return `${gate} ok`;
    if (passed === false) return `${gate} blocked`;
    return `${gate} pending`;
  };

  const runtimePreflightProbeLabel = (preflight: RuntimePreflightReport) => {
    const duration = preflight.preview.runtime_probe.duration_ms;
    return typeof duration === "number"
      ? `probe ${preflight.preview.probe_ok ? "ok" : "blocked"} ${runtimeDurationLabel(duration)}`
      : `probe ${preflight.preview.probe_ok ? "ok" : "blocked"}`;
  };

  const workerLifecycleStateLabel = (state: string) =>
    ({
      succeeded: "succeeded",
      blocked: "blocked",
      needs_review: "needs review",
      worker_failed: "worker failed",
      canceled: "canceled",
      running: "running",
      pending: "pending",
      observed: "observed",
    })[state] ?? state;

  const workerLifecycleTone = (state: string) =>
    state === "succeeded" ? "ok" : state === "pending" || state === "running" ? "pending" : "warn";

  const workerLifecycleWorkerState = (worker: WorkerLifecycleWorker | null | undefined) => {
    if (!worker) return "missing";
    if (worker.retry_exhausted) return "retry exhausted";
    if (worker.timed_out) return "timeout";
    if (worker.ok === true && worker.receipt_ok !== false) return "ok";
    if (worker.ok === false || worker.receipt_ok === false || worker.receipt_errors.length) return "blocked";
    return "observed";
  };

  const workerLifecycleRoleTone = (worker: WorkerLifecycleWorker | null | undefined) => {
    const state = workerLifecycleWorkerState(worker);
    return state === "ok" ? "ok" : state === "observed" ? "pending" : "warn";
  };

  const workerLifecycleReceiptLabel = (worker: WorkerLifecycleWorker | null | undefined) => {
    if (!worker) return "receipt missing";
    if (worker.receipt_ok === null) return "receipt pending";
    return worker.receipt_ok ? "receipt ok" : "receipt fail";
  };

  const workerLifecycleAttemptLabel = (worker: WorkerLifecycleWorker | null | undefined) => {
    if (worker?.attempt_count === null || worker?.attempt_count === undefined) return null;
    return worker.max_attempts
      ? `attempts ${worker.attempt_count}/${worker.max_attempts}`
      : `attempts ${worker.attempt_count}`;
  };

  const workerLifecycleDurationLabel = (worker: WorkerLifecycleWorker | null | undefined) =>
    worker?.duration_ms === null || worker?.duration_ms === undefined
      ? null
      : runtimeDurationLabel(worker.duration_ms);

  const workerLifecycleBudgetLabel = (lifecycle: WorkerLifecycleReport) =>
    `review budget ${lifecycle.current.reviewer_invalid_rounds_used}/${lifecycle.policy.reviewer_invalid_round_budget}`;

  const workerLifecycleRoundLabel = (round: WorkerLifecycleRound) => {
    const workers = round.worker_count ? ` ${round.worker_ok_count}/${round.worker_count}` : "";
    const decision = round.decision ? ` ${round.decision}` : "";
    const warn = round.worker_timeout_count || round.worker_retry_exhausted_count ? " retry" : "";
    return `r${round.round} ${round.status}${decision}${workers}${warn}`;
  };

  const workerLifecycleWorkerForRole = (round: WorkerLifecycleRound, role: string) =>
    round.workers.find((worker) => worker.role === role) ?? null;

  const roundHistoryLabel = (card: IssueCard) =>
    card.trace?.rounds.length
      ? card.trace.rounds
          .map((round) => {
            const workers = round.worker_count ? ` ${round.worker_ok_count}/${round.worker_count}` : "";
            const receipts = round.receipt_required_count
              ? ` r${round.receipt_required_count - round.receipt_missing_count}/${round.receipt_required_count}`
              : "";
            const warn = round.worker_timeout_count || round.worker_retry_exhausted_count ? " timeout" : "";
            return `r${round.round} ${round.status}${workers}${receipts}${warn}`;
          })
          .join(" | ")
      : null;

  const roundRecoveryLabel = (card: IssueCard) => {
    const rounds = card.trace?.rounds ?? [];
    if (card.issue.status !== "Done" || !rounds.length) return null;
    const current = card.trace?.current_round ?? 0;
    const failed = rounds
      .filter((round) => round.round < current)
      .filter(
        (round) =>
          round.status !== "kept" &&
          (round.rejected_count ||
            round.receipt_missing_count ||
            round.worker_timeout_count ||
            round.worker_retry_exhausted_count),
      )
      .map((round) => round.round);
    return failed.length ? `recovered r${failed.join(",r")} -> r${current}` : null;
  };

  const cardDoctor = (card: IssueCard) => card.doctor;

  const cardAuditFailureDetails = (card: IssueCard) =>
    card.doctor?.audit_failure_details.length
      ? card.doctor.audit_failure_details
      : card.trace?.audit_failure_details ?? [];
  const loopAuditCommand = (card: IssueCard) =>
    card.issue.loop_id ? `entrance hive loop audit ${card.issue.loop_id} --compact` : null;
  const loopEvidenceCommand = (card: IssueCard) =>
    card.issue.loop_id ? `entrance hive loop evidence ${card.issue.loop_id}` : null;
  const compactAuditFailureDetail = (detail: string) => {
    const parts = detail.split(":").filter(Boolean);
    if (parts.length < 2) return detail;
    return `${parts[0]} / ${parts[parts.length - 1]}`;
  };

  const issueAuditQuickActions = (card: IssueCard) => {
    const auditCommand = loopAuditCommand(card);
    const evidenceCommand = loopEvidenceCommand(card);
    if (!auditCommand || !evidenceCommand) return null;
    return (
      <div class="audit-preview-actions">
        <button
          type="button"
          aria-label={`Copy compact audit for issue #${card.issue.id}`}
          data-testid={`issue-audit-copy-${card.issue.id}`}
          title={auditCommand}
          onClick={() => void copyCommandAction("audit command", auditCommand)}
        >
          Audit
        </button>
        <button
          type="button"
          aria-label={`Copy evidence command for issue #${card.issue.id}`}
          data-testid={`issue-evidence-copy-${card.issue.id}`}
          title={evidenceCommand}
          onClick={() => void copyCommandAction("evidence command", evidenceCommand)}
        >
          Evidence
        </button>
      </div>
    );
  };

  const issueDetailRows = (card: IssueCard) => {
    const trace = card.trace;
    return [
      ["Status", card.issue.status],
      ["Loop", card.issue.loop_id ? `#${card.issue.loop_id}` : "unlinked"],
      ["Assignee", card.issue.assignee ?? "unassigned"],
      [
        "Claim",
        card.issue.claim_role
          ? `${card.issue.claim_role} / ${card.issue.claim_source ?? "unknown"}`
          : "unclaimed",
      ],
      ["Claimed", card.issue.claimed_at ?? "pending"],
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
  const storeSchemaLabel = () => {
    const schema = status()?.schema;
    if (!schema) return "pending";
    const tables = schema.tables.filter((table) => table.present).length;
    const indexes = schema.indexes.filter((index) => index.present).length;
    return `${schema.healthy ? "ok" : "blocked"} v${schema.user_version}/${schema.expected_user_version} tables ${tables}/${schema.tables.length} indexes ${indexes}/${schema.indexes.length}`;
  };
  const storeSchemaTitle = () => {
    const schema = status()?.schema;
    if (!schema) return "Store schema pending";
    const missing = [
      ...schema.missing_tables.map((value) => `table:${value}`),
      ...schema.missing_columns.map((value) => `column:${value}`),
      ...schema.missing_indexes.map((value) => `index:${value}`),
    ];
    return missing.length ? missing.join(" | ") : schema.schema_version;
  };

  return (
    <div class="app-shell">
      <Nav current={view()} onSelect={selectView} />

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
                  <div><dt>Schema</dt><dd title={storeSchemaTitle()}>{storeSchemaLabel()}</dd></div>
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
            <HiveWorkbenchPanel
              addIssueComment={addIssueComment}
              advanceIssue={advanceIssue}
              auditLabel={auditLabel}
              cardAuditFailureDetails={cardAuditFailureDetails}
              cardDoctor={cardDoctor}
              closeIssueComment={closeIssueComment}
              commentBody={commentBody}
              commentComposerActive={commentComposerActive}
              commentPillNode={commentPillNode}
              commentSubmitDisabled={commentSubmitDisabled}
              compactAuditFailureDetail={compactAuditFailureDetail}
              copyDoctorAction={copyDoctorAction}
              createHiveLoop={createHiveLoop}
              doctorHealthLabel={doctorHealthLabel}
              doctorHealthTone={doctorHealthTone}
              doctorReceiptLabel={doctorReceiptLabel}
              doctorRuntimeLabel={doctorRuntimeLabel}
              doctorWorkerLabel={doctorWorkerLabel}
              evidenceArtifactLabel={evidenceArtifactLabel}
              evidenceDrilldownStateLabel={evidenceDrilldownStateLabel}
              evidenceDrilldownTone={evidenceDrilldownTone}
              evidenceDrilldownWorkerLabel={evidenceDrilldownWorkerLabel}
              evidenceItemLabel={evidenceItemLabel}
              evidenceItemTone={evidenceItemTone}
              evidenceManifestCoverageLabel={evidenceManifestCoverageLabel}
              evidenceManifestEntryDigestLabel={evidenceManifestEntryDigestLabel}
              evidenceManifestEntryPathLabel={evidenceManifestEntryPathLabel}
              evidenceManifestEntrySizeLabel={evidenceManifestEntrySizeLabel}
              evidenceManifestEntryTone={evidenceManifestEntryTone}
              evidenceManifestPathLabel={evidenceManifestPathLabel}
              evidenceManifestStateLabel={evidenceManifestStateLabel}
              evidenceManifestTone={evidenceManifestTone}
              evidencePayloadDiffLabel={evidencePayloadDiffLabel}
              evidenceReceiptLabel={evidenceReceiptLabel}
              evidenceRows={evidenceRows}
              evidenceWorkerLabel={evidenceWorkerLabel}
              focusEvidence={focusEvidence}
              gateLabel={gateLabel}
              handleCommentKeyDown={handleCommentKeyDown}
              issueActionButtonAttrs={issueActionButtonAttrs}
              issueActionByName={issueActionByName}
              issueActionContractChips={issueActionContractChips}
              issueAuditQuickActions={issueAuditQuickActions}
              issueCards={issueCards}
              issueCardsForStatus={issueCardsForStatus}
              issueDecisionActions={issueDecisionActions}
              issueDecisionButtonLabel={issueDecisionButtonLabel}
              issueDetailRows={issueDetailRows}
              issueHumanActions={issueHumanActions}
              issueOptionDisabled={issueOptionDisabled}
              issuePendingLabel={issuePendingLabel}
              issueRuntimeActionAriaLabel={issueRuntimeActionAriaLabel}
              issueRuntimeActionLabel={issueRuntimeActionLabel}
              issueTimelineCountsLabel={issueTimelineCountsLabel}
              issueTimelineDecisionLabel={issueTimelineDecisionLabel}
              issueTimelineItemMeta={issueTimelineItemMeta}
              issueTimelineItemTone={issueTimelineItemTone}
              issueTimelineReceiptLabel={issueTimelineReceiptLabel}
              issueTimelineReceiptMeta={issueTimelineReceiptMeta}
              issueTimelineReceiptTone={issueTimelineReceiptTone}
              issueTimelineRoundCountsLabel={issueTimelineRoundCountsLabel}
              issueTimelineRoundMeta={issueTimelineRoundMeta}
              issueTimelineRoundTone={issueTimelineRoundTone}
              issueTimelineStateLabel={issueTimelineStateLabel}
              issueTimelineTimeLabel={issueTimelineTimeLabel}
              issueTimelineTone={issueTimelineTone}
              loopControlBudgetLabel={loopControlBudgetLabel}
              loopControlCallLabel={loopControlCallLabel}
              loopControlGateLabel={loopControlGateLabel}
              loopControlOptionTone={loopControlOptionTone}
              loopControlScoreLabel={loopControlScoreLabel}
              loopControlStateLabel={loopControlStateLabel}
              loopControlTone={loopControlTone}
              loopDashboardAdmissionLabel={loopDashboardAdmissionLabel}
              loopDashboardAgentLabel={loopDashboardAgentLabel}
              loopDashboardAgentTone={loopDashboardAgentTone}
              loopDashboardEvidenceLabel={loopDashboardEvidenceLabel}
              loopDashboardGateLabel={loopDashboardGateLabel}
              loopDashboardHumanLabel={loopDashboardHumanLabel}
              loopDashboardPacketLabel={loopDashboardPacketLabel}
              loopDashboardRoundCounts={loopDashboardRoundCounts}
              loopDashboardRoundLabel={loopDashboardRoundLabel}
              loopDashboardRoundTone={loopDashboardRoundTone}
              loopDashboardStateLabel={loopDashboardStateLabel}
              loopDashboardTone={loopDashboardTone}
              loopDashboardVerdictLabel={loopDashboardVerdictLabel}
              loopGoal={loopGoal}
              loopRuntime={loopRuntime}
              loopTitle={loopTitle}
              loopWorkerAttempts={loopWorkerAttempts}
              loopWorkerTimeoutSecs={loopWorkerTimeoutSecs}
              openIssueComment={openIssueComment}
              pendingDemoAction={pendingDemoAction}
              receiptLabel={receiptLabel}
              revealIssueDetail={revealIssueDetail}
              reviewQueueBlockerLabel={reviewQueueBlockerLabel}
              reviewQueueCards={reviewQueueCards}
              reviewQueueDecisionLabel={reviewQueueDecisionLabel}
              reviewQueueEvidence={reviewQueueEvidence}
              roleWorkerLabel={roleWorkerLabel}
              roundHistoryLabel={roundHistoryLabel}
              roundRecoveryLabel={roundRecoveryLabel}
              runIssueAction={runIssueAction}
              runIssueLoop={runIssueLoop}
              runtimePreflightBoolLabel={runtimePreflightBoolLabel}
              runtimePreflightGateLabel={runtimePreflightGateLabel}
              runtimePreflightProbeLabel={runtimePreflightProbeLabel}
              runtimePreflightStateLabel={runtimePreflightStateLabel}
              runtimePreflightTone={runtimePreflightTone}
              schemaLabel={schemaLabel}
              scoreSummaryLabel={scoreSummaryLabel}
              selectedEvidenceDrilldown={selectedEvidenceDrilldown}
              selectedEvidenceId={selectedEvidenceId}
              selectedEvidenceManifest={selectedEvidenceManifest}
              selectedIssueActivityTimeline={selectedIssueActivityTimeline}
              selectedIssueCard={selectedIssueCard}
              selectedIssueDoctor={selectedIssueDoctor}
              selectedIssueEvidenceDrilldown={selectedIssueEvidenceDrilldown}
              selectedIssueEvidenceManifest={selectedIssueEvidenceManifest}
              selectedIssueLoopControl={selectedIssueLoopControl}
              selectedIssueLoopDashboard={selectedIssueLoopDashboard}
              selectedIssueRuntimePreflight={selectedIssueRuntimePreflight}
              selectedIssueTimeline={selectedIssueTimeline}
              selectedIssueTransitionPolicy={selectedIssueTransitionPolicy}
              selectedIssueWorkerLifecycle={selectedIssueWorkerLifecycle}
              selectedLoopControl={selectedLoopControl}
              selectedLoopDashboard={selectedLoopDashboard}
              selectedRuntimePreflight={selectedRuntimePreflight}
              selectedTransitionPolicy={selectedTransitionPolicy}
              selectedWorkerLifecycle={selectedWorkerLifecycle}
              setCommentBody={setCommentBody}
              setIssueDetailPanel={setIssueDetailPanel}
              setLoopGoal={setLoopGoal}
              setLoopRuntime={setLoopRuntime}
              setLoopTitle={setLoopTitle}
              setLoopWorkerAttempts={setLoopWorkerAttempts}
              setLoopWorkerTimeoutSecs={setLoopWorkerTimeoutSecs}
              setSelectedIssueId={setSelectedIssueId}
              shouldShowTranscriptExcerpt={shouldShowTranscriptExcerpt}
              stageWorkerLabel={stageWorkerLabel}
              startDemoLoop={startDemoLoop}
              traceCountLabel={traceCountLabel}
              traceRuntimeLabel={traceRuntimeLabel}
              traceRuntimeWarnLabel={traceRuntimeWarnLabel}
              transitionPolicyActionLabel={transitionPolicyActionLabel}
              transitionPolicyBudgetLabel={transitionPolicyBudgetLabel}
              transitionPolicyStateLabel={transitionPolicyStateLabel}
              transitionPolicyTone={transitionPolicyTone}
              workerAttemptLabel={workerAttemptLabel}
              workerCommandLabel={workerCommandLabel}
              workerDurationLabel={workerDurationLabel}
              workerLabel={workerLabel}
              workerLifecycleAttemptLabel={workerLifecycleAttemptLabel}
              workerLifecycleBudgetLabel={workerLifecycleBudgetLabel}
              workerLifecycleDurationLabel={workerLifecycleDurationLabel}
              workerLifecycleReceiptLabel={workerLifecycleReceiptLabel}
              workerLifecycleRoleTone={workerLifecycleRoleTone}
              workerLifecycleRoundLabel={workerLifecycleRoundLabel}
              workerLifecycleStateLabel={workerLifecycleStateLabel}
              workerLifecycleTone={workerLifecycleTone}
              workerLifecycleWorkerForRole={workerLifecycleWorkerForRole}
              workerLifecycleWorkerState={workerLifecycleWorkerState}
              workerReceiptLabel={workerReceiptLabel}
              workerStatusLabel={workerStatusLabel}
              workerTimeoutLabel={workerTimeoutLabel}
            />
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
