type IssueCardProps = Record<string, any> & {
  card: any;
  selected: boolean;
  onOpen: () => void;
};

const stop = (event: Event) => event.stopPropagation();

export function IssueCard(props: IssueCardProps) {
  const card = () => props.card;
  const trace = () => card().trace;
  const doctor = () => props.cardDoctor?.(card()) ?? card().doctor;
  const pending = () => props.issuePendingLabel?.(card().issue.id);
  const decisionActions = () => props.issueDecisionActions?.(card()) ?? [];
  const primaryDecision = () => decisionActions()[0] ?? null;
  const canAdvance = () =>
    card().issue.loop_id && (card().issue.status === "Todo" || card().issue.status === "Doing");
  const runtimeLabel = () => {
    const mode = trace()?.worker_mode;
    const kind = trace()?.worker_kind;
    const runtime = mode ?? kind;
    if (runtime === "deterministic-worker") return "local";
    return runtime ?? "runtime pending";
  };
  const metaLine = () =>
    [card().issue.assignee ?? "Unassigned", runtimeLabel()].filter(Boolean).join(" / ");
  const primaryActionLabel = () => {
    if (pending()) return pending();
    if (canAdvance()) return "Advance";
    return primaryDecision()
      ? props.issueDecisionButtonLabel?.(card(), primaryDecision()) ?? primaryDecision().label
      : "Details";
  };
  const primaryActionDisabled = () =>
    Boolean(pending()) ||
    (primaryDecision() ? Boolean(props.issueOptionDisabled?.(card(), primaryDecision())) : false);
  const runPrimaryAction = () => {
    if (canAdvance()) {
      void props.advanceIssue?.(card());
      return;
    }
    if (primaryDecision()) {
      props.runIssueAction?.(card(), primaryDecision());
      return;
    }
    props.onOpen();
  };

  return (
    <article
      class={props.selected ? "issue-card issue-card--selected" : "issue-card"}
      data-testid={`issue-card-${card().issue.id}`}
      onClick={props.onOpen}
    >
      <header class="issue-card-head">
        <strong>{card().issue.title}</strong>
        <span>#{card().issue.id}</span>
      </header>

      <div class="issue-card-meta-line">{metaLine()}</div>

      <div class="issue-card-signals">
        {trace() ? (
          <>
            <span class={trace().audit_passed === false ? "signal signal--warn" : "signal"}>
              {props.auditLabel?.(trace()) ?? "audit pending"}
            </span>
            {trace().last_decision ? <span class="signal">{trace().last_decision}</span> : null}
          </>
        ) : doctor() ? (
          <span class={`signal signal--${props.doctorHealthTone?.(doctor().health) ?? "pending"}`}>
            {props.doctorHealthLabel?.(doctor().health) ?? doctor().health}
          </span>
        ) : null}
      </div>

      {props.commentComposerActive?.(card().issue.id, "board") ? (
        <div class="comment-box" onClick={stop}>
          <textarea
            aria-label={`Board issue #${card().issue.id} comment`}
            data-testid={`issue-comment-board-${card().issue.id}`}
            value={props.commentBody?.() ?? ""}
            onInput={(event: any) => props.setCommentBody?.(event.currentTarget.value)}
            onKeyDown={(event: any) => props.handleCommentKeyDown?.(event, card().issue.id)}
            placeholder="Comment"
          />
          <button
            type="button"
            aria-label={`Send board issue #${card().issue.id} comment`}
            data-testid={`issue-comment-board-send-${card().issue.id}`}
            disabled={props.commentSubmitDisabled?.(card().issue.id)}
            onClick={() => void props.addIssueComment?.(card().issue.id)}
          >
            {pending() ?? "Send"}
          </button>
          <button
            type="button"
            aria-label={`Close board issue #${card().issue.id} comment`}
            data-testid={`issue-comment-board-close-${card().issue.id}`}
            disabled={Boolean(pending())}
            onClick={() => props.closeIssueComment?.(card().issue.id)}
          >
            Close
          </button>
        </div>
      ) : (
        <div class="issue-card-actions" onClick={stop}>
          <button
            type="button"
            aria-label={`${primaryActionLabel()} issue #${card().issue.id} from board`}
            data-testid={`issue-action-board-primary-${card().issue.id}`}
            disabled={primaryActionDisabled()}
            onClick={runPrimaryAction}
          >
            {primaryActionLabel()}
          </button>
        </div>
      )}
    </article>
  );
}
