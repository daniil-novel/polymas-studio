import { afterAll, afterEach, beforeAll, expect, test, vi } from 'vitest';
import { act, cleanup, render, screen, within, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RoleEditor } from '../../apps/desktop/src/RoleEditor';
import { instanceSnapshot } from '../e2e/fixtures/instances';
import {
  saveDuplication,
  clearDuplication,
  readDuplication,
  type PendingDuplication,
} from '../../apps/desktop/src/role-duplication-recovery';

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
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.localStorage.clear();
});
afterAll(() => {
  HTMLDialogElement.prototype.showModal = showModal;
  HTMLDialogElement.prototype.close = close;
});

test('independent pending operations never overwrite each other and clearing one preserves the others', () => {
  const first: PendingDuplication = {
    version: 1,
    workspaceId: 'workspace-fixture',
    request: {
      revision: 8,
      command: {
        type: 'duplicate_role',
        key: '00000000-0000-4000-8000-000000000001',
        roleId: 'role-one',
        sourceVersion: 1,
        name: 'First copy',
      },
    },
  };
  const second: PendingDuplication = {
    ...first,
    request: {
      ...first.request,
      command: {
        ...first.request.command,
        key: '00000000-0000-4000-8000-000000000002',
        name: 'Second copy',
      },
    },
  };
  saveDuplication(first);
  saveDuplication(second);
  expect(readDuplication(first.workspaceId).pending).toEqual(first);
  clearDuplication(first);
  expect(readDuplication(first.workspaceId).pending).toEqual(second);
  expect(readDuplication('different-workspace').pending).toBeNull();
  clearDuplication(second);
  expect(readDuplication(first.workspaceId).pending).toBeNull();
});
function fixture() {
  const state = instanceSnapshot(5);
  state.config.value.registry.roles.push({
    ...state.config.value.registry.roles[0],
    version: 2,
    instruction: 'Новая инструкция',
    sandbox: 'workspace-write',
  });
  return state;
}
const recoveryKey = (id: string, key: string) => `polymas:role-duplication:v1:${id}:${key}`;
function reply(state: ReturnType<typeof fixture>, name = 'Новая роль') {
  return {
    revision: 9,
    result: {
      sourceRoleId: 'role-research',
      sourceVersion: 1,
      role: { ...state.config.value.registry.roles[0], id: 'role-independent', name, version: 1 },
    },
  };
}
async function open() {
  const user = userEvent.setup();
  await user.selectOptions(screen.getByLabelText('Выбрать роль'), 'role-research');
  await user.type(screen.getByLabelText('Ролевая инструкция'), ' Черновик владельца');
  await user.click(screen.getByRole('button', { name: 'Сравнить версии' }));
  await user.click(screen.getByRole('button', { name: 'Дублировать исходную версию' }));
  await screen.findByText('Новая независимая роль · v1');
  return user;
}

test('copying reviews the saved source and persists an exact request before one POST without losing the draft', async () => {
  const state = fixture();
  const command = vi.fn().mockImplementation(async (_path, body) => {
    expect(
      JSON.parse(window.localStorage.getItem(recoveryKey(state.config.value.id, body.command.key))!)
        .request,
    ).toEqual(body);
    return reply(state);
  });
  render(
    <RoleEditor
      snapshot={state}
      command={command}
      refresh={vi.fn().mockResolvedValue(state)}
      connected
    />,
  );
  const user = await open();
  const dialog = screen.getByRole('dialog', { name: 'Дублирование роли' });
  await user.clear(within(dialog).getByLabelText('Название новой роли'));
  await user.type(within(dialog).getByLabelText('Название новой роли'), 'Новая роль');
  await user.click(within(dialog).getByLabelText('Все поля новой роли'));
  expect(within(dialog).getAllByRole('article')).toHaveLength(11);
  expect(
    within(dialog).getByRole('article', { name: 'Ролевая инструкция' }).textContent,
  ).not.toContain('Черновик владельца');
  await user.click(within(dialog).getByRole('button', { name: 'Создать копию роли' }));
  expect(command).toHaveBeenCalledExactlyOnceWith('/api/configure', {
    revision: 8,
    command: {
      type: 'duplicate_role',
      key: expect.any(String),
      roleId: 'role-research',
      sourceVersion: 1,
      name: 'Новая роль',
    },
  });
  expect(await screen.findByText(/Создана роль «Новая роль» · v1/)).toBeTruthy();
  expect(readDuplication(state.config.value.id).pending).toBeNull();
  expect((screen.getByLabelText('Ролевая инструкция') as HTMLTextAreaElement).value).toContain(
    'Черновик владельца',
  );
});

