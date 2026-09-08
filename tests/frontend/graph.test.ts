import { expect, test } from 'vitest';
import { projectGraph, layoutGraph } from '../../apps/desktop/src/graph/model';
import type { Agent, Architecture, RuntimeEvent, TaskSummary } from '../../apps/desktop/src/types';

const agents = (n: number): Agent[] =>
  Array.from(
    { length: n },
    (_, i) =>
      ({
        id: `a${i}`,
        name: `Agent ${i}`,
        roleId: 'r',
        roleVersion: 1,
        status: 'idle',
        owningHolonId: null,
      }) as Agent,
  );
const arch = (patch: Partial<Architecture> = {}): Architecture => ({
  preset: 'network',
  maxConcurrency: 1,
  admission: 'queue_allowed',
  coordinatorAgentId: null,
  memoryEnabled: false,
  memoryConnectionId: null,
  holons: [],
  edges: [],
  positions: {},
  messageHopLimit: 8,
  maxMessagesPerRun: 1000,
  paused: false,
  ...patch,
});
const now = Date.parse('2026-09-08T10:00:00Z');
test('graph permissions match runtime for independent and optional coordinator modes', () => {
  const withEdge = arch({
    preset: 'independent',
    edges: [{ from: 'a0', to: 'a1' }],
    coordinatorAgentId: 'a0',
  });
  expect(projectGraph(agents(2), withEdge, [], [], true, now).edges).toHaveLength(0);
  expect(
    projectGraph(
      agents(2),
      arch({ preset: 'network', coordinatorAgentId: 'a0' }),
      [],
      [],
      true,
      now,
    ).edges,
  ).toHaveLength(2);
});
test('only fresh observed processes glow; disconnected and stale leases do not', () => {
  const tasks = [
    { assignee: 'a0', state: 'running', lease: { heartbeatAt: now / 1000 - 3 } },
    { assignee: 'a1', state: 'running', lease: { heartbeatAt: now / 1000 - 25 } },
  ] as TaskSummary[];
  const graph = projectGraph(agents(3), arch(), tasks, [], true, now);
  expect(graph.nodes.map((n) => [n.state, n.active])).toEqual([
    ['running', true],
    ['stale', false],
    ['idle', false],
  ]);
  expect(projectGraph(agents(3), arch(), tasks, [], false, now).nodes.some((n) => n.active)).toBe(
    false,
  );
});
test('a queued message does not pretend to be delivered; impulses expire and deduplicate', () => {
  const event = (sequence: number, kind: string): RuntimeEvent => ({
    sequence,
    eventId: `e${sequence}`,
    kind,
    observedAt: new Date(now - 300).toISOString(),
    agentId: 'a0',
    runId: 'run',
    body: { from: 'a0', to: 'a1', messageId: 'm1' },
  });
  const events = [
    event(1, 'message.queued'),
    event(2, 'message.delivered_to_context'),
    event(2, 'message.delivered_to_context'),
  ];
  const topology = arch({ edges: [{ from: 'a0', to: 'a1' }] });
  expect(
    projectGraph(agents(2), topology, [], events.slice(0, 1), true, now).impulses,
  ).toHaveLength(0);
  expect(projectGraph(agents(2), topology, [], events, true, now).impulses).toHaveLength(1);
  expect(projectGraph(agents(2), topology, [], events, true, now + 3000).impulses).toHaveLength(0);
});
test('coordinator is an existing agent, memory is an explicit non-agent node, and no mesh is invented', () => {
  const graph = projectGraph(
    agents(10),
    arch({
      preset: 'star',
      coordinatorAgentId: 'a0',
      memoryEnabled: true,
      memoryConnectionId: 'db',
    }),
    [],
    [],
    true,
    now,
  );
  expect(graph.nodes.filter((n) => n.kind === 'agent')).toHaveLength(10);
  expect(graph.nodes.filter((n) => n.kind === 'memory')).toHaveLength(1);
  expect(graph.edges).toHaveLength(18);
  expect(projectGraph(agents(10), arch(), [], [], true, now).edges).toHaveLength(0);
});
test('layout handles 5000 definitions deterministically with unique positions and retains pinned nodes', () => {
  const nodes = projectGraph(agents(5000), arch(), [], [], true, now).nodes;
  const first = layoutGraph(nodes, { a3: [42, 24] });
  expect(Object.keys(first)).toHaveLength(5000);
  expect(first.a3).toEqual([42, 24]);
  expect(new Set(Object.values(first).map((p) => p.join(','))).size).toBe(5000);
  expect(layoutGraph(nodes, { a3: [42, 24] })).toEqual(first);
});
