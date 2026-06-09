import { Show } from "solid-js";
import { IssueBoard } from "./hive/IssueBoard";
import { IssueDetailPanel } from "./hive/IssueDetailPanel";
import { LoopObservability } from "./hive/LoopObservability";
import { ReviewQueue } from "./hive/ReviewQueue";
import {
  COMMENT_CARD_PREVIEW_LIMIT,
  COMMENT_DETAIL_PREVIEW_LIMIT,
  ISSUE_STATUSES,
  commentPills,
  commentPreview,
  issueStatusTestId,
  operatorEventLabel,
  operatorEventStatusLabel,
} from "../lib/hive";

export type HiveWorkbenchPanelProps = Record<string, any>;

export default function HiveWorkbenchPanel(props: HiveWorkbenchPanelProps) {
  const {
    addIssueComment,
    advanceIssue,
    auditLabel,
    cardAuditFailureDetails,
    cardDoctor,
    closeIssueComment,
    commentBody,
    commentComposerActive,
    commentPillNode,
    commentSubmitDisabled,
    compactAuditFailureDetail,
    copyDoctorAction,
    createHiveLoop,
    doctorHealthLabel,
    doctorHealthTone,
    doctorReceiptLabel,
    doctorRuntimeLabel,
    doctorWorkerLabel,
    evidenceArtifactLabel,
    evidenceDrilldownStateLabel,
    evidenceDrilldownTone,
    evidenceDrilldownWorkerLabel,
    evidenceItemLabel,
    evidenceItemTone,
    evidenceManifestCoverageLabel,
    evidenceManifestEntryDigestLabel,
    evidenceManifestEntryPathLabel,
    evidenceManifestEntrySizeLabel,
    evidenceManifestEntryTone,
    evidenceManifestPathLabel,
    evidenceManifestStateLabel,
    evidenceManifestTone,
    evidencePayloadDiffLabel,
    evidenceReceiptLabel,
    evidenceRows,
    evidenceWorkerLabel,
    focusEvidence,
    gateLabel,
    handleCommentKeyDown,
    issueActionButtonAttrs,
    issueActionByName,
    issueActionContractChips,
    issueAuditQuickActions,
    issueCards,
    issueCardsForStatus,
    issueDecisionActions,
    issueDecisionButtonLabel,
    issueDetailRows,
    issueHumanActions,
    issueOptionDisabled,
    issuePendingLabel,
    issueRuntimeActionAriaLabel,
    issueRuntimeActionLabel,
    issueTimelineCountsLabel,
    issueTimelineDecisionLabel,
    issueTimelineItemMeta,
    issueTimelineItemTone,
    issueTimelineReceiptLabel,
    issueTimelineReceiptMeta,
    issueTimelineReceiptTone,
    issueTimelineRoundCountsLabel,
    issueTimelineRoundMeta,
    issueTimelineRoundTone,
    issueTimelineStateLabel,
    issueTimelineTimeLabel,
    issueTimelineTone,
    loopControlBudgetLabel,
    loopControlCallLabel,
    loopControlGateLabel,
    loopControlOptionTone,
    loopControlScoreLabel,
    loopControlStateLabel,
    loopControlTone,
    loopDashboardAdmissionLabel,
    loopDashboardAgentLabel,
    loopDashboardAgentTone,
    loopDashboardEvidenceLabel,
    loopDashboardGateLabel,
    loopDashboardHumanLabel,
    loopDashboardPacketLabel,
    loopDashboardRoundCounts,
    loopDashboardRoundLabel,
    loopDashboardRoundTone,
    loopDashboardStateLabel,
    loopDashboardTone,
    loopDashboardVerdictLabel,
    loopGoal,
    loopRuntime,
    loopTitle,
    loopWorkerAttempts,
    loopWorkerTimeoutSecs,
    openIssueComment,
    pendingDemoAction,
    receiptLabel,
    revealIssueDetail,
    reviewQueueBlockerLabel,
    reviewQueueCards,
    reviewQueueDecisionLabel,
    reviewQueueEvidence,
    roleWorkerLabel,
    roundHistoryLabel,
    roundRecoveryLabel,
    runIssueAction,
    runIssueLoop,
    runtimePreflightBoolLabel,
    runtimePreflightGateLabel,
    runtimePreflightProbeLabel,
    runtimePreflightStateLabel,
    runtimePreflightTone,
    schemaLabel,
    scoreSummaryLabel,
    selectedEvidenceDrilldown,
    selectedEvidenceId,
    selectedEvidenceManifest,
    selectedIssueActivityTimeline,
    selectedIssueCard,
    selectedIssueDoctor,
    selectedIssueEvidenceDrilldown,
    selectedIssueEvidenceManifest,
    selectedIssueLoopControl,
    selectedIssueLoopDashboard,
    selectedIssueRuntimePreflight,
    selectedIssueTimeline,
    selectedIssueTransitionPolicy,
    selectedIssueWorkerLifecycle,
    selectedLoopControl,
    selectedLoopDashboard,
    selectedRuntimePreflight,
    selectedTransitionPolicy,
    selectedWorkerLifecycle,
    setCommentBody,
    setIssueDetailPanel,
    setLoopGoal,
    setLoopRuntime,
    setLoopTitle,
    setLoopWorkerAttempts,
    setLoopWorkerTimeoutSecs,
    setSelectedIssueId,
    shouldShowTranscriptExcerpt,
    stageWorkerLabel,
    startDemoLoop,
    traceCountLabel,
    traceRuntimeLabel,
    traceRuntimeWarnLabel,
    transitionPolicyActionLabel,
    transitionPolicyBudgetLabel,
    transitionPolicyStateLabel,
    transitionPolicyTone,
    workerAttemptLabel,
    workerCommandLabel,
    workerDurationLabel,
    workerLabel,
    workerLifecycleAttemptLabel,
    workerLifecycleBudgetLabel,
    workerLifecycleDurationLabel,
    workerLifecycleReceiptLabel,
    workerLifecycleRoleTone,
    workerLifecycleRoundLabel,
    workerLifecycleStateLabel,
    workerLifecycleTone,
    workerLifecycleWorkerForRole,
    workerLifecycleWorkerState,
    workerReceiptLabel,
    workerStatusLabel,
    workerTimeoutLabel,
  } = props;

  return (
                <section class="panel-grid panel-grid--board">
                  <div class="panel-stack">
                    <article class="panel panel--form">
                      <p class="panel-kicker">Loop</p>
                      <h3>Contract</h3>
                      <input
                        aria-label="Loop title"
                        value={loopTitle()}
                        onInput={(event: any) => setLoopTitle(event.currentTarget.value)}
                        placeholder="Title"
                      />
                      <textarea
                        aria-label="Loop goal"
                        value={loopGoal()}
                        onInput={(event: any) => setLoopGoal(event.currentTarget.value)}
                        placeholder="Goal"
                      />
                      <select
                        aria-label="Loop runtime"
                        value={loopRuntime()}
                        onChange={(event: any) => setLoopRuntime(event.currentTarget.value)}
                      >
                        <option value="codex">codex</option>
                        <option value="local">local</option>
                      </select>
                      <input
                        aria-label="Worker timeout seconds"
                        type="number"
                        min="1"
                        value={loopWorkerTimeoutSecs()}
                        onInput={(event: any) => setLoopWorkerTimeoutSecs(event.currentTarget.value)}
                        placeholder="Worker timeout seconds"
                      />
                      <input
                        aria-label="Worker attempts"
                        type="number"
                        min="1"
                        max="3"
                        value={loopWorkerAttempts()}
                        onInput={(event: any) => setLoopWorkerAttempts(event.currentTarget.value)}
                        placeholder="Worker attempts"
                      />
                      <div class="form-actions">
                        <button
                          type="button"
                          data-testid="panel-run-demo"
                          disabled={Boolean(pendingDemoAction())}
                          onClick={() => void startDemoLoop()}
                        >
                          {pendingDemoAction() ?? "Run Demo"}
                        </button>
                        <button type="button" onClick={() => void createHiveLoop()}>
                          Create Loop
                        </button>
                      </div>
                    </article>
    
                    <article
                      class="panel panel--detail"
                      ref={(element: any) => {
                        setIssueDetailPanel(element);
                      }}
                    >
                      <p class="panel-kicker">Issue</p>
                      <Show
                        when={selectedIssueCard()}
                        keyed
                        fallback={
                          <div class="empty-state">
                            <span>No issues</span>
                            <button
                              type="button"
                              data-testid="issue-detail-run-demo"
                              disabled={Boolean(pendingDemoAction())}
                              onClick={() => void startDemoLoop()}
                            >
                              {pendingDemoAction() ?? "Run Demo"}
                            </button>
                          </div>
                        }
                      >
                        {(card: any) => (
                          <>
                            <IssueDetailPanel>
                              <h3>{card.issue.title}</h3>
                              <p class="muted">{card.issue.summary ?? "No summary"}</p>
                              <div class="trace-strip">
                                <span class="trace-pill">{card.issue.assignee ?? "unassigned"}</span>
                                <span class="trace-pill">{card.issue.claim_role ?? "no role"}</span>
                                <span class="trace-pill">{card.issue.claim_source ?? "no source"}</span>
                                {card.issue.claimed_at ? (
                                  <span class="trace-pill">{card.issue.claimed_at}</span>
                                ) : null}
                              </div>
                            </IssueDetailPanel>
                            <LoopObservability>
                            <Show
                              when={selectedIssueTransitionPolicy()}
                              keyed
                              fallback={
                                <div
                                  class="worker-lifecycle transition-policy worker-lifecycle--pending"
                                  data-testid={`issue-transition-policy-detail-${card.issue.id}`}
                                >
                                  <div class="stage-row-head">
                                    <strong>Transition Policy</strong>
                                    <span>{selectedTransitionPolicy.loading ? "loading" : "pending"}</span>
                                  </div>
                                  <div class="trace-strip">
                                    <span class="trace-pill">issue #{card.issue.id}</span>
                                    <span class="trace-pill">issue_transition_policy.v1</span>
                                  </div>
                                </div>
                              }
                            >
                              {(policy: any) => (
                                <div
                                  class={`worker-lifecycle transition-policy worker-lifecycle--${transitionPolicyTone(
                                    policy,
                                  )}`}
                                  data-testid={`issue-transition-policy-detail-${card.issue.id}`}
                                >
                                  <div class="stage-row-head">
                                    <strong>Transition Policy</strong>
                                    <span>{transitionPolicyStateLabel(policy.state_class)}</span>
                                  </div>
                                  <p>{policy.summary}</p>
                                  <div class="trace-strip">
                                    <span class="trace-pill">{schemaLabel(policy.schema_version)}</span>
                                    <span class="trace-pill">{schemaLabel(policy.registry.schema_version)}</span>
                                    <span class="trace-pill">{policy.policy_owner}</span>
                                    <span class="trace-pill">{policy.policy_scope}</span>
                                    <span
                                      class={
                                        policy.human_decision_required
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      {policy.human_decision_required ? "human required" : "human clear"}
                                    </span>
                                    <span class="trace-pill">
                                      allowed {policy.allowed_actions.length}
                                    </span>
                                    <span
                                      class={
                                        policy.blocked_actions.length
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      blocked {policy.blocked_actions.length}
                                    </span>
                                    <span
                                      class={
                                        policy.reviewer_budget?.reviewer_invalid_budget_exhausted
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      {transitionPolicyBudgetLabel(policy)}
                                    </span>
                                  </div>
                                  <div class="worker-lifecycle-roles transition-policy-actions">
                                    {policy.allowed_actions.slice(0, 4).map((choice: any) => (
                                      <div
                                        class={`worker-lifecycle-role worker-lifecycle-role--${
                                          choice.requires_human ? "warn" : "pending"
                                        }`}
                                        data-testid={`issue-transition-policy-action-${card.issue.id}-${choice.action.action}`}
                                      >
                                        <div class="stage-row-head">
                                          <strong>{transitionPolicyActionLabel(choice)}</strong>
                                          <span>{choice.gate}</span>
                                        </div>
                                        <p>{choice.rationale}</p>
                                        <div class="trace-strip">
                                          <span class="trace-pill">{choice.from_status}</span>
                                          <span class="trace-pill">{choice.action.source}</span>
                                          {choice.action.confirmation_arg ? (
                                            <span class="trace-pill">{choice.action.confirmation_arg}</span>
                                          ) : null}
                                        </div>
                                      </div>
                                    ))}
                                  </div>
                                  {policy.blocked_actions.length ? (
                                    <div class="worker-lifecycle-roles transition-policy-blocked">
                                      {policy.blocked_actions.slice(0, 3).map((blocked: any) => (
                                        <div
                                          class="worker-lifecycle-role worker-lifecycle-role--warn"
                                          data-testid={`issue-transition-policy-blocked-${card.issue.id}-${blocked.action}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{blocked.action}</strong>
                                            <span>{blocked.required_statuses.join("/") || "none"}</span>
                                          </div>
                                          <p>{blocked.reason}</p>
                                          {blocked.hint ? <code class="evidence-excerpt">{blocked.hint}</code> : null}
                                        </div>
                                      ))}
                                    </div>
                                  ) : null}
                                </div>
                              )}
                            </Show>
                            {card.issue.loop_id ? (
                              <Show
                                when={selectedIssueLoopControl()}
                                keyed
                                fallback={
                                  <div
                                    class="worker-lifecycle reviewer-control worker-lifecycle--pending"
                                    data-testid={`loop-control-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Reviewer Control</strong>
                                      <span>{selectedLoopControl.loading ? "loading" : "pending"}</span>
                                    </div>
                                    <div class="trace-strip">
                                      <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                      <span class="trace-pill">loop_control.v1</span>
                                    </div>
                                  </div>
                                }
                              >
                                {(control: any) => (
                                  <div
                                    class={`worker-lifecycle reviewer-control worker-lifecycle--${loopControlTone(
                                      control,
                                    )}`}
                                    data-testid={`loop-control-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Reviewer Control</strong>
                                      <span>{loopControlStateLabel(control)}</span>
                                    </div>
                                    <p>
                                      loop #{control.loop_id} round{" "}
                                      {control.state.current_round ?? "pending"} / reviewer{" "}
                                      {control.state.reviewer_decision ?? "pending"}
                                      {control.state.reviewer_reason_code
                                        ? ` / ${control.state.reviewer_reason_code}`
                                        : ""}
                                    </p>
                                    <div class="trace-strip">
                                      <span class="trace-pill">{schemaLabel(control.schema_version)}</span>
                                      <span class="trace-pill">
                                        issue {control.state.issue_status ?? "none"}
                                      </span>
                                      <span class="trace-pill">
                                        loop {control.state.loop_status ?? "pending"}
                                      </span>
                                      <span
                                        class={
                                          control.state.reviewer_invalid_budget_exhausted
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {loopControlBudgetLabel(control)}
                                      </span>
                                      <span
                                        class={
                                          control.reviewer_gate_surface.gates.runtime_preflight?.passed ===
                                          false
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {loopControlGateLabel(
                                          control.reviewer_gate_surface.gates.runtime_preflight,
                                        )}
                                      </span>
                                      <span class="trace-pill">
                                        lifecycle {control.state.lifecycle_state ?? "pending"}
                                      </span>
                                      <span class="trace-pill">
                                        evidence {control.state.evidence_manifest_state ?? "pending"}
                                      </span>
                                      <span class="trace-pill">
                                        drift {control.reviewer_gate_surface.target_drift_check.state}
                                      </span>
                                    </div>
                                    <div class="worker-lifecycle-roles reviewer-control-gates">
                                      <div class="worker-lifecycle-role worker-lifecycle-role--pending">
                                        <div class="stage-row-head">
                                          <strong>Runtime Gate</strong>
                                          <span>
                                            {control.reviewer_gate_surface.gates.runtime_preflight?.state ??
                                              "pending"}
                                          </span>
                                        </div>
                                        <p>
                                          {control.reviewer_gate_surface.gates.runtime_preflight?.gate ??
                                            "runtime_policy_ready"}
                                        </p>
                                        <div class="trace-strip">
                                          <span class="trace-pill">
                                            {control.state.runtime_preflight_state ?? "pending"}
                                          </span>
                                        </div>
                                      </div>
                                      <div
                                        class={`worker-lifecycle-role worker-lifecycle-role--${
                                          control.reviewer_gate_surface.gates.worker_lifecycle
                                            ?.missing_roles?.length
                                            ? "warn"
                                            : "pending"
                                        }`}
                                      >
                                        <div class="stage-row-head">
                                          <strong>Role Coverage</strong>
                                          <span>
                                            {control.reviewer_gate_surface.gates.worker_lifecycle
                                              ?.observed_roles?.length ?? 0}
                                            /
                                            {control.reviewer_gate_surface.gates.worker_lifecycle
                                              ?.expected_roles?.length ?? 0}
                                          </span>
                                        </div>
                                        <p>
                                          {(control.reviewer_gate_surface.gates.worker_lifecycle
                                            ?.observed_roles ?? ["pending"]
                                          ).join(", ")}
                                        </p>
                                        <div class="trace-strip">
                                          {(control.reviewer_gate_surface.gates.worker_lifecycle
                                            ?.missing_roles ?? []
                                          ).map((role: any) => (
                                            <span class="trace-pill trace-pill--warn">missing {role}</span>
                                          ))}
                                          {(control.reviewer_gate_surface.gates.worker_lifecycle
                                            ?.failures ?? []
                                          )
                                            .slice(0, 2)
                                            .map((failure: any) => (
                                              <span class="trace-pill trace-pill--warn">{failure}</span>
                                            ))}
                                        </div>
                                      </div>
                                      <div class="worker-lifecycle-role worker-lifecycle-role--pending">
                                        <div class="stage-row-head">
                                          <strong>Evidence</strong>
                                          <span>
                                            {control.reviewer_gate_surface.gates.evidence_manifest?.state ??
                                              "pending"}
                                          </span>
                                        </div>
                                        <p>
                                          receipts{" "}
                                          {control.reviewer_gate_surface.gates.evidence_manifest?.coverage
                                            ?.receipt_count ?? 0}
                                          {" / "}digests{" "}
                                          {control.reviewer_gate_surface.gates.evidence_manifest?.coverage
                                            ?.digest_count ?? 0}
                                        </p>
                                        <div class="trace-strip">
                                          <span class="trace-pill">
                                            evidence{" "}
                                            {control.reviewer_gate_surface.gates.evidence_manifest
                                              ?.coverage?.evidence_count ?? 0}
                                          </span>
                                          <span class="trace-pill">
                                            entries{" "}
                                            {control.reviewer_gate_surface.gates.evidence_manifest
                                              ?.coverage?.entry_count ?? 0}
                                          </span>
                                        </div>
                                      </div>
                                    </div>
                                    {control.reviewer_gate_surface.score_vector.length ? (
                                      <div class="trace-strip">
                                        {control.reviewer_gate_surface.score_vector.map((metric: any) => (
                                          <span class="trace-pill">{loopControlScoreLabel(metric)}</span>
                                        ))}
                                      </div>
                                    ) : null}
                                    <div class="worker-lifecycle-roles reviewer-control-options">
                                      {control.operator_decision_surface.options.slice(0, 3).map((option: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${loopControlOptionTone(
                                            option,
                                          )}`}
                                          data-testid={`loop-control-option-${card.issue.id}-${option.key}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>
                                              {option.key}. {option.label}
                                            </strong>
                                            <span>{option.enabled ? "available" : "blocked"}</span>
                                          </div>
                                          <p>{option.summary}</p>
                                          <div class="trace-strip">
                                            <span class="trace-pill">{loopControlCallLabel(option)}</span>
                                            {option.call?.arguments?.human_confirmed === true ? (
                                              <span class="trace-pill trace-pill--warn">
                                                human_confirmed
                                              </span>
                                            ) : null}
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                    {control.human_decision_boundary.options.length ? (
                                      <div class="doctor-lines">
                                        {control.human_decision_boundary.options
                                          .slice(0, 3)
                                          .map((option: any) => (
                                            <span>{option}</span>
                                          ))}
                                      </div>
                                    ) : null}
                                  </div>
                                )}
                              </Show>
                            ) : null}
                            {card.issue.loop_id ? (
                              <Show
                                when={selectedIssueLoopDashboard()}
                                keyed
                                fallback={
                                  <div
                                    class="doctor-summary loop-dashboard doctor-summary--pending"
                                    data-testid={`loop-dashboard-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Loop Dashboard</strong>
                                      <span>{selectedLoopDashboard.loading ? "loading" : "pending"}</span>
                                    </div>
                                    <div class="trace-strip">
                                      <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                      <span class="trace-pill">loop_dashboard.v1</span>
                                    </div>
                                  </div>
                                }
                              >
                                {(dashboard: any) => (
                                  <div
                                    class={`doctor-summary loop-dashboard doctor-summary--${loopDashboardTone(
                                      dashboard.dashboard_state,
                                    )}`}
                                    data-testid={`loop-dashboard-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Loop Dashboard</strong>
                                      <span>{loopDashboardStateLabel(dashboard.dashboard_state)}</span>
                                    </div>
                                    <p>{dashboard.summary}</p>
                                    <div class="trace-strip">
                                      <span class="trace-pill">{schemaLabel(dashboard.schema_version)}</span>
                                      <span class="trace-pill">loop #{dashboard.loop_id}</span>
                                      <span class="trace-pill">round {dashboard.current_round}</span>
                                      <span class="trace-pill">{dashboard.runtime}</span>
                                      <span class="trace-pill">
                                        {dashboard.kernel.route_from}
                                        {" -> "}
                                        {dashboard.kernel.route_to}
                                      </span>
                                      <span
                                        class={
                                          dashboard.kernel.gate_passed === false
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {loopDashboardGateLabel(dashboard)}
                                      </span>
                                      <span class="trace-pill">
                                        workers{" "}
                                        {dashboard.agents.filter((agent: any) => agent.state === "ok").length}/
                                        {dashboard.agents.length}
                                      </span>
                                      <span
                                        class={
                                          dashboard.reviewer.reviewer_invalid_budget_exhausted
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        review budget {dashboard.reviewer.reviewer_invalid_rounds_used}/
                                        {dashboard.reviewer.reviewer_invalid_round_budget}
                                      </span>
                                      <span class="trace-pill">
                                        decision {dashboard.reviewer.decision ?? "pending"}
                                      </span>
                                      <span
                                        class={
                                          dashboard.human_decision.required
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {loopDashboardHumanLabel(dashboard)}
                                      </span>
                                      {dashboard.kernel.blocker ? (
                                        <span class="trace-pill trace-pill--warn">
                                          {dashboard.kernel.blocker}
                                        </span>
                                      ) : null}
                                    </div>
                                    <div class="worker-lifecycle-roles">
                                      {dashboard.agents.map((agent: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${loopDashboardAgentTone(
                                            agent,
                                          )}`}
                                          data-testid={`loop-dashboard-agent-${card.issue.id}-${agent.role}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{agent.role}</strong>
                                            <span>{loopDashboardAgentLabel(agent)}</span>
                                          </div>
                                          <p>{agent.summary ?? "No worker receipt"}</p>
                                          <div class="trace-strip">
                                            <span
                                              class={
                                                agent.state === "ok"
                                                  ? "trace-pill"
                                                  : "trace-pill trace-pill--warn"
                                              }
                                            >
                                              {agent.worker_kind ?? "missing"}
                                            </span>
                                            {agent.worker_mode ? (
                                              <span class="trace-pill">{agent.worker_mode}</span>
                                            ) : null}
                                            {agent.receipt_ok === false ? (
                                              <span class="trace-pill trace-pill--warn">receipt fail</span>
                                            ) : null}
                                            {agent.timed_out ? (
                                              <span class="trace-pill trace-pill--warn">timeout</span>
                                            ) : null}
                                            {agent.retry_exhausted ? (
                                              <span class="trace-pill trace-pill--warn">retry exhausted</span>
                                            ) : null}
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                    <div
                                      class="loop-dashboard-rounds"
                                      data-testid={`loop-dashboard-rounds-${card.issue.id}`}
                                    >
                                      {dashboard.rounds.map((round: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${loopDashboardRoundTone(
                                            round,
                                          )}`}
                                          data-testid={`loop-dashboard-round-${card.issue.id}-${round.round}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{loopDashboardRoundLabel(round)}</strong>
                                            <span>{loopDashboardRoundCounts(round)}</span>
                                          </div>
                                          <div class="trace-strip">
                                            <span class="trace-pill">
                                              workers {round.worker_ok_count}/{round.worker_count}
                                            </span>
                                            {round.retry_lineage ? (
                                              <span class="trace-pill trace-pill--warn">
                                                {round.retry_lineage}
                                              </span>
                                            ) : null}
                                            {round.blocker ? (
                                              <span class="trace-pill trace-pill--warn">
                                                {round.blocker}
                                              </span>
                                            ) : null}
                                            {round.receipt_missing_count ? (
                                              <span class="trace-pill trace-pill--warn">
                                                missing {round.receipt_missing_count}
                                              </span>
                                            ) : null}
                                            {round.rejected_count ? (
                                              <span class="trace-pill trace-pill--warn">
                                                rejected {round.rejected_count}
                                              </span>
                                            ) : null}
                                          </div>
                                          <div class="trace-strip">
                                            {round.groups.packets.slice(0, 4).map((packet: any) => (
                                              <span class="trace-pill">{loopDashboardPacketLabel(packet)}</span>
                                            ))}
                                            {round.groups.admissions.slice(0, 4).map((admission: any) => (
                                              <span
                                                class={
                                                  admission.result === "rejected"
                                                    ? "trace-pill trace-pill--warn"
                                                    : "trace-pill"
                                                }
                                              >
                                                {loopDashboardAdmissionLabel(admission)}
                                              </span>
                                            ))}
                                            {round.groups.evidence.slice(0, 4).map((evidence: any) => (
                                              <span
                                                class={
                                                  evidence.worker_ok === false ||
                                                  evidence.admission_result === "rejected"
                                                    ? "trace-pill trace-pill--warn"
                                                    : "trace-pill"
                                                }
                                              >
                                                {loopDashboardEvidenceLabel(evidence)}
                                              </span>
                                            ))}
                                            {round.groups.verdicts.slice(0, 2).map((verdict: any) => (
                                              <span
                                                class={
                                                  verdict.decision === "keep"
                                                    ? "trace-pill"
                                                    : "trace-pill trace-pill--warn"
                                                }
                                              >
                                                {loopDashboardVerdictLabel(verdict)}
                                              </span>
                                            ))}
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                    {dashboard.health.failed_checks.length ||
                                    dashboard.health.missing_receipts.length ||
                                    dashboard.health.worker_failures.length ||
                                    dashboard.kernel.failures.length ? (
                                      <div class="doctor-lines">
                                        {dashboard.health.failed_checks.map((check: any) => (
                                          <span>check {check}</span>
                                        ))}
                                        {dashboard.health.missing_receipts.map((receipt: any) => (
                                          <span>missing {receipt}</span>
                                        ))}
                                        {dashboard.health.worker_failures.map((failure: any) => (
                                          <span>{failure}</span>
                                        ))}
                                        {dashboard.kernel.failures.map((failure: any) => (
                                          <span>{failure}</span>
                                        ))}
                                      </div>
                                    ) : null}
                                    {dashboard.next_actions.length ? (
                                      <div class="doctor-actions">
                                        {dashboard.next_actions.slice(0, 2).map((action: any, index: any) => (
                                          <div class="doctor-action-row">
                                            <code>{action}</code>
                                            <button
                                              type="button"
                                              aria-label={`Copy loop dashboard action ${action}`}
                                              data-testid={`loop-dashboard-action-copy-${card.issue.id}-${index}`}
                                              onClick={() => void copyDoctorAction(action)}
                                            >
                                              Copy
                                            </button>
                                          </div>
                                        ))}
                                      </div>
                                    ) : null}
                                  </div>
                                )}
                              </Show>
                            ) : null}
                            {card.issue.loop_id ? (
                              <Show
                                when={selectedIssueEvidenceDrilldown()}
                                keyed
                                fallback={
                                  <div
                                    class="worker-lifecycle evidence-drilldown worker-lifecycle--pending"
                                    data-testid={`evidence-drilldown-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Evidence Drilldown</strong>
                                      <span>
                                        {selectedEvidenceDrilldown.loading ? "loading" : "pending"}
                                      </span>
                                    </div>
                                    <div class="trace-strip">
                                      <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                      <span class="trace-pill">evidence_drilldown.v1</span>
                                    </div>
                                  </div>
                                }
                              >
                                {(drilldown: any) => (
                                  <div
                                    class={`worker-lifecycle evidence-drilldown worker-lifecycle--${evidenceDrilldownTone(
                                      drilldown.drilldown_state,
                                    )}`}
                                    data-testid={`evidence-drilldown-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Evidence Drilldown</strong>
                                      <span>{evidenceDrilldownStateLabel(drilldown.drilldown_state)}</span>
                                    </div>
                                    <p>{drilldown.summary}</p>
                                    <div class="trace-strip">
                                      <span class="trace-pill">{schemaLabel(drilldown.schema_version)}</span>
                                      <span class="trace-pill">loop #{drilldown.loop_id}</span>
                                      <span class="trace-pill">round {drilldown.current_round}</span>
                                      <span class="trace-pill">{drilldown.runtime}</span>
                                      <span class="trace-pill">evidence {drilldown.evidence_count}</span>
                                      <span
                                        class={
                                          drilldown.blockers.length
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        blockers {drilldown.blockers.length}
                                      </span>
                                      <span
                                        class={
                                          drilldown.human_decision.required
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        human {drilldown.human_decision.options.length}
                                      </span>
                                    </div>
                                    {drilldown.blockers.length ? (
                                      <div class="doctor-lines evidence-blocker-surfaces">
                                        {drilldown.blockers.slice(0, 3).map((blocker: any) => (
                                          <div
                                            class="evidence-blocker-surface"
                                            data-testid={`evidence-blocker-decision-${card.issue.id}-${blocker.evidence_id ?? "loop"}`}
                                          >
                                            <span>
                                              blocker {blocker.evidence_id ? `#${blocker.evidence_id}` : blocker.scope} {blocker.reason}
                                            </span>
                                            <div class="trace-strip">
                                              <span
                                                class={
                                                  blocker.decision_surface.required
                                                    ? "trace-pill trace-pill--warn"
                                                    : "trace-pill"
                                                }
                                              >
                                                decision {blocker.decision_surface.primary_action ?? "comment"}
                                              </span>
                                              <span class="trace-pill">
                                                actions {blocker.decision_surface.actions.length}
                                              </span>
                                              {blocker.decision_surface.issue_status ? (
                                                <span class="trace-pill">
                                                  {blocker.decision_surface.issue_status}
                                                </span>
                                              ) : null}
                                            </div>
                                            {blocker.decision_surface.actions.length ? (
                                              <div class="record-actions evidence-blocker-actions">
                                                {blocker.decision_surface.actions.slice(0, 4).map((choice: any) => (
                                                  <button
                                                    type="button"
                                                    aria-label={`${choice.issue_action.label} blocker ${blocker.evidence_id ?? "loop"}`}
                                                    data-testid={`evidence-blocker-action-${card.issue.id}-${blocker.evidence_id ?? "loop"}-${choice.issue_action.action}`}
                                                    disabled={issueOptionDisabled(card, choice.issue_action)}
                                                    title={choice.reason}
                                                    onClick={() => runIssueAction(card, choice.issue_action)}
                                                    {...issueActionButtonAttrs(choice.issue_action)}
                                                  >
                                                    {issueDecisionButtonLabel(card, choice.issue_action)}
                                                  </button>
                                                ))}
                                              </div>
                                            ) : null}
                                          </div>
                                        ))}
                                      </div>
                                    ) : null}
                                    <div class="worker-lifecycle-roles evidence-drilldown-items">
                                      {drilldown.items.slice(0, 6).map((item: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${evidenceItemTone(
                                            item,
                                          )}`}
                                          data-testid={`evidence-drilldown-item-${card.issue.id}-${item.id}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{evidenceItemLabel(item)}</strong>
                                            <span>{item.admission_result ?? "recorded"}</span>
                                          </div>
                                          <p>{item.summary}</p>
                                          <div class="trace-strip">
                                            <span
                                              class={
                                                item.worker?.ok === false
                                                  ? "trace-pill trace-pill--warn"
                                                  : "trace-pill"
                                              }
                                            >
                                              {evidenceDrilldownWorkerLabel(item)}
                                            </span>
                                            <span
                                              class={
                                                item.receipt?.ok === false
                                                  ? "trace-pill trace-pill--warn"
                                                  : "trace-pill"
                                              }
                                            >
                                              {evidenceReceiptLabel(item)}
                                            </span>
                                            <span class="trace-pill">{evidenceArtifactLabel(item)}</span>
                                            <span class="trace-pill">{evidencePayloadDiffLabel(item)}</span>
                                            {item.blocker ? (
                                              <span class="trace-pill trace-pill--warn">{item.blocker}</span>
                                            ) : null}
                                          </div>
                                          <div class="trace-strip">
                                            {item.payload.top_level_keys.slice(0, 8).map((key: any) => (
                                              <span class="trace-pill">{key}</span>
                                            ))}
                                          </div>
                                          {item.worker?.transcript_excerpt ? (
                                            <code class="evidence-excerpt">
                                              {item.worker.transcript_excerpt}
                                            </code>
                                          ) : item.payload.excerpt ? (
                                            <code class="evidence-excerpt">{item.payload.excerpt}</code>
                                          ) : null}
                                        </div>
                                      ))}
                                    </div>
                                    {drilldown.next_actions.length ? (
                                      <div class="doctor-actions">
                                        {drilldown.next_actions.slice(0, 2).map((action: any, index: any) => (
                                          <div class="doctor-action-row">
                                            <code>{action}</code>
                                            <button
                                              type="button"
                                              aria-label={`Copy evidence drilldown action ${action}`}
                                              data-testid={`evidence-drilldown-action-copy-${card.issue.id}-${index}`}
                                              onClick={() => void copyDoctorAction(action)}
                                            >
                                              Copy
                                            </button>
                                          </div>
                                        ))}
                                      </div>
                                    ) : null}
                                  </div>
                                )}
                              </Show>
                            ) : null}
                            {card.issue.loop_id ? (
                              <Show
                                when={selectedIssueEvidenceManifest()}
                                keyed
                                fallback={
                                  <div
                                    class="worker-lifecycle evidence-manifest worker-lifecycle--pending"
                                    data-testid={`evidence-manifest-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Evidence Manifest</strong>
                                      <span>
                                        {selectedEvidenceManifest.loading ? "loading" : "pending"}
                                      </span>
                                    </div>
                                    <div class="trace-strip">
                                      <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                      <span class="trace-pill">evidence_manifest.v1</span>
                                    </div>
                                  </div>
                                }
                              >
                                {(manifest: any) => (
                                  <div
                                    class={`worker-lifecycle evidence-manifest worker-lifecycle--${evidenceManifestTone(
                                      manifest.manifest_state,
                                    )}`}
                                    data-testid={`evidence-manifest-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Evidence Manifest</strong>
                                      <span>{evidenceManifestStateLabel(manifest.manifest_state)}</span>
                                    </div>
                                    <p>{manifest.summary}</p>
                                    <div class="trace-strip">
                                      <span class="trace-pill">{schemaLabel(manifest.schema_version)}</span>
                                      <span class="trace-pill">loop #{manifest.loop_id}</span>
                                      <span class="trace-pill">round {manifest.current_round}</span>
                                      <span class="trace-pill">{manifest.runtime}</span>
                                      <span class="trace-pill">
                                        {evidenceManifestCoverageLabel(manifest.coverage)}
                                      </span>
                                      <span
                                        class={
                                          manifest.coverage.path_missing_count ||
                                          manifest.coverage.path_unverified_count
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {evidenceManifestPathLabel(manifest.coverage)}
                                      </span>
                                      <span class="trace-pill">digests {manifest.coverage.digest_count}</span>
                                    </div>
                                    <div class="worker-lifecycle-roles evidence-manifest-entries">
                                      {manifest.entries.slice(0, 6).map((entry: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${evidenceManifestEntryTone(
                                            entry,
                                          )}`}
                                          data-testid={`evidence-manifest-entry-${card.issue.id}-${entry.id}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{entry.label}</strong>
                                            <span>{entry.verified ? "verified" : "unverified"}</span>
                                          </div>
                                          <p>{entry.summary}</p>
                                          <div class="trace-strip">
                                            <span class="trace-pill">{entry.source}</span>
                                            <span class="trace-pill">{entry.entry_kind}</span>
                                            <span class="trace-pill">{evidenceManifestEntryDigestLabel(entry)}</span>
                                            <span
                                              class={
                                                entry.path_status === "missing"
                                                  ? "trace-pill trace-pill--warn"
                                                  : "trace-pill"
                                              }
                                            >
                                              {evidenceManifestEntryPathLabel(entry)}
                                            </span>
                                            {entry.schema_version ? (
                                              <span class="trace-pill">{schemaLabel(entry.schema_version)}</span>
                                            ) : null}
                                            {evidenceManifestEntrySizeLabel(entry) ? (
                                              <span class="trace-pill">{evidenceManifestEntrySizeLabel(entry)}</span>
                                            ) : null}
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                    {manifest.next_actions.length ? (
                                      <div class="doctor-actions">
                                        {manifest.next_actions.slice(0, 2).map((action: any, index: any) => (
                                          <div class="doctor-action-row">
                                            <code>{action}</code>
                                            <button
                                              type="button"
                                              aria-label={`Copy evidence manifest action ${action}`}
                                              data-testid={`evidence-manifest-action-copy-${card.issue.id}-${index}`}
                                              onClick={() => void copyDoctorAction(action)}
                                            >
                                              Copy
                                            </button>
                                          </div>
                                        ))}
                                      </div>
                                    ) : null}
                                  </div>
                                )}
                              </Show>
                            ) : null}
                            <Show
                              when={selectedIssueActivityTimeline()}
                              keyed
                              fallback={
                                <div
                                  class="worker-lifecycle issue-timeline worker-lifecycle--pending"
                                  data-testid={`issue-timeline-detail-${card.issue.id}`}
                                >
                                  <div class="stage-row-head">
                                    <strong>Activity Timeline</strong>
                                    <span>{selectedIssueTimeline.loading ? "loading" : "pending"}</span>
                                  </div>
                                  <div class="trace-strip">
                                    <span class="trace-pill">issue #{card.issue.id}</span>
                                    <span class="trace-pill">issue_timeline.v1</span>
                                  </div>
                                </div>
                              }
                            >
                              {(timeline: any) => (
                                <div
                                  class={`worker-lifecycle issue-timeline worker-lifecycle--${issueTimelineTone(
                                    timeline.timeline_state,
                                  )}`}
                                  data-testid={`issue-timeline-detail-${card.issue.id}`}
                                >
                                  <div class="stage-row-head">
                                    <strong>Activity Timeline</strong>
                                    <span>{issueTimelineStateLabel(timeline.timeline_state)}</span>
                                  </div>
                                  <p>{timeline.summary}</p>
                                  <div class="trace-strip">
                                    <span class="trace-pill">{schemaLabel(timeline.schema_version)}</span>
                                    <span class="trace-pill">issue #{timeline.issue.id}</span>
                                    {timeline.loop_id ? (
                                      <span class="trace-pill">loop #{timeline.loop_id}</span>
                                    ) : null}
                                    <span class="trace-pill">{issueTimelineCountsLabel(timeline)}</span>
                                    <span class="trace-pill">rounds {timeline.rounds.length}</span>
                                    <span
                                      class={
                                        timeline.counts.blocker_count || timeline.counts.receipt_issue_count
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      blockers {timeline.counts.blocker_count}
                                    </span>
                                    <span class="trace-pill">operator {timeline.counts.operator_event_count}</span>
                                    <span
                                      class={
                                        timeline.human_decision.required
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      {issueTimelineDecisionLabel(timeline.human_decision)}
                                    </span>
                                    <span
                                      class={
                                        timeline.human_decision.required &&
                                        !timeline.counts.decision_receipt_count
                                          ? "trace-pill trace-pill--warn"
                                          : "trace-pill"
                                      }
                                    >
                                      receipts {timeline.decision_receipts.length}
                                    </span>
                                  </div>
                                  {timeline.rounds.length ? (
                                    <div class="worker-lifecycle-roles issue-timeline-rounds">
                                      {timeline.rounds.slice(-4).map((round: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${issueTimelineRoundTone(
                                            round,
                                          )}`}
                                          data-testid={`issue-timeline-round-${card.issue.id}-${round.round ?? "issue"}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{round.label}</strong>
                                            <span>{round.state}</span>
                                          </div>
                                          <p>{issueTimelineRoundCountsLabel(round)}</p>
                                          <div class="trace-strip">
                                            <span class="trace-pill">items {round.item_count}</span>
                                            <span
                                              class={
                                                round.blocker_count
                                                  ? "trace-pill trace-pill--warn"
                                                  : "trace-pill"
                                              }
                                            >
                                              blockers {round.blocker_count}
                                            </span>
                                            <span class="trace-pill">operator {round.operator_event_count}</span>
                                            <span class="trace-pill">{issueTimelineRoundMeta(round)}</span>
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                  ) : null}
                                  {timeline.human_decision.required ? (
                                    <div
                                      class="doctor-lines evidence-blocker-surfaces issue-timeline-decision"
                                      data-testid={`issue-timeline-decision-${card.issue.id}`}
                                    >
                                      <div class="evidence-blocker-surface">
                                        <span>{timeline.human_decision.summary}</span>
                                        <div class="trace-strip">
                                          <span class="trace-pill trace-pill--warn">
                                            {issueTimelineDecisionLabel(timeline.human_decision)}
                                          </span>
                                          <span class="trace-pill">
                                            actions {timeline.human_decision.actions.length}
                                          </span>
                                          {timeline.human_decision.issue_status ? (
                                            <span class="trace-pill">
                                              {timeline.human_decision.issue_status}
                                            </span>
                                          ) : null}
                                        </div>
                                        <div class="record-actions evidence-blocker-actions">
                                          {timeline.human_decision.actions.slice(0, 4).map((choice: any) => (
                                            <button
                                              type="button"
                                              aria-label={`${choice.issue_action.label} issue timeline decision`}
                                              data-testid={`issue-timeline-action-${card.issue.id}-${choice.issue_action.action}`}
                                              disabled={issueOptionDisabled(card, choice.issue_action)}
                                              title={choice.reason}
                                              onClick={() => runIssueAction(card, choice.issue_action)}
                                              {...issueActionButtonAttrs(choice.issue_action)}
                                            >
                                              {issueDecisionButtonLabel(card, choice.issue_action)}
                                            </button>
                                          ))}
                                        </div>
                                      </div>
                                    </div>
                                  ) : null}
                                  {timeline.decision_receipts.length ? (
                                    <div class="worker-lifecycle-roles issue-timeline-receipts">
                                      {timeline.decision_receipts.slice(-3).map((receipt: any) => (
                                        <div
                                          class={`worker-lifecycle-role worker-lifecycle-role--${issueTimelineReceiptTone(
                                            receipt,
                                          )}`}
                                          data-testid={`issue-timeline-receipt-${card.issue.id}-${receipt.id}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>{issueTimelineReceiptLabel(receipt)}</strong>
                                            <span>{receipt.human_confirmed ? "confirmed" : "pending"}</span>
                                          </div>
                                          <p>{receipt.note_excerpt ?? receipt.linked_resource}</p>
                                          <div class="trace-strip">
                                            <span class="trace-pill">{receipt.source}</span>
                                            {receipt.receipt_schema_version ? (
                                              <span class="trace-pill">
                                                {schemaLabel(receipt.receipt_schema_version)}
                                              </span>
                                            ) : null}
                                            {receipt.policy_schema_version ? (
                                              <span class="trace-pill">
                                                {schemaLabel(receipt.policy_schema_version)}
                                              </span>
                                            ) : null}
                                            <span class="trace-pill">{issueTimelineReceiptMeta(receipt)}</span>
                                          </div>
                                        </div>
                                      ))}
                                    </div>
                                  ) : null}
                                  <div class="worker-lifecycle-roles issue-timeline-items">
                                    {timeline.items.slice(-8).map((item: any) => (
                                      <div
                                        class={`worker-lifecycle-role worker-lifecycle-role--${issueTimelineItemTone(
                                          item,
                                        )}`}
                                        data-testid={`issue-timeline-item-${card.issue.id}-${item.id}`}
                                      >
                                        <div class="stage-row-head">
                                          <strong>{item.title}</strong>
                                          <span>{issueTimelineTimeLabel(item)}</span>
                                        </div>
                                        <p>{item.summary}</p>
                                        <div class="trace-strip">
                                          <span class="trace-pill">{issueTimelineItemMeta(item)}</span>
                                          <span class="trace-pill">{item.event_kind}</span>
                                          <span class="trace-pill">permalink</span>
                                          <button
                                            type="button"
                                            class="trace-pill trace-pill--button"
                                            aria-label={`Copy issue timeline item permalink ${item.id}`}
                                            data-testid={`issue-timeline-item-permalink-${card.issue.id}-${item.id}`}
                                            title={item.permalink}
                                            onClick={() => void copyDoctorAction(item.permalink)}
                                          >
                                            Copy
                                          </button>
                                          {item.schema_version ? (
                                            <span class="trace-pill">{schemaLabel(item.schema_version)}</span>
                                          ) : null}
                                          {item.comment_id ? (
                                            <span class="trace-pill">comment #{item.comment_id}</span>
                                          ) : null}
                                          {item.evidence_id ? (
                                            <span class="trace-pill">evidence #{item.evidence_id}</span>
                                          ) : null}
                                          {item.verdict_id ? (
                                            <span class="trace-pill">verdict #{item.verdict_id}</span>
                                          ) : null}
                                          {item.blocker ? (
                                            <span class="trace-pill trace-pill--warn">{item.blocker}</span>
                                          ) : null}
                                        </div>
                                        {item.body_excerpt ? (
                                          <code class="evidence-excerpt">{item.body_excerpt}</code>
                                        ) : null}
                                      </div>
                                    ))}
                                  </div>
                                  {timeline.next_actions.length ? (
                                    <div class="doctor-actions">
                                      {timeline.next_actions.slice(0, 2).map((action: any, index: any) => (
                                        <div class="doctor-action-row">
                                          <code>{action}</code>
                                          <button
                                            type="button"
                                            aria-label={`Copy issue timeline action ${action}`}
                                            data-testid={`issue-timeline-action-copy-${card.issue.id}-${index}`}
                                            onClick={() => void copyDoctorAction(action)}
                                          >
                                            Copy
                                          </button>
                                        </div>
                                      ))}
                                    </div>
                                  ) : null}
                                </div>
                              )}
                            </Show>
                            <Show when={selectedIssueDoctor()} keyed>
                              {(doctor: any) => (
                                <div class={`doctor-summary doctor-summary--${doctorHealthTone(doctor.health)}`}>
                                  <div class="stage-row-head">
                                    <strong>Doctor</strong>
                                    <span>{doctorHealthLabel(doctor.health)}</span>
                                  </div>
                                  <p>{doctor.summary}</p>
                                  <div class="trace-strip">
                                    <span class="trace-pill">{schemaLabel(doctor.schema_version)}</span>
                                    <span class="trace-pill">round {doctor.current_round}</span>
                                    <Show when={roundHistoryLabel(card)}>
                                      {(label: any) => (
                                        <span class="trace-pill" title={label()}>
                                          rounds {card.trace?.rounds.length ?? 0}
                                        </span>
                                      )}
                                    </Show>
                                    <Show when={roundRecoveryLabel(card)}>
                                      {(label: any) => <span class="trace-pill trace-pill--ok">{label()}</span>}
                                    </Show>
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
                                      {doctor.failed_checks.map((check: any) => (
                                        <span>check {check}</span>
                                      ))}
                                      {doctor.audit_failure_details.map((detail: any) => (
                                        <span>detail {detail}</span>
                                      ))}
                                      {doctor.missing_receipts.map((receipt: any) => (
                                        <span>missing {receipt}</span>
                                      ))}
                                      {doctor.worker_failures.map((failure: any) => (
                                        <span>{failure}</span>
                                      ))}
                                    </div>
                                  ) : null}
                                  {doctor.next_actions.length ? (
                                    <div class="doctor-actions">
                                      {doctor.next_actions.slice(0, 3).map((action: any, index: any) => (
                                        <div class="doctor-action-row">
                                          <code>{action}</code>
                                          <button
                                            type="button"
                                            aria-label={`Copy doctor action ${action}`}
                                            data-testid={`doctor-action-copy-${card.issue.id}-${index}`}
                                            onClick={() => void copyDoctorAction(action)}
                                          >
                                            Copy
                                          </button>
                                        </div>
                                      ))}
                                    </div>
                                  ) : null}
                                </div>
                              )}
                            </Show>
                            {card.issue.loop_id ? (
                              <Show
                                when={selectedIssueRuntimePreflight()}
                                keyed
                                fallback={
                                  <div
                                    class="worker-lifecycle runtime-preflight worker-lifecycle--pending"
                                    data-testid={`runtime-preflight-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Runtime Preflight</strong>
                                      <span>
                                        {selectedRuntimePreflight.loading ? "loading" : "pending"}
                                      </span>
                                    </div>
                                    <div class="trace-strip">
                                      <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                      <span class="trace-pill">runtime_preflight.v1</span>
                                    </div>
                                  </div>
                                }
                              >
                                {(preflight: any) => (
                                  <div
                                    class={`worker-lifecycle runtime-preflight worker-lifecycle--${runtimePreflightTone(
                                      preflight.preflight_state,
                                    )}`}
                                    data-testid={`runtime-preflight-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Runtime Preflight</strong>
                                      <span>{runtimePreflightStateLabel(preflight.preflight_state)}</span>
                                    </div>
                                    <p>{preflight.summary}</p>
                                    <div class="trace-strip">
                                      <span class="trace-pill">{schemaLabel(preflight.schema_version)}</span>
                                      <span class="trace-pill">loop #{preflight.loop_id}</span>
                                      <span class="trace-pill">round {preflight.current_round}</span>
                                      <span class="trace-pill">{preflight.runtime}</span>
                                      <span class="trace-pill">
                                        {preflight.policy.route_from}
                                        {" -> "}
                                        {preflight.policy.route_to}
                                      </span>
                                      <span class="trace-pill">{preflight.policy.object_kind}</span>
                                      <span
                                        class={
                                          preflight.current?.gate_passed === false
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {runtimePreflightGateLabel(preflight)}
                                      </span>
                                      <span
                                        class={
                                          preflight.preview.supported
                                            ? "trace-pill"
                                            : "trace-pill trace-pill--warn"
                                        }
                                      >
                                        {runtimePreflightBoolLabel("policy", preflight.preview.supported)}
                                      </span>
                                      <span
                                        class={
                                          preflight.preview.probe_ok
                                            ? "trace-pill"
                                            : "trace-pill trace-pill--warn"
                                        }
                                      >
                                        {runtimePreflightProbeLabel(preflight)}
                                      </span>
                                      {preflight.current?.result ? (
                                        <span
                                          class={
                                            preflight.current.result === "rejected"
                                              ? "trace-pill trace-pill--warn"
                                              : "trace-pill"
                                          }
                                        >
                                          {preflight.current.result}
                                        </span>
                                      ) : null}
                                      {preflight.current?.receipt_missing.length ? (
                                        <span class="trace-pill trace-pill--warn">
                                          missing {preflight.current.receipt_missing.join(", ")}
                                        </span>
                                      ) : null}
                                      {preflight.preview.blocker ? (
                                        <span class="trace-pill trace-pill--warn">
                                          {preflight.preview.blocker}
                                        </span>
                                      ) : null}
                                    </div>
                                    <div class="worker-lifecycle-rounds">
                                      {preflight.policy.supported_runtimes.map((runtime: any) => (
                                        <span
                                          class={
                                            runtime === preflight.runtime
                                              ? "trace-pill trace-pill--ok"
                                              : "trace-pill"
                                          }
                                        >
                                          {runtime}
                                        </span>
                                      ))}
                                    </div>
                                    {(() => {
                                      const capability = preflight.preview.capability_preview;
                                      return (
                                        <div
                                          class="worker-lifecycle-role worker-lifecycle-role--pending"
                                          data-testid={`runtime-capability-preview-${card.issue.id}`}
                                        >
                                          <div class="stage-row-head">
                                            <strong>Capability Preview</strong>
                                            <span>
                                              {capability.worker_spawn_ready ? "spawn ready" : "spawn blocked"}
                                            </span>
                                          </div>
                                          <div class="trace-strip">
                                            <span class="trace-pill">
                                              {schemaLabel(capability.schema_version)}
                                            </span>
                                            <span
                                              class={
                                                capability.worker_spawn_ready
                                                  ? "trace-pill trace-pill--ok"
                                                  : "trace-pill trace-pill--warn"
                                              }
                                            >
                                              worker spawn{" "}
                                              {capability.worker_spawn_ready ? "ready" : "blocked"}
                                            </span>
                                            <span class="trace-pill">
                                              sandbox {capability.sandbox.filesystem}
                                            </span>
                                            <span class="trace-pill">
                                              network {capability.sandbox.network}
                                            </span>
                                            <span class="trace-pill">
                                              artifacts {capability.artifact_capture.mode}
                                            </span>
                                            <span class="trace-pill">
                                              human {capability.human_boundary.confirmation_arg}
                                            </span>
                                            <span class="trace-pill">
                                              review budget{" "}
                                              {capability.human_boundary.reviewer_invalid_round_budget}
                                            </span>
                                            <span class="trace-pill">
                                              worker ctx{" "}
                                              {capability.worker_context.required.length
                                                ? capability.worker_context.required.join(", ")
                                                : "none"}
                                            </span>
                                          </div>
                                          {capability.worker_spawn_blockers.length ? (
                                            <div class="doctor-lines">
                                              {capability.worker_spawn_blockers.map((blocker: any) => (
                                                <span>{blocker}</span>
                                              ))}
                                            </div>
                                          ) : null}
                                        </div>
                                      );
                                    })()}
                                    {preflight.failures.length ? (
                                      <div class="doctor-lines">
                                        {preflight.failures.map((failure: any) => (
                                          <span>{failure}</span>
                                        ))}
                                      </div>
                                    ) : null}
                                    {preflight.next_actions.length ? (
                                      <div class="doctor-actions">
                                        {preflight.next_actions.slice(0, 2).map((action: any, index: any) => (
                                          <div class="doctor-action-row">
                                            <code>{action}</code>
                                            <button
                                              type="button"
                                              aria-label={`Copy runtime preflight action ${action}`}
                                              data-testid={`runtime-preflight-action-copy-${card.issue.id}-${index}`}
                                              onClick={() => void copyDoctorAction(action)}
                                            >
                                              Copy
                                            </button>
                                          </div>
                                        ))}
                                      </div>
                                    ) : null}
                                  </div>
                                )}
                              </Show>
                            ) : null}
                            {card.issue.loop_id ? (
                              <Show
                                when={selectedIssueWorkerLifecycle()}
                                keyed
                                fallback={
                                  <div
                                    class="worker-lifecycle worker-lifecycle--pending"
                                    data-testid={`worker-lifecycle-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Worker Lifecycle</strong>
                                      <span>
                                        {selectedWorkerLifecycle.loading ? "loading" : "pending"}
                                      </span>
                                    </div>
                                    <div class="trace-strip">
                                      <span class="trace-pill">loop #{card.issue.loop_id}</span>
                                      <span class="trace-pill">worker_lifecycle.v1</span>
                                    </div>
                                  </div>
                                }
                              >
                                {(lifecycle: any) => (
                                  <div
                                    class={`worker-lifecycle worker-lifecycle--${workerLifecycleTone(
                                      lifecycle.lifecycle_state,
                                    )}`}
                                    data-testid={`worker-lifecycle-detail-${card.issue.id}`}
                                  >
                                    <div class="stage-row-head">
                                      <strong>Worker Lifecycle</strong>
                                      <span>{workerLifecycleStateLabel(lifecycle.lifecycle_state)}</span>
                                    </div>
                                    <p>{lifecycle.summary}</p>
                                    <div class="trace-strip">
                                      <span class="trace-pill">{schemaLabel(lifecycle.schema_version)}</span>
                                      <span class="trace-pill">loop #{lifecycle.loop_id}</span>
                                      <span class="trace-pill">round {lifecycle.current_round}</span>
                                      <span class="trace-pill">{lifecycle.runtime}</span>
                                      <span
                                        class={
                                          lifecycle.current.reviewer_invalid_budget_exhausted
                                            ? "trace-pill trace-pill--warn"
                                            : "trace-pill"
                                        }
                                      >
                                        {workerLifecycleBudgetLabel(lifecycle)}
                                      </span>
                                      <span class="trace-pill">fallback {lifecycle.policy.fallback_status}</span>
                                      <span class="trace-pill">{lifecycle.current.worker_ok_count}/{lifecycle.current.worker_count} workers</span>
                                      {lifecycle.current.missing_roles.length ? (
                                        <span class="trace-pill trace-pill--warn">
                                          missing {lifecycle.current.missing_roles.join(", ")}
                                        </span>
                                      ) : null}
                                      {lifecycle.current.worker_timeout_count ||
                                      lifecycle.current.worker_retry_exhausted_count ? (
                                        <span class="trace-pill trace-pill--warn">
                                          {lifecycle.current.worker_timeout_count} timeout /{" "}
                                          {lifecycle.current.worker_retry_exhausted_count} exhausted
                                        </span>
                                      ) : null}
                                    </div>
                                    <div class="worker-lifecycle-roles">
                                      {lifecycle.current.expected_roles.map((role: any) => {
                                        const worker = workerLifecycleWorkerForRole(lifecycle.current, role);
                                        return (
                                          <div
                                            class={`worker-lifecycle-role worker-lifecycle-role--${workerLifecycleRoleTone(
                                              worker,
                                            )}`}
                                            data-testid={`worker-lifecycle-role-${card.issue.id}-${role}`}
                                          >
                                            <div class="stage-row-head">
                                              <strong>{role}</strong>
                                              <span>{workerLifecycleWorkerState(worker)}</span>
                                            </div>
                                            <p>
                                              {worker?.evidence_summary ??
                                                worker?.action ??
                                                "No worker receipt"}
                                            </p>
                                            <div class="trace-strip">
                                              <span
                                                class={
                                                  workerLifecycleWorkerState(worker) === "ok"
                                                    ? "trace-pill"
                                                    : "trace-pill trace-pill--warn"
                                                }
                                              >
                                                {worker?.kind ?? "missing"}
                                              </span>
                                              <span
                                                class={
                                                  worker?.receipt_ok === false || !worker
                                                    ? "trace-pill trace-pill--warn"
                                                    : "trace-pill"
                                                }
                                              >
                                                {workerLifecycleReceiptLabel(worker)}
                                              </span>
                                              {worker?.evidence_kind ? (
                                                <span class="trace-pill">{worker.evidence_kind}</span>
                                              ) : null}
                                              {worker?.mode ? <span class="trace-pill">{worker.mode}</span> : null}
                                              {workerLifecycleDurationLabel(worker) ? (
                                                <span class="trace-pill">{workerLifecycleDurationLabel(worker)}</span>
                                              ) : null}
                                              {worker?.timeout_secs !== null && worker?.timeout_secs !== undefined ? (
                                                <span class="trace-pill">limit {worker.timeout_secs}s</span>
                                              ) : null}
                                              {workerLifecycleAttemptLabel(worker) ? (
                                                <span class="trace-pill">{workerLifecycleAttemptLabel(worker)}</span>
                                              ) : null}
                                              {worker?.action ? <span class="trace-pill">{worker.action}</span> : null}
                                              {worker?.gate_count !== null && worker?.gate_count !== undefined ? (
                                                <span class="trace-pill">gates {worker.gate_count}</span>
                                              ) : null}
                                              {worker?.timed_out ? (
                                                <span class="trace-pill trace-pill--warn">timeout</span>
                                              ) : null}
                                              {worker?.retry_exhausted ? (
                                                <span class="trace-pill trace-pill--warn">retry exhausted</span>
                                              ) : null}
                                              {worker?.receipt_errors.map((field: any) => (
                                                <span class="trace-pill trace-pill--warn">receipt {field}</span>
                                              ))}
                                            </div>
                                            {worker?.transcript_excerpt ? (
                                              <p class="muted">{worker.transcript_excerpt}</p>
                                            ) : null}
                                          </div>
                                        );
                                      })}
                                    </div>
                                    <div class="worker-lifecycle-rounds">
                                      {lifecycle.rounds.map((round: any) => (
                                        <span
                                          class={
                                            round.failures.length ||
                                            round.worker_timeout_count ||
                                            round.worker_retry_exhausted_count ||
                                            round.reviewer_invalid_budget_exhausted
                                              ? "trace-pill trace-pill--warn"
                                              : "trace-pill"
                                          }
                                          title={round.failures.join(" | ") || undefined}
                                        >
                                          {workerLifecycleRoundLabel(round)}
                                        </span>
                                      ))}
                                    </div>
                                    {lifecycle.failures.length ? (
                                      <div class="doctor-lines">
                                        {lifecycle.failures.map((failure: any) => (
                                          <span>{failure}</span>
                                        ))}
                                      </div>
                                    ) : null}
                                    {lifecycle.next_actions.length ? (
                                      <div class="doctor-actions">
                                        {lifecycle.next_actions.slice(0, 2).map((action: any, index: any) => (
                                          <div class="doctor-action-row">
                                            <code>{action}</code>
                                            <button
                                              type="button"
                                              aria-label={`Copy worker lifecycle action ${action}`}
                                              data-testid={`worker-lifecycle-action-copy-${card.issue.id}-${index}`}
                                              onClick={() => void copyDoctorAction(action)}
                                            >
                                              Copy
                                            </button>
                                          </div>
                                        ))}
                                      </div>
                                    ) : null}
                                  </div>
                                )}
                              </Show>
                            ) : null}
                            </LoopObservability>
                            {issueHumanActions(card).length ||
                            (card.issue.loop_id &&
                              (card.issue.status === "Todo" || card.issue.status === "Doing")) ? (
                              <div class="decision-options">
                                {card.issue.loop_id &&
                                (card.issue.status === "Todo" || card.issue.status === "Doing") ? (
                                  <button
                                    type="button"
                                    aria-label={`Advance issue #${card.issue.id} from detail`}
                                    data-testid={`issue-action-detail-advance-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => void advanceIssue(card)}
                                  >
                                    {issuePendingLabel(card.issue.id) ?? "Advance"}
                                  </button>
                                ) : null}
                                {card.issue.loop_id && card.issue.status === "Todo" ? (
                                  <button
                                    type="button"
                                    aria-label={issueRuntimeActionAriaLabel(card, false, "detail")}
                                    {...issueActionButtonAttrs(issueActionByName(card, "run"))}
                                    data-testid={`issue-action-detail-run-${card.issue.id}`}
                                    disabled={Boolean(issuePendingLabel(card.issue.id))}
                                    onClick={() => void runIssueLoop(card)}
                                  >
                                    {issueRuntimeActionLabel(card, false)}
                                  </button>
                                ) : null}
                                {issueHumanActions(card).map((action: any) => (
                                  <button
                                    type="button"
                                    aria-label={
                                      action.action === "retry"
                                        ? issueRuntimeActionAriaLabel(card, true, "detail")
                                        : `${action.label} issue #${card.issue.id} from detail`
                                    }
                                    data-testid={`issue-action-detail-${action.action}-${card.issue.id}`}
                                    disabled={issueOptionDisabled(card, action)}
                                    onClick={() => runIssueAction(card, action)}
                                    {...issueActionButtonAttrs(action)}
                                  >
                                    {issueDecisionButtonLabel(card, action)}
                                  </button>
                                ))}
                              </div>
                            ) : null}
                            {card.actions.length ? issueActionContractChips(card, "detail") : null}
                            {commentComposerActive(card.issue.id, "detail") ? (
                              <div class="comment-box comment-box--detail">
                                <textarea
                                  aria-label={`Detail issue #${card.issue.id} comment`}
                                  data-testid={`issue-comment-detail-${card.issue.id}`}
                                  value={commentBody()}
                                  onInput={(event: any) => setCommentBody(event.currentTarget.value)}
                                  onKeyDown={(event: any) => handleCommentKeyDown(event, card.issue.id)}
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
                                {issueDecisionActions(card).map((action: any) => (
                                  <button
                                    type="button"
                                    aria-label={
                                      action.action === "retry"
                                        ? issueRuntimeActionAriaLabel(card, true, "detail composer")
                                        : `${action.label} issue #${card.issue.id} from detail composer`
                                    }
                                    data-testid={`issue-action-detail-composer-${action.action}-${card.issue.id}`}
                                    disabled={issueOptionDisabled(card, action)}
                                    onClick={() => runIssueAction(card, action)}
                                    {...issueActionButtonAttrs(action)}
                                  >
                                    {issueDecisionButtonLabel(card, action)}
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
                            <div class="comment-stack comment-stack--detail">
                              <h4>Comments</h4>
                              {card.comments.map((comment: any) => (
                                <div class="comment-line comment-line--detail">
                                  <div class="comment-line-head">
                                    <strong>{comment.author}</strong>
                                    <div class="comment-tags">
                                      {commentPills(comment).map((pill: any) => commentPillNode(card.issue.id, pill))}
                                    </div>
                                  </div>
                                  <span>{commentPreview(comment, COMMENT_DETAIL_PREVIEW_LIMIT)}</span>
                                </div>
                              ))}
                            </div>
                            <dl class="detail-grid">
                              {issueDetailRows(card).map(([label, value]: [string, any]) => (
                                <div>
                                  <dt>{label}</dt>
                                  <dd>{value}</dd>
                                </div>
                              ))}
                            </dl>
                            {card.trace?.operator_events.length ? (
                              <div class="operator-trail">
                                <h4>Operator Trail</h4>
                                {card.trace.operator_events.map((event: any) => (
                                  <div class="operator-event">
                                    <strong>{operatorEventLabel(event)}</strong>
                                    <span>{operatorEventStatusLabel(event)}</span>
                                    <p>{event.note ?? event.summary}</p>
                                  </div>
                                ))}
                              </div>
                            ) : null}
                            {card.trace?.stages.length ? (
                              <div class="stage-timeline">
                                {card.trace.stages.map((stage: any) => (
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
                                {card.trace.evidence.map((evidence: any) => (
                                  <div
                                    class={
                                      selectedEvidenceId() === evidence.id
                                        ? "evidence-row evidence-row--selected"
                                        : "evidence-row"
                                    }
                                    data-testid={`evidence-row-${evidence.id}`}
                                    ref={(element: any) => evidenceRows.set(evidence.id, element)}
                                  >
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
                                      {evidence.missing_receipts.map((receipt: any) => (
                                        <span class="trace-pill trace-pill--warn">missing {receipt}</span>
                                      ))}
                                      {evidence.packet_envelope_errors.map((field: any) => (
                                        <span class="trace-pill trace-pill--warn">invalid {field}</span>
                                      ))}
                                      {evidence.worker_receipt_errors.map((field: any) => (
                                        <span class="trace-pill trace-pill--warn">receipt {field}</span>
                                      ))}
                                      {evidence.operator_options.map((option: any) => (
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
                          </>
                        )}
                      </Show>
                    </article>
                  </div>
    
                  <article class="panel panel--board">
                    <p class="panel-kicker">Issues</p>
                    <h3>Status board</h3>
                    <ReviewQueue>
                    <div class="review-queue" data-testid="review-queue">
                      <div class="review-queue-head">
                        <div>
                          <strong>Review queue</strong>
                          <span>
                            {reviewQueueCards().length
                              ? `${reviewQueueCards().length} need decision`
                              : "clear"}
                          </span>
                        </div>
                        <span>Blocked / Needs Review</span>
                      </div>
                      {reviewQueueCards().length ? (
                        <div class="review-queue-list">
                          {reviewQueueCards().map((card: any) => (
                            <div
                              class="review-queue-item"
                              data-testid={`review-queue-issue-${card.issue.id}`}
                            >
                              <div class="review-queue-item-head">
                                <div>
                                  <strong>#{card.issue.id}</strong>
                                  <span>{card.issue.title}</span>
                                </div>
                                <span class="trace-pill trace-pill--warn">{card.issue.status}</span>
                              </div>
                              <div class="trace-strip">
                                <span class="trace-pill">R {card.trace?.current_round ?? "?"}</span>
                                <span class="trace-pill">{reviewQueueDecisionLabel(card)}</span>
                                <span class="trace-pill">{reviewQueueBlockerLabel(card)}</span>
                                {card.doctor ? (
                                  <span class="trace-pill">{doctorWorkerLabel(card.doctor)}</span>
                                ) : null}
                                {card.doctor ? (
                                  <span class="trace-pill">{doctorReceiptLabel(card.doctor)}</span>
                                ) : null}
                              </div>
                              <p class="muted">{card.issue.summary ?? card.doctor?.summary ?? "Decision pending"}</p>
                              {reviewQueueEvidence(card).length ? (
                                <div class="review-queue-evidence">
                                  {reviewQueueEvidence(card).map((evidence: any) => (
                                    <button
                                      type="button"
                                      data-testid={`review-queue-evidence-${card.issue.id}-${evidence.id}`}
                                      title={evidence.summary}
                                      onClick={() => focusEvidence(card.issue.id, evidence.id)}
                                    >
                                      {evidence.stage_role ?? evidence.kind} E#{evidence.id}
                                    </button>
                                  ))}
                                </div>
                              ) : null}
                              <div class="record-actions">
                                {issueDecisionActions(card).map((action: any) => (
                                  <button
                                    type="button"
                                    aria-label={
                                      action.action === "retry"
                                        ? issueRuntimeActionAriaLabel(card, true, "review queue")
                                        : `${action.label} issue #${card.issue.id} from review queue`
                                    }
                                    data-testid={`review-queue-action-${action.action}-${card.issue.id}`}
                                    disabled={issueOptionDisabled(card, action)}
                                    onClick={() => runIssueAction(card, action)}
                                    {...issueActionButtonAttrs(action)}
                                  >
                                    {issueDecisionButtonLabel(card, action)}
                                  </button>
                                ))}
                                <button
                                  type="button"
                                  aria-label={`Comment on issue #${card.issue.id} from review queue`}
                                  data-testid={`review-queue-action-comment-${card.issue.id}`}
                                  disabled={Boolean(issuePendingLabel(card.issue.id))}
                                  onClick={() => openIssueComment(card.issue.id, "board")}
                                  {...issueActionButtonAttrs(issueActionByName(card, "comment"))}
                                >
                                  Comment
                                </button>
                                <button
                                  type="button"
                                  aria-label={`Show issue #${card.issue.id} details from review queue`}
                                  data-testid={`review-queue-action-details-${card.issue.id}`}
                                  onClick={() => {
                                    setSelectedIssueId(card.issue.id);
                                    revealIssueDetail();
                                  }}
                                >
                                  Details
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      ) : null}
                    </div>
                    </ReviewQueue>
                    <div class="workbench-summary" data-testid="local-workbench-summary">
                      <div>
                        <strong>Local workbench</strong>
                        <span>{issueCards()?.length ?? 0} issues</span>
                        <span>{reviewQueueCards().length} need decision</span>
                      </div>
                    </div>
                    <IssueBoard>
                    <div class="board-columns">
                      {ISSUE_STATUSES.map((statusName: any) => (
                        <section
                          class="board-column"
                          data-testid={`issue-column-${issueStatusTestId(statusName)}`}
                        >
                          <div class="board-column-head">
                            <strong>{statusName}</strong>
                            <span>{issueCardsForStatus(statusName).length}</span>
                          </div>
                          <ul class="record-list board-column-list">
                            {issueCardsForStatus(statusName).length ? (
                              issueCardsForStatus(statusName).map((card: any) => (
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
                                  <div class="trace-strip">
                                    <span class="trace-pill">{card.issue.assignee ?? "unassigned"}</span>
                                    <span class="trace-pill">{card.issue.claim_role ?? "no role"}</span>
                                    <span class="trace-pill">{card.issue.claim_source ?? "no source"}</span>
                                  </div>
                                  <Show when={cardDoctor(card)} keyed>
                                    {(doctor: any) => (
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
                                        .map((detail: any) => (
                                          <span title={detail}>{compactAuditFailureDetail(detail)}</span>
                                        ))}
                                      {cardAuditFailureDetails(card).length > 2 ? (
                                        <span>+{cardAuditFailureDetails(card).length - 2} more</span>
                                      ) : null}
                                      {issueAuditQuickActions(card)}
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
                                    {card.comments.slice(-2).map((comment: any) => (
                                      <div class="comment-line comment-line--compact">
                                        <div class="comment-line-head">
                                          <strong>{comment.author}</strong>
                                          <div class="comment-tags">
                                            {commentPills(comment)
                                              .slice(0, 3)
                                              .map((pill: any) => commentPillNode(card.issue.id, pill))}
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
                                        onInput={(event: any) => setCommentBody(event.currentTarget.value)}
                                        onKeyDown={(event: any) => handleCommentKeyDown(event, card.issue.id)}
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
                                      {issueDecisionActions(card).map((action: any) => (
                                        <button
                                          type="button"
                                          aria-label={
                                            action.action === "retry"
                                              ? issueRuntimeActionAriaLabel(card, true, "board composer")
                                              : `${action.label} issue #${card.issue.id} from board composer`
                                          }
                                          data-testid={`issue-action-board-composer-${action.action}-${card.issue.id}`}
                                          disabled={issueOptionDisabled(card, action)}
                                          onClick={() => runIssueAction(card, action)}
                                          {...issueActionButtonAttrs(action)}
                                        >
                                          {issueDecisionButtonLabel(card, action)}
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
                                      {card.issue.loop_id &&
                                      (card.issue.status === "Todo" || card.issue.status === "Doing") ? (
                                        <button
                                          type="button"
                                          aria-label={`Advance issue #${card.issue.id} from board`}
                                          data-testid={`issue-action-board-advance-${card.issue.id}`}
                                          disabled={Boolean(issuePendingLabel(card.issue.id))}
                                          onClick={() => void advanceIssue(card)}
                                        >
                                          {issuePendingLabel(card.issue.id) ?? "Advance"}
                                        </button>
                                      ) : null}
                                      {card.issue.loop_id && card.issue.status === "Todo" ? (
                                        <button
                                          type="button"
                                          aria-label={issueRuntimeActionAriaLabel(
                                            card,
                                            false,
                                            "board",
                                          )}
                                          data-testid={`issue-action-board-run-${card.issue.id}`}
                                          disabled={Boolean(issuePendingLabel(card.issue.id))}
                                          onClick={() => void runIssueLoop(card)}
                                          {...issueActionButtonAttrs(issueActionByName(card, "run"))}
                                        >
                                          {issueRuntimeActionLabel(card, false)}
                                        </button>
                                      ) : null}
                                      {issueDecisionActions(card).map((action: any) => (
                                        <button
                                          type="button"
                                          aria-label={
                                            action.action === "retry"
                                              ? issueRuntimeActionAriaLabel(card, true, "board")
                                              : `${action.label} issue #${card.issue.id} from board`
                                          }
                                          data-testid={`issue-action-board-${action.action}-${card.issue.id}`}
                                          disabled={issueOptionDisabled(card, action)}
                                          onClick={() => runIssueAction(card, action)}
                                          {...issueActionButtonAttrs(action)}
                                        >
                                          {issueDecisionButtonLabel(card, action)}
                                        </button>
                                      ))}
                                      <button
                                        type="button"
                                        aria-label={`Show issue #${card.issue.id} details`}
                                        data-testid={`issue-action-board-details-${card.issue.id}`}
                                        onClick={() => {
                                          setSelectedIssueId(card.issue.id);
                                          revealIssueDetail();
                                        }}
                                      >
                                        Details
                                      </button>
                                      <button
                                        type="button"
                                        aria-label={`Comment on issue #${card.issue.id} from board`}
                                        data-testid={`issue-action-board-comment-${card.issue.id}`}
                                        disabled={Boolean(issuePendingLabel(card.issue.id))}
                                        onClick={() => openIssueComment(card.issue.id, "board")}
                                        {...issueActionButtonAttrs(issueActionByName(card, "comment"))}
                                      >
                                        Comment
                                      </button>
                                    </div>
                                  )}
                                </li>
                              ))
                            ) : (
                              <li class="record-card issue-card issue-card--empty">
                                <span>No issues</span>
                                {statusName === "Todo" && !(issueCards() ?? []).length ? (
                                  <>
                                    <button
                                      type="button"
                                      data-testid="issue-empty-run-demo"
                                      disabled={Boolean(pendingDemoAction())}
                                      onClick={() => void startDemoLoop()}
                                    >
                                      {pendingDemoAction() ?? "Run Demo"}
                                    </button>
                                  </>
                                ) : null}
                              </li>
                            )}
                          </ul>
                        </section>
                      ))}
                    </div>
                    </IssueBoard>
                  </article>
                </section>
  );
}
