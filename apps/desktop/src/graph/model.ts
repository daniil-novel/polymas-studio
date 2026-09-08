import type { Agent, Architecture, RuntimeEvent, TaskSummary } from '../types';

export const HEARTBEAT_STALE_MS = 15_000;
export const IMPULSE_MS = 1800;
export interface GraphNode {
  id: string;
  label: string;
  kind: 'agent' | 'memory';
  group: string;
  state: string;
  active: boolean;
  coordinator: boolean;
}
export interface GraphEdge {
  id: string;
  from: string;
  to: string;
}
export interface Impulse extends GraphEdge {
  time: number;
  kind: 'message' | 'memory';
}
export interface GraphProjection {
  nodes: GraphNode[];
  edges: GraphEdge[];
  impulses: Impulse[];
}
export type Positions = Record<string, [number, number]>;
export function stablePositions(
  before: Pick<GraphNode, 'id' | 'group'>[],
  next: Pick<GraphNode, 'id' | 'group'>[],
  automatic: Positions,
  pinned: Positions,
): Positions {
  const previous = new Map(before.map((n) => [n.id, n.group]));
  const result: Positions = {};
  for (const node of next) {
    if (pinned[node.id]) result[node.id] = pinned[node.id];
    else if (previous.get(node.id) === node.group && automatic[node.id])
      result[node.id] = automatic[node.id];
  }
  return result;
}

// This projection is shared by the canvas and the accessible list. A static
// topology edge never implies an actual delivery or an active process.
export function projectGraph(
  agents: Agent[],
  architecture: Architecture,
  tasks: TaskSummary[],
  events: RuntimeEvent[],
  connected: boolean,
  now: number,
): GraphProjection {
  const states = new Map<string, TaskSummary>();
  for (const task of tasks) {
    if (['running', 'stopping', 'unknown'].includes(task.state)) states.set(task.assignee, task);
    else if (
      task.state === 'queued' &&
      (!states.has(task.assignee) || states.get(task.assignee)?.state === 'waiting')
    )
      states.set(task.assignee, task);
    else if (task.state === 'waiting' && !states.has(task.assignee))
      states.set(task.assignee, task);
  }
  const ownership = new Map<string, string>();
  const coordinators = new Set<string>();
  if (architecture.coordinatorAgentId) coordinators.add(architecture.coordinatorAgentId);
  for (const holon of architecture.holons) {
    for (const member of holon.members) ownership.set(member, holon.id);
    if (holon.coordinatorAgentId) coordinators.add(holon.coordinatorAgentId);
  }
  const nodes: GraphNode[] = agents
    .filter((a) => a.status !== 'archived')
    .map((a) => {
      const task = states.get(a.id);
      let state: string = task?.state ?? 'idle';
      if (
        task &&
        ['running', 'stopping'].includes(state) &&
        (!connected || !task.lease || now - task.lease.heartbeatAt * 1000 > HEARTBEAT_STALE_MS)
      )
        state = 'stale';
      return {
        id: a.id,
        label: a.name,
        kind: 'agent',
        group: ownership.get(a.id) ?? a.roleId,
        state,
        active: state === 'running',
        coordinator: coordinators.has(a.id),
      };
    });
  const ids = new Set(nodes.map((n) => n.id));
  const edges = new Map<string, GraphEdge>();
  const add = (from: string, to: string) => {
    if (architecture.preset === 'single' || architecture.preset === 'independent') return;
    if (from !== to && ids.has(from) && ids.has(to))
      edges.set(`${from}>${to}`, { id: `${from}>${to}`, from, to });
  };
  for (const edge of architecture.edges) add(edge.from, edge.to);
  if (architecture.coordinatorAgentId) {
    for (const node of nodes) {
      add(architecture.coordinatorAgentId, node.id);
      add(node.id, architecture.coordinatorAgentId);
    }
  }
  for (const holon of architecture.holons)
    if (holon.coordinatorAgentId) {
      for (const member of holon.members) {
        add(holon.coordinatorAgentId, member);
        add(member, holon.coordinatorAgentId);
      }
    }
  if (architecture.memoryEnabled && architecture.memoryConnectionId) {
    const id = `memory:${architecture.memoryConnectionId}`;
    nodes.push({
      id,
      label: 'Общая память',
      kind: 'memory',
      group: 'memory',
      state: 'idle',
      active: false,
      coordinator: false,
    });
    ids.add(id);
  }
  const impulses = new Map<string, Impulse>();
  if (connected)
    for (const event of events) {
      const time = Date.parse(event.observedAt);
      if (!Number.isFinite(time) || now - time < 0 || now - time > IMPULSE_MS) continue;
      if (event.kind !== 'message.delivered_to_context') continue;
      const { from, to } = event.body;
      if (typeof from === 'string' && typeof to === 'string' && ids.has(from) && ids.has(to)) {
        impulses.set(event.eventId, { id: event.eventId, from, to, time, kind: 'message' });
      }
    }
  return { nodes, edges: [...edges.values()], impulses: [...impulses.values()] };
}

// O(N log N) deterministic cluster placement runs in a worker. It avoids an
// unbounded O(N²) force simulation and never changes a user's pinned positions.
export function layoutGraph(
  nodes: Pick<GraphNode, 'id' | 'group' | 'kind' | 'coordinator'>[],
  pinned: Positions,
): Positions {
  const result: Positions = {};
  const groups = new Map<string, typeof nodes>();
  for (const node of nodes) {
    const group = groups.get(node.group) ?? [];
    group.push(node);
    groups.set(node.group, group);
  }
  const entries = [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
  const groupRadius = Math.max(120, Math.sqrt(nodes.length) * 30);
  entries.forEach(([, members], g) => {
    const angle = (g / entries.length) * Math.PI * 2 - Math.PI / 2;
    const cx = entries.length === 1 ? 0 : Math.cos(angle) * groupRadius;
    const cy = entries.length === 1 ? 0 : Math.sin(angle) * groupRadius;
    members
      .sort((a, b) => a.id.localeCompare(b.id))
      .forEach((node, i) => {
        if (pinned[node.id]?.every(Number.isFinite)) result[node.id] = pinned[node.id];
        else if (node.kind === 'memory') result[node.id] = [0, 0];
        else {
          const radius = 34 * Math.sqrt(i + 1);
          const theta = i * Math.PI * (3 - Math.sqrt(5));
          result[node.id] = [cx + radius * Math.cos(theta), cy + radius * Math.sin(theta)];
        }
      });
  });
  return result;
}
