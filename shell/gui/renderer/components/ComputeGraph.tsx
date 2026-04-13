import { createEffect, onCleanup, onMount, createSignal } from "solid-js";

import { createGraphSimulation, type SimEdge, type SimNode } from "../features/dashboard/graphEngine";
import type { GraphStore } from "../features/dashboard/graphStore";
import "./ComputeGraph.css";

export type GraphLayoutMode = "force" | "tree";

interface ComputeGraphProps {
  store: GraphStore;
  onNodeSelect?: (nodeId: string) => void;
  onNodeAction?: (nodeId: string, kind: string) => void;
  selectedNodeId?: string | null;
  layoutMode?: GraphLayoutMode;
}

/* ── Color palette (Carbon — desaturated) ─────────────── */
const TONE_COLORS: Record<string, string> = {
  nota:     "#7c83c9",
  active:   "#5a9e82",
  steady:   "#7a8a5e",
  warming:  "#9e8a4a",
  caution:  "#9e5a5a",
  archived: "rgba(250,250,250,0.12)",
};
const DEFAULT_NODE_COLOR = "rgba(250,250,250,0.18)";
const LABEL_COLOR_PRIMARY = "#c8ccd4";
const LABEL_COLOR_SECONDARY = "rgba(250,250,250,0.40)";

const EDGE_COLOR_ACTIVE = "rgba(124,131,201,0.30)";
const EDGE_COLOR_DEFAULT = "rgba(255,255,255,0.08)";

/* ── Node initials for in-circle rendering ─────────────── */
const NODE_INITIALS: Record<string, string> = {
  nota: "N",
  arch: "A",
  dev:  "D",
  agent: "Ag",
};

/* ── Tree layout constants ─────────────────────────────── */
const TREE_LAYER_GAP = 100;
const TREE_NODE_GAP = 120;
const ARROW_SIZE = 6;

/* ── Tree layout engine ────────────────────────────────── */
interface TreeNode {
  id: string;
  children: TreeNode[];
  node: SimNode;
  x: number;
  y: number;
}

function computeTreeLayout(nodes: SimNode[], edges: SimEdge[]) {
  // Build adjacency from edges (source → children)
  const childrenMap = new Map<string, string[]>();
  const hasParent = new Set<string>();

  for (const edge of edges) {
    const sourceId = typeof edge.source === "object" ? (edge.source as SimNode).id : String(edge.source);
    const targetId = typeof edge.target === "object" ? (edge.target as SimNode).id : String(edge.target);
    if (!childrenMap.has(sourceId)) {
      childrenMap.set(sourceId, []);
    }
    childrenMap.get(sourceId)!.push(targetId);
    hasParent.add(targetId);
  }

  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  // Find root(s): nodes with no incoming edges
  const roots = nodes.filter((n) => !hasParent.has(n.id));
  if (roots.length === 0 && nodes.length > 0) {
    roots.push(nodes[0]);
  }

  // Build tree recursively
  const visited = new Set<string>();
  function buildTree(nodeId: string): TreeNode | null {
    if (visited.has(nodeId)) return null;
    visited.add(nodeId);
    const node = nodeMap.get(nodeId);
    if (!node) return null;

    const childIds = childrenMap.get(nodeId) ?? [];
    const children = childIds
      .map((cid) => buildTree(cid))
      .filter((t): t is TreeNode => t !== null);

    return { id: nodeId, children, node, x: 0, y: 0 };
  }

  const forest = roots
    .map((r) => buildTree(r.id))
    .filter((t): t is TreeNode => t !== null);

  // Also add orphans (no edges at all)
  for (const n of nodes) {
    if (!visited.has(n.id)) {
      forest.push({ id: n.id, children: [], node: n, x: 0, y: 0 });
    }
  }

  // Assign positions: each tree gets laid out left to right, layers top to bottom
  let globalOffsetX = 0;

  function measureWidth(tree: TreeNode): number {
    if (tree.children.length === 0) return 1;
    return tree.children.reduce((sum, c) => sum + measureWidth(c), 0);
  }

  function assignPositions(tree: TreeNode, layer: number, leftX: number): number {
    tree.y = layer * TREE_LAYER_GAP;
    const width = measureWidth(tree);

    if (tree.children.length === 0) {
      tree.x = leftX * TREE_NODE_GAP;
      return width;
    }

    let currentLeft = leftX;
    for (const child of tree.children) {
      const childWidth = assignPositions(child, layer + 1, currentLeft);
      currentLeft += childWidth;
    }

    // Center parent over children
    const firstChild = tree.children[0];
    const lastChild = tree.children[tree.children.length - 1];
    tree.x = (firstChild.x + lastChild.x) / 2;
    return width;
  }

  for (const tree of forest) {
    const width = measureWidth(tree);
    assignPositions(tree, 0, globalOffsetX);
    globalOffsetX += width;
  }

  // Flatten to position map
  const positions = new Map<string, { x: number; y: number }>();

  function flatten(tree: TreeNode) {
    positions.set(tree.id, { x: tree.x, y: tree.y });
    for (const child of tree.children) {
      flatten(child);
    }
  }

  for (const tree of forest) {
    flatten(tree);
  }

  // Center all positions around 0,0
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  for (const pos of positions.values()) {
    minX = Math.min(minX, pos.x);
    maxX = Math.max(maxX, pos.x);
    minY = Math.min(minY, pos.y);
    maxY = Math.max(maxY, pos.y);
  }
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  for (const pos of positions.values()) {
    pos.x -= cx;
    pos.y -= cy;
  }

  return positions;
}

