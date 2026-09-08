import { useState } from 'react';
import { Modal } from './Modal';
import { RoleComparisonFields, ROLE_FIELD_COUNT, changedRoleFields } from './RoleComparisonFields';
import type { Role } from './types';
import './role-comparison.css';

export interface RoleComparison {
  versions: Role[];
  revision: number;
}
export function RoleComparisonDialog({
  comparison,
  currentRevision,
  connected,
  onClose,
  onRestore,
  onDuplicate,
}: {
  comparison: RoleComparison;
  currentRevision: number;
  connected: boolean;
  onClose: () => void;
  onRestore?: (source: Role) => void;
  onDuplicate?: (source: Role) => void;
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
  const changed = before && after ? changedRoleFields(before, after) : [];
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
                Изменённых полей: {changed.length} из {ROLE_FIELD_COUNT}
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
            {onRestore && (
              <div className="role-diff-restore-action">
                <button
                  disabled={
                    !connected ||
                    currentRevision !== revision ||
                    before.version >= versions[0].version
                  }
                  onClick={() => onRestore(before)}
                >
                  Восстановить исходную версию
                </button>
                <p className="small muted">
                  Откроется проверка создания новой версии. Назначения экземпляров сохранятся.
                </p>
              </div>
            )}
            {onDuplicate && (
              <div className="role-diff-restore-action">
                <button
                  disabled={!connected || currentRevision !== revision}
                  onClick={() => onDuplicate(before)}
                >
                  Дублировать исходную версию
                </button>
                <p className="small muted">
                  Создать отдельную роль из сохранённого источника. Сначала проверьте имя и поля
                  копии.
                </p>
              </div>
            )}
            <RoleComparisonFields before={before} after={after} onlyChanged={onlyChanged} />
          </>
        ) : (
          <p>Нет сохранённых версий для сравнения.</p>
        )}
      </div>
    </Modal>
  );
}
