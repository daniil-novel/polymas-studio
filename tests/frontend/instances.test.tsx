import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RoleEditor } from '../../apps/desktop/src/RoleEditor';
import { projectGraph } from '../../apps/desktop/src/graph/model';
import { instanceSnapshot } from '../e2e/fixtures/instances';
import type { TaskSummary } from '../../apps/desktop/src/types';

afterEach(cleanup);
function setup(count = 5000, connected = true) {
  const snapshot = instanceSnapshot(count);
  const nodes = projectGraph(
    snapshot.config.value.registry.agents,
    snapshot.config.value.architecture,
    snapshot.agentActivity!,
    [],
    connected,
    Date.now(),
  ).nodes;
  const command = vi.fn();
  render(<RoleEditor snapshot={snapshot} command={command} nodes={nodes} />);
  return { snapshot, command };
}

test('the instance directory bounds rows and finds an exact identity beyond its first window', async () => {
  const { command } = setup();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  const table = screen.getByRole('table', { name: 'Экземпляры агентов' });
  expect(table.getAttribute('aria-rowcount')).toBe('5000');
  expect(within(table).getAllByRole('row').length).toBeLessThan(40);
  await user.type(screen.getByLabelText('Поиск экземпляров'), 'AGENT-DIRECTORY-4998');
  const filtered = screen.getByRole('table', { name: 'Экземпляры агентов' });
  expect(within(filtered).getByText('Исследователь · 4999')).toBeTruthy();
  expect(within(filtered).getByText('Выполняет задачу')).toBeTruthy();
  expect(within(filtered).getByText('Проверка источников')).toBeTruthy();
  expect(within(filtered).getByText('task-old-active')).toBeTruthy();
  expect(within(filtered).getByText('fixture-worker')).toBeTruthy();
  expect(command).not.toHaveBeenCalled();
});

test('filters intersect and a new search resets the scroll window', async () => {
  setup(150);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  const viewport = screen.getByRole('region', { name: 'Прокрутка экземпляров' });
  fireEvent.scroll(viewport, { target: { scrollTop: 6000 } });
  await user.type(screen.getByLabelText('Поиск экземпляров'), 'Исследователь · 0149');
  expect(screen.getByText('Исследователь · 0149')).toBeTruthy();
  await user.selectOptions(screen.getByLabelText('Состояние экземпляров'), 'archived');
  expect(screen.getByText('Экземпляры не найдены')).toBeTruthy();
  await user.clear(screen.getByLabelText('Поиск экземпляров'));
  expect(screen.getByText('Исследователь · 0150')).toBeTruthy();
  expect(screen.queryByText('Исследователь · 0149')).toBeNull();
  await user.selectOptions(screen.getByLabelText('Состояние экземпляров'), 'all');
  await user.selectOptions(screen.getByLabelText('Порядок экземпляров'), 'name-desc');
  const rows = within(screen.getByRole('table')).getAllByRole('row');
  expect(rows[1].textContent).toContain('Исследователь · 0150');
});

test('connection loss shows stale activity and clipboard failures stay in the UI', async () => {
  setup(5, false);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.selectOptions(screen.getByLabelText('Состояние экземпляров'), 'stale');
  expect(screen.getByText('Исследователь · 0004')).toBeTruthy();
  expect(screen.getByRole('table').textContent).toContain('Нет свежих данных');
  const copy = vi
    .spyOn(navigator.clipboard, 'writeText')
    .mockRejectedValueOnce(new Error('denied'));
  await user.click(screen.getByRole('button', { name: 'Скопировать ID Исследователь · 0004' }));
  expect(copy).toHaveBeenCalledWith('agent-directory-0003');
  expect((await screen.findByRole('alert')).textContent).toContain('Не удалось скопировать ID');
  copy.mockRestore();
});

test('a legacy snapshot cannot label a completed task as the current running attempt', async () => {
  const snapshot = instanceSnapshot(4);
  const active = snapshot.agentActivity![0];
  snapshot.tasks = [
    active,
    { ...active, id: 'completed-task', state: 'accepted', lease: null },
  ] as TaskSummary[];
  snapshot.agentActivity = undefined;
  const nodes = projectGraph(
    snapshot.config.value.registry.agents,
    snapshot.config.value.architecture,
    snapshot.tasks,
    [],
    true,
    Date.now(),
  ).nodes;
  render(<RoleEditor snapshot={snapshot} command={vi.fn()} nodes={nodes} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.selectOptions(screen.getByLabelText('Состояние экземпляров'), 'running');
  expect(screen.getByRole('table').textContent).toContain('task-old-active');
  expect(screen.getByRole('table').textContent).not.toContain('completed-task');
});
