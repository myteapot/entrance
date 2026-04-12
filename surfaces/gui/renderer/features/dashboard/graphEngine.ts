import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
} from "d3-force";
import type { SimulationLinkDatum, SimulationNodeDatum } from "d3-force";

import type { GraphEdge, GraphNode } from "./graphStore";

export interface SimNode extends SimulationNodeDatum {
  id: string;
  kind: string;
  label: string;
  detail: string;
  tone: string;
  createdAt: number;
  archivedAt: number | null;
  fx?: number | null;
  fy?: number | null;
}

export interface SimEdge extends SimulationLinkDatum<SimNode> {
  kind: string;
}

export function createGraphSimulation(onTick: (nodes: SimNode[], edges: SimEdge[]) => void) {
  let simNodes: SimNode[] = [];
  let simEdges: SimEdge[] = [];

  const linkForce = forceLink<SimNode, SimEdge>()
    .id((node) => node.id)
    .distance(100)
    .strength(0.3);

  const simulation = forceSimulation<SimNode>()
    .force("charge", forceManyBody().strength(-200).distanceMax(400))
    .force("center", forceCenter(0, 0))
    .force(
      "collide",
      forceCollide<SimNode>().radius((node) => (node.kind === "nota" ? 40 : 22)),
    )
    .force("link", linkForce)
    .alphaDecay(0.02)
    .on("tick", () => onTick(simNodes, simEdges));

  const update = (nodes: GraphNode[], edges: GraphEdge[]) => {
    const existingNodes = new Map(simNodes.map((node) => [node.id, node]));
    simNodes = nodes.map((node) => {
      const previous = existingNodes.get(node.id);
      return previous
        ? {
            ...node,
            x: previous.x ?? node.x,
            y: previous.y ?? node.y,
            vx: previous.vx ?? node.vx,
            vy: previous.vy ?? node.vy,
            fx: previous.fx ?? node.fx,
            fy: previous.fy ?? node.fy,
          }
        : ({ ...node } as SimNode);
    });

    simEdges = edges.map((edge) => ({
      source: edge.source,
      target: edge.target,
      kind: edge.kind,
    }));

    simulation.nodes(simNodes);
    linkForce.links(simEdges);
    simulation.alpha(0.3).restart();
  };

  const stop = () => simulation.stop();

  const reheat = () => simulation.alpha(0.5).restart();

  return { update, stop, reheat };
}
