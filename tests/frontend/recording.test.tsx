import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RecordingWorkspace } from '../../apps/desktop/src/recording/RecordingWorkspace';
import { recordingFrame, recordingTimeline } from '../../apps/desktop/src/recording/model';
import type { RecordingManifest } from '../../apps/desktop/src/recording/model';
import type { ApiClient } from '../../apps/desktop/src/api';
import type { RuntimeEvent } from '../../apps/desktop/src/types';

vi.mock('../../apps/desktop/src/graph/GraphView', () => ({
  GraphView: ({ projection }: { projection: { impulses: { from: string; to: string }[] } }) => (
    <div data-testid="recorded-graph">
      {projection.impulses.map((p) => `${p.from} → ${p.to}`).join(',')}
    </div>
  ),
}));
afterEach(cleanup);
const start = Date.parse('2026-01-01T10:00:00Z');
const manifest = {
  schemaVersion: 1,
  runId: 'run-old',
  title: 'Recorded fixture',
  configRevision: 3,
  createdAt: new Date(start).toISOString(),
  workspaceName: 'History',
  head: 12,
  agents: [
    { id: 'agent-1', name: 'Original agent', roleId: 'role', roleVersion: 1, status: 'idle' },
  ],
  roles: [{ id: 'role', version: 1, name: 'Original role' }],
  tasks: [
    {
      id: 'task-1',
      assignee: 'agent-1',
      createdAt: new Date(start).toISOString(),
      dependencies: [],
      roleId: 'role',
      roleVersion: 1,
    },
  ],
  architecture: {
    preset: 'independent',
    holons: [],
    edges: [],
    positions: {},
    memoryEnabled: true,
    memoryConnectionId: 'source',
  },
} as unknown as RecordingManifest;
const event = (
  sequence: number,
  seconds: number,
  kind: string,
  body: Record<string, unknown>,
): RuntimeEvent => ({
  sequence,
  eventId: `event-${sequence}`,
  observedAt: new Date(start + seconds * 1000).toISOString(),
  runId: 'run-old',
  agentId: 'agent-1',
  kind,
  body,
});
const events = [
  event(2, 0, 'task.state_changed', { taskId: 'task-1', state: 'queued' }),
  event(5, 1, 'task.state_changed', { taskId: 'task-1', state: 'running' }),
  event(9, 25, 'knowledge.read_completed', { connectionId: 'source', taskId: 'task-1' }),
  event(12, 30, 'task.state_changed', { taskId: 'task-1', state: 'accepted' }),
];

test('recording reconstructs past state and memory pulses using the recorded clock, without inventing heartbeat freshness', () => {
  const timeline = recordingTimeline(events);
  const frame = recordingFrame(manifest, timeline, start + 25_400);
  expect(frame.nodes[0]).toMatchObject({ label: 'Original agent', state: 'running', active: true });
  expect(frame.frameTime).toBe(start + 25_400);
  expect(frame.impulses.map((p) => [p.from, p.to])).toEqual([['memory:source', 'agent-1']]);
  expect(recordingFrame(manifest, timeline, start + 500).nodes[0].state).toBe('queued');
  expect(recordingFrame(manifest, timeline, start + 31_000).nodes[0]).toMatchObject({
    state: 'accepted',
    active: false,
  });
  expect(recordingFrame(manifest, timeline, start + 31_000).impulses).toHaveLength(0);
  const reorderedClock = recordingTimeline([
    events[1],
    { ...events[2], observedAt: new Date(start).toISOString() },
  ]);
  expect(reorderedClock.entries[1].time).toBeGreaterThan(reorderedClock.entries[0].time);
  expect(reorderedClock.entries[1].event.observedAt).toBe(new Date(start).toISOString());
});

test('recording UI loads fixed-bound pages and only reads while seeking, changing speed and inspecting a past event', async () => {
  const request = vi.fn(async (path: string, body?: unknown) => {
    expect(body).toBeUndefined();
    if (path.endsWith('/recording')) return manifest;
    if (path.includes('after=0&through=12'))
      return { events: events.slice(0, 2), cursor: 5, through: 12, complete: false };
    if (path.includes('after=5&through=12'))
      return { events: events.slice(2), cursor: 12, through: 12, complete: true };
    if (path.includes('after=8&through=9'))
      return { events: [events[2]], cursor: 9, through: 9, complete: true };
    throw new Error(`Unexpected ${path}`);
  });
  const onExit = vi.fn();
  render(
    <RecordingWorkspace
      runId="run-old"
      client={{ request } as unknown as ApiClient}
      onExit={onExit}
    />,
  );
  const user = userEvent.setup();
  expect(await screen.findByRole('heading', { name: 'Recorded fixture' })).toBeTruthy();
  await screen.findByLabelText('Позиция записи');
  await user.click(screen.getByRole('button', { name: /#9.*knowledge.read_completed/ }));
  expect(screen.getByTestId('recorded-graph').textContent).toContain('memory:source → agent-1');
  expect(await screen.findByText('connectionId', { exact: false })).toBeTruthy();
  await user.selectOptions(screen.getByLabelText('Скорость записи'), '2');
  fireEvent.change(screen.getByLabelText('Позиция записи'), {
    target: { value: String(start + 31_000) },
  });
  expect(screen.getByTestId('recorded-graph').textContent).toBe('');
  expect(screen.queryByRole('button', { name: 'Принять результат' })).toBeNull();
  expect(request.mock.calls.every((call) => call[1] === undefined)).toBe(true);
  await user.click(screen.getByRole('button', { name: 'Вернуться к текущему запуску' }));
  expect(onExit).toHaveBeenCalledOnce();
});

test('a late response from the previous recording cannot replace the newly selected run', async () => {
  let resolveOld!: (v: unknown) => void;
  const request = vi.fn((path: string) => {
    if (path === '/api/runs/run-old/recording')
      return new Promise((r) => {
        resolveOld = r;
      });
    if (path === '/api/runs/new/recording')
      return Promise.resolve({ ...manifest, runId: 'new', title: 'New recording', head: 0 });
    return Promise.resolve({ events: [], cursor: 0, through: 0, complete: true });
  });
  const client = { request } as unknown as ApiClient;
  const view = render(<RecordingWorkspace runId="run-old" client={client} onExit={vi.fn()} />);
  view.rerender(<RecordingWorkspace runId="new" client={client} onExit={vi.fn()} />);
  await screen.findByRole('heading', { name: 'New recording' });
  resolveOld(manifest);
  await waitFor(() =>
    expect(screen.queryByRole('heading', { name: 'Recorded fixture' })).toBeNull(),
  );
});
