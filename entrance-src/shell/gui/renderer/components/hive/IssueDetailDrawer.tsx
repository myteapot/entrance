import { createSignal } from "solid-js";
import {
  COMMENT_DETAIL_PREVIEW_LIMIT,
  commentPills,
  commentPreview,
  operatorEventLabel,
} from "../../lib/hive";
import { LoopObservability } from "./LoopObservability";

type IssueDetailDrawerProps = Record<string, any> & {
  card: any;
  onClose: () => void;
};

const tabs = ["Overview", "Comments", "Evidence", "Timeline", "Review", "Runtime"] as const;
type DrawerTab = (typeof tabs)[number];

const list = <T,>(value: T[] | undefined | null): T[] => value ?? [];

export function IssueDetailDrawer(props: IssueDetailDrawerProps) {
  const [tab, setTab] = createSignal<DrawerTab>("Overview");
  const card = () => props.card;
  const trace = () => card().trace;
  const dashboard = () => props.selectedIssueLoopDashboard?.() ?? props.selectedLoopDashboard?.();
  const policy = () => props.selectedIssueTransitionPolicy?.() ?? props.selectedTransitionPolicy?.();
  const timeline = () => props.selectedIssueTimeline?.() ?? props.selectedIssueActivityTimeline?.();
  const drilldown = () => props.selectedIssueEvidenceDrilldown?.() ?? props.selectedEvidenceDrilldown?.();
  const manifest = () => props.selectedIssueEvidenceManifest?.() ?? props.selectedEvidenceManifest?.();
  const preflight = () => props.selectedIssueRuntimePreflight?.() ?? props.selectedRuntimePreflight?.();
  const lifecycle = () => props.selectedIssueWorkerLifecycle?.() ?? props.selectedWorkerLifecycle?.();
  const pending = () => props.issuePendingLabel?.(card().issue.id);
  const humanActions = () => props.issueHumanActions?.(card()) ?? [];
  const auditFailures = () => [
    ...list(trace()?.audit_failure_details),
    ...list(card().doctor?.audit_failure_details),
    ...list(dashboard()?.health?.audit_failure_details),
  ];

  return (
    <aside class="issue-detail-drawer" data-testid={`issue-detail-drawer-${card().issue.id}`}>
      <header class="drawer-head">
        <div>
          <span>{card().issue.status}</span>
          <h3>{card().issue.title}</h3>
          <p>{card().issue.summary ?? "No summary"}</p>
        </div>
        <button type="button" aria-label="Close issue detail" onClick={props.onClose}>
          Close
        </button>
      </header>

      <div class="drawer-meta">
        <span>#{card().issue.id}</span>
        <span>{card().issue.assignee ?? "No assignee"}</span>
        <span>{card().issue.claim_role ?? "No role"}</span>
        <span>{card().issue.claim_source ?? "No source"}</span>
      </div>

      <LoopObservability {...props} card={card()} />

      {auditFailures().length ? (
        <div class="drawer-warning">
          <strong>Audit failure</strong>
          {auditFailures().slice(0, 3).map((detail) => (
            <span>{props.compactAuditFailureDetail?.(detail) ?? detail}</span>
          ))}
        </div>
      ) : null}

      <div class="drawer-actions">
        {card().issue.loop_id && (card().issue.status === "Todo" || card().issue.status === "Doing") ? (
          <button
            type="button"
            data-testid={`issue-action-detail-advance-${card().issue.id}`}
            disabled={Boolean(pending())}
            onClick={() => void props.advanceIssue?.(card())}
          >
            {pending() ?? "Advance"}
          </button>
        ) : null}
        {humanActions().map((action: any) => (
          <button
            type="button"
            data-testid={`issue-action-detail-${action.action}-${card().issue.id}`}
            disabled={props.issueOptionDisabled?.(card(), action)}
            {...props.issueActionButtonAttrs?.(action)}
            onClick={() => props.runIssueAction?.(card(), action)}
          >
            {props.issueDecisionButtonLabel?.(card(), action)}
          </button>
        ))}
        {humanActions().some((action: any) => action.action === "comment") ? null : (
          <button
            type="button"
            data-testid={`issue-action-detail-comment-${card().issue.id}`}
            disabled={Boolean(pending())}
            onClick={() => props.openIssueComment?.(card().issue.id, "detail")}
          >
            Comment
          </button>
        )}
      </div>

      <nav class="drawer-tabs" aria-label="Issue detail tabs">
        {tabs.map((tabName) => (
          <button
            type="button"
            class={tab() === tabName ? "is-active" : ""}
            onClick={() => setTab(tabName)}
          >
            {tabName}
          </button>
        ))}
      </nav>

      <section class="drawer-body">
        {tab() === "Overview" ? (
          <div class="drawer-section">
            <dl class="detail-grid detail-grid--drawer">
              {(props.issueDetailRows?.(card()) ?? []).map(([label, value]: [string, any]) => (
                <div>
                  <dt>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
            {policy() ? (
              <div class="drawer-block">
                <strong>Transition policy</strong>
                <p>{policy().summary}</p>
                <div class="drawer-meta">
                  <span>{policy().state_class}</span>
                  <span>{policy().human_decision_required ? "human required" : "auto clear"}</span>
                  <span>{policy().next_actions?.[0] ?? "no next action"}</span>
                </div>
              </div>
            ) : null}
            {trace()?.stages?.length ? (
              <div class="drawer-block">
                <strong>Agent stages</strong>
                {trace().stages.map((stage: any) => (
                  <div class="drawer-row">
                    <span>{stage.role}</span>
                    <strong>{stage.status}</strong>
                    <p>{stage.summary ?? stage.evidence_summary ?? "No stage summary"}</p>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}

        {tab() === "Comments" ? (
          <div class="drawer-section">
            {props.commentComposerActive?.(card().issue.id, "detail") ? (
              <div class="comment-box comment-box--detail">
                <textarea
                  aria-label={`Detail issue #${card().issue.id} comment`}
                  data-testid={`issue-comment-detail-${card().issue.id}`}
                  value={props.commentBody?.() ?? ""}
                  onInput={(event: any) => props.setCommentBody?.(event.currentTarget.value)}
                  onKeyDown={(event: any) => props.handleCommentKeyDown?.(event, card().issue.id)}
                  placeholder="Comment"
                />
                <button
                  type="button"
                  data-testid={`issue-comment-detail-send-${card().issue.id}`}
                  disabled={props.commentSubmitDisabled?.(card().issue.id)}
                  onClick={() => void props.addIssueComment?.(card().issue.id)}
                >
                  {pending() ?? "Send"}
                </button>
                <button
                  type="button"
                  data-testid={`issue-comment-detail-close-${card().issue.id}`}
                  disabled={Boolean(pending())}
                  onClick={() => props.closeIssueComment?.(card().issue.id)}
                >
                  Close
                </button>
              </div>
            ) : null}
            <div class="comment-stack comment-stack--detail">
              {card().comments.map((comment: any) => (
                <div class="comment-line comment-line--detail">
                  <div class="comment-line-head">
                    <strong>{comment.author}</strong>
                    <div class="comment-tags">
                      {commentPills(comment).map((pill: any) =>
                        props.commentPillNode?.(card().issue.id, pill),
                      )}
                    </div>
                  </div>
                  <span>{commentPreview(comment, COMMENT_DETAIL_PREVIEW_LIMIT)}</span>
                </div>
              ))}
            </div>
          </div>
        ) : null}

        {tab() === "Evidence" ? (
          <div class="drawer-section">
            {manifest() ? (
              <div class="drawer-block">
                <strong>Manifest</strong>
                <p>{manifest().summary}</p>
                <div class="drawer-meta">
                  <span>{manifest().coverage.evidence_count} evidence</span>
                  <span>{manifest().coverage.entry_count} entries</span>
                  <span>{manifest().manifest_state}</span>
                </div>
              </div>
            ) : null}
            {(drilldown()?.items ?? trace()?.evidence ?? []).map((evidence: any) => (
              <div class="evidence-row" data-testid={`evidence-row-${evidence.id}`}>
                <div class="stage-row-head">
                  <strong>{evidence.stage_role ?? evidence.kind}</strong>
                  <span>#{evidence.id}</span>
                </div>
                <p>{evidence.summary}</p>
                <div class="drawer-meta">
                  <span>{evidence.kind}</span>
                  <span>{evidence.admission_result ?? "admission pending"}</span>
                  <span>{props.evidenceWorkerLabel?.(evidence) ?? evidence.worker?.mode ?? "worker pending"}</span>
                </div>
              </div>
            ))}
          </div>
        ) : null}

        {tab() === "Timeline" ? (
          <div class="drawer-section">
            {timeline() ? (
              <>
                <div class="drawer-block">
                  <strong>Timeline</strong>
                  <p>{timeline().summary}</p>
                </div>
                {timeline().items.map((item: any) => (
                  <div class="drawer-row">
                    <span>{props.issueTimelineTimeLabel?.(item) ?? item.timestamp}</span>
                    <strong>{item.title}</strong>
                    <p>{item.summary}</p>
                  </div>
                ))}
              </>
            ) : (
              <span class="muted">Timeline pending</span>
            )}
            {trace()?.operator_events?.length ? (
              <div class="drawer-block">
                <strong>Operator trail</strong>
                {trace().operator_events.map((event: any) => (
                  <div class="drawer-row">
                    <span>{operatorEventLabel(event)}</span>
                    <strong>{event.action ?? event.kind}</strong>
                    <p>{event.note ?? event.summary}</p>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}

        {tab() === "Review" ? (
          <div class="drawer-section">
            <div class="drawer-block">
              <strong>Reviewer verdict</strong>
              <p>{dashboard()?.reviewer?.reason_code ?? trace()?.reason_code ?? "No reason code"}</p>
              <div class="drawer-meta">
                <span>{dashboard()?.reviewer?.decision ?? trace()?.last_decision ?? "pending"}</span>
                <span>
                  invalid {dashboard()?.reviewer?.reviewer_invalid_rounds_used ?? "0"}/
                  {dashboard()?.reviewer?.reviewer_invalid_round_budget ?? "3"}
                </span>
                <span>{policy()?.reviewer_budget?.fallback_status ?? "Blocked fallback"}</span>
              </div>
            </div>
            {(dashboard()?.reviewer?.score_vector ?? trace()?.score_vector ?? []).map((score: any) => (
              <div class="drawer-row">
                <span>{score.name}</span>
                <strong>{score.value ?? score.score ?? "pending"}</strong>
                <p>{score.summary ?? "No score summary"}</p>
              </div>
            ))}
          </div>
        ) : null}

        {tab() === "Runtime" ? (
          <div class="drawer-section">
            {preflight() ? (
              <div class="drawer-block">
                <strong>Runtime preflight</strong>
                <p>{preflight().summary}</p>
                <div class="drawer-meta">
                  <span>{preflight().runtime}</span>
                  <span>{preflight().preflight_state}</span>
                  <span>{preflight().preview?.supported ? "supported" : "unsupported"}</span>
                </div>
              </div>
            ) : null}
            {lifecycle() ? (
              <div class="drawer-block">
                <strong>Worker lifecycle</strong>
                <p>{lifecycle().summary}</p>
                {lifecycle().rounds.map((round: any) => (
                  <div class="drawer-row">
                    <span>Round {round.round}</span>
                    <strong>{round.status}</strong>
                    <p>
                      workers {round.worker_ok_count}/{round.worker_count}, timeout{" "}
                      {round.worker_timeout_count}
                    </p>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}
      </section>
    </aside>
  );
}
