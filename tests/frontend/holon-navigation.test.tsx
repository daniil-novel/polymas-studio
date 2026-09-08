import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from '../../apps/desktop/src/App';
import { ApiClient } from '../../apps/desktop/src/api';

afterEach(cleanup);

test('holon navigation filters all descendants, intersects search, and returns to all agents without mutations', async () => {
  // JSDOM has no native dialog top layer; Chromium/CU cover the real modal.
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
  };
  const client = new ApiClient();
  const request = vi.spyOn(client, 'request').mockImplementation(async (path) => {
    if (path === '/api/state')
      return {
        config: {
          revision: 1,
          value: {
            schemaVersion: 1,
            id: 'holon-navigation',
            name: 'Holon fixture',
            bindings: [],
            registry: {
              roles: [],
              agents: Array.from({ length: 4 }, (_, i) => ({
                id: `agent-${i}`,
                name: `Навигатор ${i}`,
                roleId: 'r',
                roleVersion: 1,
                status: 'idle',
              })),
            },
            architecture: {
              preset: 'holonic',
              maxConcurrency: 1,
              positions: {},
              edges: [],
              memoryEnabled: false,
              holons: [
                {
                  id: 'parent',
                  name: 'Исследование',
                  parentId: null,
                  members: ['agent-0'],
                  coordinatorAgentId: null,
                },
                {
                  id: 'child',
                  name: 'Проверка',
                  parentId: 'parent',
                  members: ['agent-1', 'agent-2'],
                  coordinatorAgentId: null,
                },
                {
                  id: 'empty',
                  name: 'Пустая группа',
                  parentId: null,
                  members: [],
                  coordinatorAgentId: null,
                },
              ],
            },
          },
        },
        connections: [],
        cursor: 0,
        runs: [],
        tasks: [],
        taskCount: 0,
        agentRuntime: [],
        paused: true,
        runtime: { ready: false, capacity: 1, probe: null, error: null },
      };
    return { events: [], cursor: 0, head: 0 };
  });
  render(<App client={client} />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'Список' }));
  await user.click(screen.getByRole('button', { name: /Исследование/ }));
  let list = screen.getByLabelText('Список узлов');
  expect(
    within(list)
      .getAllByRole('button')
      .map((b) => b.textContent),
  ).toEqual(['Навигатор 0Готовagent-0', 'Навигатор 1Готовagent-1', 'Навигатор 2Готовagent-2']);
  await user.type(screen.getByLabelText('Поиск агентов'), 'Навигатор 2');
  await user.click(screen.getByRole('button', { name: 'Задача холону' }));
  const dialog = screen.getByRole('dialog');
  expect((within(dialog).getByLabelText('Холон-получатель') as HTMLSelectElement).value).toBe(
    'parent',
  );
  await user.click(within(dialog).getByRole('button', { name: 'Закрыть диалог' }));
  expect(within(list).getAllByRole('button')).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: /Проверка/ }));
  expect((screen.getByLabelText('Поиск агентов') as HTMLInputElement).value).toBe('');
  expect(within(list).getAllByRole('button')).toHaveLength(2);
  await user.click(screen.getByRole('button', { name: /Пустая группа/ }));
  expect(within(list).queryAllByRole('button')).toHaveLength(0);
  await user.click(screen.getByRole('button', { name: 'Все агенты' }));
  list = screen.getByLabelText('Список узлов');
  expect(within(list).getAllByRole('button')).toHaveLength(4);
  expect(request.mock.calls.every(([, body]) => body === undefined)).toBe(true);
});
