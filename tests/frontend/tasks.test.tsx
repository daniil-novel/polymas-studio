import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TaskComposer, resolveRecipients } from '../../apps/desktop/src/TaskComposer';
import type { Agent, Architecture, Snapshot } from '../../apps/desktop/src/types';
afterEach(cleanup);
const agents = [
  { id: 'a1', name: 'Первый', status: 'idle' },
  { id: 'a2', name: 'Второй', status: 'idle' },
] as Agent[];
const architecture = {
  coordinatorAgentId: null,
  holons: [],
  maxConcurrency: 1,
} as unknown as Architecture;
const snapshot = {
  config: { revision: 4, value: { registry: { agents, roles: [] }, architecture } },
  runtime: { ready: true, capacity: 1 },
  paused: false,
  connections: [],
  tasks: [],
  runs: [],
} as unknown as Snapshot;
test('an agent message has one explicit recipient and preserves retry identity after an uncertain response', async () => {
  const command = vi
    .fn()
    .mockRejectedValueOnce(new Error('Нет связи'))
    .mockResolvedValueOnce({ id: 'run1' });
  render(<TaskComposer snapshot={snapshot} command={command} initialAgentId="a2" />);
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Задача или сообщение'), 'Проверь два факта');
  await user.click(screen.getByRole('button', { name: 'Начать' }));
  expect(await screen.findByText('Нет связи')).toBeTruthy();
  await user.click(screen.getByRole('button', { name: 'Начать' }));
  expect(command.mock.calls[0][1]).toMatchObject({
    input: { target: { type: 'agent', id: 'a2' }, prompt: 'Проверь два факта', newSession: false },
  });
  expect(command.mock.calls[0][1].key).toBe(command.mock.calls[1][1].key);
});
test('switching recipient does not carry the other agent draft into its chat', async () => {
  render(<TaskComposer snapshot={snapshot} command={vi.fn()} initialAgentId="a1" />);
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Задача или сообщение'), 'Личный черновик первого');
  await user.selectOptions(screen.getByLabelText('Агент-получатель'), 'a2');
  expect((screen.getByLabelText('Задача или сообщение') as HTMLTextAreaElement).value).toBe('');
  await user.selectOptions(screen.getByLabelText('Агент-получатель'), 'a1');
  expect((screen.getByLabelText('Задача или сообщение') as HTMLTextAreaElement).value).toBe(
    'Личный черновик первого',
  );
});
test('system coordinator stays inside N and holon recursive dispatch resolves all descendants exactly once', () => {
  expect(
    resolveRecipients(
      agents,
      { ...architecture, coordinatorAgentId: 'a2' },
      { type: 'system', broadcast: true },
    ),
  ).toEqual(['a2']);
  const topology = {
    ...architecture,
    holons: [
      { id: 'h1', parentId: null, members: ['a1'], coordinatorAgentId: 'a1' },
      { id: 'h2', parentId: 'h1', members: ['a2'], coordinatorAgentId: null },
    ],
  } as Architecture;
  expect(
    resolveRecipients(agents, topology, { type: 'holon', id: 'h1', recursive: false }),
  ).toEqual(['a1']);
  expect(resolveRecipients(agents, topology, { type: 'holon', id: 'h1', recursive: true })).toEqual(
    ['a1', 'a2'],
  );
});
