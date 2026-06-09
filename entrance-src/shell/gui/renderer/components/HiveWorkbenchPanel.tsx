import { Match, Switch } from "solid-js";
import { IssueWorkbench } from "./hive/IssueWorkbench";
import { ReviewInbox } from "./hive/ReviewInbox";

export type HiveWorkbenchPanelProps = Record<string, any>;

export default function HiveWorkbenchPanel(props: HiveWorkbenchPanelProps) {
  return (
    <Switch>
      <Match when={props.workbenchMode === "reviews"}>
        <ReviewInbox {...props} cards={props.reviewQueueCards?.() ?? []} />
      </Match>
      <Match when={true}>
        <IssueWorkbench {...props} />
      </Match>
    </Switch>
  );
}