test('an uncertain copy survives remount and retries only the same key and fields after fresh review', async () => {
  const state = fixture();
  const refresh = vi.fn().mockResolvedValue(state);
  const command = vi.fn().mockRejectedValue(new Error('Ответ потерян'));
  const view = render(
    <RoleEditor snapshot={state} command={command} refresh={refresh} connected />,
  );
  const user = await open();
  await user.clear(screen.getByLabelText('Название новой роли'));
  await user.type(screen.getByLabelText('Название новой роли'), 'Новая роль');
  await user.click(screen.getByRole('button', { name: 'Создать копию роли' }));
  expect(await screen.findByRole('alert')).toHaveProperty(
    'textContent',
    expect.stringContaining('Ответ потерян'),
  );
  expect(screen.getByLabelText('Название новой роли')).toHaveProperty('disabled', true);
  expect(screen.getByRole('button', { name: 'Повторить сохранённый запрос' })).toHaveProperty(
    'disabled',
    true,
  );
  const original = command.mock.calls[0][1];
  view.unmount();
  const fresh = structuredClone(state);
  fresh.config.revision = 10;
  refresh.mockResolvedValue(fresh);
  command.mockResolvedValue(reply(state));
  render(<RoleEditor snapshot={fresh} command={command} refresh={refresh} connected />);
  await user.click(screen.getByRole('button', { name: 'Продолжить дублирование' }));
  const retry = await screen.findByRole('button', { name: 'Повторить сохранённый запрос' });
  await waitFor(() => expect(retry).toHaveProperty('disabled', false));
  expect(command).toHaveBeenCalledTimes(1);
  await user.click(retry);
  expect(command.mock.calls[1][1]).toEqual({ ...original, revision: 10 });
  expect(await screen.findByText(/Создана роль «Новая роль» · v1/)).toBeTruthy();
});

test('stale data and reconnect require a fresh read, and byte limits or failed persistence prohibit POST', async () => {
  const state = fixture();
  const refresh = vi.fn().mockResolvedValue(state);
  const command = vi.fn();
  const view = render(
    <RoleEditor snapshot={state} command={command} refresh={refresh} connected />,
  );
  const user = await open();
  const apply = screen.getByRole('button', { name: 'Создать копию роли' });
  const fresh = structuredClone(state);
  fresh.config.revision++;
  view.rerender(<RoleEditor snapshot={fresh} command={command} refresh={refresh} connected />);
  expect(apply).toHaveProperty('disabled', true);
  view.rerender(
    <RoleEditor snapshot={fresh} command={command} refresh={refresh} connected={false} />,
  );
  let resolve!: (value: typeof state) => void;
  refresh.mockImplementationOnce(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  view.rerender(<RoleEditor snapshot={fresh} command={command} refresh={refresh} connected />);
  expect(apply).toHaveProperty('disabled', true);
  await act(async () => resolve(fresh));
  await waitFor(() => expect(apply).toHaveProperty('disabled', false));
  await user.clear(screen.getByLabelText('Название новой роли'));
  await user.type(screen.getByLabelText('Название новой роли'), 'я'.repeat(101));
  expect(apply).toHaveProperty('disabled', true);
  await user.clear(screen.getByLabelText('Название новой роли'));
  await user.type(screen.getByLabelText('Название новой роли'), 'Новая роль');
  vi.spyOn(window.Storage.prototype, 'setItem').mockImplementation(() => {
    throw new Error('Quota');
  });
  await user.click(apply);
  expect(await screen.findByRole('alert')).toHaveProperty(
    'textContent',
    expect.stringContaining('Запрос не отправлен'),
  );
  expect(command).not.toHaveBeenCalled();
});

test('malformed success cannot erase a recoverable operation or report a created role', async () => {
  const state = fixture();
  const wrong = reply(state);
  wrong.result.role.id = 'role-research';
  const command = vi.fn().mockResolvedValue(wrong);
  render(
    <RoleEditor
      snapshot={state}
      command={command}
      refresh={vi.fn().mockResolvedValue(state)}
      connected
    />,
  );
  const user = await open();
  await user.click(screen.getByRole('button', { name: 'Создать копию роли' }));
  expect(await screen.findByRole('alert')).toHaveProperty(
    'textContent',
    expect.stringContaining('Ответ не подтверждает'),
  );
  expect(readDuplication(state.config.value.id).pending).not.toBeNull();
  expect(screen.queryByText(/Создана роль «/)).toBeNull();
});

test('failed receipt cleanup retries local cleanup without another POST, and foreign pending data fails closed', async () => {
  const state = fixture();
  const command = vi.fn().mockResolvedValue(reply(state, 'Исследователь · копия'));
  const view = render(
    <RoleEditor
      snapshot={state}
      command={command}
      refresh={vi.fn().mockResolvedValue(state)}
      connected
    />,
  );
  const user = await open();
  const remove = vi.spyOn(window.Storage.prototype, 'removeItem').mockImplementation(() => {
    throw new Error('Denied');
  });
  await user.click(screen.getByRole('button', { name: 'Создать копию роли' }));
  const clear = await screen.findByRole('button', { name: 'Завершить локальную запись' });
  expect(screen.queryByRole('button', { name: 'Повторить сохранённый запрос' })).toBeNull();
  remove.mockRestore();
  await user.click(clear);
  expect(command).toHaveBeenCalledTimes(1);
  view.unmount();
  window.localStorage.setItem(
    recoveryKey(state.config.value.id, '00000000-0000-4000-8000-000000000001'),
    JSON.stringify({ version: 1, workspaceId: 'foreign', request: {} }),
  );
  render(
    <RoleEditor
      snapshot={state}
      command={command}
      refresh={vi.fn().mockResolvedValue(state)}
      connected
    />,
  );
  await open();
  expect(screen.getByRole('button', { name: 'Создать копию роли' })).toHaveProperty(
    'disabled',
    true,
  );
  expect(screen.getByText(/Не удалось прочитать незавершённый запрос/)).toBeTruthy();
  expect(command).toHaveBeenCalledTimes(1);
});
