import { afterEach, expect, test, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { App } from '../../apps/desktop/src/App';
import { ApiClient } from '../../apps/desktop/src/api';
import type { GraphProjection } from '../../apps/desktop/src/graph/model';
import type { Snapshot, RuntimeEvent } from '../../apps/desktop/src/types';

// Observe the real App -> renderer projection boundary; canvas has its own tests.
vi.mock('../../apps/desktop/src/graph/GraphView', () => ({
  GraphView: ({
    projection,
    onSelect,
  }: {
    projection: GraphProjection;
    onSelect: (id: string) => void;
  }) => (
    <div>
      <output aria-label="Импульсы памяти">
        {projection.impulses.filter((p) => p.kind === 'memory').length}
      </output>
      <button onClick={() => onSelect('agent-clock')}>Выбрать тестовый узел</button>
    </div>
  ),
}));
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});
const base = Date.parse('2026-09-08T10:00:00Z');
function setup(running = false) {
  vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
  vi.setSystemTime(base);
  const client = new ApiClient();
  let deliver: (value: unknown) => void = () => {};
  const state = {
    config: {
      revision: 1,
      value: {
        schemaVersion: 1,
        id: 'workspace-clock',
        name: 'Clock fixture',
        bindings: [],
        registry: {
          roles: [],
          agents: [
            {
              id: 'agent-clock',
              name: 'Clock agent',
              roleId: 'role',
              roleVersion: 1,
              status: 'idle',
            },
          ],
        },
        architecture: {
          preset: 'network',
          maxConcurrency: 1,
          holons: [],
          edges: [],
          positions: {},
          memoryEnabled: true,
          memoryConnectionId: 'db',
        },
      },
    },
    connections: [],
    cursor: 0,
    runs: [],
    paused: false,
    agentRuntime: [],
    taskCount: running ? 1 : 0,
    tasks: running
      ? [
          {
            id: 'task-clock',
            assignee: 'agent-clock',
            state: 'running',
            lease: { heartbeatAt: base / 1000 },
          },
        ]
      : [],
    runtime: { ready: true, capacity: 1, probe: null, error: null },
  } as unknown as Snapshot;
  vi.spyOn(client, 'request').mockImplementation(async (path) => {
    if (path === '/api/state') return state;
    return new Promise((resolve) => {
      deliver = resolve;
    });
  });
  return {
    state,
    client,
    deliver: (events: RuntimeEvent[]) =>
      deliver({ events, cursor: events.length, head: events.length }),
  };
}
test('a newly received event reaches the graph before the next periodic clock tick', async () => {
  const { client, deliver } = setup();
  const view = render(<App client={client} />);
  await screen.findByLabelText('Импульсы памяти');
  vi.setSystemTime(base + 750);
  await act(async () =>
    deliver([
      {
        sequence: 1,
        eventId: 'fresh',
        kind: 'knowledge.read_completed',
        observedAt: new Date(base + 700).toISOString(),
        agentId: 'agent-clock',
        runId: 'run',
        body: { connectionId: 'db' },
      },
    ]),
  );
  expect(screen.getByLabelText('Импульсы памяти').textContent).toBe('1');
  vi.setSystemTime(base + 2600);
  view.rerender(<App client={client} />);
  expect(screen.getByLabelText('Импульсы памяти').textContent).toBe('0');
});

test('agent activity outside the recent task page still drives the graph and cancellation', async () => {
  const { client, deliver, state } = setup(true);
  state.agentActivity = state.tasks;
  state.tasks = [];
  state.taskCount = 1001;
  render(<App client={client} />);
  await screen.findByLabelText('Импульсы памяти');
  await act(async () => deliver([]));
  expect(screen.getByTestId('running-count').textContent).toBe('1 работают');
  expect(screen.getByRole('button', { name: 'Остановить' })).toHaveProperty('disabled', false);
});
test('header and inspector stop reporting a stale lease as running without disabling cancellation', async () => {
  const { client, deliver } = setup(true);
  const view = render(<App client={client} />);
  await screen.findByLabelText('Импульсы памяти');
  await act(async () => deliver([]));
  expect(screen.getByTestId('running-count').textContent).toBe('1 работают');
  vi.setSystemTime(base + 16000);
  view.rerender(<App client={client} />);
  expect(screen.getByTestId('running-count').textContent).toBe('0 работают');
  expect(screen.getByRole('button', { name: 'Остановить' })).toHaveProperty('disabled', false);
  fireEvent.click(screen.getByRole('button', { name: 'Выбрать тестовый узел' }));
  expect(screen.getByText('Нет свежих данных')).toBeTruthy();
});
