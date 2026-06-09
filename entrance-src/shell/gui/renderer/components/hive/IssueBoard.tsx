import type { JSX } from "solid-js";

export type IssueBoardProps = {
  children: JSX.Element;
};

export function IssueBoard(props: IssueBoardProps) {
  return (
    <main class="issue-board-shell" data-testid="issue-board">
      {props.children}
    </main>
  );
}
