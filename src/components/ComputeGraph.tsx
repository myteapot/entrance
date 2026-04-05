import { createEffect, onCleanup, onMount, createSignal } from "solid-js";

import { createGraphSimulation, type SimEdge, type SimNode } from "../features/dashboard/graphEngine";
import type { GraphStore } from "../features/dashboard/graphStore";
import "./ComputeGraph.css";

interface ComputeGraphProps {
  store: GraphStore;
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

    // G1-SKIN-TODO: implement full Canvas rendering here.
    context.fillStyle = "hsl(225, 8%, 10%)";
    context.fillRect(0, 0, width, height);

    context.save();
    context.translate(width / 2 + panX, height / 2 + panY);
    context.scale(zoom, zoom);

    const left = (-width / 2 - panX) / zoom;
    const right = (width / 2 - panX) / zoom;
    const top = (-height / 2 - panY) / zoom;
    const bottom = (height / 2 - panY) / zoom;

    context.beginPath();
    for (let x = Math.floor(left / 50) * 50; x < right; x += 50) {
      context.moveTo(x, top);
      context.lineTo(x, bottom);
    }
    for (let y = Math.floor(top / 50) * 50; y < bottom; y += 50) {
      context.moveTo(left, y);
      context.lineTo(right, y);
    }
    context.strokeStyle = "hsla(225, 5%, 16%, 0.5)";
    context.lineWidth = 1 / zoom;
    context.stroke();

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
      context.strokeStyle = isActive ? "hsla(185, 20%, 50%, 0.35)" : "hsla(225, 5%, 30%, 0.25)";
      context.lineWidth = isActive ? 1.5 / zoom : 1 / zoom;
      context.stroke();
    }

    for (const node of currentNodes) {
      let radius = nodeRadius(node.kind);

      const isHovered = hoveredNodeId === node.id;
      if (isHovered) {
        radius += 2;
      }

      let fillStyle = "hsl(225, 5%, 52%)";
      let shadowColor = fillStyle;
      let shadowBlur = 0;
      let opacity = 1;

      switch(node.tone) {
        case "nota":
          fillStyle = "hsl(42, 25%, 55%)";
          shadowBlur = 8 + Math.sin(_timestamp * 0.002) * 4;
          break;
        case "active":
          fillStyle = "hsl(185, 20%, 50%)";
          shadowBlur = 6 + Math.sin(_timestamp * 0.004) * 3;
          break;
        case "steady":
          fillStyle = "hsl(150, 18%, 48%)";
          shadowBlur = 4;
          break;
        case "warming":
          fillStyle = "hsl(35, 25%, 52%)";
          shadowBlur = 5 + Math.sin(_timestamp * 0.0015) * 3;
          break;
        case "caution":
          fillStyle = "hsl(0, 20%, 52%)";
          shadowBlur = 6 + Math.sin(_timestamp * 0.006) * 4;
          break;
        case "archived":
          fillStyle = "hsl(225, 5%, 32%)";
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

      context.beginPath();
      context.arc(node.x ?? 0, node.y ?? 0, radius, 0, Math.PI * 2);
      context.fillStyle = fillStyle;
      context.shadowColor = shadowColor;
      context.shadowBlur = Math.min(15, shadowBlur);
      context.fill();

      context.shadowBlur = 0;

      if (isHovered) {
        context.lineWidth = 1 / zoom;
        context.strokeStyle = "hsl(225, 5%, 28%)";
        context.stroke();
      }

      context.font = node.kind === "nota" ? "bold 12px Inter, sans-serif" : "11px Inter, sans-serif";
      context.fillStyle = node.kind === "nota" ? "hsl(225, 5%, 78%)" : "hsl(225, 5%, 52%)";
      context.textAlign = "center";
      context.fillText(node.label, node.x ?? 0, (node.y ?? 0) + radius + 12);

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
    lastMouseX = event.clientX;
    lastMouseY = event.clientY;

    if (hitNode && hitNode.id !== "nota") {
      draggedNodeId = hitNode.id;
      hitNode.fx = graphPoint.x;
      hitNode.fy = graphPoint.y;
    }
  };

  const handleMouseUp = () => {
    if (draggedNodeId) {
      const node = currentNodes.find((candidate) => candidate.id === draggedNodeId);
      if (node) {
        node.fx = null;
        node.fy = null;
      }
    }

    isDragging = false;
    draggedNodeId = null;
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
    canvasRef.addEventListener("mouseleave", handleMouseUp);
    canvasRef.addEventListener("wheel", handleWheel, { passive: false });
    animationFrameId = requestAnimationFrame(loop);

    onCleanup(() => {
      observer.disconnect();
      if (animationFrameId !== undefined) {
        cancelAnimationFrame(animationFrameId);
      }
      simulation.stop();
      canvasRef?.removeEventListener("mousemove", handleMouseMove);
      canvasRef?.removeEventListener("mousedown", handleMouseDown);
      canvasRef?.removeEventListener("mouseup", handleMouseUp);
      canvasRef?.removeEventListener("mouseleave", handleMouseUp);
      canvasRef?.removeEventListener("wheel", handleWheel);
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
