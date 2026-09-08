import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { App } from '../../apps/desktop/src/App';
import { ApiClient } from '../../apps/desktop/src/api';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
test('editor renders real registry counts and stays disconnected on an API failure', async () => {
  const api = new ApiClient('test-token');
  vi.stubGlobal(
    'fetch',
    vi.fn(
      async () =>
        new Response(JSON.stringify({ code: 'unauthorized', message: 'Подключение отклонено' }), {
          status: 401,
        }),
    ),
  );
  render(<App client={api} />);
  expect(await screen.findByText('Подключение отклонено')).toBeTruthy();
  expect(screen.queryByText('24 агента')).toBeNull();
});
test('an empty workspace never fabricates running agents or events', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            config: {
              revision: 1,
              value: {
                schemaVersion: 1,
                id: 'workspace-test',
                name: 'Моя колония',
                registry: { roles: [], agents: [] },
                architecture: {
                  maxConcurrency: 1,
                  holons: [],
                  edges: [],
                  positions: {},
                  memoryEnabled: false,
                },
                bindings: [],
              },
            },
            connections: [],
            cursor: 0,
            runs: [],
            tasks: [],
            taskCount: 0,
            agentRuntime: [],
            paused: false,
            runtime: { ready: false, capacity: 1, probe: null, error: null },
            events: [],
            head: 0,
          }),
          { status: 200 },
        ),
    ),
  );
  render(<App client={new ApiClient('test-token')} />);
  expect(await screen.findByText('Моя колония')).toBeTruthy();
  expect(await screen.findByText('Создайте первого агента')).toBeTruthy();
  expect(screen.getByTestId('agent-count').textContent).toContain('0');
  expect(screen.getByTestId('running-count').textContent).toContain('0');
});
