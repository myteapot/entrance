import { IssueWorkbench } from "./hive/IssueWorkbench";

export type HiveWorkbenchPanelProps = Record<string, any>;

export default function HiveWorkbenchPanel(props: HiveWorkbenchPanelProps) {
  return <IssueWorkbench {...props} />;
}
