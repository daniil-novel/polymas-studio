import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RoleEditor } from '../../apps/desktop/src/RoleEditor';
import { instanceSnapshot } from '../e2e/fixtures/instances';

afterEach(() => {
  cleanup();
  window.localStorage.clear();
  vi.restoreAllMocks();
});
const key = (id: string) => `polymas:instance-columns:v1:${id}`;
async function open() {
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: /Экземпляры/ }));
  await user.click(screen.getByText('Колонки', { exact: true }));
  return user;
}

test('hiding optional columns preserves selection and persists per workspace across remount', async () => {
  const snapshot = instanceSnapshot(5);
  const command = vi.fn();
  const view = render(<RoleEditor snapshot={snapshot} command={command} />);
  const user = await open();
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }));
  await user.click(screen.getByRole('checkbox', { name: 'Показывать ID' }));
  await user.click(screen.getByRole('checkbox', { name: 'Показывать исполнителя' }));
  const table = screen.getByRole('table');
  expect(within(table).queryByRole('columnheader', { name: /^ID$/ })).toBeNull();
  expect(within(table).queryByRole('columnheader', { name: 'Исполнитель' })).toBeNull();
  expect(
    (screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }) as HTMLInputElement)
      .checked,
  ).toBe(true);
  expect(table.getAttribute('aria-colcount')).toBe('7');
  view.unmount();
  render(<RoleEditor snapshot={snapshot} command={command} />);
  await open();
  expect(
    (screen.getByRole('checkbox', { name: 'Показывать ID' }) as HTMLInputElement).checked,
  ).toBe(false);
  expect(
    (screen.getByRole('checkbox', { name: 'Показывать исполнителя' }) as HTMLInputElement).checked,
  ).toBe(false);
  expect(command).not.toHaveBeenCalled();
});

test('workspace changes never carry a hidden-column preference or selection to another workspace', async () => {
  const first = instanceSnapshot(5);
  const second = structuredClone(first);
  second.config.value.id = 'other-workspace';
  const command = vi.fn();
  const view = render(<RoleEditor snapshot={first} command={command} />);
  const user = await open();
  await user.click(screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }));
  await user.click(screen.getByRole('checkbox', { name: 'Показывать холон' }));
  view.rerender(<RoleEditor snapshot={second} command={command} />);
  expect(
    within(screen.getByRole('table')).getByRole('columnheader', { name: 'Холон' }),
  ).toBeTruthy();
  expect(
    (screen.getByRole('checkbox', { name: 'Выбрать Исследователь · 0001' }) as HTMLInputElement)
      .checked,
  ).toBe(false);
  view.rerender(<RoleEditor snapshot={first} command={command} />);
  expect(
    within(screen.getByRole('table')).queryByRole('columnheader', { name: 'Холон' }),
  ).toBeNull();
});

test('invalid preferences retain essential columns and reset restores the default view', async () => {
  const snapshot = instanceSnapshot(5);
  window.localStorage.setItem(
    key(snapshot.config.value.id),
    JSON.stringify({ version: 1, hidden: ['name', 'state', 'unknown'] }),
  );
  render(<RoleEditor snapshot={snapshot} command={vi.fn()} />);
  const user = await open();
  expect(within(screen.getByRole('table')).getAllByRole('columnheader')).toHaveLength(9);
  expect(
    (screen.getByRole('button', { name: 'Стандартные колонки' }) as HTMLButtonElement).disabled,
  ).toBe(false);
  await user.click(screen.getByRole('checkbox', { name: 'Показывать задачу' }));
  await user.click(screen.getByRole('button', { name: 'Стандартные колонки' }));
  expect(within(screen.getByRole('table')).getAllByRole('columnheader')).toHaveLength(9);
  expect(JSON.parse(window.localStorage.getItem(key(snapshot.config.value.id))!).hidden).toEqual(
    [],
  );
});

test('storage and clipboard failures keep the directory usable with a visible full identity', async () => {
  const snapshot = instanceSnapshot(5);
  render(<RoleEditor snapshot={snapshot} command={vi.fn()} />);
  const user = await open();
  vi.spyOn(window.Storage.prototype, 'setItem').mockImplementation(() => {
    throw new Error('Quota');
  });
  await user.click(screen.getByRole('checkbox', { name: 'Показывать ID' }));
  expect(screen.getByText(/Не удалось сохранить колонки/)).toBeTruthy();
  expect(
    within(screen.getByRole('table')).queryByRole('columnheader', { name: /^ID$/ }),
  ).toBeNull();
  vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('Clipboard disabled'));
  await user.click(screen.getByRole('button', { name: 'Скопировать ID Исследователь · 0001' }));
  expect((await screen.findByRole('alert')).textContent).toContain('agent-directory-0000');
});
