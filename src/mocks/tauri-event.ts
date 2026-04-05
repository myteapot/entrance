/**
 * Mock for @tauri-apps/api/event — browser-mode development.
 *
 * Provides `listen()` and `emit()` stubs. For graph events, periodically
 * emits fake GraphUpdateEvent payloads so the ComputeGraph shows live nodes.
 */

type EventCallback<T> = (event: { payload: T; event: string; id: number }) => void;
type UnlistenFn = () => void;

const listeners = new Map<string, Set<EventCallback<unknown>>>();
let nextEventId = 1;

// Simulate graph:update events so the compute graph "grows"
const MOCK_GRAPH_NODES = [
  {
    kind: "NodeCreated",
    id: "mock-alloc-1",
    node_kind: "allocation",
    label: "Evidence Pipeline",
    parent_id: "nota",
    detail: "V1-alpha evidence collection",
    tone: "steady",
  },
  {
    kind: "NodeCreated",
    id: "mock-alloc-2",
    node_kind: "allocation",
    label: "ReturnRoute Exec",
    parent_id: "nota",
    detail: "Return route state changes",
    tone: "steady",
  },
  {
    kind: "NodeCreated",
    id: "mock-alloc-3",
    node_kind: "allocation",
    label: "Supervision Restart",
    parent_id: "nota",
    detail: "RestartChild atomic transaction",
    tone: "active",
  },
  {
    kind: "NodeStateChanged",
    id: "mock-alloc-1",
    tone: "archived",
    detail: "Merged to main",
  },
  {
    kind: "NodeCreated",
    id: "mock-alloc-4",
    node_kind: "allocation",
    label: "Mock Bridge",
    parent_id: "nota",
    detail: "Browser dev mode IPC mock",
    tone: "active",
  },
  {
    kind: "EdgeCreated",
    source_id: "mock-alloc-3",
    target_id: "mock-alloc-4",
    edge_kind: "depends_on",
  },
];

let graphEmitIndex = 0;
let graphInterval: ReturnType<typeof setInterval> | null = null;

function startGraphEmitter() {
  if (graphInterval) return;
  graphInterval = setInterval(() => {
    const eventSet = listeners.get("graph:update");
    if (!eventSet || eventSet.size === 0) return;

    if (graphEmitIndex >= MOCK_GRAPH_NODES.length) return;

    const payload = JSON.stringify(MOCK_GRAPH_NODES[graphEmitIndex]!);
    for (const cb of eventSet) {
      cb({ payload, event: "graph:update", id: nextEventId++ });
    }
    graphEmitIndex++;
  }, 5000);
}

export async function listen<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  const set = listeners.get(event)!;
  set.add(handler as EventCallback<unknown>);

  // Start graph emitter when someone listens to graph:update
  if (event === "graph:update") {
    startGraphEmitter();
  }

  console.debug(`[mock] listen("${event}") registered`);

  return () => {
    set.delete(handler as EventCallback<unknown>);
    if (set.size === 0) {
      listeners.delete(event);
    }
  };
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  console.debug(`[mock] emit("${event}")`, payload);
  const set = listeners.get(event);
  if (set) {
    for (const cb of set) {
      cb({ payload, event, id: nextEventId++ });
    }
  }
}

export async function once<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  const unlisten = await listen<T>(event, (e) => {
    handler(e);
    unlisten();
  });
  return unlisten;
}

export class TauriEvent {
  static readonly WINDOW_RESIZED = "tauri://resize";
  static readonly WINDOW_MOVED = "tauri://move";
  static readonly WINDOW_CLOSE_REQUESTED = "tauri://close-requested";
  static readonly WINDOW_DESTROYED = "tauri://destroyed";
  static readonly WINDOW_FOCUS = "tauri://focus";
  static readonly WINDOW_BLUR = "tauri://blur";
  static readonly WINDOW_SCALE_FACTOR_CHANGED = "tauri://scale-change";
  static readonly WINDOW_THEME_CHANGED = "tauri://theme-changed";
}
