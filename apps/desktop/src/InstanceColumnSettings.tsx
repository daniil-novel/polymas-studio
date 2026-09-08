import { useState } from 'react';

export const instanceColumns = [
  { id: 'selection', title: 'Выбор', width: 45, optional: false },
  { id: 'name', title: 'Экземпляр', width: 230, optional: false },
  { id: 'id', title: 'ID', width: 220, optional: true, toggle: 'Показывать ID' },
  {
    id: 'role',
    title: 'Версия роли',
    width: 205,
    optional: true,
    toggle: 'Показывать версию роли',
  },
  { id: 'holon', title: 'Холон', width: 180, optional: true, toggle: 'Показывать холон' },
  { id: 'state', title: 'Состояние', width: 185, optional: false },
  { id: 'task', title: 'Задача', width: 200, optional: true, toggle: 'Показывать задачу' },
  {
    id: 'worker',
    title: 'Исполнитель',
    width: 170,
    optional: true,
    toggle: 'Показывать исполнителя',
  },
  { id: 'copy', title: 'Копия ID', width: 85, optional: false },
] as const;
export type InstanceColumnId = (typeof instanceColumns)[number]['id'];
const optional = instanceColumns.filter((column) => column.optional);
function read(key: string): { hidden: InstanceColumnId[]; warning: string } {
  try {
    const text = window.localStorage.getItem(key);
    if (!text) return { hidden: [], warning: '' };
    if (text.length > 2048) throw new Error('Invalid preference size');
    const value: unknown = JSON.parse(text);
    if (
      !value ||
      typeof value !== 'object' ||
      !('version' in value) ||
      value.version !== 1 ||
      !('hidden' in value) ||
      !Array.isArray(value.hidden) ||
      value.hidden.length > optional.length ||
      !value.hidden.every((id: unknown) => optional.some((column) => column.id === id)) ||
      new Set(value.hidden).size !== value.hidden.length
    )
      throw new Error('Invalid preferences');
    return { hidden: value.hidden as InstanceColumnId[], warning: '' };
  } catch {
    return { hidden: [], warning: 'Не удалось прочитать колонки. Используется стандартный вид.' };
  }
}
export function useInstanceColumns(workspaceId: string) {
  const key = `polymas:instance-columns:v1:${workspaceId}`;
  const [preference, setPreference] = useState(() => read(key));
  function save(hidden: InstanceColumnId[]) {
    setPreference({ hidden, warning: '' });
    try {
      window.localStorage.setItem(key, JSON.stringify({ version: 1, hidden }));
    } catch {
      setPreference({
        hidden,
        warning: 'Не удалось сохранить колонки. Настройка действует в текущем окне.',
      });
    }
  }
  return {
    ...preference,
    save,
    columns: instanceColumns.filter((column) => !preference.hidden.includes(column.id)),
  };
}
export function InstanceColumnSettings({
  hidden,
  warning,
  save,
}: ReturnType<typeof useInstanceColumns>) {
  return (
    <details className="instance-column-settings">
      <summary>Колонки</summary>
      <fieldset>
        <legend>Показать в каталоге</legend>
        {optional.map((column) => (
          <label key={column.id}>
            <input
              type="checkbox"
              checked={!hidden.includes(column.id)}
              onChange={(event) =>
                save(
                  event.target.checked
                    ? hidden.filter((id) => id !== column.id)
                    : [...hidden, column.id],
                )
              }
            />
            {column.toggle}
          </label>
        ))}
      </fieldset>
      <div>
        <button disabled={hidden.length === 0 && !warning} onClick={() => save([])}>
          Стандартные колонки
        </button>
        <p className="small muted">
          Выбор, имя, состояние и копирование ID видны всегда. Настройка сохраняется для этой
          рабочей области.
        </p>
      </div>
      {warning && (
        <p role="status" className="small warning">
          {warning}
        </p>
      )}
    </details>
  );
}
