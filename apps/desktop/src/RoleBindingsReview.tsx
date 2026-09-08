import { useState } from 'react';
import type { Binding, Snapshot } from './types';

export function RoleBindingsReview({
  bindings,
  snapshot,
}: {
  bindings: Binding[];
  snapshot: Snapshot;
}) {
  const [page, setPage] = useState(0);
  const current = Math.min(page, Math.max(0, Math.ceil(bindings.length / 5) - 1));
  const names = new Map(
    snapshot.connections.map((connection) => [connection.value.id, connection.value.name]),
  );
  return (
    <details>
      <summary>Привязки MCP выбранной роли ({bindings.length})</summary>
      {bindings.length === 0 ? (
        <p className="small muted">У роли нет собственных привязок.</p>
      ) : (
        <>
          <ul className="archive-selection" aria-label="Привязки выбранной роли">
            {bindings.slice(current * 5, (current + 1) * 5).map((binding) => (
              <li key={binding.id}>
                <strong>{names.get(binding.connectionId) ?? binding.connectionId}</strong>
                <code>{binding.connectionId}</code>
                <span>
                  {binding.allowWrite ? 'Запись с подтверждением' : 'Только чтение'} ·{' '}
                  {binding.required ? 'Обязательное подключение' : 'Необязательное подключение'}
                </span>
                <span>Область данных: {binding.namespace}</span>
                <details>
                  <summary>Инструменты ({binding.tools.length})</summary>
                  <p className="small">{binding.tools.join(', ')}</p>
                </details>
              </li>
            ))}
          </ul>
          <nav className="instance-pager" aria-label="Страницы привязок роли">
            <button disabled={current === 0} onClick={() => setPage(current - 1)}>
              Предыдущие привязки
            </button>
            <span>
              {current + 1} / {Math.ceil(bindings.length / 5)}
            </span>
            <button
              disabled={(current + 1) * 5 >= bindings.length}
              onClick={() => setPage(current + 1)}
            >
              Следующие привязки
            </button>
          </nav>
        </>
      )}
      <p className="small muted">
        Персональные привязки сохранятся. Разрешения старых задач не расширяются; доступ проверяется
        при выполнении.
      </p>
    </details>
  );
}
