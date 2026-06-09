type DiagnosticsTab = "status" | "commands" | "notes" | "runtime";

type DiagnosticsViewProps = Record<string, any> & {
  activeTab: DiagnosticsTab;
  onSelectTab: (tab: DiagnosticsTab) => void;
};

const tabs: Array<{ key: DiagnosticsTab; label: string }> = [
  { key: "status", label: "Status" },
  { key: "commands", label: "Commands" },
  { key: "notes", label: "Notes" },
  { key: "runtime", label: "Runtime" },
];

export default function DiagnosticsView(props: DiagnosticsViewProps) {
  return (
    <section class="diagnostics-workbench" data-testid="diagnostics-view">
      <nav class="diagnostics-tabs" aria-label="Diagnostics">
        {tabs.map((tab) => (
          <button
            type="button"
            class={props.activeTab === tab.key ? "is-active" : ""}
            onClick={() => props.onSelectTab(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {props.activeTab === "status" ? (
        <div class="diagnostics-pane diagnostics-pane--status">
          <div class="diagnostics-summary">
            <div>
              <span>Schema</span>
              <strong
                class={props.status?.()?.schema?.healthy ? "diagnostic-health diagnostic-health--ok" : "diagnostic-health diagnostic-health--blocked"}
                title={props.storeSchemaTitle?.()}
              >
                {props.storeSchemaLabel?.()}
              </strong>
            </div>
            <div>
              <span>Generated</span>
              <strong>{props.status?.()?.generated_at ?? "pending"}</strong>
            </div>
          </div>

          <dl class="diagnostics-table">
            {(props.diagnosticsRows?.() ?? []).map((row: any) => (
              <div>
                <dt>{row.label}</dt>
                <dd title={row.title ?? row.value}>{row.value}</dd>
              </div>
            ))}
          </dl>
        </div>
      ) : null}

      {props.activeTab === "commands" ? (
        <div class="diagnostics-pane">
          <div class="tool-form-row">
            <input
              value={props.launcherQuery?.() ?? ""}
              onInput={(event: any) => props.setLauncherQuery?.(event.currentTarget.value)}
              placeholder="Search commands"
            />
            <button type="button" onClick={() => void props.refreshLauncherIndex?.()}>
              Refresh index
            </button>
          </div>

          <div class="tool-table" data-testid="diagnostics-commands-table">
            <div class="tool-table-head">
              <span>Name</span>
              <span>Command</span>
              <span>Source</span>
              <span>Score</span>
              <span>Actions</span>
            </div>
            {(props.launcherItems?.() ?? []).length ? (
              (props.launcherItems?.() ?? []).map((item: any) => (
                <div class="tool-table-row">
                  <strong>{item.name}</strong>
                  <code>{item.command}</code>
                  <span>{item.source}</span>
                  <span>{item.score.toFixed(2)}</span>
                  <span class="tool-row-actions">
                    <button type="button" onClick={() => void props.launchItem?.(item)}>
                      Launch
                    </button>
                    <button type="button" onClick={() => void props.pinItem?.(item)}>
                      {item.pinned ? "Unpin" : "Pin"}
                    </button>
                  </span>
                </div>
              ))
            ) : (
              <div class="tool-empty-row">No commands indexed</div>
            )}
          </div>
        </div>
      ) : null}

      {props.activeTab === "notes" ? (
        <div class="diagnostics-pane">
          <div class="tool-form-row tool-form-row--notes">
            <input
              value={props.drawerTitle?.() ?? ""}
              onInput={(event: any) => props.setDrawerTitle?.(event.currentTarget.value)}
              placeholder="Note title"
            />
            <input
              value={props.drawerBody?.() ?? ""}
              onInput={(event: any) => props.setDrawerBody?.(event.currentTarget.value)}
              placeholder="Note body"
            />
            <button type="button" onClick={() => void props.addDrawerNote?.()}>
              Create note
            </button>
          </div>

          <div class="tool-table" data-testid="diagnostics-notes-table">
            <div class="tool-table-head tool-table-head--notes">
              <span>Title</span>
              <span>Kind</span>
              <span>Storage</span>
              <span>Updated</span>
            </div>
            {(props.drawerItems?.() ?? []).length ? (
              (props.drawerItems?.() ?? []).map((item: any) => (
                <div class="tool-table-row tool-table-row--notes">
                  <strong>{item.title}</strong>
                  <span>{item.kind}</span>
                  <code>{item.storage_path ?? "db-only"}</code>
                  <span>{item.updated_at}</span>
                </div>
              ))
            ) : (
              <div class="tool-empty-row">No notes stored</div>
            )}
          </div>

          <div class="tool-table tool-table--compact" data-testid="diagnostics-note-history">
            <div class="tool-table-head tool-table-head--history">
              <span>History</span>
              <span>Commit</span>
            </div>
            {(props.drawerHistory?.()?.commits ?? []).length ? (
              (props.drawerHistory?.()?.commits ?? []).map((commit: any) => (
                <div class="tool-table-row tool-table-row--history">
                  <strong>{commit.summary}</strong>
                  <code>{commit.id}</code>
                </div>
              ))
            ) : (
              <div class="tool-empty-row">No note history</div>
            )}
          </div>
        </div>
      ) : null}

      {props.activeTab === "runtime" ? (
        <div class="diagnostics-pane">
          <div class="runtime-summary-row">
            <span>Ready {props.hiveSummary?.()?.ready_runs ?? 0}</span>
            <span>Total runs {props.hiveSummary?.()?.total_runs ?? 0}</span>
            <span>Loops {props.hiveLoops?.()?.length ?? 0}</span>
          </div>

          <div class="tool-form-row">
            <input
              value={props.hiveTitle?.() ?? ""}
              onInput={(event: any) => props.setHiveTitle?.(event.currentTarget.value)}
              placeholder="Dispatch title"
            />
            <input
              value={props.hiveProject?.() ?? ""}
              onInput={(event: any) => props.setHiveProject?.(event.currentTarget.value)}
              placeholder="Project path"
            />
            <button type="button" onClick={() => void props.dispatchHive?.()}>
              Persist dispatch
            </button>
          </div>

          <div class="tool-table" data-testid="diagnostics-loops-table">
            <div class="tool-table-head tool-table-head--runtime">
              <span>Loop</span>
              <span>Status</span>
              <span>Phase</span>
              <span>Runtime</span>
              <span>Action</span>
            </div>
            {(props.hiveLoops?.() ?? []).length ? (
              (props.hiveLoops?.() ?? []).map((loop: any) => (
                <div class="tool-table-row tool-table-row--runtime">
                  <strong>{loop.title}</strong>
                  <span>{loop.status}</span>
                  <span>{loop.active_phase} / round {loop.current_round}</span>
                  <code>{loop.runtime}</code>
                  <span class="tool-row-actions">
                    {loop.status === "todo" ? (
                      <button
                        type="button"
                        disabled={Boolean(props.loopPendingLabel?.(loop.id))}
                        onClick={() => void props.runHiveLoop?.(loop)}
                      >
                        {props.loopPendingLabel?.(loop.id) ?? "Run"}
                      </button>
                    ) : null}
                  </span>
                </div>
              ))
            ) : (
              <div class="tool-empty-row">No loops recorded</div>
            )}
          </div>

          <div class="tool-table tool-table--compact" data-testid="diagnostics-runs-table">
            <div class="tool-table-head tool-table-head--runs">
              <span>Run</span>
              <span>Status</span>
              <span>Project</span>
            </div>
            {(props.hiveRuns?.() ?? []).length ? (
              (props.hiveRuns?.() ?? []).map((run: any) => (
                <div class="tool-table-row tool-table-row--runs">
                  <strong>{run.title}</strong>
                  <span>{run.status}</span>
                  <code>{run.project_dir ?? "no project"}</code>
                </div>
              ))
            ) : (
              <div class="tool-empty-row">No dispatch runs</div>
            )}
          </div>
        </div>
      ) : null}
    </section>
  );
}
