import { afterAll, afterEach, beforeAll, expect, test, vi } from 'vitest';
import { act, cleanup, render, screen, within, waitFor } from '@testing-library/react';
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
function fixture() {
  const state = instanceSnapshot(5);
  state.config.value.registry.roles.push({
    ...state.config.value.registry.roles[0],
    version: 2,
    instruction: 'Изменённая инструкция',
    sandbox: 'workspace-write',
  });
  return state;
}
async function open() {
  const user = userEvent.setup();
  await user.selectOptions(screen.getByLabelText('Выбрать роль'), 'role-research');
  await user.type(screen.getByLabelText('Ролевая инструкция'), ' Локальный черновик');
  await user.click(screen.getByRole('button', { name: 'Сравнить версии' }));
  await user.click(screen.getByRole('button', { name: 'Восстановить исходную версию' }));
  return user;
}

test('restoring reviews fresh current fields and submits source identity once while preserving the editor draft', async () => {
  const state = fixture();
  const command = vi
    .fn()
    .mockResolvedValue({
      revision: 9,
      result: { sourceVersion: 1, role: { ...state.config.value.registry.roles[0], version: 3 } },
    });
  const refresh = vi.fn().mockResolvedValue(state);
  render(<RoleEditor snapshot={state} command={command} refresh={refresh} connected />);
  const user = await open();
  const dialog = screen.getByRole('dialog', { name: 'Восстановление версии роли' });
  expect(await within(dialog).findByText('Будет создана версия v3 из v1')).toBeTruthy();
  expect(
    within(dialog).getByRole('article', { name: 'Доступ к рабочей папке' }).textContent,
  ).toContain('Только чтение');
  await user.click(within(dialog).getByRole('button', { name: 'Создать восстановленную версию' }));
  expect(command).toHaveBeenCalledExactlyOnceWith('/api/configure', {
    revision: 8,
    command: {
      type: 'restore_role',
      roleId: 'role-research',
      sourceVersion: 1,
      expectedVersion: 2,
    },
  });
  expect(await screen.findByText(/Восстановленная версия v3 создана/)).toBeTruthy();
  expect((screen.getByLabelText('Ролевая инструкция') as HTMLTextAreaElement).value).toContain(
    'Локальный черновик',
  );
});

test('a lost response requires another read and already-restored fields prohibit another version', async () => {
  const state = fixture();
  const command = vi.fn().mockRejectedValue(new Error('Ответ потерян'));
  const refresh = vi.fn().mockResolvedValue(state);
  render(<RoleEditor snapshot={state} command={command} refresh={refresh} connected />);
  const user = await open();
  const apply = (await screen.findByRole('button', {
    name: 'Создать восстановленную версию',
  })) as HTMLButtonElement;
  await user.click(apply);
  expect(await screen.findByRole('alert')).toHaveProperty(
    'textContent',
    expect.stringContaining('Ответ потерян'),
  );
  expect(apply.disabled).toBe(true);
  const done = structuredClone(state);
  done.config.revision++;
  done.config.value.registry.roles.push({ ...done.config.value.registry.roles[0], version: 3 });
  refresh.mockResolvedValue(done);
  await user.click(screen.getByRole('button', { name: 'Обновить проверку' }));
  expect(await screen.findByText('Последняя версия уже содержит выбранные поля')).toBeTruthy();
  expect(apply.disabled).toBe(true);
  expect(command).toHaveBeenCalledTimes(1);
});

test('stale and reconnect block submission until a fresh current version is reviewed', async () => {
  const state = fixture();
  const refresh = vi.fn().mockResolvedValue(state);
  const command = vi.fn().mockResolvedValue({});
  const view = render(
    <RoleEditor snapshot={state} command={command} refresh={refresh} connected />,
  );
  const user = await open();
  const apply = (await screen.findByRole('button', {
    name: 'Создать восстановленную версию',
  })) as HTMLButtonElement;
  const latest = structuredClone(state);
  latest.config.revision++;
  latest.config.value.registry.roles.push({
    ...latest.config.value.registry.roles[1],
    version: 3,
    instruction: 'Свежая инструкция',
  });
  view.rerender(<RoleEditor snapshot={latest} command={command} refresh={refresh} connected />);
  expect(apply.disabled).toBe(true);
  view.rerender(
    <RoleEditor snapshot={latest} command={command} refresh={refresh} connected={false} />,
  );
  let resolve!: (value: typeof state) => void;
  refresh.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  view.rerender(<RoleEditor snapshot={latest} command={command} refresh={refresh} connected />);
  expect(apply.disabled).toBe(true);
  await act(async () => resolve(latest));
  await waitFor(() => expect(apply.disabled).toBe(false));
  expect(screen.getByText('Будет создана версия v4 из v1')).toBeTruthy();
  await user.click(screen.getByRole('button', { name: 'Закрыть восстановление' }));
  expect(command).not.toHaveBeenCalled();
});
