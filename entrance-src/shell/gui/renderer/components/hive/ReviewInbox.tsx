import { createMemo, createSignal, Show } from "solid-js";
import { IssueDetailDrawer } from "./IssueDetailDrawer";

type ReviewInboxProps = Record<string, any> & {
  cards: any[];
};

const latestEvidenceSummary = (card: any) => {
  const items = card.trace?.evidence ?? [];
  const evidence = items[items.length - 1];
  if (!evidence) return "No evidence yet";
  return evidence.summary || `${evidence.kind} #${evidence.id}`;
};

export function ReviewInbox(props: ReviewInboxProps) {
  const [drawerOpen, setDrawerOpen] = createSignal(false);
  const selectedCard = createMemo(() => props.selectedIssueCard?.() ?? props.cards[0] ?? null);

  const openCard = (card: any) => {
    props.setSelectedIssueId?.(card.issue.id);
    setDrawerOpen(true);
  };

  const closeDrawer = () => setDrawerOpen(false);

  return (
    <section class="review-inbox" data-testid="review-inbox">
      <div class="review-inbox-layout">
        <main class="review-inbox-main">
          <header class="review-inbox-head">
            <strong>Review inbox</strong>
            <span>{props.cards.length} pending</span>
          </header>

          {props.cards.length ? (
            <div class="review-inbox-list">
              {props.cards.map((card: any) => (
                <article
                  class="review-inbox-row"
                  data-testid={`review-inbox-issue-${card.issue.id}`}
                  onClick={() => openCard(card)}
                >
                  <div class="review-row-title">
                    <strong>{card.issue.title}</strong>
                    <span>#{card.issue.id}</span>
                  </div>
                  <div class="review-row-status">
                    <span>{card.issue.status}</span>
                    <small>{props.reviewQueueDecisionLabel?.(card) ?? card.trace?.last_decision ?? "pending"}</small>
                  </div>
                  <p>{props.reviewQueueBlockerLabel?.(card) ?? latestEvidenceSummary(card)}</p>
                  <div class="review-row-evidence" onClick={(event) => event.stopPropagation()}>
                    {(props.reviewQueueEvidence?.(card) ?? []).slice(0, 3).map((evidence: any) => (
                      <button
                        type="button"
                        data-testid={`review-inbox-evidence-${card.issue.id}-${evidence.id}`}
                        onClick={() => props.focusEvidence?.(card.issue.id, evidence.id)}
                      >
                        {evidence.kind} #{evidence.id}
                      </button>
                    ))}
                  </div>
                  <div class="review-row-actions" onClick={(event) => event.stopPropagation()}>
                    {(props.issueDecisionActions?.(card) ?? []).slice(0, 2).map((action: any) => (
                      <button
                        type="button"
                        data-testid={`review-inbox-action-${action.action}-${card.issue.id}`}
                        disabled={props.issueOptionDisabled?.(card, action)}
                        {...props.issueActionButtonAttrs?.(action)}
                        onClick={() => props.runIssueAction?.(card, action)}
                      >
                        {props.issueDecisionButtonLabel?.(card, action)}
                      </button>
                    ))}
                    <button
                      type="button"
                      data-testid={`review-inbox-action-details-${card.issue.id}`}
                      onClick={() => openCard(card)}
                    >
                      Details
                    </button>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <div class="review-empty" data-testid="review-inbox-empty">No reviews pending</div>
          )}
        </main>

        <Show when={drawerOpen() && selectedCard()}>
          {(card) => <IssueDetailDrawer {...props} card={card()} onClose={closeDrawer} />}
        </Show>
      </div>
    </section>
  );
}
