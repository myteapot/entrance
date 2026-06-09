import type { JSX } from "solid-js";

export type IssueBoardProps = {
  children: JSX.Element;
};

export function IssueBoard(props: IssueBoardProps) {
  return <>{props.children}</>;
}
