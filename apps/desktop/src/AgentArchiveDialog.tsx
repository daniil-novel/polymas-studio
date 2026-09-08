import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal } from './Modal';
import { stateLabels, type Snapshot } from './types';
import type { Command } from './RoleEditor';
import type { RefreshWorkspace } from './AgentNameEditor';

const unfinished = new Set(['queued', 'waiting', 'waiting_user', 'running', 'stopping', 'unknown']);
function busyAgents(snapshot: Snapshot): Set<string> {
  const ids = new Set(
    (snapshot.agentActivity ?? snapshot.tasks)
      .filter((task) => unfinished.has(task.state))
      .map((task) => task.assignee),
  );
  for (const agent of snapshot.config.value.registry.agents)
    if (!['idle', 'failed', 'archived'].includes(agent.status)) ids.add(agent.id);
  return ids;
}
export function AgentArchiveDialog({
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
  const [review, setReview] = useState<{
    value: Snapshot;
    seen: Snapshot;
    activation: object;
  } | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [needsRead, setNeedsRead] = useState(false);
  const [query, setQuery] = useState('');
  const [page, setPage] = useState(0);
  const latest = useRef(snapshot);
  const reads = useRef<AbortController | null>(null);
  const inFlight = useRef(false);
  useEffect(() => {
    latest.current = snapshot;
  }, [snapshot]);
  const load = useCallback(async () => {
    reads.current?.abort();
    const abort = new AbortController();
    reads.current = abort;
    const seen = latest.current;
    setLoading(true);
    setError('');
    try {
      const value = await refresh(abort.signal);
      if (abort.signal.aborted) return;
      if (value.config.value.id !== workspaceId) throw new Error('Рабочая область изменилась');
      setReview({ value, seen, activation });
      setNeedsRead(false);
    } catch (reason) {
      if (!abort.signal.aborted) {
        setError(
          reason instanceof Error ? reason.message : 'Не удалось проверить выбранные экземпляры',
        );
        setNeedsRead(true);
      }
    } finally {
      if (!abort.signal.aborted) setLoading(false);
    }
  }, [refresh, workspaceId, activation]);
  useEffect(() => {
    if (connected) void load();
    return () => reads.current?.abort();
  }, [connected, load]);
  const selected = new Set(ids);
  const config = review?.value.config.value;
  const indexed = new Map(config?.registry.agents.map((agent) => [agent.id, agent]) ?? []);
  const items = ids.map((id) => ({ id, agent: indexed.get(id) }));
  const archived = items.filter((item) => item.agent?.status === 'archived').length;
  const missing = items.filter((item) => !item.agent).length;
  const blocked = review ? busyAgents(review.value) : new Set<string>();
  // A newer observed busy state may revoke readiness, but never silently expand the batch.
  if (review && snapshot !== review.seen && snapshot !== review.value)
    for (const id of busyAgents(snapshot)) blocked.add(id);
  const blockedCount = ids.filter((id) => blocked.has(id)).length;
  const stale = !!review && snapshot.config.revision > review.value.config.revision;
  const activeCount =
    config?.registry.agents.filter((agent) => agent.status !== 'archived').length ?? 0;
  const remaining = Math.max(0, activeCount - ids.length + archived + missing);
  const removedEdges =
    config?.architecture.edges.filter((edge) => selected.has(edge.from) || selected.has(edge.to))
      .length ?? 0;
  const removedBindings =
    config?.bindings.filter(
      (binding) => binding.subject.type === 'agent' && selected.has(binding.subject.id),
    ).length ?? 0;
  const removedCoordinators =
    config?.architecture.holons.filter(
      (holon) => holon.coordinatorAgentId && selected.has(holon.coordinatorAgentId),
    ).length ?? 0;
  const coordinator = config?.architecture.coordinatorAgentId;
  const matched = items.filter((item) =>
    `${item.id} ${item.agent?.name ?? ''}`
      .toLocaleLowerCase('ru')
      .includes(query.trim().toLocaleLowerCase('ru')),
  );
  const currentPage = Math.min(page, Math.max(0, Math.ceil(matched.length / 20) - 1));
  const ready =
    connected &&
    review?.activation === activation &&
    !loading &&
    !busy &&
    !needsRead &&
    !stale &&
    !missing &&
    !archived &&
    !blockedCount &&
    ids.length > 0 &&
    ids.length <= 5000 &&
    snapshot.config.value.id === workspaceId;
  async function archive() {
    if (!ready || !review || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    try {
      await command('/api/configure', {
        revision: review.value.config.revision,
        command: { type: 'archive_agents', agentIds: ids },
      });
      onSaved();
    } catch (reason) {
      setNeedsRead(true);
      setError(reason instanceof Error ? reason.message : 'Не удалось подтвердить архивирование');
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <Modal
      label="Архивировать экземпляры"
      className="agent-archive-modal"
      onClose={() => {
        if (!inFlight.current) onClose();
      }}
    >
      <div className="agent-archive-editor">
        <h2>Архивировать экземпляры</h2>
        <p>Выбрано для архива: {ids.length}</p>
        <p className="muted">
          История, файлы, сессии и общая память сохранятся. Архивированные агенты не получают новые
          задачи.
        </p>
        <p className="small muted">
          Участие выбранных агентов в холонах и закрепление их узлов будут сняты.
        </p>
        {review && (
          <>
            <div className="archive-impact">
              <span>
                Останется вне архива: <strong>{remaining}</strong>
              </span>
              <span>
                Параллелизм C:{' '}
                <strong>
                  {config!.architecture.maxConcurrency} →{' '}
                  {Math.min(config!.architecture.maxConcurrency, Math.max(1, remaining))}
                </strong>
              </span>
              <span>
                Будут удалены связи: <strong>{removedEdges}</strong>
              </span>
              <span>
                Персональные MCP-привязки: <strong>{removedBindings}</strong>
              </span>
              <span>
                Будут сняты координаторы холонов: <strong>{removedCoordinators}</strong>
              </span>
              {coordinator && (selected.has(coordinator) || remaining < 2) && (
                <span>Координатор системы будет снят.</span>
              )}
              {remaining === 0 && <span>Архитектура станет пустым черновиком.</span>}
            </div>
            <label>
              Поиск в выбранных
              <input
                type="search"
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value);
                  setPage(0);
                }}
              />
            </label>
            <ul className="archive-selection" aria-label="Выбранные экземпляры">
              {matched.slice(currentPage * 20, (currentPage + 1) * 20).map(({ id, agent }) => (
                <li key={id}>
                  <strong>{agent?.name ?? 'Экземпляр не найден'}</strong>
                  <code>{id}</code>
                  <span>
                    {!agent
                      ? 'Недоступен'
                      : agent.status === 'archived'
                        ? stateLabels.archived
                        : blocked.has(id)
                          ? 'Сначала завершите работу и сверку'
                          : 'Готов к архивированию'}
                  </span>
                </li>
              ))}
            </ul>
            <nav className="instance-pager" aria-label="Страницы выбранных экземпляров">
              <button disabled={currentPage === 0} onClick={() => setPage(currentPage - 1)}>
                Назад
              </button>
              <span>
                {matched.length ? currentPage + 1 : 0} / {Math.ceil(matched.length / 20)}
              </span>
              <button
                disabled={(currentPage + 1) * 20 >= matched.length}
                onClick={() => setPage(currentPage + 1)}
              >
                Далее
              </button>
            </nav>
            {blockedCount > 0 && (
              <p className="form-error">
                Незавершённая работа: {blockedCount}. Сначала остановите задачи и выполните сверку;
                весь набор останется без изменений.
              </p>
            )}
            {missing > 0 && (
              <p className="form-error">
                Недоступные экземпляры: {missing}. Закройте форму и проверьте выбор.
              </p>
            )}
            {archived > 0 && (
              <p className="form-error">
                {archived === ids.length
                  ? 'Все выбранные экземпляры уже в архиве'
                  : `Уже в архиве: ${archived}. Закройте форму и обновите выбор.`}
              </p>
            )}
          </>
        )}
        {loading && connected && <p role="status">Проверка выбранных экземпляров…</p>}
        {stale && (
          <p className="form-error">
            Конфигурация изменилась. Обновите проверку и просмотрите последствия.
          </p>
        )}
        {!connected && (
          <p className="form-error">Нет связи со службой. Архивирование недоступно.</p>
        )}
        {error && (
          <p className="form-error" role="alert">
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
          <button className="danger" disabled={!ready} onClick={() => void archive()}>
            {busy ? 'Архивирование…' : 'Подтвердить архивирование'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
