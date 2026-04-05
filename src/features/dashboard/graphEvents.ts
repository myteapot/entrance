import { listen } from "@tauri-apps/api/event";

export interface GraphUpdateEvent {
  kind: "NodeCreated" | "NodeStateChanged" | "NodeArchived" | "EdgeCreated";
  id?: string;
  node_kind?: string;
  label?: string;
  parent_id?: string | null;
  detail?: string;
  tone?: string;
  source_id?: string;
  target_id?: string;
  edge_kind?: string;
}

export interface NotaDialogEvent {
  dialog_id: string;
  kind: "ApprovalRequired" | "Escalation" | "BudgetWarning" | "Info";
  title: string;
  body: string;
  context_json: string;
  allocation_id: number | null;
  transaction_id: number | null;
  actions: { action_key: string; label: string; tone: string }[];
}

export const listenToGraphUpdates = (callback: (event: GraphUpdateEvent) => void) =>
  listen<string>("graph:update", (event) => {
    try {
      callback(JSON.parse(event.payload) as GraphUpdateEvent);
    } catch {
      // Ignore malformed event payloads until backend emitters are fully wired.
    }
  });

export const listenToNotaDialogs = (callback: (event: NotaDialogEvent) => void) =>
  listen<string>("nota:dialog", (event) => {
    try {
      callback(JSON.parse(event.payload) as NotaDialogEvent);
    } catch {
      // Ignore malformed event payloads until backend emitters are fully wired.
    }
  });

export interface SystemPulseEvent {
  health: "Green" | "Yellow" | "Red";
  agent_tier: string;
  active_instances: number;
  stale_instances: number;
  stopped_instances: number;
  active_tasks: number;
  stale_tasks: number;
  pending_approvals?: number;
  pending_work?: number;
  total_instances?: number;
  tick_interval_secs?: number;
  stale_threshold_multiplier?: number;
}

export const listenToSystemPulse = (callback: (event: SystemPulseEvent) => void) =>
  listen<string>("system:pulse", (event) => {
    try {
      callback(JSON.parse(event.payload) as SystemPulseEvent);
    } catch {
      // Ignore malformed event payloads until backend emitters are fully wired.
    }
  });
