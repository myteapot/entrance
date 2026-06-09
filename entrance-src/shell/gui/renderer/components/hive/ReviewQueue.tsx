import type { JSX } from "solid-js";

export type ReviewQueueProps = {
  children: JSX.Element;
};

export function ReviewQueue(props: ReviewQueueProps) {
  return <>{props.children}</>;
}
