import { commentPreview, compactText } from "../../lib/hive";

type IssueCardProps = Record<string, any> & {
  card: any;
  selected: boolean;
  onOpen: () => void;
};

const stop = (event: Event) => event.stopPropagation();

const latestComment = (card: any) =>
  card.comments.length ? card.comments[card.comments.length - 1] : null;

export function IssueCard(props: IssueCardProps) {
  const card = () => props.card;
  const trace = () => card().trace;
  const doctor = () => props.cardDoctor?.(card()) ?? card().doctor;
  const latest = () => latestComment(card());
  const pending = () => props.issuePendingLabel?.(card().issue.id);

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

      <p>{compactText(card().issue.summary ?? "No summary", 120)}</p>

      <div class="issue-card-meta">
        <span>{card().issue.assignee ?? "No assignee"}</span>
        <span>{card().issue.claim_role ?? "No role"}</span>
        <span>{trace()?.worker_mode ?? trace()?.worker_kind ?? "runtime pending"}</span>
      </div>

      <div class="issue-card-signals">
        {doctor() ? (
          <span class={`signal signal--${props.doctorHealthTone?.(doctor().health) ?? "pending"}`}>
            {props.doctorHealthLabel?.(doctor().health) ?? doctor().health}
          </span>
        ) : null}
        {trace() ? (
          <>
            <span class={trace().audit_passed === false ? "signal signal--warn" : "signal"}>
              {props.auditLabel?.(trace()) ?? "audit pending"}
            </span>
            {trace().last_decision ? <span class="signal">{trace().last_decision}</span> : null}
            {props.scoreSummaryLabel?.(trace()) ? (
              <span class="signal">{props.scoreSummaryLabel(trace())}</span>
            ) : null}
          </>
        ) : null}
      </div>

      {latest() ? (
        <div class="issue-card-latest">
          <strong>{latest().author}</strong>
          <span>{commentPreview(latest(), 72)}</span>
        </div>
      ) : null}

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
          {card().issue.loop_id && (card().issue.status === "Todo" || card().issue.status === "Doing") ? (
            <button
              type="button"
              aria-label={`Advance issue #${card().issue.id} from board`}
              data-testid={`issue-action-board-advance-${card().issue.id}`}
              disabled={Boolean(pending())}
              onClick={() => void props.advanceIssue?.(card())}
            >
              {pending() ?? "Advance"}
            </button>
          ) : null}
          {props.issueDecisionActions?.(card()).map((action: any) => (
            <button
              type="button"
              aria-label={`${action.label} issue #${card().issue.id} from board`}
              data-testid={`issue-action-board-${action.action}-${card().issue.id}`}
              disabled={props.issueOptionDisabled?.(card(), action)}
              {...props.issueActionButtonAttrs?.(action)}
              onClick={() => props.runIssueAction?.(card(), action)}
            >
              {props.issueDecisionButtonLabel?.(card(), action)}
            </button>
          ))}
          <button
            type="button"
            aria-label={`Show issue #${card().issue.id} details`}
            data-testid={`issue-action-board-details-${card().issue.id}`}
            onClick={props.onOpen}
          >
            Details
          </button>
          <button
            type="button"
            aria-label={`Comment on issue #${card().issue.id} from board`}
            data-testid={`issue-action-board-comment-${card().issue.id}`}
            disabled={Boolean(pending())}
            {...props.issueActionButtonAttrs?.(props.issueActionByName?.(card(), "comment"))}
            onClick={() => props.openIssueComment?.(card().issue.id, "board")}
          >
            Comment
          </button>
        </div>
      )}
    </article>
  );
}
