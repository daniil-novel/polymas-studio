import { afterAll, afterEach, beforeAll, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RoleEditor } from '../../apps/desktop/src/RoleEditor';
import { instanceSnapshot } from '../e2e/fixtures/instances';

const showModal = HTMLDialogElement.prototype.showModal;
const close = HTMLDialogElement.prototype.close;
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
  };
});
afterEach(cleanup);
afterAll(() => {
  HTMLDialogElement.prototype.showModal = showModal;
  HTMLDialogElement.prototype.close = close;
});

test('a missing selected identity is never described as ready for archiving', async () => {
  const snapshot = instanceSnapshot(5);
  const fresh = structuredClone(snapshot);
  fresh.config.value.registry.agents.shift();
  render(
    <RoleEditor
      snapshot={snapshot}
      command={vi.fn()}
      refresh={vi.fn().mockResolvedValue(fresh)}
      connected
    />,
  );
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }));
  await user.click(screen.getByRole('button', { name: 'Архивировать выбранные' }));
  expect(await screen.findByText('Экземпляр не найден')).toBeTruthy();
  expect(screen.getByRole('list', { name: 'Выбранные экземпляры' }).textContent).not.toContain(
    'Готов к архивированию',
  );
  expect(
    (screen.getByRole('button', { name: 'Подтвердить архивирование' }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
});

test('a large review has bounded rows and can inspect an identity outside the first page', async () => {
  const snapshot = instanceSnapshot();
  render(
    <RoleEditor
      snapshot={snapshot}
      command={vi.fn()}
      refresh={vi.fn().mockResolvedValue(snapshot)}
      connected
    />,
  );
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('button', { name: 'Выбрать все найденные' }));
  await user.click(screen.getByRole('button', { name: 'Архивировать выбранные' }));
  expect(await screen.findByText('Выбрано для архива: 4999')).toBeTruthy();
  expect(
    screen.getByRole('list', { name: 'Выбранные экземпляры' }).children.length,
  ).toBeLessThanOrEqual(20);
  await user.click(screen.getByRole('button', { name: 'Далее' }));
  expect(screen.getByRole('list', { name: 'Выбранные экземпляры' }).textContent).toContain(
    'agent-directory-0020',
  );
  await user.type(screen.getByLabelText('Поиск в выбранных'), 'agent-directory-4998');
  expect(screen.getByRole('list', { name: 'Выбранные экземпляры' }).textContent).toContain(
    'Исследователь · 4999',
  );
  expect(screen.getByRole('list', { name: 'Выбранные экземпляры' }).children.length).toBe(1);
});
test('selection survives filters and the reviewed batch sends every exact selected identity once', async () => {
  const snapshot = instanceSnapshot(5);
  const command = vi.fn().mockResolvedValue({ revision: 9 });
  const refresh = vi.fn().mockResolvedValue(snapshot);
  render(<RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }));
  await user.type(screen.getByLabelText('Поиск экземпляров'), 'agent-directory-0001');
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0002' }));
  expect(screen.getByText('Выбрано: 2 · вне фильтра: 1')).toBeTruthy();
  await user.click(screen.getByRole('button', { name: 'Архивировать выбранные' }));
  expect(await screen.findByText('Выбрано для архива: 2')).toBeTruthy();
  expect(screen.getByRole('dialog').textContent).toContain('agent-directory-0000');
  expect(screen.getByRole('dialog').textContent).toContain('agent-directory-0001');
  await user.click(screen.getByRole('button', { name: 'Подтвердить архивирование' }));
  expect(command).toHaveBeenCalledExactlyOnceWith('/api/configure', {
    revision: 8,
    command: { type: 'archive_agents', agentIds: ['agent-directory-0000', 'agent-directory-0001'] },
  });
  expect(await screen.findByText('Выбранные экземпляры архивированы')).toBeTruthy();
});

test('a running member blocks the whole batch and an uncertain response cannot be retried blindly', async () => {
  let snapshot = instanceSnapshot(5);
  const command = vi.fn().mockRejectedValue(new Error('Ответ потерян'));
  const refresh = vi.fn(() => Promise.resolve(snapshot));
  render(<RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('button', { name: 'Выбрать все найденные' }));
  await user.click(screen.getByRole('button', { name: 'Архивировать выбранные' }));
  expect(await screen.findByText(/Незавершённая работа: 1/)).toBeTruthy();
  expect(
    (screen.getByRole('button', { name: 'Подтвердить архивирование' }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(command).not.toHaveBeenCalled();
  snapshot = structuredClone(snapshot);
  snapshot.agentActivity = [];
  await user.click(screen.getByRole('button', { name: 'Обновить проверку' }));
  await user.click(screen.getByRole('button', { name: 'Подтвердить архивирование' }));
  expect((await screen.findByRole('alert')).textContent).toContain('Ответ потерян');
  expect(
    (screen.getByRole('button', { name: 'Подтвердить архивирование' }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  snapshot.config.revision++;
  snapshot.config.value.registry.agents.forEach((agent) => {
    agent.status = 'archived';
  });
  await user.click(screen.getByRole('button', { name: 'Обновить проверку' }));
  expect(await screen.findByText('Все выбранные экземпляры уже в архиве')).toBeTruthy();
  expect(command).toHaveBeenCalledTimes(1);
});

test('a configuration change does not silently change the confirmed batch or discard its selection', async () => {
  const snapshot = instanceSnapshot(5);
  const refresh = vi.fn().mockResolvedValue(snapshot);
  const command = vi.fn();
  const view = render(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />,
  );
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }));
  await user.click(screen.getByRole('button', { name: 'Архивировать выбранные' }));
  const changed = structuredClone(snapshot);
  changed.config.revision++;
  changed.config.value.registry.agents[0].status = 'archived';
  view.rerender(<RoleEditor snapshot={changed} command={command} refresh={refresh} connected />);
  expect(
    (screen.getByRole('button', { name: 'Подтвердить архивирование' }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  await user.click(screen.getByRole('button', { name: 'Закрыть' }));
  expect(screen.getByText('Выбрано: 1 · вне фильтра: 1')).toBeTruthy();
  expect(command).not.toHaveBeenCalled();
});
