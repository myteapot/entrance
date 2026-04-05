import { createEffect, onCleanup, onMount } from "solid-js";

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

  const simulation = createGraphSimulation((nodes, edges) => {
    currentNodes = nodes;
    currentEdges = edges;
  });

  const screenToGraph = (screenX: number, screenY: number) => ({
    x: (screenX - width / 2 - panX) / zoom,
    y: (screenY - height / 2 - panY) / zoom,
  });

  const findNodeAt = (graphX: number, graphY: number): SimNode | null => {
    for (let index = currentNodes.length - 1; index >= 0; index -= 1) {
      const node = currentNodes[index];
      const radius = node.kind === "nota" ? 24 : 14;
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
    context.fillStyle = "#111418";
    context.fillRect(0, 0, width, height);

    context.save();
    context.translate(width / 2 + panX, height / 2 + panY);
    context.scale(zoom, zoom);

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

      context.beginPath();
      context.moveTo(source.x ?? 0, source.y ?? 0);
      context.lineTo(target.x ?? 0, target.y ?? 0);
      context.strokeStyle = "rgba(100,120,140,0.3)";
      context.lineWidth = 1;
      context.stroke();
    }

    for (const node of currentNodes) {
      const radius = node.kind === "nota" ? 24 : 14;
      const isHovered = hoveredNodeId === node.id;

      context.beginPath();
      context.arc(node.x ?? 0, node.y ?? 0, radius, 0, Math.PI * 2);
      context.fillStyle = node.kind === "nota" ? "#b8a060" : "#607080";
      context.fill();

      if (isHovered) {
        context.strokeStyle = "#e6edf3";
        context.lineWidth = 2;
        context.stroke();
      }

      context.fillStyle = "#aaa";
      context.font = "11px sans-serif";
      context.textAlign = "center";
      context.fillText(node.label, node.x ?? 0, (node.y ?? 0) + radius + 14);
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
    const nodes = props.store.state.nodes.slice();
    const edges = props.store.state.edges.slice();
    simulation.update(nodes, edges);
  });

  onMount(() => {
    const pruneTimer = window.setInterval(() => props.store.pruneArchived(), 5000);
    onCleanup(() => window.clearInterval(pruneTimer));
  });

  return (
    <div class="compute-graph-container">
      <canvas ref={canvasRef} class="compute-graph-canvas" />
    </div>
  );
};

export default ComputeGraph;
