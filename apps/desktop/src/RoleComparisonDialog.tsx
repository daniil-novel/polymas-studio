import { useState } from 'react';
import { Modal } from './Modal';
import { TEXT_PAGE_SIZE, textPage } from './text-pages';
import type { Role, RoleDraft } from './types';
import './role-comparison.css';

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
export interface RoleComparison {
  versions: Role[];
  revision: number;
}
export function RoleComparisonDialog({
  comparison,
  currentRevision,
  connected,
  onClose,
}: {
  comparison: RoleComparison;
  currentRevision: number;
  connected: boolean;
  onClose: () => void;
}) {
  const { versions, revision } = comparison;
  const [beforeVersion, setBefore] = useState(versions[1]?.version ?? versions[0]?.version);
  const [afterVersion, setAfter] = useState(versions[0]?.version);
  const [query, setQuery] = useState('');
  const [onlyChanged, setOnlyChanged] = useState(false);
  const before = versions.find((v) => v.version === beforeVersion);
  const after = versions.find((v) => v.version === afterVersion);
  const needle = query.trim().toLowerCase().replace(/^v/, '');
  const matching = versions.filter((v) => !needle || String(v.version) === needle);
  const changed = before && after ? fields.filter((field) => before[field] !== after[field]) : [];
  const options = (selected: number) => {
    const list = matching.slice(0, 100);
    const role = versions.find((v) => v.version === selected);
    if (role && !list.includes(role)) list.unshift(role);
    return list.map((v) => (
      <option key={v.version} value={v.version}>
        v{v.version} · {v.name}
      </option>
    ));
  };
  return (
    <Modal label="Сравнение версий роли" className="role-comparison-modal" onClose={onClose}>
      <div className="role-comparison">
        <header>
          <div>
            <h2>Сравнение версий роли</h2>
            <code>{versions[0]?.id}</code>
          </div>
          <button onClick={onClose}>Закрыть сравнение</button>
        </header>
        <p className="small muted">
          Сохранённые данные · ревизия {revision} · версий: {versions.length}
        </p>
        <p className="small muted">
          Сравниваются сохранённые поля роли. Черновик и MCP-привязки не входят в это сравнение.
        </p>
        {currentRevision > revision && (
          <p className="warning">
            Показана сохранённая ревизия {revision}. Конфигурация изменилась; для нового снимка
            закройте и откройте сравнение.
          </p>
        )}
        {!connected && (
          <p className="warning">Нет связи со службой. Доступен сохранённый снимок сравнения.</p>
        )}
        {before && after ? (
          <>
            <div className="role-diff-selectors">
              <label>
                Поиск версии
                <input
                  type="search"
                  value={query}
                  placeholder="v2 или 2"
                  onChange={(event) => setQuery(event.target.value)}
                />
              </label>
              <label>
                Исходная версия
                <select
                  value={beforeVersion}
                  onChange={(event) => setBefore(Number(event.target.value))}
                >
                  {options(beforeVersion)}
                </select>
              </label>
              <label>
                Версия для сравнения
                <select
                  value={afterVersion}
                  onChange={(event) => setAfter(Number(event.target.value))}
                >
                  {options(afterVersion)}
                </select>
              </label>
            </div>
            {matching.length > 100 && (
              <p className="small muted">
                Показаны первые 100 версий и выбранные; найдите нужный номер через поиск.
              </p>
            )}
            {matching.length === 0 && (
              <p className="small muted">Версия не найдена. Выбранные версии сохранены.</p>
            )}
            {versions.length === 1 && (
              <p className="small muted">
                У роли одна сохранённая версия. Обе стороны показывают её поля.
              </p>
            )}
            <div className="role-diff-summary">
              <strong>
                Изменённых полей: {changed.length} из {fields.length}
              </strong>
              <label>
                <input
                  type="checkbox"
                  checked={onlyChanged}
                  onChange={(event) => setOnlyChanged(event.target.checked)}
                />
                Только изменённые поля
              </label>
            </div>
            {onlyChanged && changed.length === 0 && <p>Выбранные версии имеют одинаковые поля.</p>}
            {fields
              .filter((field) => !onlyChanged || changed.includes(field))
              .map((field) => (
                <article
                  key={`${field}:${beforeVersion}:${afterVersion}`}
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
                      version={beforeVersion}
                    />
                    <TextValue
                      text={display(after, field)}
                      label="Новое значение"
                      version={afterVersion}
                    />
                  </div>
                </article>
              ))}
          </>
        ) : (
          <p>Нет сохранённых версий для сравнения.</p>
        )}
      </div>
    </Modal>
  );
}
