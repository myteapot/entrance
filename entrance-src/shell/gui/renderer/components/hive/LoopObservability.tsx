type LoopObservabilityProps = Record<string, any> & {
  card: any;
};

const first = (values: Array<string | null | undefined>) => values.find((value) => value);

export function LoopObservability(props: LoopObservabilityProps) {
  const dashboard = () => props.selectedIssueLoopDashboard?.() ?? props.selectedLoopDashboard?.();
  const policy = () => props.selectedIssueTransitionPolicy?.() ?? props.selectedTransitionPolicy?.();
  const preflight = () => props.selectedIssueRuntimePreflight?.() ?? props.selectedRuntimePreflight?.();
  const lifecycle = () => props.selectedIssueWorkerLifecycle?.() ?? props.selectedWorkerLifecycle?.();
  const control = () => props.selectedIssueLoopControl?.() ?? props.selectedLoopControl?.();
  const trace = () => props.card.trace;
  const primaryAction = () =>
    first([
      policy()?.next_actions?.[0],
      dashboard()?.primary_next_action,
      control()?.state?.primary_action,
      trace()?.human_options?.[0],
    ]);

  return (
    <section class="observability-summary">
      <div class="observability-card">
        <span>Next action</span>
        <strong>{primaryAction() ?? "none"}</strong>
      </div>
      <div class="observability-card">
        <span>Review</span>
        <strong>{dashboard()?.reviewer?.decision ?? trace()?.last_decision ?? "pending"}</strong>
      </div>
      <div class="observability-card">
        <span>Audit</span>
        <strong>{trace() ? props.auditLabel?.(trace()) : dashboard()?.health?.health ?? "pending"}</strong>
      </div>
      <div class="observability-card">
        <span>Runtime</span>
        <strong>{preflight()?.runtime ?? dashboard()?.runtime ?? trace()?.worker_mode ?? "pending"}</strong>
      </div>
      <div class="observability-card">
        <span>Workers</span>
        <strong>
          {lifecycle()?.current
            ? `${lifecycle().current.worker_ok_count}/${lifecycle().current.worker_count}`
            : props.roleWorkerLabel?.(props.card) ?? "pending"}
        </strong>
      </div>
    </section>
  );
}
