import { createMemo, createSignal, Show } from "solid-js";
import { ISSUE_STATUSES, issueStatusTestId } from "../../lib/hive";
import { IssueBoard } from "./IssueBoard";
import { IssueColumn } from "./IssueColumn";
import { IssueDetailDrawer } from "./IssueDetailDrawer";
import { ReviewQueue } from "./ReviewQueue";

type IssueWorkbenchProps = Record<string, any>;

type IssueGroup = {
  label: string;
  cards: any[];
};

const assigneeLabel = (card: any) => card.issue.assignee ?? "No assignee";

const sortGroupLabels = (left: string, right: string) => {
  if (left === "No assignee") return right === "No assignee" ? 0 : 1;
  if (right === "No assignee") return -1;
  return left.localeCompare(right);
};

export function IssueWorkbench(props: IssueWorkbenchProps) {
  const [drawerOpen, setDrawerOpen] = createSignal(false);
  const mode = () => props.workbenchMode ?? "issues";
  const cards = createMemo(() => props.issueCards?.() ?? []);
  const reviewCards = createMemo(() => props.reviewQueueCards?.() ?? []);
  const selectedCard = createMemo(() => props.selectedIssueCard?.() ?? null);
  const selectedIssueId = createMemo(() => selectedCard()?.issue.id ?? null);

  const cardsForStatus = (statusName: string) =>
    props.issueCardsForStatus?.(statusName) ??
    cards().filter((card: any) => card.issue.status === statusName);

  const groupsForStatus = (statusName: string): IssueGroup[] => {
    const groups = new Map<string, any[]>();
    for (const card of cardsForStatus(statusName)) {
      const label = assigneeLabel(card);
      groups.set(label, [...(groups.get(label) ?? []), card]);
    }
    return [...groups.entries()]
      .sort(([left], [right]) => sortGroupLabels(left, right))
      .map(([label, groupCards]) => ({ label, cards: groupCards }));
  };

  const openCard = (card: any) => {
    props.setSelectedIssueId?.(card.issue.id);
    setDrawerOpen(true);
  };

  const closeDrawer = () => setDrawerOpen(false);

  const hiddenStatuses = createMemo(() =>
    ISSUE_STATUSES.filter((statusName) => cardsForStatus(statusName).length === 0),
  );

  return (
    <section class="issue-workbench" data-testid="issue-workbench" data-mode={mode()}>
      <div class="workbench-toolbar">
        <span>
          {cards().length} issues
          {reviewCards().length ? ` / ${reviewCards().length} need decision` : ""}
        </span>
        <div class="workbench-actions">
          <button
            type="button"
            data-testid="panel-run-demo"
            disabled={Boolean(props.pendingDemoAction?.())}
            onClick={() => void props.startDemoLoop?.()}
          >
            {props.pendingDemoAction?.() ?? "Run Demo"}
          </button>
          <button type="button" onClick={() => void props.createHiveLoop?.()}>
            Create Loop
          </button>
        </div>
      </div>

      <div class="workbench-commandbar">
        <input
          aria-label="Loop title"
          value={props.loopTitle?.() ?? ""}
          onInput={(event: any) => props.setLoopTitle?.(event.currentTarget.value)}
          placeholder="Loop title"
        />
        <input
          aria-label="Loop goal"
          value={props.loopGoal?.() ?? ""}
          onInput={(event: any) => props.setLoopGoal?.(event.currentTarget.value)}
          placeholder="Goal"
        />
        <select
          aria-label="Loop runtime"
          value={props.loopRuntime?.() ?? "codex"}
          onChange={(event: any) => props.setLoopRuntime?.(event.currentTarget.value)}
        >
          <option value="codex">codex</option>
          <option value="local">local</option>
        </select>
        <input
          aria-label="Worker timeout seconds"
          type="number"
          min="1"
          value={props.loopWorkerTimeoutSecs?.() ?? ""}
          onInput={(event: any) => props.setLoopWorkerTimeoutSecs?.(event.currentTarget.value)}
          placeholder="Timeout"
        />
        <input
          aria-label="Worker attempts"
          type="number"
          min="1"
          max="3"
          value={props.loopWorkerAttempts?.() ?? ""}
          onInput={(event: any) => props.setLoopWorkerAttempts?.(event.currentTarget.value)}
          placeholder="Attempts"
        />
      </div>

      <ReviewQueue {...props} cards={reviewCards()} onOpenIssue={openCard} />

      <div class={drawerOpen() ? "workbench-layout workbench-layout--drawer" : "workbench-layout"}>
        <IssueBoard total={cards().length}>
          {ISSUE_STATUSES.map((statusName) => (
            <IssueColumn
              statusName={statusName}
              testId={`issue-column-${issueStatusTestId(statusName)}`}
              groups={groupsForStatus(statusName)}
              count={cardsForStatus(statusName).length}
              selectedIssueId={selectedIssueId()}
              onOpenIssue={openCard}
              emptyAction={
                statusName === "Todo" && cards().length === 0
                  ? {
                      label: props.pendingDemoAction?.() ?? "Run Demo",
                      disabled: Boolean(props.pendingDemoAction?.()),
                      onClick: () => void props.startDemoLoop?.(),
                    }
                  : null
              }
              cardProps={props}
            />
          ))}
        </IssueBoard>

        <aside class="hidden-columns" aria-label="Hidden columns">
          <strong>Hidden columns</strong>
          {(hiddenStatuses().length ? hiddenStatuses() : ["Todo", "Blocked", "Canceled"]).map(
            (statusName) => (
              <div class="hidden-column-pill">
                <span>{statusName}</span>
                <small>{cardsForStatus(statusName).length}</small>
              </div>
            ),
          )}
        </aside>

        <Show
          when={drawerOpen() && selectedCard()}
          fallback={
            <aside class="issue-detail-drawer issue-detail-drawer--empty">
              <strong>Select an issue</strong>
              <span>Open a card to inspect comments, evidence, timeline, review, and runtime state.</span>
            </aside>
          }
        >
          {(card) => (
            <IssueDetailDrawer
              {...props}
              card={card()}
              onClose={closeDrawer}
            />
          )}
        </Show>
      </div>
    </section>
  );
}