const ComputeGraph = (props: ComputeGraphProps) => {
  let canvasRef: HTMLCanvasElement | undefined;
  let animationFrameId: number | undefined;
  let currentNodes: SimNode[] = [];
  let currentEdges: SimEdge[] = [];
  let width = 0;
  let height = 0;
  let pixelRatio = 1;
  let zoom = 1;
  let panX = 0;
  let panY = 0;
  let hoveredNodeId: string | null = null;
  let draggedNodeId: string | null = null;
  let isDragging = false;
  let lastMouseX = 0;
  let lastMouseY = 0;
  let didDrag = false;

  /* ── Tree layout positions (only used in tree mode) ──── */
  let treePositions = new Map<string, { x: number; y: number }>();

  /* ── Click detection state ───────────────────────────── */
  let lastClickNodeId: string | null = null;
  let lastClickTime = 0;
  const DOUBLE_CLICK_MS = 300;

  const [tooltip, setTooltip] = createSignal<{ x: number; y: number; node: SimNode | null }>({
    x: 0,
    y: 0,
    node: null,
  });

  const simulation = createGraphSimulation((nodes, edges) => {
    currentNodes = nodes;
    currentEdges = edges;
  });

  const layoutMode = () => props.layoutMode ?? "force";

  const getNodeX = (node: SimNode): number => {
    if (layoutMode() === "tree") {
      const pos = treePositions.get(node.id);
      return pos?.x ?? 0;
    }
    return node.x ?? 0;
  };

  const getNodeY = (node: SimNode): number => {
    if (layoutMode() === "tree") {
      const pos = treePositions.get(node.id);
      return pos?.y ?? 0;
    }
    return node.y ?? 0;
  };

  const screenToGraph = (screenX: number, screenY: number) => ({
    x: (screenX - width / 2 - panX) / zoom,
    y: (screenY - height / 2 - panY) / zoom,
  });

  const nodeRadius = (kind: string) => {
    if (kind === "nota") return 22;
    if (kind === "arch") return 18;
    if (kind === "dev") return 14;
    if (kind === "agent") return 10;
    if (kind === "allocation") return 12;
    if (kind === "receipt") return 8;
    if (kind === "checkpoint") return 14;
    if (kind === "supervision") return 10;
    if (kind === "dialog") return 12;
    return 14;
  };

  const findNodeAt = (graphX: number, graphY: number): SimNode | null => {
    for (let index = currentNodes.length - 1; index >= 0; index -= 1) {
      const node = currentNodes[index];
      const radius = nodeRadius(node.kind);
      const nx = getNodeX(node);
      const ny = getNodeY(node);
      const dx = nx - graphX;
      const dy = ny - graphY;
      if (dx * dx + dy * dy <= radius * radius) {
        return node;
      }
    }

    return null;
  };

  /* ── Draw an arrowhead at the end of a line ──────────── */
  const drawArrow = (ctx: CanvasRenderingContext2D, fromX: number, fromY: number, toX: number, toY: number, targetRadius: number) => {
    const angle = Math.atan2(toY - fromY, toX - fromX);
    // Stop at the edge of the target circle
    const endX = toX - Math.cos(angle) * targetRadius;
    const endY = toY - Math.sin(angle) * targetRadius;

    const size = ARROW_SIZE / zoom;
    ctx.beginPath();
    ctx.moveTo(endX, endY);
    ctx.lineTo(
      endX - size * Math.cos(angle - Math.PI / 6),
      endY - size * Math.sin(angle - Math.PI / 6),
    );
    ctx.lineTo(
      endX - size * Math.cos(angle + Math.PI / 6),
      endY - size * Math.sin(angle + Math.PI / 6),
    );
    ctx.closePath();
    ctx.fill();
  };

  const renderFrame = (_timestamp: number) => {
    const canvas = canvasRef;
    if (!canvas || width === 0 || height === 0) {
      return;
    }

    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }

    context.setTransform(1, 0, 0, 1, 0, 0);
    context.clearRect(0, 0, canvas.width, canvas.height);
    context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);

    /* ── Background ──────────────────────────────────── */
    context.fillStyle = "#09090b";
    context.fillRect(0, 0, width, height);

    context.save();
    context.translate(width / 2 + panX, height / 2 + panY);
    context.scale(zoom, zoom);

    const left = (-width / 2 - panX) / zoom;
    const right = (width / 2 - panX) / zoom;
    const top = (-height / 2 - panY) / zoom;
    const bottom = (height / 2 - panY) / zoom;

    /* ── Grid ────────────────────────────────────────── */
    context.beginPath();
    for (let x = Math.floor(left / 50) * 50; x < right; x += 50) {
      context.moveTo(x, top);
      context.lineTo(x, bottom);
    }
    for (let y = Math.floor(top / 50) * 50; y < bottom; y += 50) {
      context.moveTo(left, y);
      context.lineTo(right, y);
    }
    context.strokeStyle = "rgba(255,255,255,0.03)";
    context.lineWidth = 1 / zoom;
    context.stroke();

    const isTreeMode = layoutMode() === "tree";

    /* ── Edges ───────────────────────────────────────── */
    for (const edge of currentEdges) {
      const source =
        typeof edge.source === "object"
          ? edge.source
          : currentNodes.find((node) => node.id === edge.source);
      const target =
        typeof edge.target === "object"
          ? edge.target
          : currentNodes.find((node) => node.id === edge.target);
      if (!source || !target) {
        continue;
      }

      const sx = getNodeX(source);
      const sy = getNodeY(source);
      const tx = getNodeX(target);
      const ty = getNodeY(target);

      const isActive = source.tone === "active" || target.tone === "active" || source.kind === "nota" || target.kind === "nota";
      const edgeColor = isActive ? EDGE_COLOR_ACTIVE : EDGE_COLOR_DEFAULT;

      context.beginPath();
      context.moveTo(sx, sy);
      context.lineTo(tx, ty);
      context.strokeStyle = edgeColor;
      context.lineWidth = isActive ? 1.5 / zoom : 1 / zoom;
      context.stroke();

      /* ── Arrowhead (tree mode only) ──────────────── */
      if (isTreeMode) {
        context.fillStyle = edgeColor;
        drawArrow(context, sx, sy, tx, ty, nodeRadius(target.kind));
      }
    }

    /* ── Nodes ───────────────────────────────────────── */
    const selectedId = props.selectedNodeId;

    for (const node of currentNodes) {
      let radius = nodeRadius(node.kind);
      const nx = getNodeX(node);
      const ny = getNodeY(node);

      const isHovered = hoveredNodeId === node.id;
      const isSelected = selectedId === node.id;
      if (isHovered) {
        radius += 2;
      }

      let fillStyle = DEFAULT_NODE_COLOR;
      let opacity = 1;

      switch(node.tone) {
        case "nota":
          fillStyle = TONE_COLORS.nota;
          break;
        case "active":
          fillStyle = TONE_COLORS.active;
          break;
        case "steady":
          fillStyle = TONE_COLORS.steady;
          break;
        case "warming":
          fillStyle = TONE_COLORS.warming;
          break;
        case "caution":
          fillStyle = TONE_COLORS.caution;
          break;
        case "archived":
          fillStyle = TONE_COLORS.archived;
          opacity = 0.5;
          break;
      }

      context.save();
      context.globalAlpha = opacity;

      /* ── Selected ring (static) ─────────────────── */
      if (isSelected) {
        context.beginPath();
        context.arc(nx, ny, radius + 5, 0, Math.PI * 2);
        context.strokeStyle = fillStyle;
        context.lineWidth = 1.5 / zoom;
        context.globalAlpha = 0.5;
        context.stroke();
        context.globalAlpha = opacity;
      }

      /* ── Node circle ─────────────────────────────── */
      context.beginPath();
      context.arc(nx, ny, radius, 0, Math.PI * 2);
      context.fillStyle = fillStyle;
      context.fill();

      if (isHovered) {
        context.lineWidth = 1 / zoom;
        context.strokeStyle = "rgba(255,255,255,0.12)";
        context.stroke();
      }

      /* ── Node initials (inside circle) ───────────── */
      const initials = NODE_INITIALS[node.kind];
      if (initials && radius >= 12) {
        const fontSize = Math.max(8, Math.round(radius * 0.65));
        context.font = `600 ${fontSize}px Inter, sans-serif`;
        context.fillStyle = "#09090b";
        context.textAlign = "center";
        context.textBaseline = "middle";
        context.fillText(initials, nx, ny);
      }

      /* ── Node label (below circle) ───────────────── */
      context.font = node.kind === "nota" ? "bold 12px Inter, sans-serif" : "11px Inter, sans-serif";
      context.fillStyle = node.kind === "nota" ? LABEL_COLOR_PRIMARY : LABEL_COLOR_SECONDARY;
      context.textAlign = "center";
      context.textBaseline = "top";
      context.fillText(node.label, nx, ny + radius + 6);

      context.restore();
    }

    context.restore();
  };

  const loop = (timestamp: number) => {
    renderFrame(timestamp);
    animationFrameId = requestAnimationFrame(loop);
  };

  const handleMouseMove = (event: MouseEvent) => {
    if (!canvasRef) {
      return;
    }

    const rect = canvasRef.getBoundingClientRect();
    const screenX = event.clientX - rect.left;
    const screenY = event.clientY - rect.top;

    if (isDragging && draggedNodeId && layoutMode() === "force") {
      didDrag = true;
      const graphPoint = screenToGraph(screenX, screenY);
      const node = currentNodes.find((candidate) => candidate.id === draggedNodeId);
      if (node) {
        node.fx = graphPoint.x;
        node.fy = graphPoint.y;
      }
      simulation.reheat();
      return;
    }

    if (isDragging && !draggedNodeId) {
      didDrag = true;
      panX += event.clientX - lastMouseX;
      panY += event.clientY - lastMouseY;
      lastMouseX = event.clientX;
      lastMouseY = event.clientY;
      return;
    }

    const graphPoint = screenToGraph(screenX, screenY);
    const hitNode = findNodeAt(graphPoint.x, graphPoint.y);
    hoveredNodeId = hitNode?.id ?? null;
    canvasRef.style.cursor = hitNode ? "pointer" : "grab";

    if (hitNode && !isDragging) {
      setTooltip({ x: screenX, y: screenY, node: hitNode });
    } else {
      setTooltip((prev) => ({ ...prev, node: null }));
    }
  };

  const handleMouseDown = (event: MouseEvent) => {
    if (!canvasRef) {
      return;
    }

    const rect = canvasRef.getBoundingClientRect();
    const graphPoint = screenToGraph(event.clientX - rect.left, event.clientY - rect.top);
    const hitNode = findNodeAt(graphPoint.x, graphPoint.y);

    isDragging = true;
    didDrag = false;
    lastMouseX = event.clientX;
    lastMouseY = event.clientY;

    if (hitNode && hitNode.id !== "nota" && layoutMode() === "force") {
      draggedNodeId = hitNode.id;
      hitNode.fx = graphPoint.x;
      hitNode.fy = graphPoint.y;
    }
  };

  const handleMouseUp = (event: MouseEvent) => {
    if (draggedNodeId) {
      const node = currentNodes.find((candidate) => candidate.id === draggedNodeId);
      if (node) {
        node.fx = null;
        node.fy = null;
      }
    }

    /* ── Click / double-click detection ──────────── */
    if (!didDrag && canvasRef) {
      const rect = canvasRef.getBoundingClientRect();
      const graphPoint = screenToGraph(event.clientX - rect.left, event.clientY - rect.top);
      const hitNode = findNodeAt(graphPoint.x, graphPoint.y);

      if (hitNode) {
        const now = Date.now();
        if (hitNode.id === lastClickNodeId && now - lastClickTime < DOUBLE_CLICK_MS) {
          // Double-click → action
          props.onNodeAction?.(hitNode.id, hitNode.kind);
          lastClickNodeId = null;
          lastClickTime = 0;
        } else {
          // Single click → select
          lastClickNodeId = hitNode.id;
          lastClickTime = now;
          props.onNodeSelect?.(hitNode.id);
        }
      } else {
        // Clicked empty space → deselect
        lastClickNodeId = null;
        lastClickTime = 0;
        props.onNodeSelect?.("");
      }
    }

    isDragging = false;
    draggedNodeId = null;
    didDrag = false;
    setTooltip((prev) => ({ ...prev, node: null }));
  };

  const handleWheel = (event: WheelEvent) => {
    event.preventDefault();
    const factor = event.deltaY > 0 ? 0.9 : 1.1;
    zoom = Math.max(0.2, Math.min(3, zoom * factor));
  };

  onMount(() => {
    if (!canvasRef) {
      return;
    }

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry || !canvasRef) {
        return;
      }

      pixelRatio = window.devicePixelRatio || 1;
      width = entry.contentRect.width;
      height = entry.contentRect.height;
      canvasRef.width = Math.max(1, Math.floor(width * pixelRatio));
      canvasRef.height = Math.max(1, Math.floor(height * pixelRatio));
    });

    observer.observe(canvasRef);
    canvasRef.style.cursor = "grab";
    canvasRef.addEventListener("mousemove", handleMouseMove);
    canvasRef.addEventListener("mousedown", handleMouseDown);
    canvasRef.addEventListener("mouseup", handleMouseUp);
    canvasRef.addEventListener("mouseleave", () => {
      if (draggedNodeId) {
        const node = currentNodes.find((c) => c.id === draggedNodeId);
        if (node) { node.fx = null; node.fy = null; }
      }
      isDragging = false;
      draggedNodeId = null;
      didDrag = false;
      setTooltip((prev) => ({ ...prev, node: null }));
    });
    canvasRef.addEventListener("wheel", handleWheel, { passive: false });
    animationFrameId = requestAnimationFrame(loop);

    onCleanup(() => {
      observer.disconnect();
      if (animationFrameId !== undefined) {
        cancelAnimationFrame(animationFrameId);
      }
      simulation.stop();
    });
  });

  createEffect(() => {
    const nodes = props.store.state.nodes.filter((node) => !node.hiddenByFilter);
    const visibleNodeIds = new Set(nodes.map((node) => node.id));
    const edges = props.store.state.edges.filter(
      (edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target),
    );

    if (layoutMode() === "tree") {
      // In tree mode, compute static positions and feed the nodes/edges
      // to our local state without using the d3 simulation
      simulation.stop();
      const simNodes: SimNode[] = nodes.map((n) => ({ ...n } as SimNode));
      const simEdges: SimEdge[] = edges.map((e) => ({ source: e.source, target: e.target, kind: e.kind }));
      treePositions = computeTreeLayout(simNodes, simEdges);
      currentNodes = simNodes;
      currentEdges = simEdges;
    } else {
      simulation.update(nodes, edges);
    }
  });

  onMount(() => {
    const pruneTimer = window.setInterval(() => props.store.pruneArchived(), 5000);
    onCleanup(() => window.clearInterval(pruneTimer));
  });

  return (
    <div class="compute-graph-container">
      <canvas ref={canvasRef} class="compute-graph-canvas" />
      <div
        class={`compute-graph-tooltip ${tooltip().node ? 'is-visible' : ''}`}
        style={{ left: `${tooltip().x + 16}px`, top: `${tooltip().y + 16}px` }}
      >
        <div class="compute-graph-tooltip__label">{tooltip().node?.label ?? ""}</div>
        <div class="compute-graph-tooltip__detail">{tooltip().node?.detail ?? ""}</div>
        <div class="compute-graph-tooltip__tone">{tooltip().node?.tone ?? ""}</div>
      </div>
    </div>
  );
};

export default ComputeGraph;
