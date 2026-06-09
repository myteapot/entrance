type ReviewQueueProps = Record<string, any> & {
  cards: any[];
  onOpenIssue: (card: any) => void;
};

export function ReviewQueue(props: ReviewQueueProps) {
  if (!props.cards.length && props.workbenchMode !== "reviews") {
    return null;
  }

  return (
    <section class="review-queue" data-testid="review-queue">
      <header class="review-queue-head">
        <div>
          <strong>Review queue</strong>
          <span>{props.cards.length ? "Needs decision" : "Clear"}</span>
        </div>
        <span>Blocked / Needs Review</span>
      </header>
      {props.cards.length ? (
        <div class="review-queue-list">
          {props.cards.map((card: any) => (
            <article class="review-queue-item" data-testid={`review-queue-issue-${card.issue.id}`}>
              <div class="review-queue-item-head">
                <div>
                  <strong>{card.issue.title}</strong>
                  <span>#{card.issue.id}</span>
                </div>
                <span>{props.reviewQueueDecisionLabel?.(card) ?? card.issue.status}</span>
              </div>
              <p>{props.reviewQueueBlockerLabel?.(card) ?? card.issue.summary ?? "No blocker summary"}</p>
              <div class="review-queue-evidence">
                {(props.reviewQueueEvidence?.(card) ?? []).slice(0, 4).map((evidence: any) => (
                  <button
                    type="button"
                    data-testid={`review-queue-evidence-${card.issue.id}-${evidence.id}`}
                    onClick={() => props.focusEvidence?.(evidence.id)}
                  >
                    {evidence.kind} #{evidence.id}
                  </button>
                ))}
              </div>
              <div class="issue-card-actions">
                {(props.issueDecisionActions?.(card) ?? []).map((action: any) => (
                  <button
                    type="button"
                    data-testid={`review-queue-action-${action.action}-${card.issue.id}`}
                    disabled={props.issueOptionDisabled?.(card, action)}
                    {...props.issueActionButtonAttrs?.(action)}
                    onClick={() => props.runIssueAction?.(card, action)}
                  >
                    {props.issueDecisionButtonLabel?.(card, action)}
                  </button>
                ))}
                <button
                  type="button"
                  data-testid={`review-queue-action-details-${card.issue.id}`}
                  onClick={() => props.onOpenIssue(card)}
                >
                  Details
                </button>
              </div>
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}
