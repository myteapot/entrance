import type { JSX } from "solid-js";

export type IssueBoardProps = {
  total: number;
  children: JSX.Element;
};

export function IssueBoard(props: IssueBoardProps) {
  return (
    <main class="issue-board-shell" data-testid="issue-board">
      <div class="issue-board-head">
        <strong>Status board</strong>
        <span>{props.total} issues</span>
      </div>
      <div class="issue-board-columns">{props.children}</div>
    </main>
  );
}
