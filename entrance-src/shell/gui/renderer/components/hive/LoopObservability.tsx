import type { JSX } from "solid-js";

export type LoopObservabilityProps = {
  children: JSX.Element;
};

export function LoopObservability(props: LoopObservabilityProps) {
  return <>{props.children}</>;
}
