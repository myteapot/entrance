import { invoke } from "@tauri-apps/api/core";
import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { GraphNode } from "../features/dashboard/graphStore";
import "./NodeInspector.css";

interface AgentInstanceSummary {
  id: number;
  role: string;
  status: string;
  display_name: string;
  last_heartbeat_at: string | null;
}

interface NodeInspectorProps {
  node: GraphNode | null;
  onClose: () => void;
  onOpenDialog?: () => void;
}

const formatRelativeTimestamp = (value: string) => {
  const diffMs = Date.now() - new Date(value).getTime();
  const diffMinutes = Math.round(Math.abs(diffMs) / 60_000);

  if (diffMinutes < 1) {
    return "just now";
  }

  if (diffMinutes < 60) {
    return `${diffMinutes}m ago`;
  }

  const diffHours = Math.round(diffMinutes / 60);
  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }

  const diffDays = Math.round(diffHours / 24);
  return `${diffDays}d ago`;
};

const kindLabel = (kind: string) => {
  switch (kind) {
    case "nota": return "NOTA Core";
    case "arch": return "Architect";
    case "dev": return "Developer";
    case "agent": return "Agent";
    case "allocation": return "Allocation";
    case "receipt": return "Receipt";
    case "checkpoint": return "Checkpoint";
    case "supervision": return "Supervision";
    case "dialog": return "Dialog";
    default: return kind.charAt(0).toUpperCase() + kind.slice(1);
  }
};

const toneLabel = (tone: string) => {
  switch (tone) {
    case "nota": return "Core";
    case "active": return "Active";
    case "steady": return "Steady";
    case "warming": return "Warming";
    case "caution": return "Caution";
    case "archived": return "Archived";
    default: return tone;
  }
};

const toneClass = (tone: string) => {
  switch (tone) {
    case "nota": return "tone-nota";
    case "active": return "tone-active";
    case "steady": return "tone-steady";
    case "warming": return "tone-warming";
    case "caution": return "tone-caution";
    case "archived": return "tone-archived";
    default: return "";
  }
};

const NodeInspector = (props: NodeInspectorProps) => {
  const [instances, setInstances] = createSignal<AgentInstanceSummary[]>([]);
  const [loadError, setLoadError] = createSignal<string | null>(null);

  const isInstanceKind = (kind: string) =>
    ["nota", "arch", "dev", "agent"].includes(kind);

  /* Fetch related instances when node changes */
  createEffect(() => {
    const node = props.node;
    if (!node || !isInstanceKind(node.kind)) {
      setInstances([]);
      return;
    }

    void (async () => {
      try {
        setLoadError(null);
        const allInstances = await invoke<AgentInstanceSummary[]>("list_agent_instances");
        const related = (allInstances ?? []).filter(
          (inst) => inst.role.toLowerCase() === node.kind.toLowerCase(),
        );
        setInstances(related);
      } catch (err) {
        setLoadError(err instanceof Error ? err.message : String(err));
        setInstances([]);
      }
    })();
  });

  /* Esc to close */
  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      props.onClose();
    }
  };

  createEffect(() => {
    if (props.node) {
      document.addEventListener("keydown", handleKeyDown);
      onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
    }
  });

  return (
    <Show when={props.node}>
      {(node) => (
        <aside class="node-inspector" aria-label="Node inspector">
          <header class="node-inspector__header">
            <div class="node-inspector__title-row">
              <span class={`node-inspector__dot ${toneClass(node().tone)}`} />
              <h3 class="node-inspector__name">{kindLabel(node().kind)}</h3>
              <span class={`node-inspector__tone-badge ${toneClass(node().tone)}`}>
                {toneLabel(node().tone)}
              </span>
            </div>
            <button
              type="button"
              class="node-inspector__close"
              onClick={props.onClose}
              title="Close (Esc)"
            >
              ×
            </button>
          </header>

          <div class="node-inspector__body">
            <div class="node-inspector__section">
              <span class="node-inspector__label">Label</span>
              <span class="node-inspector__value">{node().label}</span>
            </div>

            <Show when={node().detail}>
              <div class="node-inspector__section">
                <span class="node-inspector__label">Detail</span>
                <span class="node-inspector__value">{node().detail}</span>
              </div>
            </Show>

            <div class="node-inspector__section">
              <span class="node-inspector__label">Node ID</span>
              <code class="node-inspector__code">{node().id}</code>
            </div>

            <div class="node-inspector__section">
              <span class="node-inspector__label">Kind</span>
              <span class="node-inspector__value">{node().kind}</span>
            </div>

            <Show when={node().parentId}>
              <div class="node-inspector__section">
                <span class="node-inspector__label">Parent</span>
                <code class="node-inspector__code">{node().parentId}</code>
              </div>
            </Show>

            <Show when={node().instanceDepth !== null}>
              <div class="node-inspector__section">
                <span class="node-inspector__label">Instance depth</span>
                <span class="node-inspector__value">Layer {node().instanceDepth}</span>
              </div>
            </Show>
          </div>

          {/* Related instances */}
          <Show when={isInstanceKind(node().kind) && instances().length > 0}>
            <div class="node-inspector__instances">
              <span class="node-inspector__label">
                Related instances ({instances().length})
              </span>
              <ul class="node-inspector__instance-list">
                {instances().map((inst) => (
                  <li class="node-inspector__instance-item">
                    <span class={`node-inspector__instance-dot is-${inst.status.toLowerCase()}`} />
                    <span class="node-inspector__instance-name">{inst.display_name}</span>
                    <span class="node-inspector__instance-status">{inst.status}</span>
                    <Show when={inst.last_heartbeat_at}>
                      {(hb) => (
                        <span class="node-inspector__instance-heartbeat">
                          {formatRelativeTimestamp(hb())}
                        </span>
                      )}
                    </Show>
                  </li>
                ))}
              </ul>
            </div>
          </Show>

          <Show when={loadError()}>
            <p class="node-inspector__error">{loadError()}</p>
          </Show>

          {/* Actions */}
          <footer class="node-inspector__actions">
            <Show when={node().kind === "nota" && props.onOpenDialog}>
              <button
                type="button"
                class="node-inspector__btn node-inspector__btn--primary"
                onClick={() => props.onOpenDialog?.()}
              >
                Open NOTA Dialog
              </button>
            </Show>
          </footer>
        </aside>
      )}
    </Show>
  );
};

export default NodeInspector;
