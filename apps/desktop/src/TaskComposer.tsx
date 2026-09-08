import { useRef, useState } from 'react';
import { Send } from 'lucide-react';
import { AgentPicker } from './AgentPicker';
import type { Command } from './RoleEditor';
import type { Agent, Architecture, Snapshot } from './types';
import './tasks.css';

export type Target =
  | { type: 'agent'; id: string }
  | { type: 'agents'; ids: string[] }
  | { type: 'system'; broadcast: boolean }
  | { type: 'holon'; id: string; recursive: boolean };
export function resolveRecipients(
  agents: Agent[],
  architecture: Architecture,
  target: Target,
): string[] {
  const active = agents.filter((a) => a.status !== 'archived');
  if (target.type === 'agent') return active.some((a) => a.id === target.id) ? [target.id] : [];
  if (target.type === 'agents') return target.ids.filter((id) => active.some((a) => a.id === id));
  if (target.type === 'system')
    return architecture.coordinatorAgentId
      ? [architecture.coordinatorAgentId]
      : active.slice(0, target.broadcast ? active.length : 1).map((a) => a.id);
  const holon = architecture.holons.find((h) => h.id === target.id);
  if (!holon) return [];
  if (!target.recursive && holon.coordinatorAgentId) return [holon.coordinatorAgentId];
  const included = new Set([holon.id]);
  if (target.recursive) {
    let size: number;
    do {
      size = included.size;
      for (const h of architecture.holons)
        if (h.parentId && included.has(h.parentId)) included.add(h.id);
    } while (size !== included.size);
  }
  return [
    ...new Set(architecture.holons.filter((h) => included.has(h.id)).flatMap((h) => h.members)),
  ];
}
interface Draft {
  prompt: string;
  title: string;
  criteria: string;
  priority: number;
  newSession: boolean;
  completion: 'all_required' | 'first_accepted' | 'manual' | 'quorum';
  quorum: number;
  dependencies: string[];
}
const blank: Draft = {
  prompt: '',
  title: '',
  criteria: '',
  priority: 0,
  newSession: false,
  completion: 'all_required',
  quorum: 1,
  dependencies: [],
};
export function TaskComposer({
  snapshot,
  command,
  initialAgentId,
  initialHolonId,
  onStarted,
  compact = false,
}: {
  snapshot: Snapshot;
  command: Command;
  initialAgentId?: string;
  initialHolonId?: string;
  onStarted?: (id: string) => void;
  compact?: boolean;
}) {
  const agents = snapshot.config.value.registry.agents.filter((a) => a.status !== 'archived');
  const architecture = snapshot.config.value.architecture;
  const [target, setTarget] = useState<Target>(() =>
    initialHolonId
      ? { type: 'holon', id: initialHolonId, recursive: false }
      : { type: 'agent', id: initialAgentId ?? agents[0]?.id ?? '' },
  );
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const identity = JSON.stringify(target);
  const draft = drafts[identity] ?? blank;
  const patch = (value: Partial<Draft>) =>
    setDrafts((old) => ({ ...old, [identity]: { ...(old[identity] ?? blank), ...value } }));
  const receipt = useRef({ body: '', key: '' });
  const ids = resolveRecipients(agents, architecture, target);
  const selectedName =
    target.type === 'agent'
      ? agents.find((a) => a.id === target.id)?.name
      : target.type === 'holon'
        ? architecture.holons.find((h) => h.id === target.id)?.name
        : 'Система';
  const valid =
    !!draft.prompt.trim() &&
    draft.prompt.length <= 100000 &&
    ids.length > 0 &&
    (draft.completion !== 'quorum' ||
      (Number.isInteger(draft.quorum) && draft.quorum >= 1 && draft.quorum <= ids.length));
  async function submit() {
    if (!valid || busy || !snapshot.runtime.ready) return;
    const input = {
      title: draft.title.trim() || draft.prompt.trim().slice(0, 100),
      prompt: draft.prompt,
      criteria: draft.criteria,
      target,
      completion:
        draft.completion === 'quorum'
          ? { type: 'quorum', required: draft.quorum }
          : { type: draft.completion },
      priority: draft.priority,
      dependencies: draft.dependencies,
      newSession: draft.newSession,
    };
    const body = JSON.stringify(input);
    if (receipt.current.body !== body) receipt.current = { body, key: crypto.randomUUID() };
    setBusy(true);
    setError('');
    setNotice('');
    try {
      const run = (await command('/api/runs', { key: receipt.current.key, input })) as {
        id: string;
      };
      setDrafts((old) => ({ ...old, [identity]: { ...blank } }));
      receipt.current = { body: '', key: '' };
      setNotice(
        `Задача отправлена: ${ids.length} ${ids.length === 1 ? 'получатель' : 'получателей'}`,
      );
      onStarted?.(run.id);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось отправить задачу');
    } finally {
      setBusy(false);
    }
  }
  return (
    <form
      className={`task-composer ${compact ? 'compact' : ''}`}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="composer-heading">
        <h3>{compact ? 'Новая задача' : 'Задача или чат'}</h3>
        <span className="small muted">Ревизия {snapshot.config.revision}</span>
      </div>
      <div className="target-controls">
        <label>
          Адресация
          <select
            disabled={busy}
            value={target.type}
            onChange={(e) => {
              setError('');
              setNotice('');
              setTarget(
                e.target.value === 'system'
                  ? { type: 'system', broadcast: false }
                  : e.target.value === 'holon'
                    ? { type: 'holon', id: architecture.holons[0]?.id ?? '', recursive: false }
                    : { type: 'agent', id: agents[0]?.id ?? '' },
              );
            }}
          >
            <option value="agent">Конкретному агенту</option>
            <option value="holon" disabled={!architecture.holons.length}>
              Холону
            </option>
            <option value="system">Системе</option>
          </select>
        </label>
        {target.type === 'agent' && (
          <AgentPicker
            label="Агент-получатель"
            agents={agents}
            value={target.id}
            onChange={(id) => {
              setTarget({ type: 'agent', id: id ?? '' });
              setNotice('');
            }}
            optional={false}
          />
        )}
        {target.type === 'holon' && (
          <label>
            Холон-получатель
            <select
              value={target.id}
              onChange={(e) => setTarget({ ...target, id: e.target.value })}
            >
              {architecture.holons.map((h) => (
                <option value={h.id} key={h.id}>
                  {h.name}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
      {target.type === 'system' && (
        <p className="small muted">
          {architecture.coordinatorAgentId
            ? `Получатель: ${agents.find((a) => a.id === architecture.coordinatorAgentId)?.name}. Координатор входит в N.`
            : `Координатор не назначен. ${target.broadcast ? 'Каждый агент получит отдельную задачу; общего синтеза нет.' : `Получатель: ${agents[0]?.name ?? 'не выбран'}.`}`}
        </p>
      )}
      {target.type === 'system' && !architecture.coordinatorAgentId && (
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={target.broadcast}
            onChange={(e) => setTarget({ ...target, broadcast: e.target.checked })}
          />
          Отправить всем {agents.length} агентам отдельные задачи
        </label>
      )}
      {target.type === 'holon' && (
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={target.recursive}
            onChange={(e) => setTarget({ ...target, recursive: e.target.checked })}
          />
          Всем участникам, включая вложенные холоны
        </label>
      )}
      <label>
        Задача или сообщение
        <textarea
          rows={compact ? 3 : 5}
          maxLength={100000}
          value={draft.prompt}
          disabled={busy}
          onChange={(e) => patch({ prompt: e.target.value })}
          placeholder={`Что нужно сделать: ${selectedName ?? 'выберите получателя'}…`}
        />
      </label>
      {!compact && (
        <label>
          Критерии приёмки
          <textarea
            rows={2}
            maxLength={20000}
            value={draft.criteria}
            onChange={(e) => patch({ criteria: e.target.value })}
            placeholder="По каким признакам результат можно принять"
          />
        </label>
      )}
      <details>
        <summary>Параметры задачи и завершения</summary>
        <div className="advanced-fields">
          <label>
            Название запуска
            <input
              maxLength={200}
              value={draft.title}
              onChange={(e) => patch({ title: e.target.value })}
            />
          </label>
          {compact && (
            <label>
              Критерии приёмки
              <textarea
                rows={2}
                maxLength={20000}
                value={draft.criteria}
                onChange={(e) => patch({ criteria: e.target.value })}
              />
            </label>
          )}
          <div className="two-fields">
            <label>
              Правило завершения
              <select
                value={draft.completion}
                onChange={(e) => patch({ completion: e.target.value as Draft['completion'] })}
              >
                <option value="all_required">Все результаты приняты</option>
                <option value="first_accepted">Первый принятый результат</option>
                <option value="quorum">Принято заданное количество</option>
                <option value="manual">Ручная приёмка запуска</option>
              </select>
            </label>
            <label>
              Приоритет
              <input
                type="number"
                min={-100}
                max={100}
                value={draft.priority}
                onChange={(e) => patch({ priority: Number(e.target.value) })}
              />
            </label>
          </div>
          {draft.completion === 'quorum' && (
            <label>
              Сколько результатов принять
              <input
                type="number"
                min={1}
                max={ids.length}
                value={draft.quorum}
                onChange={(e) => patch({ quorum: Number(e.target.value) })}
              />
            </label>
          )}
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={draft.newSession}
              onChange={(e) => patch({ newSession: e.target.checked })}
            />
            Начать новую сессию вместо продолжения истории
          </label>
          <label>
            После принятых задач
            <input
              value={draft.dependencies.join(', ')}
              onChange={(e) =>
                patch({
                  dependencies: e.target.value
                    .split(',')
                    .map((s) => s.trim())
                    .filter(Boolean),
                })
              }
              placeholder="ID задач через запятую"
            />
          </label>
        </div>
      </details>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {notice && (
        <p className="form-success" role="status">
          {notice}
        </p>
      )}
      <div className="composer-review">
        <span>
          {ids.length} адресатов · C = {architecture.maxConcurrency} · {snapshot.runtime.capacity}{' '}
          слотов
        </span>
        <span>
          {snapshot.paused
            ? 'Очередь на паузе'
            : snapshot.runtime.ready
              ? 'Codex готов'
              : 'Настройте Codex в исполнителях'}
        </span>
      </div>
      <button
        type="submit"
        className="primary"
        disabled={!valid || busy || !snapshot.runtime.ready}
      >
        <Send size={16} />
        {busy ? 'Отправка…' : 'Начать'}
      </button>
    </form>
  );
}
