import { afterAll, afterEach, beforeAll, expect, test, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
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
  const snapshot = instanceSnapshot(5);
  const first = snapshot.config.value.registry.roles[0];
  snapshot.config.value.registry.roles.push({
    ...first,
    version: 2,
    instruction: 'Сопоставляй источники и отмечай расхождения.',
    sandbox: 'workspace-write',
    maxOutputTokens: 2048,
  });
  return snapshot;
}
async function open() {
  const user = userEvent.setup();
  await user.selectOptions(screen.getByLabelText('Выбрать роль'), 'role-research');
  await user.click(screen.getByRole('button', { name: 'Сравнить версии' }));
  return user;
}

test('compares every saved field and distinguishes changed permissions and unset limits without commands', async () => {
  const snapshot = fixture();
  const command = vi.fn();
  render(<RoleEditor snapshot={snapshot} command={command} connected />);
  const user = await open();
  const dialog = screen.getByRole('dialog', { name: 'Сравнение версий роли' });
  expect(within(dialog).getByText('Изменённых полей: 3 из 11')).toBeTruthy();
  expect(within(dialog).getAllByRole('article')).toHaveLength(11);
  expect(within(dialog).getByRole('article', { name: 'Ролевая инструкция' }).textContent).toContain(
    'Проверять источники.',
  );
  expect(
    within(dialog).getByRole('article', { name: 'Доступ к рабочей папке' }).textContent,
  ).toContain('Запись в рабочую папку агента');
  const output = within(dialog).getByRole('article', { name: 'Лимит токенов результата' });
  expect(output.textContent).toContain('Не задан');
  expect(output.textContent).toContain('2048');
  await user.click(within(dialog).getByRole('checkbox', { name: 'Только изменённые поля' }));
  expect(within(dialog).getAllByRole('article')).toHaveLength(3);
  expect(command).not.toHaveBeenCalled();
});

test('a saved comparison is not replaced by a new revision and closing preserves an unsaved draft', async () => {
  const snapshot = fixture();
  const command = vi.fn();
  const view = render(<RoleEditor snapshot={snapshot} command={command} connected />);
  const user = userEvent.setup();
  await user.selectOptions(screen.getByLabelText('Выбрать роль'), 'role-research');
  await user.type(screen.getByLabelText('Ролевая инструкция'), ' Локальный черновик');
  await user.click(screen.getByRole('button', { name: 'Сравнить версии' }));
  const next = structuredClone(snapshot);
  next.config.revision++;
  next.config.value.registry.roles.push({
    ...next.config.value.registry.roles[1],
    version: 3,
    instruction: 'Другой редактор',
  });
  view.rerender(<RoleEditor snapshot={next} command={command} connected={false} />);
  const dialog = screen.getByRole('dialog', { name: 'Сравнение версий роли' });
  expect(dialog.textContent).not.toContain('Другой редактор');
  expect(dialog.textContent).not.toContain('Локальный черновик');
  expect(within(dialog).getByText(/Показана сохранённая ревизия 8/)).toBeTruthy();
  expect(within(dialog).getByText(/Нет связи/)).toBeTruthy();
  await user.click(within(dialog).getByRole('button', { name: 'Закрыть сравнение' }));
  expect((screen.getByLabelText('Ролевая инструкция') as HTMLTextAreaElement).value).toContain(
    'Локальный черновик',
  );
  expect(command).not.toHaveBeenCalled();
});

test('thousands of versions have bounded selectors, exact search and complete Unicode text pages', async () => {
  const snapshot = fixture();
  const first = snapshot.config.value.registry.roles[0];
  const long = 'я'.repeat(32767) + '🧠' + 'Конец инструкции';
  snapshot.config.value.registry.roles = Array.from({ length: 5000 }, (_, i) => ({
    ...first,
    version: i + 1,
    instruction: i === 0 ? long : `Версия ${i + 1}`,
  }));
  render(<RoleEditor snapshot={snapshot} command={vi.fn()} connected />);
  const user = await open();
  const dialog = screen.getByRole('dialog', { name: 'Сравнение версий роли' });
  const before = within(dialog).getByRole('combobox', {
    name: 'Исходная версия',
  }) as HTMLSelectElement;
  expect(before.options.length).toBeLessThanOrEqual(101);
  await user.type(within(dialog).getByLabelText('Поиск версии'), 'v1');
  expect(before.options.length).toBeLessThanOrEqual(2);
  await user.selectOptions(before, '1');
  const instruction = within(dialog).getByRole('article', { name: 'Ролевая инструкция' });
  const original = within(instruction).getByRole('region', { name: 'Исходное значение' });
  let joined = within(original).getByRole('textbox').textContent;
  expect(joined!.length).toBeLessThanOrEqual(32768);
  await user.click(within(original).getByRole('button', { name: 'Следующая часть' }));
  joined += within(original).getByRole('textbox').textContent!;
  expect(joined).toBe(long);
});
