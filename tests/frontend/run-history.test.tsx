import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, within, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TasksWorkspace } from '../../apps/desktop/src/TasksWorkspace';
import type { ApiClient } from '../../apps/desktop/src/api';
import type { Snapshot } from '../../apps/desktop/src/types';

vi.mock('../../apps/desktop/src/TaskDetail', () => ({
  TaskDetail: ({ taskId }: { taskId: string }) => (
    <section aria-label="Подробности задачи">
      <p>Открыта {taskId}</p>
      <textarea aria-label="Заметка результата" />
    </section>
  ),
}));
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test('hidden history stays idle and returning to it requires fresh run data before actions', async () => {
  let hold = false;
  let release: ((value: ReturnType<typeof run>) => void) | undefined;
  const request = vi.fn(async (path: string) => {
    if (path.startsWith('/api/runs?')) return { items: [], nextCursor: null };
    if (path.includes('/tasks?')) return { items: [task('old-task')], nextCursor: null };
    if (hold)
      return new Promise<ReturnType<typeof run>>((resolve) => {
        release = resolve;
      });
    return run();
  });
  const props = {
    snapshot,
    client: { request } as unknown as ApiClient,
    command: vi.fn(),
    events: [],
    onContinue: vi.fn(),
    onReplay: vi.fn(),
    initialRunId: 'old-run',
  };
  const view = render(<TasksWorkspace {...props} active={false} />);
  expect(request).not.toHaveBeenCalled();
  view.rerender(<TasksWorkspace {...props} active />);
  await screen.findByText('Открыта old-task');
  view.rerender(<TasksWorkspace {...props} active={false} />);
  hold = true;
  view.rerender(<TasksWorkspace {...props} active />);
  expect(
    (screen.getByRole('button', { name: 'Остановить запуск' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  await act(async () => release?.(run()));
  expect(
    (screen.getByRole('button', { name: 'Остановить запуск' }) as HTMLButtonElement).disabled,
  ).toBe(false);
});
const snapshot = {
  config: {
    revision: 4,
    value: {
      registry: {
        agents: [{ id: 'a1', name: 'Исследователь', status: 'idle' }],
        roles: [],
      },
      architecture: { coordinatorAgentId: null, holons: [], maxConcurrency: 1 },
    },
  },
  runtime: { ready: true, capacity: 1 },
  paused: false,
  connections: [],
  tasks: [],
  runs: [],
  taskCount: 1017,
} as unknown as Snapshot;
const run = (id = 'old-run') => ({
  id,
  title: id === 'old-run' ? 'Старый запуск' : 'Новый запуск',
  createdAt: '2026-09-08T12:00:00Z',
  configRevision: 3,
  taskCount: 1017,
  accepted: false,
  stopped: false,
  completion: { type: 'manual' },
});
const task = (id: string) => ({
  id,
  runId: 'old-run',
  assignee: 'a1',
  state: 'failed',
  attempt: 1,
  createdAt: '2026-09-08T12:00:00Z',
  completedAt: null,
});

test('pages persisted history independently of the live window and retains the selected result and composer draft', async () => {
  const request = vi.fn(async (path: string) => {
    const url = new URL(path, 'http://fixture');
    if (url.pathname === '/api/runs') {
      if (url.searchParams.get('query')) return { items: [], nextCursor: null };
      return url.searchParams.has('cursor')
        ? { items: [run()], nextCursor: null }
        : { items: [run('new-run')], nextCursor: 'runs-page-2' };
    }
    if (url.pathname === '/api/runs/old-run') return run();
    if (url.pathname === '/api/runs/old-run/tasks')
      return url.searchParams.has('cursor')
        ? { items: [task('ancient-task')], nextCursor: null }
        : { items: [task('recent-task')], nextCursor: 'tasks-page-2' };
    throw new Error(`Unexpected ${path}`);
  });
  const command = vi.fn();
  render(
    <TasksWorkspace
      snapshot={snapshot}
      events={[]}
      client={{ request } as unknown as ApiClient}
      command={command}
      onContinue={vi.fn()}
      onReplay={vi.fn()}
    />,
  );
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Задача или сообщение'), 'Сохранить мой черновик');
  const runPager = await screen.findByRole('navigation', { name: 'Страницы запусков' });
  await user.click(within(runPager).getByRole('button', { name: 'Далее' }));
  await user.click(await screen.findByRole('button', { name: /Старый запуск/ }));
  expect(await screen.findByText('Открыта recent-task')).toBeTruthy();
  const taskPager = screen.getByRole('navigation', { name: 'Страницы задач запуска' });
  await user.click(within(taskPager).getByRole('button', { name: 'Далее' }));
  expect(await screen.findByText('Открыта ancient-task')).toBeTruthy();
  await user.type(screen.getByLabelText('Заметка результата'), 'Проверил результат');
  const result = screen.getByRole('region', { name: 'Подробности задачи' });
  await user.type(screen.getByLabelText('Поиск запусков'), 'Нет такого');
  expect(await screen.findByText('Запуски не найдены')).toBeTruthy();
  expect(screen.getByRole('region', { name: 'Подробности задачи' })).toBe(result);
  expect((screen.getByLabelText('Заметка результата') as HTMLTextAreaElement).value).toBe(
    'Проверил результат',
  );
  expect(within(runPager).getByText('Стр. 1')).toBeTruthy();
  await user.selectOptions(screen.getByLabelText('Состояние задач запуска'), 'failed');
  expect(await screen.findByText('Открыта recent-task')).toBeTruthy();
  expect(
    request.mock.calls.some(([path]) => path.includes('state=failed') && !path.includes('cursor=')),
  ).toBe(true);
  await user.click(screen.getByRole('button', { name: 'Новая задача' }));
  expect((screen.getByLabelText('Задача или сообщение') as HTMLTextAreaElement).value).toBe(
    'Сохранить мой черновик',
  );
  expect(command).not.toHaveBeenCalled();
});

test('a deep linked old run survives background refresh; failed refresh disables its actions until a fresh read', async () => {
  let tick: (() => void) | undefined;
  const interval = window.setInterval.bind(window);
  vi.spyOn(window, 'setInterval').mockImplementation((handler, delay, ...args) => {
    if (delay === 5000) {
      tick = handler as () => void;
      return 42 as unknown as ReturnType<typeof window.setInterval>;
    }
    return interval(handler, delay, ...args) as unknown as ReturnType<typeof window.setInterval>;
  });
  let fail = false;
  let prepend = false;
  const request = vi.fn(async (path: string) => {
    const url = new URL(path, 'http://fixture');
    if (url.pathname === '/api/runs') return { items: [], nextCursor: null };
    if (url.pathname === '/api/runs/old-run') {
      if (fail) throw new Error('Связь потеряна');
      return run();
    }
    if (url.pathname === '/api/runs/old-run/tasks')
      return {
        items: [...(prepend ? [task('new-arrival')] : []), task('old-task')],
        nextCursor: null,
      };
    throw new Error(path);
  });
  const command = vi.fn().mockResolvedValue({ ok: true });
  render(
    <TasksWorkspace
      snapshot={snapshot}
      events={[]}
      client={{ request } as unknown as ApiClient}
      command={command}
      onContinue={vi.fn()}
      onReplay={vi.fn()}
      initialRunId="old-run"
    />,
  );
  const user = userEvent.setup();
  expect(await screen.findByText('Открыта old-task')).toBeTruthy();
  const result = screen.getByRole('region', { name: 'Подробности задачи' });
  await user.type(screen.getByLabelText('Заметка результата'), 'Мой вывод');
  prepend = true;
  await act(async () => tick?.());
  expect(screen.getByRole('region', { name: 'Подробности задачи' })).toBe(result);
  expect(screen.getByText('Открыта old-task')).toBeTruthy();
  fail = true;
  await act(async () => tick?.());
  expect(await screen.findByText('Связь потеряна')).toBeTruthy();
  expect(
    (screen.getByRole('button', { name: 'Остановить запуск' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  expect(screen.getByRole('region', { name: 'Подробности задачи' })).toBe(result);
  expect((screen.getByLabelText('Заметка результата') as HTMLTextAreaElement).value).toBe(
    'Мой вывод',
  );
  fail = false;
  await act(async () => tick?.());
  await user.click(screen.getByRole('button', { name: 'Остановить запуск' }));
  expect(command).toHaveBeenCalledWith('/api/runs/old-run/action', { action: 'cancel' });
});
