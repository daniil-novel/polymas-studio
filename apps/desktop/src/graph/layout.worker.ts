import { layoutGraph, type GraphNode, type Positions } from './model';
self.onmessage = (
  event: MessageEvent<{ version: number; nodes: GraphNode[]; pinned: Positions }>,
) => {
  const { version, nodes, pinned } = event.data;
  self.postMessage({ version, positions: layoutGraph(nodes, pinned) });
};
