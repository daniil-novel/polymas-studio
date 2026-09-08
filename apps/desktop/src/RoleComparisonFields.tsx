import { useState } from 'react';
import { TEXT_PAGE_SIZE, textPage } from './text-pages';
import type { Role, RoleDraft } from './types';

const labels: Record<keyof RoleDraft, string> = {
  name: 'Название роли',
  description: 'Описание',
  instruction: 'Ролевая инструкция',
  outputContract: 'Формат результата',
  acceptanceCriteria: 'Критерии приёмки',
  escalationRule: 'Правило эскалации',
  model: 'Модель',
  sandbox: 'Доступ к рабочей папке',
  timeoutSeconds: 'Таймаут, секунд',
  maxTurns: 'Максимум попыток',
  maxOutputTokens: 'Лимит токенов результата',
};
const fields = Object.keys(labels) as (keyof RoleDraft)[];
function display(role: Role, field: keyof RoleDraft) {
  const value = role[field];
  if (value === null) return field === 'model' ? 'По умолчанию в Codex' : 'Не задан';
  if (value === '') return 'Пустое значение';
  if (field === 'sandbox')
    return value === 'read-only' ? 'Только чтение' : 'Запись в рабочую папку агента';
  return String(value);
}
function TextValue({ text, label, version }: { text: string; label: string; version: number }) {
  const [page, setPage] = useState(0);
  const count = Math.max(1, Math.ceil(text.length / TEXT_PAGE_SIZE));
  return (
    <section className="role-diff-value" aria-label={label}>
      <span className="small muted">
        {label} · v{version}
      </span>
      <pre
        role="textbox"
        aria-label={label}
        aria-readonly="true"
        aria-multiline="true"
        tabIndex={0}
      >
        {textPage(text, page)}
      </pre>
      {count > 1 && (
        <nav className="role-diff-pages" aria-label={`Части: ${label}`}>
          <button disabled={page === 0} onClick={() => setPage(page - 1)}>
            Предыдущая часть
          </button>
          <span>
            {page + 1} / {count}
          </span>
          <button disabled={page + 1 === count} onClick={() => setPage(page + 1)}>
            Следующая часть
          </button>
        </nav>
      )}
    </section>
  );
}
export const ROLE_FIELD_COUNT = fields.length;
export function changedRoleFields(before: Role, after: Role) {
  return fields.filter((field) => before[field] !== after[field]);
}
export function RoleComparisonFields({
  before,
  after,
  onlyChanged = false,
}: {
  before: Role;
  after: Role;
  onlyChanged?: boolean;
}) {
  const changed = changedRoleFields(before, after);
  return (
    <>
      {fields
        .filter((field) => !onlyChanged || changed.includes(field))
        .map((field) => (
          <article
            key={`${field}:${before.version}:${after.version}`}
            className={`role-diff-field ${changed.includes(field) ? 'changed' : ''}`}
            aria-label={labels[field]}
          >
            <h3>
              {labels[field]}
              <span>{changed.includes(field) ? 'Изменено' : 'Без изменений'}</span>
            </h3>
            <div className="role-diff-pair">
              <TextValue
                text={display(before, field)}
                label="Исходное значение"
                version={before.version}
              />
              <TextValue
                text={display(after, field)}
                label="Новое значение"
                version={after.version}
              />
            </div>
          </article>
        ))}
    </>
  );
}
