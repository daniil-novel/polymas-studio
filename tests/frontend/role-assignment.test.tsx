import { afterAll, afterEach, beforeAll, expect, test, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
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

function data() {
  const snapshot = instanceSnapshot(5);
  snapshot.config.value.registry.roles.push({
    ...snapshot.config.value.registry.roles[0],
    version: 2,
    instruction: 'Новая инструкция для будущих задач',
  });
  return snapshot;
}
async function selectAndOpen() {
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }));
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0004' }));
  await user.click(screen.getByRole('button', { name: 'Назначить версию роли' }));
  await user.selectOptions(
    await screen.findByLabelText('Версия роли'),
    JSON.stringify(['role-research', 2]),
  );
  return user;
}

test('review includes a running member and sends one exact assignment for future tasks', async () => {
  const snapshot = data();
  snapshot.config.value.bindings.push({
    id: 'binding-review',
    connectionId: 'connection-records',
    subject: { type: 'role', id: 'role-research' },
    tools: ['append_record'],
    allowWrite: true,
    required: true,
    namespace: 'reports',
  });
  const command = vi.fn().mockResolvedValue({ revision: 9 });
  render(
    <RoleEditor
      snapshot={snapshot}
      command={command}
      refresh={vi.fn().mockResolvedValue(snapshot)}
      connected
    />,
  );
  const user = await selectAndOpen();
  expect(screen.getByText('Новая инструкция для будущих задач')).toBeTruthy();
  await user.click(screen.getByText('Привязки MCP выбранной роли (1)'));
  const rights = screen.getByRole('list', { name: 'Привязки выбранной роли' });
  expect(rights.textContent).toContain('reports');
  expect(rights.textContent).toContain('append_record');
  expect(rights.textContent).toContain('Запись с подтверждением');
  expect(screen.getByText(/Экземпляров с незавершёнными задачами: 1/)).toBeTruthy();
  await user.click(screen.getByRole('button', { name: 'Применить к новым задачам' }));
  expect(command).toHaveBeenCalledExactlyOnceWith('/api/configure', {
    revision: 8,
    command: {
      type: 'pin_role',
      agentIds: ['agent-directory-0000', 'agent-directory-0003'],
      roleId: 'role-research',
      version: 2,
    },
  });
  expect(screen.getByText('Версия роли назначена выбранным экземплярам')).toBeTruthy();
});

test('stale or uncertain review never silently retries and retains the chosen version', async () => {
  const snapshot = data();
  const command = vi.fn().mockRejectedValue(new Error('Ответ потерян'));
  const refresh = vi.fn().mockResolvedValue(snapshot);
  const view = render(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />,
  );
  const user = await selectAndOpen();
  const changed = structuredClone(snapshot);
  changed.config.revision++;
  view.rerender(<RoleEditor snapshot={changed} command={command} refresh={refresh} connected />);
  const apply = screen.getByRole('button', {
    name: 'Применить к новым задачам',
  }) as HTMLButtonElement;
  expect(apply.disabled).toBe(true);
  refresh.mockResolvedValue(changed);
  await user.click(screen.getByRole('button', { name: 'Обновить проверку' }));
  await user.click(apply);
  expect(await screen.findByRole('alert')).toHaveProperty(
    'textContent',
    expect.stringContaining('Ответ потерян'),
  );
  expect(apply.disabled).toBe(true);
  expect(command).toHaveBeenCalledTimes(1);
  const done = structuredClone(changed);
  done.config.revision++;
  for (const index of [0, 3]) done.config.value.registry.agents[index].roleVersion = 2;
  refresh.mockResolvedValue(done);
  await user.click(screen.getByRole('button', { name: 'Обновить проверку' }));
  expect(await screen.findByText('Выбранная версия уже назначена всем экземплярам')).toBeTruthy();
  expect(apply.disabled).toBe(true);
});

test('an archived selected identity blocks the complete assignment and is not dropped from the review', async () => {
  const snapshot = data();
  const fresh = structuredClone(snapshot);
  fresh.config.value.registry.agents[0].status = 'archived';
  const command = vi.fn();
  render(
    <RoleEditor
      snapshot={snapshot}
      command={command}
      refresh={vi.fn().mockResolvedValue(fresh)}
      connected
    />,
  );
  await selectAndOpen();
  expect(screen.getByRole('list', { name: 'Выбранные экземпляры' }).textContent).toContain(
    'agent-directory-0000',
  );
  expect(
    (screen.getByRole('button', { name: 'Применить к новым задачам' }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(command).not.toHaveBeenCalled();
});

test('five thousand selected definitions and versions can be inspected without unbounded DOM options', async () => {
  const snapshot = instanceSnapshot();
  const first = snapshot.config.value.registry.roles[0];
  snapshot.config.value.registry.roles = Array.from({ length: 5000 }, (_, i) => ({
    ...first,
    version: i + 1,
  }));
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
  await user.click(screen.getByRole('button', { name: 'Назначить версию роли' }));
  const versions = (await screen.findByLabelText('Версия роли')) as HTMLSelectElement;
  expect(versions.options.length).toBeLessThanOrEqual(101);
  await user.type(screen.getByLabelText('Поиск роли'), 'v2');
  expect(versions.options.length).toBe(2);
  await user.selectOptions(versions, JSON.stringify(['role-research', 2]));
  const list = screen.getByRole('list', { name: 'Выбранные экземпляры' });
  expect(list.children.length).toBe(20);
  await user.click(screen.getByRole('button', { name: 'Далее' }));
  expect(list.textContent).toContain('agent-directory-0020');
  await user.type(screen.getByLabelText('Поиск в выбранных'), 'agent-directory-4998');
  expect(list.children.length).toBe(1);
  expect(list.textContent).toContain('agent-directory-4998');
});

test('reconnection cannot reuse the preceding review while a new read is pending', async () => {
  const snapshot = data();
  const refresh = vi.fn().mockResolvedValue(snapshot);
  const command = vi.fn().mockResolvedValue({});
  const view = render(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />,
  );
  const user = await selectAndOpen();
  const apply = screen.getByRole('button', {
    name: 'Применить к новым задачам',
  }) as HTMLButtonElement;
  view.rerender(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected={false} />,
  );
  expect(apply.disabled).toBe(true);
  let finish!: (value: typeof snapshot) => void;
  refresh.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  view.rerender(<RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />);
  expect(apply.disabled).toBe(true);
  await act(async () => finish(snapshot));
  await waitFor(() => expect(apply.disabled).toBe(false));
  await user.click(apply);
  expect(command).toHaveBeenCalledTimes(1);
});
