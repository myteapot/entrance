import { createMemo, createSignal, Show } from "solid-js";
import { ISSUE_STATUSES } from "../../lib/hive";
import { IssueBoard } from "./IssueBoard";
import { IssueCard } from "./IssueCard";
import { IssueDetailDrawer } from "./IssueDetailDrawer";

type IssueWorkbenchProps = Record<string, any>;

const coreStatuses = new Set(["Todo", "Doing", "Needs Review", "Done"]);
const assigneeLabel = (card: any) => card.issue.assignee ?? "No assignee";

const sortGroupLabels = (left: string, right: string) => {
  if (left === "No assignee") return right === "No assignee" ? 0 : 1;
  if (right === "No assignee") return -1;
  return left.localeCompare(right);
};

export function IssueWorkbench(props: IssueWorkbenchProps) {
  const [drawerOpen, setDrawerOpen] = createSignal(false);
  const [composerOpen, setComposerOpen] = createSignal(false);
  const cards = createMemo(() => props.issueCards?.() ?? []);
  const selectedCard = createMemo(() => props.selectedIssueCard?.() ?? null);
  const selectedIssueId = createMemo(() => selectedCard()?.issue.id ?? null);

  const cardsForStatus = (statusName: string) =>
    props.issueCardsForStatus?.(statusName) ??
    cards().filter((card: any) => card.issue.status === statusName);

  const visibleStatuses = createMemo(() =>
    ISSUE_STATUSES.filter((statusName) => coreStatuses.has(statusName) || cardsForStatus(statusName).length > 0),
  );
  const hiddenStatuses = createMemo(() =>
    ISSUE_STATUSES.filter((statusName) => !visibleStatuses().includes(statusName)),
  );
  const swimlaneGroups = createMemo(() => {
    const groups = new Map<string, any[]>();
    for (const card of cards()) {
      const label = assigneeLabel(card);
      groups.set(label, [...(groups.get(label) ?? []), card]);
    }
    if (!groups.size) groups.set("No assignee", []);
    return [...groups.entries()]
      .sort(([left], [right]) => sortGroupLabels(left, right))
      .map(([label, groupCards]) => ({ label, cards: groupCards }));
  });

  const groupCardsForStatus = (groupCards: any[], statusName: string) =>
    groupCards.filter((card) => card.issue.status === statusName);

  const openCard = (card: any) => {
    props.setSelectedIssueId?.(card.issue.id);
    setDrawerOpen(true);
  };

  const closeDrawer = () => setDrawerOpen(false);

  const createIssue = async () => {
    await props.createHiveLoop?.();
    setComposerOpen(false);
  };

  return (
    <section class="issue-workbench" data-testid="issue-workbench" data-mode="issues">
      <div class="workbench-toolbar">
        <span>{cards().length} issues</span>
        <div class="workbench-actions">
          <button
            type="button"
            data-testid="issue-new-button"
            onClick={() => setComposerOpen((current) => !current)}
          >
            New issue
          </button>
        </div>
      </div>

      <Show when={composerOpen()}>
        <div class="issue-composer" data-testid="issue-composer">
          <input
            aria-label="Loop title"
            value={props.loopTitle?.() ?? ""}
            onInput={(event: any) => props.setLoopTitle?.(event.currentTarget.value)}
            placeholder="Issue title"
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
          <button type="button" onClick={() => void createIssue()}>
            Create
          </button>
          <button type="button" onClick={() => setComposerOpen(false)}>
            Cancel
          </button>
        </div>
      </Show>

      <div class={drawerOpen() ? "workbench-layout workbench-layout--drawer" : "workbench-layout"}>
        <IssueBoard>
          <div class="issue-board-view">
            <div class="issue-status-grid" style={`--status-count: ${visibleStatuses().length}`}>
              {visibleStatuses().map((statusName) => (
                <header class="issue-status-head" data-testid={`issue-column-${statusName.toLowerCase().replace(/\s+/g, "-")}`}>
                  <div>
                    <span class={`status-dot status-dot--${statusName.toLowerCase().replace(/\s+/g, "-")}`} />
                    <strong>{statusName}</strong>
                  </div>
                  <span>{cardsForStatus(statusName).length}</span>
                </header>
              ))}
            </div>

            <div class="issue-swimlanes">
              {swimlaneGroups().map((group) => (
                <section class="issue-swimlane">
                  <header class="issue-swimlane-head">
                    <span>{group.label}</span>
                    <small>{group.cards.length}</small>
                  </header>
                  <div class="issue-swimlane-grid" style={`--status-count: ${visibleStatuses().length}`}>
                    {visibleStatuses().map((statusName) => (
                      <div class="issue-swimlane-cell" data-status={statusName}>
                        {groupCardsForStatus(group.cards, statusName).map((card) => (
                          <IssueCard
                            {...props}
                            card={card}
                            selected={selectedIssueId() === card.issue.id}
                            onOpen={() => openCard(card)}
                          />
                        ))}
                      </div>
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </div>
        </IssueBoard>

        <aside class="hidden-columns hidden-columns--rail" aria-label="Hidden columns">
          <strong>Hidden</strong>
          {hiddenStatuses().length ? (
            hiddenStatuses().map((statusName) => (
              <div class="hidden-column-pill">
                <span>{statusName}</span>
                <small>{cardsForStatus(statusName).length}</small>
              </div>
            ))
          ) : (
            <span class="hidden-column-empty">None</span>
          )}
        </aside>

        <Show when={drawerOpen() && selectedCard()}>
          {(card) => <IssueDetailDrawer {...props} card={card()} onClose={closeDrawer} />}
        </Show>
      </div>
    </section>
  );
}
