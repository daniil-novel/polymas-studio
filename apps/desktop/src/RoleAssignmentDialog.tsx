import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal } from './Modal';
import { SelectedInstances } from './SelectedInstances';
import { RoleBindingsReview } from './RoleBindingsReview';
import type { Role, Snapshot } from './types';
import type { RefreshWorkspace } from './AgentNameEditor';
import type { Command } from './RoleEditor';

const keyOf = (role: Role) => JSON.stringify([role.id, role.version]);
const unfinished = new Set(['queued', 'waiting', 'waiting_user', 'running', 'stopping', 'unknown']);
export function RoleAssignmentDialog({
  ids,
  snapshot,
  connected,
  refresh,
  command,
  onClose,
  onSaved,
}: {
  ids: string[];
  snapshot: Snapshot;
  connected: boolean;
  refresh: RefreshWorkspace;
  command: Command;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [workspaceId] = useState(snapshot.config.value.id);
  const activation = useMemo(() => ({ connected }), [connected]);
  const [review, setReview] = useState<{ value: Snapshot; activation: object } | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [needsRead, setNeedsRead] = useState(false);
  const [error, setError] = useState('');
  const [target, setTarget] = useState('');
  const [query, setQuery] = useState('');
  const reads = useRef<AbortController | null>(null);
  const inFlight = useRef(false);
  const load = useCallback(async () => {
    reads.current?.abort();
    const abort = new AbortController();
    reads.current = abort;
    setLoading(true);
    setError('');
    try {
      const value = await refresh(abort.signal);
      if (abort.signal.aborted) return;
      if (value.config.value.id !== workspaceId) throw new Error('Рабочая область изменилась');
      setReview({ value, activation });
      setNeedsRead(false);
    } catch (reason) {
      if (!abort.signal.aborted) {
        setNeedsRead(true);
        setError(reason instanceof Error ? reason.message : 'Не удалось проверить экземпляры');
      }
    } finally {
      if (!abort.signal.aborted) setLoading(false);
    }
  }, [refresh, workspaceId, activation]);
  useEffect(() => {
    if (connected) void load();
    return () => reads.current?.abort();
  }, [load, connected]);
  const config = review?.value.config.value;
  const agents = new Map(config?.registry.agents.map((a) => [a.id, a]) ?? []);
  const role = config?.registry.roles.find((r) => keyOf(r) === target);
  const terms = query.trim().toLocaleLowerCase('ru').split(/\s+/).filter(Boolean);
  const matches = (config?.registry.roles ?? [])
    .filter((r) =>
      terms.every((term) =>
        /^v\d+$/.test(term)
          ? r.version === Number(term.slice(1))
          : `${r.name} ${r.id}`.toLocaleLowerCase('ru').includes(term),
      ),
    )
    .sort(
      (a, b) =>
        a.name.localeCompare(b.name, 'ru') || a.id.localeCompare(b.id) || b.version - a.version,
    );
  const options = matches.slice(0, 100);
  if (role && !options.includes(role)) options.unshift(role);
  const unavailable = ids.filter(
    (id) => !agents.has(id) || agents.get(id)!.status === 'archived',
  ).length;
  const changed = role
    ? ids.filter((id) => {
        const agent = agents.get(id);
        return agent && (agent.roleId !== role.id || agent.roleVersion !== role.version);
      }).length
    : 0;
  const selected = new Set(ids);
  const activity = review?.value.agentActivity ?? review?.value.tasks ?? [];
  const working = new Set(
    activity
      .filter((task) => selected.has(task.assignee) && unfinished.has(task.state))
      .map((task) => task.assignee),
  ).size;
  const stale = !!review && snapshot.config.revision > review.value.config.revision;
  const ready =
    connected &&
    review?.activation === activation &&
    !loading &&
    !busy &&
    !needsRead &&
    !stale &&
    !unavailable &&
    role &&
    changed > 0 &&
    ids.length > 0 &&
    ids.length <= 5000 &&
    snapshot.config.value.id === workspaceId;
  async function assign() {
    if (!ready || !role || !review || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    try {
      await command('/api/configure', {
        revision: review.value.config.revision,
        command: { type: 'pin_role', agentIds: ids, roleId: role.id, version: role.version },
      });
      onSaved();
    } catch (reason) {
      setNeedsRead(true);
      setError(reason instanceof Error ? reason.message : 'Не удалось подтвердить назначение');
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <Modal
      label="Назначить версию роли"
      className="agent-archive-modal"
      onClose={() => {
        if (!inFlight.current) onClose();
      }}
    >
      <div className="agent-archive-editor role-assignment-editor">
        <h2>Назначить версию роли</h2>
        <p>Выбрано экземпляров: {ids.length}</p>
        <p className="muted">
          Уже созданные задачи сохранят свои инструкции, включая очередь и продолжение сессии.
          Выбранная версия применяется при создании новых задач.
        </p>
        {review && (
          <>
            <label>
              Поиск роли
              <input
                type="search"
                value={query}
                disabled={busy}
                placeholder="Название, ID или v2"
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            <label>
              Версия роли
              <select
                value={target}
                disabled={busy}
                onChange={(event) => setTarget(event.target.value)}
              >
                <option value="">Выберите версию</option>
                {options.map((r) => (
                  <option key={keyOf(r)} value={keyOf(r)}>
                    {r.name} · v{r.version} · {r.id}
                  </option>
                ))}
              </select>
            </label>
            {matches.length > 100 && (
              <p className="small muted">
                Найдено версий: {matches.length}. Показаны первые 100; уточните поиск.
              </p>
            )}
            {role && (
              <div className="assignment-role-review">
                <strong>
                  {role.name} · v{role.version}
                </strong>
                <code>{role.id}</code>
                <p className="small">
                  {role.sandbox === 'read-only'
                    ? 'Файлы только для чтения'
                    : 'Запись в рабочую папку'}{' '}
                  · {role.model ?? 'Модель Codex по умолчанию'} · до {role.timeoutSeconds} с /{' '}
                  {role.maxTurns} шагов
                </p>
                <details open>
                  <summary>Инструкция выбранной версии</summary>
                  <pre>{role.instruction}</pre>
                </details>
                <details>
                  <summary>Результат и критерии</summary>
                  <pre>
                    {[role.outputContract, role.acceptanceCriteria, role.escalationRule]
                      .filter(Boolean)
                      .join('\n\n')}
                  </pre>
                </details>
                <RoleBindingsReview
                  key={role.id}
                  snapshot={review.value}
                  bindings={config!.bindings.filter(
                    (binding) => binding.subject.type === 'role' && binding.subject.id === role.id,
                  )}
                />
              </div>
            )}
            <p>
              Назначение изменится у {changed} из {ids.length} экземпляров.
            </p>
            {working > 0 && (
              <p className="small muted">
                Экземпляров с незавершёнными задачами: {working}. Их закреплённые версии не
                изменятся.
              </p>
            )}
            <SelectedInstances
              items={ids.map((id) => {
                const agent = agents.get(id);
                return {
                  id,
                  name: agent?.name ?? 'Экземпляр не найден',
                  detail: !agent
                    ? 'Недоступен'
                    : agent.status === 'archived'
                      ? 'В архиве'
                      : `${agent.roleId} · v${agent.roleVersion}`,
                };
              })}
            />
            {unavailable > 0 && (
              <p className="form-error">
                Недоступные или архивные экземпляры: {unavailable}. Закройте форму и проверьте весь
                выбор.
              </p>
            )}
            {role && changed === 0 && !unavailable && (
              <p>Выбранная версия уже назначена всем экземплярам</p>
            )}
          </>
        )}
        {loading && connected && <p role="status">Проверка данных…</p>}
        {stale && (
          <p className="form-error">
            Конфигурация изменилась. Обновите проверку перед назначением.
          </p>
        )}
        {!connected && <p className="form-error">Нет связи со службой. Назначение недоступно.</p>}
        {error && (
          <p role="alert" className="form-error">
            {error} Обновите проверку перед повтором.
          </p>
        )}
        <div className="dialog-actions">
          <button disabled={busy} onClick={onClose}>
            Закрыть
          </button>
          <button disabled={!connected || loading || busy} onClick={() => void load()}>
            Обновить проверку
          </button>
          <button className="primary" disabled={!ready} onClick={() => void assign()}>
            Применить к новым задачам
          </button>
        </div>
      </div>
    </Modal>
  );
}
