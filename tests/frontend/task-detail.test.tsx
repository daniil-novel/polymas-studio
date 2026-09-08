import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TaskDetail } from '../../apps/desktop/src/TaskDetail';
import { ApiClient } from '../../apps/desktop/src/api';
import type { Task } from '../../apps/desktop/src/types';
afterEach(cleanup);
const fixture = (id: string): Task =>
  ({
    id,
    runId: 'run1',
    assignee: 'a1',
    state: 'awaiting_acceptance',
    prompt: 'Проверь источник',
    criteria: 'Есть ссылка',
    result: `Результат ${id}`,
    role: {
      id: 'r1',
      name: 'Исследователь',
      version: 1,
      createdAt: '2026-09-08T10:00:00Z',
      description: '',
      instruction: 'Проверяй факты',
      outputContract: 'markdown',
      acceptanceCriteria: '',
      escalationRule: '',
      model: null,
      sandbox: 'read-only',
      timeoutSeconds: 600,
      maxTurns: 20,
      maxOutputTokens: null,
    },
    attempt: 1,
    artifactIds: ['blob1'],
    lease: null,
    error: null,
    dependencies: [],
    createdAt: '2026-09-08T10:00:00Z',
    completedAt: '2026-09-08T10:00:05Z',
    newSession: false,
  }) as Task;
test('a completed provider result stays unaccepted until the operator accepts that exact task', async () => {
  const client = new ApiClient();
  vi.spyOn(client, 'request').mockResolvedValue(fixture('t1'));
  const command = vi.fn().mockResolvedValue({ ok: true });
  render(
    <TaskDetail taskId="t1" client={client} command={command} events={[]} agentName="Первый" />,
  );
  expect(await screen.findByText('Результат t1')).toBeTruthy();
  expect(command).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole('button', { name: 'Принять результат' }));
  expect(command).toHaveBeenCalledWith('/api/tasks/t1/action', { action: 'accept' });
});
test('late data from a previously selected task cannot appear in another agent result', async () => {
  const client = new ApiClient();
  let release: (task: Task) => void = () => {};
  vi.spyOn(client, 'request').mockImplementation((path) =>
    path.endsWith('old')
      ? new Promise((resolve) => {
          release = resolve;
        })
      : Promise.resolve(fixture('new')),
  );
  const props = { client, command: vi.fn(), events: [], agentName: 'Агент' };
  const view = render(<TaskDetail {...props} taskId="old" />);
  view.rerender(<TaskDetail {...props} taskId="new" />);
  expect(await screen.findByText('Результат new')).toBeTruthy();
  release(fixture('old'));
  await waitFor(() => expect(screen.queryByText('Результат old')).toBeNull());
});
