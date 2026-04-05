import { createEffect, onCleanup, onMount, createSignal } from "solid-js";

import { createGraphSimulation, type SimEdge, type SimNode } from "../features/dashboard/graphEngine";
import type { GraphStore } from "../features/dashboard/graphStore";
import "./ComputeGraph.css";

interface ComputeGraphProps {
  store: GraphStore;
  onNodeSelect?: (nodeId: string) => void;
  onNodeAction?: (nodeId: string, kind: string) => void;
  selectedNodeId?: string | null;
}

/* ── Color palette (Tech Noir) ─────────────────────────── */
const TONE_COLORS: Record<string, string> = {
  nota:     "#818cf8",
  active:   "#34d399",
  steady:   "#a3e635",
  warming:  "#fbbf24",
  caution:  "#f87171",
  archived: "rgba(250,250,250,0.15)",
};
const DEFAULT_NODE_COLOR = "rgba(250,250,250,0.25)";
const LABEL_COLOR_PRIMARY = "#e0e7ff";
const LABEL_COLOR_SECONDARY = "rgba(250,250,250,0.50)";

/* ── Node initials for in-circle rendering ─────────────── */
const NODE_INITIALS: Record<string, string> = {
  nota: "N",
  arch: "A",
  dev:  "D",
  agent: "Ag",
};

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
      const dx = (node.x ?? 0) - graphX;
      const dy = (node.y ?? 0) - graphY;
      if (dx * dx + dy * dy <= radius * radius) {
        return node;
      }
    }

    return null;
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

      const isActive = source.tone === "active" || target.tone === "active" || source.kind === "nota" || target.kind === "nota";

      context.beginPath();
      context.moveTo(source.x ?? 0, source.y ?? 0);
      context.lineTo(target.x ?? 0, target.y ?? 0);
      context.strokeStyle = isActive ? "rgba(129,140,248,0.25)" : "rgba(255,255,255,0.06)";
      context.lineWidth = isActive ? 1.5 / zoom : 1 / zoom;
      context.stroke();
    }

    /* ── Nodes ───────────────────────────────────────── */
    const selectedId = props.selectedNodeId;

    for (const node of currentNodes) {
      let radius = nodeRadius(node.kind);

      const isHovered = hoveredNodeId === node.id;
      const isSelected = selectedId === node.id;
      if (isHovered) {
        radius += 2;
      }

      let fillStyle = DEFAULT_NODE_COLOR;
      let shadowColor = fillStyle;
      let shadowBlur = 0;
      let opacity = 1;

      switch(node.tone) {
        case "nota":
          fillStyle = TONE_COLORS.nota;
          shadowBlur = 8 + Math.sin(_timestamp * 0.002) * 4;
          break;
        case "active":
          fillStyle = TONE_COLORS.active;
          shadowBlur = 6 + Math.sin(_timestamp * 0.004) * 3;
          break;
        case "steady":
          fillStyle = TONE_COLORS.steady;
          shadowBlur = 4;
          break;
        case "warming":
          fillStyle = TONE_COLORS.warming;
          shadowBlur = 5 + Math.sin(_timestamp * 0.0015) * 3;
          break;
        case "caution":
          fillStyle = TONE_COLORS.caution;
          shadowBlur = 6 + Math.sin(_timestamp * 0.006) * 4;
          break;
        case "archived":
          fillStyle = TONE_COLORS.archived;
          shadowColor = "transparent";
          shadowBlur = 0;
          opacity = 0.5;
          break;
      }

      shadowColor = fillStyle;

      if (isHovered) {
        shadowBlur += 4;
      }

      context.save();
      context.globalAlpha = opacity;

      /* ── Selected ring (pulse glow) ──────────────── */
      if (isSelected) {
        const pulseRadius = radius + 6 + Math.sin(_timestamp * 0.003) * 2;
        context.beginPath();
        context.arc(node.x ?? 0, node.y ?? 0, pulseRadius, 0, Math.PI * 2);
        context.strokeStyle = fillStyle;
        context.lineWidth = 2 / zoom;
        context.globalAlpha = 0.4 + Math.sin(_timestamp * 0.003) * 0.15;
        context.stroke();
        context.globalAlpha = opacity;
      }

      /* ── Node circle ─────────────────────────────── */
      context.beginPath();
      context.arc(node.x ?? 0, node.y ?? 0, radius, 0, Math.PI * 2);
      context.fillStyle = fillStyle;
      context.shadowColor = shadowColor;
      context.shadowBlur = Math.min(15, shadowBlur);
      context.fill();

      context.shadowBlur = 0;

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
        context.fillText(initials, node.x ?? 0, node.y ?? 0);
      }

      /* ── Node label (below circle) ───────────────── */
      context.font = node.kind === "nota" ? "bold 12px Inter, sans-serif" : "11px Inter, sans-serif";
      context.fillStyle = node.kind === "nota" ? LABEL_COLOR_PRIMARY : LABEL_COLOR_SECONDARY;
      context.textAlign = "center";
      context.textBaseline = "top";
      context.fillText(node.label, node.x ?? 0, (node.y ?? 0) + radius + 6);

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

    if (isDragging && draggedNodeId) {
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

    if (hitNode && hitNode.id !== "nota") {
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
    simulation.update(nodes, edges);
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
