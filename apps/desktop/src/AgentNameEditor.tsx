import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Modal } from './Modal';
import type { Agent, Snapshot } from './types';
import type { Command } from './RoleEditor';

export type RefreshWorkspace = (signal?: AbortSignal) => Promise<Snapshot>;
export function AgentNameEditor({
  agentId,
  snapshot,
  connected,
  refresh,
  command,
  onClose,
  onSaved,
}: {
  agentId: string;
  snapshot: Snapshot;
  connected: boolean;
  refresh: RefreshWorkspace;
  command: Command;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [workspaceId] = useState(snapshot.config.value.id);
  const activation = useMemo(() => ({ connected }), [connected]);
  const [base, setBase] = useState<{ agent: Agent; revision: number; activation: object } | null>(
    null,
  );
  const [name, setName] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [needsRead, setNeedsRead] = useState(false);
  const initialized = useRef(false);
  const inFlight = useRef(false);
  const reads = useRef<AbortController | null>(null);
  const load = useCallback(async () => {
    reads.current?.abort();
    const abort = new AbortController();
    reads.current = abort;
    setLoading(true);
    setError('');
    try {
      const value = await refresh(abort.signal);
      if (abort.signal.aborted) return;
      const agent = value.config.value.registry.agents.find((item) => item.id === agentId);
      if (value.config.value.id !== workspaceId || !agent)
        throw new Error('Экземпляр больше недоступен в этой рабочей области');
      setBase({ agent, revision: value.config.revision, activation });
      if (!initialized.current) {
        setName(agent.name);
        initialized.current = true;
      }
      setNeedsRead(false);
    } catch (reason) {
      if (!abort.signal.aborted) {
        setError(reason instanceof Error ? reason.message : 'Не удалось обновить данные');
        setNeedsRead(true);
      }
    } finally {
      if (!abort.signal.aborted) setLoading(false);
    }
  }, [refresh, agentId, workspaceId, activation]);
  useEffect(() => {
    if (connected) void load();
    return () => reads.current?.abort();
  }, [connected, load]);
  const current = snapshot.config.value.registry.agents.find((agent) => agent.id === agentId);
  const stale = !!base && snapshot.config.revision > base.revision;
  const bytes = new TextEncoder().encode(name.trim()).length;
  const sameName = !!base && name.trim() === base.agent.name;
  const ready =
    connected &&
    base?.activation === activation &&
    !loading &&
    !busy &&
    !needsRead &&
    !stale &&
    !!current &&
    snapshot.config.value.id === workspaceId;
  async function save() {
    if (!ready || !base || inFlight.current || bytes < 1 || bytes > 200 || sameName) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    try {
      await command('/api/configure', {
        revision: base.revision,
        command: { type: 'rename_agent', agentId, name: name.trim() },
      });
      onSaved();
    } catch (reason) {
      setNeedsRead(true);
      setError(reason instanceof Error ? reason.message : 'Не удалось сохранить имя');
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <Modal
      label="Переименовать экземпляр"
      className="agent-name-modal"
      onClose={() => {
        if (!inFlight.current) onClose();
      }}
    >
      <form
        className="agent-name-editor"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <h2>Переименовать экземпляр</h2>
        <p className="muted">ID, рабочая папка, сессии и история сохранятся.</p>
        <label>
          Идентификатор
          <input value={agentId} readOnly />
        </label>
        {base && (
          <p className="small">
            Текущее имя: <strong>{stale ? current?.name : base.agent.name}</strong>
          </p>
        )}
        <label>
          Имя экземпляра
          <input
            value={name}
            disabled={!base || busy}
            maxLength={200}
            onChange={(event) => setName(event.target.value)}
            autoFocus
          />
        </label>
        {bytes > 200 && (
          <p className="form-error">Имя превышает 200 байт в UTF-8. Сократите его.</p>
        )}
        {loading && connected && (
          <p role="status" className="small muted">
            Обновление данных…
          </p>
        )}
        {!connected && (
          <p className="form-error">Нет связи со службой. Черновик сохранён в форме.</p>
        )}
        {stale && (
          <p className="form-error">
            Конфигурация изменилась. Обновите данные и проверьте текущее имя.
          </p>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error} Обновите данные перед повторным сохранением.
          </p>
        )}
        {sameName && !loading && !stale && !needsRead && (
          <p className="small muted">Это имя уже сохранено</p>
        )}
        <div className="dialog-actions">
          <button type="button" disabled={busy} onClick={onClose}>
            Закрыть
          </button>
          <button
            type="button"
            disabled={!connected || loading || busy}
            onClick={() => void load()}
          >
            Обновить данные
          </button>
          <button
            className="primary"
            type="submit"
            disabled={!ready || bytes < 1 || bytes > 200 || sameName}
          >
            {busy ? 'Сохранение…' : 'Сохранить имя'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
