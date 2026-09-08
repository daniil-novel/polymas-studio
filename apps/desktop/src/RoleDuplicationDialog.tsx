import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal } from './Modal';
import { RoleComparisonFields, changedRoleFields } from './RoleComparisonFields';
import type { RefreshWorkspace } from './AgentNameEditor';
import type { Command } from './RoleEditor';
import type { Role, Snapshot } from './types';
import type { DuplicationSource } from './RoleDuplicationActions';
import {
  suggestedCopyName,
  validCopyName,
  type PendingDuplication,
} from './role-duplication-recovery';
import './role-comparison.css';

export function RoleDuplicationDialog({
  source: selection,
  snapshot,
  connected,
  refresh,
  command,
  pendingRequest,
  recoveryError,
  persist,
  clear,
  onClose,
  onSaved,
}: {
  source: DuplicationSource | null;
  snapshot: Snapshot;
  connected: boolean;
  refresh: RefreshWorkspace;
  command: Command;
  pendingRequest: PendingDuplication | null;
  recoveryError: string;
  persist: (value: PendingDuplication) => void;
  clear: (value: PendingDuplication) => void;
  onClose: () => void;
  onSaved: (role: Role) => void;
}) {
  const [workspaceId] = useState(snapshot.config.value.id);
  const target = pendingRequest?.request.command ?? selection;
  const roleId = target?.roleId;
  const sourceVersion = target?.sourceVersion;
  const [name, setName] = useState(
    () =>
      pendingRequest?.request.command.name ??
      suggestedCopyName(
        snapshot.config.value.registry.roles.find(
          (r) => r.id === roleId && r.version === sourceVersion,
        )?.name ?? '',
      ),
  );
  const activation = useMemo(() => ({ connected }), [connected]);
  const [review, setReview] = useState<{ value: Snapshot; activation: object } | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [needsRead, setNeedsRead] = useState(false);
  const [error, setError] = useState('');
  const [allFields, setAllFields] = useState(false);
  const [confirmed, setConfirmed] = useState<Role | null>(null);
  const pendingRead = useRef<AbortController | null>(null);
  const inFlight = useRef(false);
  const sentRequest = useRef(pendingRequest);
  const mounted = useRef(true);
  const load = useCallback(async () => {
    pendingRead.current?.abort();
    const abort = new AbortController();
    pendingRead.current = abort;
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
        setError(reason instanceof Error ? reason.message : 'Не удалось проверить источник');
      }
    } finally {
      if (!abort.signal.aborted) setLoading(false);
    }
  }, [refresh, workspaceId, activation]);
  useEffect(() => {
    mounted.current = true;
    if (connected) void load();
    return () => {
      mounted.current = false;
      pendingRead.current?.abort();
    };
  }, [connected, load]);
  const source = review?.value.config.value.registry.roles.find(
    (r) => r.id === roleId && r.version === sourceVersion,
  );
  const copyName = pendingRequest?.request.command.name ?? name;
  const stale = review && snapshot.config.revision > review.value.config.revision;
  const ready =
    connected &&
    review?.activation === activation &&
    !loading &&
    !busy &&
    !needsRead &&
    !stale &&
    !recoveryError &&
    !confirmed &&
    source &&
    validCopyName(copyName) &&
    snapshot.config.value.id === workspaceId;
  function complete(role: Role) {
    try {
      if (!sentRequest.current) throw new Error('Не найдена запись запроса');
      clear(sentRequest.current);
      onSaved(role);
    } catch {
      setError(
        'Копия создана. Не удалось очистить локальную запись; повторите только завершение записи.',
      );
    }
  }
  async function duplicate() {
    if (!ready || !source || !review || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    try {
      const request = {
        revision: review.value.config.revision,
        command: pendingRequest?.request.command ?? {
          type: 'duplicate_role' as const,
          key: crypto.randomUUID(),
          roleId: source.id,
          sourceVersion: source.version,
          name: copyName,
        },
      };
      const saved: PendingDuplication = { version: 1, workspaceId, request };
      persist(saved);
      sentRequest.current = saved;
      const response = (await command('/api/configure', request)) as {
        result?: { role?: Role; sourceRoleId?: string; sourceVersion?: number };
      } | null;
      if (!mounted.current) return;
      const result = response?.result;
      const role = result?.role;
      if (
        !role ||
        typeof role.id !== 'string' ||
        !role.id ||
        role.id === source.id ||
        role.version !== 1 ||
        typeof role.createdAt !== 'string' ||
        result?.sourceRoleId !== source.id ||
        result?.sourceVersion !== source.version ||
        changedRoleFields({ ...source, name: copyName }, role).length > 0
      )
        throw new Error('Ответ не подтверждает создание точной копии.');
      setConfirmed(role);
      complete(role);
    } catch (reason) {
      if (mounted.current) {
        setNeedsRead(true);
        setError(reason instanceof Error ? reason.message : 'Не удалось подтвердить копирование');
      }
    } finally {
      inFlight.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  return (
    <Modal
      label="Дублирование роли"
      className="role-comparison-modal"
      onClose={() => {
        if (!inFlight.current) onClose();
      }}
    >
      <div className="role-comparison">
        <header>
          <div>
            <h2>Дублирование роли</h2>
            <code>{roleId}</code>
          </div>
          <button disabled={busy} onClick={onClose}>
            Закрыть дублирование
          </button>
        </header>
        <p className="muted">
          Копия получит отдельный ID. Экземпляры и MCP-привязки не копируются. Черновик основной
          роли сохраняется.
        </p>
        <h3>Новая независимая роль · v1</h3>
        {pendingRequest && (
          <p className="warning">
            Сохранён незавершённый запрос: «{copyName}», источник v{sourceVersion}. Повтор
            использует тот же ключ; новые параметры недоступны до подтверждения результата.
          </p>
        )}
        <label>
          Название новой роли
          <input
            value={copyName}
            disabled={!!pendingRequest || busy || !!confirmed}
            onChange={(event) => setName(event.target.value)}
            maxLength={200}
          />
        </label>
        {!validCopyName(copyName) && (
          <p className="form-error">Название: от 1 до 200 байт UTF-8.</p>
        )}
        {source && (
          <>
            <p className="small muted">
              Источник: {source.name} · v{sourceVersion} · ревизия {review!.value.config.revision}
            </p>
            <label className="restore-all-fields">
              <input
                type="checkbox"
                checked={allFields}
                onChange={(event) => setAllFields(event.target.checked)}
              />
              Все поля новой роли
            </label>
            <RoleComparisonFields
              before={source}
              after={{ ...source, name: copyName, version: 1 }}
              onlyChanged={!allFields}
            />
          </>
        )}
        {review && !source && <p className="form-error">Сохранённый источник не найден.</p>}
        {loading && connected && <p role="status">Проверка сохранённого источника…</p>}
        {stale && <p className="form-error">Конфигурация изменилась. Обновите проверку.</p>}
        {!connected && <p className="form-error">Нет связи со службой. Копирование недоступно.</p>}
        {recoveryError && <p className="form-error">{recoveryError}</p>}
        {error && (
          <p className="form-error" role="alert">
            {error}
            {!confirmed && ' Обновите проверку перед повтором.'}
          </p>
        )}
        {confirmed && (
          <p role="status">
            Копия подтверждена: {confirmed.name} · {confirmed.id}
          </p>
        )}
        <div className="dialog-actions">
          {confirmed ? (
            <button className="primary" onClick={() => complete(confirmed)}>
              Завершить локальную запись
            </button>
          ) : (
            <>
              <button disabled={!connected || loading || busy} onClick={() => void load()}>
                Обновить проверку
              </button>
              <button className="primary" disabled={!ready} onClick={() => void duplicate()}>
                {pendingRequest ? 'Повторить сохранённый запрос' : 'Создать копию роли'}
              </button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}
