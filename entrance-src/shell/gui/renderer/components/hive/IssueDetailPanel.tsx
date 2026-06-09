import type { JSX } from "solid-js";

export type IssueDetailPanelProps = {
  children: JSX.Element;
};

export function IssueDetailPanel(props: IssueDetailPanelProps) {
  return <>{props.children}</>;
}
