import { createStore, produce } from "solid-js/store";

import type { GraphUpdateEvent } from "./graphEvents";

export interface GraphNode {
  id: string;
  kind: string;
  label: string;
  detail: string;
  tone: string;
  baseTone: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  createdAt: number;
  archivedAt: number | null;
  parentId: string | null;
  instanceDepth: number | null;
  hiddenByFilter: boolean;
  isArchived: boolean;
  fx?: number | null;
  fy?: number | null;
}

export interface GraphEdge {
  source: string;
  target: string;
  kind: string;
}

export interface GraphState {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

const normalizeNodeKind = (kind: string) => kind.trim().toLowerCase();

const instanceDepthForKind = (kind: string): number | null => {
  switch (normalizeNodeKind(kind)) {
    case "nota":
      return 1;
    case "arch":
      return 2;
    case "dev":
      return 3;
    case "agent":
      return 4;
    default:
      return null;
  }
};

const resolveVisibleTone = (node: Pick<GraphNode, "baseTone" | "hiddenByFilter" | "isArchived">) =>
  node.hiddenByFilter || node.isArchived ? "archived" : node.baseTone;

const hasEdge = (edges: GraphEdge[], edge: GraphEdge) =>
  edges.some(
    (candidate) =>
      candidate.source === edge.source &&
      candidate.target === edge.target &&
      candidate.kind === edge.kind,
  );

export function createGraphStore() {
  let visibleDepth = 4;
  const [state, setState] = createStore<GraphState>({
    nodes: [
      {
        id: "nota",
        kind: "nota",
        label: "NOTA",
        detail: "Core runtime",
        tone: "nota",
        baseTone: "nota",
        x: 0,
        y: 0,
        vx: 0,
        vy: 0,
        createdAt: Date.now(),
        archivedAt: null,
        parentId: null,
        instanceDepth: null,
        hiddenByFilter: false,
        isArchived: false,
        fx: 0,
        fy: 0,
      },
    ],
    edges: [],
  });

  const addNode = (
    node: Omit<
      GraphNode,
      | "x"
      | "y"
      | "vx"
      | "vy"
      | "createdAt"
      | "archivedAt"
      | "baseTone"
      | "parentId"
      | "instanceDepth"
      | "hiddenByFilter"
      | "isArchived"
    >,
    parentId?: string,
  ) => {
    setState(
      produce((graph) => {
        const normalizedKind = normalizeNodeKind(node.kind);
        const nextInstanceDepth = instanceDepthForKind(normalizedKind);
        const hiddenByFilter =
          nextInstanceDepth !== null && nextInstanceDepth > visibleDepth;
        const existingNode = graph.nodes.find((candidate) => candidate.id === node.id);
        if (existingNode) {
          existingNode.kind = normalizedKind;
          existingNode.label = node.label;
          existingNode.detail = node.detail;
          existingNode.baseTone = node.tone;
          existingNode.parentId = parentId ?? existingNode.parentId;
          existingNode.instanceDepth = nextInstanceDepth;
          existingNode.hiddenByFilter = hiddenByFilter;
          if (node.tone !== "archived") {
            existingNode.isArchived = false;
            existingNode.archivedAt = null;
          }
          existingNode.tone = resolveVisibleTone(existingNode);
        } else {
          const parent = parentId
            ? graph.nodes.find((candidate) => candidate.id === parentId)
            : graph.nodes[0];
          const angle = Math.random() * Math.PI * 2;
          const distance = 80 + Math.random() * 60;
          graph.nodes.push({
            ...node,
            kind: normalizedKind,
            tone: hiddenByFilter ? "archived" : node.tone,
            baseTone: node.tone,
            x: (parent?.x ?? 0) + Math.cos(angle) * distance,
            y: (parent?.y ?? 0) + Math.sin(angle) * distance,
            vx: 0,
            vy: 0,
            createdAt: Date.now(),
            archivedAt: null,
            parentId: parentId ?? null,
            instanceDepth: nextInstanceDepth,
            hiddenByFilter,
            isArchived: false,
          });
        }

        if (parentId) {
          const edge = { source: parentId, target: node.id, kind: "spawn" };
          if (!hasEdge(graph.edges, edge)) {
            graph.edges.push(edge);
          }
        }
      }),
    );
  };

  const updateNodeTone = (id: string, tone: string, detail?: string) => {
    setState(
      produce((graph) => {
        const node = graph.nodes.find((candidate) => candidate.id === id);
        if (!node) {
          return;
        }

        node.baseTone = tone;
        node.tone = resolveVisibleTone(node);
        if (detail !== undefined) {
          node.detail = detail;
        }
      }),
    );
  };

  const archiveNode = (id: string) => {
    setState(
      produce((graph) => {
        const node = graph.nodes.find((candidate) => candidate.id === id);
        if (!node) {
          return;
        }

        node.isArchived = true;
        node.tone = "archived";
        node.archivedAt = Date.now();
      }),
    );
  };

  const addEdge = (edge: GraphEdge) => {
    setState(
      produce((graph) => {
        if (!hasEdge(graph.edges, edge)) {
          graph.edges.push(edge);
        }
      }),
    );
  };

  const pruneArchived = (maxAgeMs: number = 10_000) => {
    const cutoff = Date.now() - maxAgeMs;
    setState(
      produce((graph) => {
        const deadIds = new Set(
          graph.nodes
            .filter(
              (node) =>
                node.isArchived &&
                !node.hiddenByFilter &&
                node.archivedAt !== null &&
                node.archivedAt < cutoff,
            )
            .map((node) => node.id),
        );
        graph.nodes = graph.nodes.filter((node) => !deadIds.has(node.id));
        graph.edges = graph.edges.filter(
          (edge) => !deadIds.has(edge.source) && !deadIds.has(edge.target),
        );
      }),
    );
  };

  const setVisibleDepth = (nextVisibleDepth: number) => {
    visibleDepth = nextVisibleDepth;
    setState(
      produce((graph) => {
        for (const node of graph.nodes) {
          if (node.instanceDepth === null) {
            continue;
          }

          node.hiddenByFilter = node.instanceDepth > visibleDepth;
          if (!node.hiddenByFilter && !node.isArchived) {
            node.archivedAt = null;
          }
          node.tone = resolveVisibleTone(node);
        }
      }),
    );
  };

  const handleGraphEvent = (event: GraphUpdateEvent) => {
    switch (event.kind) {
      case "NodeCreated":
        if (event.id && event.node_kind) {
          addNode(
            {
              id: event.id,
              kind: event.node_kind,
              label: event.label ?? "",
              detail: event.detail ?? "",
              tone: event.tone ?? "active",
            },
            event.parent_id ?? undefined,
          );
        }
        break;
      case "NodeStateChanged":
        if (event.id) {
          updateNodeTone(event.id, event.tone ?? "steady", event.detail);
        }
        break;
      case "NodeArchived":
        if (event.id) {
          archiveNode(event.id);
        }
        break;
      case "EdgeCreated":
        if (event.source_id && event.target_id) {
          addEdge({
            source: event.source_id,
            target: event.target_id,
            kind: event.edge_kind ?? "link",
          });
        }
        break;
    }
  };

  return {
    state,
    addNode,
    updateNodeTone,
    archiveNode,
    addEdge,
    pruneArchived,
    setVisibleDepth,
    handleGraphEvent,
  };
}

export type GraphStore = ReturnType<typeof createGraphStore>;
