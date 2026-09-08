import { IMPULSE_MS, projectGraph, type GraphProjection } from '../graph/model';
import type { Agent, Architecture, RuntimeEvent, TaskState } from '../types';

export interface RecordingManifest {
  schemaVersion: number;
  runId: string;
  title: string;
  configRevision: number;
  workspaceName: string;
  createdAt: string;
  head: number;
  agents: Agent[];
  roles: { id: string; version: number; name: string }[];
  architecture: Architecture;
  tasks: {
    id: string;
    assignee: string;
    createdAt: string;
    dependencies: string[];
    roleId: string;
    roleVersion: number;
  }[];
}
export interface RecordedEvent {
  event: RuntimeEvent;
  time: number;
}
interface StatePoint {
  time: number;
  state: TaskState;
}
export interface RecordingTimeline {
  entries: RecordedEvent[];
  states: Map<string, StatePoint[]>;
}
const states = new Set([
  'queued',
  'running',
  'waiting',
  'waiting_user',
  'stopping',
  'awaiting_acceptance',
  'accepted',
  'rejected',
  'failed',
  'cancelled',
  'unknown',
]);

// Retain bounded graph metadata. Full provider bodies are fetched only on selection.
export function compactEvent(event: RuntimeEvent): RuntimeEvent {
  const body: Record<string, unknown> = {};
  for (const key of ['taskId', 'state', 'from', 'to', 'connectionId', 'tool', 'success']) {
    const value = event.body[key];
    if (typeof value === 'boolean' || (typeof value === 'string' && value.length <= 1000))
      body[key] = value;
  }
  return { ...event, body };
}
export function recordingTimeline(events: RuntimeEvent[]): RecordingTimeline {
  let time = 0;
  const changes = new Map<string, StatePoint[]>();
  const entries = events.map((event) => {
    const observed = Date.parse(event.observedAt);
    time = Math.max(time + 1, Number.isFinite(observed) ? observed : time + 1);
    if (
      event.kind === 'task.state_changed' &&
      typeof event.body.taskId === 'string' &&
      typeof event.body.state === 'string' &&
      states.has(event.body.state)
    ) {
      const list = changes.get(event.body.taskId) ?? [];
      list.push({ time, state: event.body.state as TaskState });
      changes.set(event.body.taskId, list);
    }
    return { event, time };
  });
  return { entries, states: changes };
}
export function upperBound<T extends { time: number }>(items: T[], time: number): number {
  let low = 0,
    high = items.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (items[middle].time <= time) low = middle + 1;
    else high = middle;
  }
  return low;
}
export function recordingFrame(
  manifest: RecordingManifest,
  timeline: RecordingTimeline,
  time: number,
): GraphProjection {
  const end = upperBound(timeline.entries, time);
  const start = upperBound(timeline.entries, time - IMPULSE_MS);
  const pulses = timeline.entries
    .slice(start, end)
    .map((item) => ({ ...item.event, observedAt: new Date(item.time).toISOString() }));
  const graph = projectGraph(manifest.agents, manifest.architecture, [], pulses, true, time);
  const agentStates = new Map<string, StatePoint>();
  const priority = (state: string) =>
    ({ running: 6, stopping: 6, unknown: 5, waiting_user: 4, queued: 3, waiting: 2 })[state] ?? 1;
  for (const task of manifest.tasks) {
    const changes = timeline.states.get(task.id) ?? [];
    const index = upperBound(changes, time) - 1;
    const point = changes[index];
    if (!point) continue;
    const previous = agentStates.get(task.assignee);
    if (
      !previous ||
      priority(point.state) > priority(previous.state) ||
      (priority(point.state) === priority(previous.state) && point.time > previous.time)
    )
      agentStates.set(task.assignee, point);
  }
  const participants = new Set(manifest.tasks.map((t) => t.assignee));
  return {
    ...graph,
    frameTime: time,
    nodes: graph.nodes.map((node) => {
      if (node.kind === 'memory') return node;
      const state =
        agentStates.get(node.id)?.state ??
        (participants.has(node.id) ? 'unobserved' : 'not_participating');
      return { ...node, state, active: state === 'running' };
    }),
  };
}
