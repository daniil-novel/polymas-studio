import { afterAll, afterEach, beforeAll, expect, test, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RoleEditor } from '../../apps/desktop/src/RoleEditor';
import { instanceSnapshot } from '../e2e/fixtures/instances';
import type { Snapshot } from '../../apps/desktop/src/types';

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
async function openEditor() {
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByRole('button', { name: 'Переименовать Исследователь · 0001' }));
  return user;
}
test('rename waits for a fresh read and sends only the exact agent identity and reviewed revision', async () => {
  const snapshot = instanceSnapshot(3);
  let finish: (value: Snapshot) => void = () => {};
  const refresh = vi.fn(
    () =>
      new Promise<Snapshot>((resolve) => {
        finish = resolve;
      }),
  );
  const command = vi.fn().mockResolvedValue({ revision: 10 });
  render(<RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />);
  const user = await openEditor();
  expect(
    (screen.getByRole('button', { name: 'Сохранить имя' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  const fresh = structuredClone(snapshot);
  fresh.config.revision = 9;
  finish(fresh);
  await waitFor(() =>
    expect((screen.getByLabelText('Имя экземпляра') as HTMLInputElement).disabled).toBe(false),
  );
  await user.clear(screen.getByLabelText('Имя экземпляра'));
  await user.type(screen.getByLabelText('Имя экземпляра'), 'Проверяющий');
  await user.click(screen.getByRole('button', { name: 'Сохранить имя' }));
  expect(command).toHaveBeenCalledExactlyOnceWith('/api/configure', {
    revision: 9,
    command: { type: 'rename_agent', agentId: 'agent-directory-0000', name: 'Проверяющий' },
  });
  expect(await screen.findByText('Имя экземпляра сохранено')).toBeTruthy();
});

test('a stale or uncertain rename preserves the draft and requires a new read before another write', async () => {
  const snapshot = instanceSnapshot(3);
  const refresh = vi.fn().mockResolvedValue(snapshot);
  const command = vi.fn().mockRejectedValue(new Error('Ответ потерян'));
  const view = render(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />,
  );
  const user = await openEditor();
  await user.clear(screen.getByLabelText('Имя экземпляра'));
  await user.type(screen.getByLabelText('Имя экземпляра'), 'Мой черновик');
  const changed = structuredClone(snapshot);
  changed.config.revision = 9;
  changed.config.value.registry.agents[0].name = 'Изменено в другом окне';
  view.rerender(<RoleEditor snapshot={changed} command={command} refresh={refresh} connected />);
  expect(
    (screen.getByRole('button', { name: 'Сохранить имя' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  expect((screen.getByLabelText('Имя экземпляра') as HTMLInputElement).value).toBe('Мой черновик');
  refresh.mockResolvedValue(changed);
  await user.click(screen.getByRole('button', { name: 'Обновить данные' }));
  await user.click(screen.getByRole('button', { name: 'Сохранить имя' }));
  expect(await screen.findByText(/Ответ потерян/)).toBeTruthy();
  expect(
    (screen.getByRole('button', { name: 'Сохранить имя' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  expect(command).toHaveBeenCalledTimes(1);
  changed.config.revision = 10;
  changed.config.value.registry.agents[0].name = 'Мой черновик';
  await user.click(screen.getByRole('button', { name: 'Обновить данные' }));
  expect(await screen.findByText('Это имя уже сохранено')).toBeTruthy();
  expect(command).toHaveBeenCalledTimes(1);
});

test('disconnect disables rename and the name limit counts UTF-8 bytes', async () => {
  const snapshot = instanceSnapshot(3);
  const refresh = vi.fn().mockResolvedValue(snapshot);
  const command = vi.fn();
  const view = render(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected />,
  );
  const user = await openEditor();
  await user.clear(screen.getByLabelText('Имя экземпляра'));
  await user.type(screen.getByLabelText('Имя экземпляра'), 'Я'.repeat(101));
  expect(
    (screen.getByRole('button', { name: 'Сохранить имя' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  await user.clear(screen.getByLabelText('Имя экземпляра'));
  await user.type(screen.getByLabelText('Имя экземпляра'), 'Разрешённое имя');
  view.rerender(
    <RoleEditor snapshot={snapshot} command={command} refresh={refresh} connected={false} />,
  );
  expect(
    (screen.getByRole('button', { name: 'Сохранить имя' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  expect(command).not.toHaveBeenCalled();
});
