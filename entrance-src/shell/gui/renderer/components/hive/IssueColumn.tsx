import { IssueCard } from "./IssueCard";

type IssueColumnProps = {
  statusName: string;
  testId: string;
  groups: Array<{ label: string; cards: any[] }>;
  count: number;
  selectedIssueId: number | null;
  onOpenIssue: (card: any) => void;
  emptyAction: {
    label: string;
    disabled: boolean;
    onClick: () => void;
  } | null;
  cardProps: Record<string, any>;
};

export function IssueColumn(props: IssueColumnProps) {
  return (
    <section
      class={`issue-column${props.count > 0 ? " issue-column--has-cards" : ""}`}
      data-testid={props.testId}
    >
      <header class="issue-column-head">
        <div>
          <span class={`status-dot status-dot--${props.statusName.toLowerCase().replace(/\s+/g, "-")}`} />
          <strong>{props.statusName}</strong>
        </div>
        <span>{props.count}</span>
      </header>

      <div class="issue-column-body">
        {props.groups.length ? (
          props.groups.map((group) => (
            <section class="assignee-group">
              <header class="assignee-group-head">
                <span>{group.label}</span>
                <small>{group.cards.length}</small>
              </header>
              {group.cards.map((card) => (
                <IssueCard
                  {...props.cardProps}
                  card={card}
                  selected={props.selectedIssueId === card.issue.id}
                  onOpen={() => props.onOpenIssue(card)}
                />
              ))}
            </section>
          ))
        ) : (
          <div class="issue-empty">
            <span>No issues</span>
            {props.emptyAction ? (
              <button
                type="button"
                data-testid="issue-empty-run-demo"
                disabled={props.emptyAction.disabled}
                onClick={props.emptyAction.onClick}
              >
                {props.emptyAction.label}
              </button>
            ) : null}
          </div>
        )}
      </div>
    </section>
  );
}
