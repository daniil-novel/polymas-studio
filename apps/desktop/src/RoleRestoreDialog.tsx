import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal } from './Modal';
import { RoleComparisonFields, changedRoleFields } from './RoleComparisonFields';
import type { RefreshWorkspace } from './AgentNameEditor';
import type { Command } from './RoleEditor';
import type { Snapshot } from './types';

export function RoleRestoreDialog({
  roleId,
  sourceVersion,
  snapshot,
  connected,
  refresh,
  command,
  onClose,
  onSaved,
}: {
  roleId: string;
  sourceVersion: number;
  snapshot: Snapshot;
  connected: boolean;
  refresh: RefreshWorkspace;
  command: Command;
  onClose: () => void;
  onSaved: (version: number) => void;
}) {
  const [workspaceId] = useState(snapshot.config.value.id);
  const activation = useMemo(() => ({ connected }), [connected]);
  const [review, setReview] = useState<{ value: Snapshot; activation: object } | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [needsRead, setNeedsRead] = useState(false);
  const [error, setError] = useState('');
  const [allFields, setAllFields] = useState(false);
  const pending = useRef<AbortController | null>(null);
  const inFlight = useRef(false);
  const load = useCallback(async () => {
    pending.current?.abort();
    const abort = new AbortController();
    pending.current = abort;
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
        setError(reason instanceof Error ? reason.message : 'Не удалось прочитать версии роли');
      }
    } finally {
      if (!abort.signal.aborted) setLoading(false);
    }
  }, [refresh, workspaceId, activation]);
  useEffect(() => {
    if (connected) void load();
    return () => pending.current?.abort();
  }, [connected, load]);
  const versions =
    review?.value.config.value.registry.roles.filter((role) => role.id === roleId) ?? [];
  const source = versions.find((role) => role.version === sourceVersion);
  const latest = versions.reduce<(typeof versions)[number] | undefined>(
    (best, role) => (!best || role.version > best.version ? role : best),
    undefined,
  );
  const unchanged = source && latest && changedRoleFields(source, latest).length === 0;
  const allowed =
    source && latest && source.version < latest.version && latest.version < 4294967295;
  const stale = review && snapshot.config.revision > review.value.config.revision;
  const ready =
    connected &&
    review?.activation === activation &&
    !loading &&
    !busy &&
    !needsRead &&
    !stale &&
    allowed &&
    !unchanged &&
    snapshot.config.value.id === workspaceId;
  async function restore() {
    if (!ready || !latest || !review || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    const newVersion = latest.version + 1;
    try {
      await command('/api/configure', {
        revision: review.value.config.revision,
        command: { type: 'restore_role', roleId, sourceVersion, expectedVersion: latest.version },
      });
      onSaved(newVersion);
    } catch (reason) {
      setNeedsRead(true);
      setError(reason instanceof Error ? reason.message : 'Не удалось подтвердить восстановление');
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <Modal
      label="Восстановление версии роли"
      className="role-comparison-modal"
      onClose={() => {
        if (!inFlight.current) onClose();
      }}
    >
      <div className="role-comparison">
        <header>
          <div>
            <h2>Восстановление версии роли</h2>
            <code>{roleId}</code>
          </div>
          <button disabled={busy} onClick={onClose}>
            Закрыть восстановление
          </button>
        </header>
        <p className="muted">
          Старые версии, задачи и назначения экземпляров сохранятся. Для применения новой версии
          назначьте её экземплярам отдельно.
        </p>
        {source && latest && (
          <>
            {allowed && !unchanged && (
              <h3>
                Будет создана версия v{latest.version + 1} из v{sourceVersion}
              </h3>
            )}
            <p className="small muted">
              Текущая версия: v{latest.version} · источник: v{sourceVersion} · ревизия{' '}
              {review!.value.config.revision}
            </p>
            {unchanged && <p>Последняя версия уже содержит выбранные поля</p>}
            <label className="restore-all-fields">
              <input
                type="checkbox"
                checked={allFields}
                onChange={(event) => setAllFields(event.target.checked)}
              />
              Все поля восстановленной версии
            </label>
            {allowed && (
              <RoleComparisonFields
                key={review!.value.config.revision}
                before={latest}
                after={{ ...source, version: latest.version + 1 }}
                onlyChanged={!allFields}
              />
            )}
          </>
        )}
        {review && !allowed && (
          <p className="form-error">
            Источник недоступен для восстановления. Выберите более раннюю версию; номер новой версии
            должен оставаться в допустимом диапазоне.
          </p>
        )}
        {loading && connected && <p role="status">Проверка сохранённых версий…</p>}
        {stale && <p className="form-error">Конфигурация изменилась. Обновите проверку.</p>}
        {!connected && (
          <p className="form-error">Нет связи со службой. Восстановление недоступно.</p>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error} Обновите проверку перед повтором.
          </p>
        )}
        <div className="dialog-actions">
          <button disabled={!connected || loading || busy} onClick={() => void load()}>
            Обновить проверку
          </button>
          <button className="primary" disabled={!ready} onClick={() => void restore()}>
            Создать восстановленную версию
          </button>
        </div>
      </div>
    </Modal>
  );
}
