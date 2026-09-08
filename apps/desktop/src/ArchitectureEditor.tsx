import { useState } from 'react';
import {
  ArrowRight,
  Database,
  GitBranch,
  Layers3,
  Plus,
  Save,
  Trash2,
  Users,
  X,
} from 'lucide-react';
import { AgentPicker } from './AgentPicker';
import type { Command } from './RoleEditor';
import type { Architecture, Holon, Snapshot } from './types';
import './architecture.css';

const presets: { value: Architecture['preset']; label: string; hint: string }[] = [
  { value: 'single', label: 'Один агент', hint: 'Один экземпляр Codex CLI' },
  { value: 'independent', label: 'Независимые', hint: 'Отдельные задачи без обмена' },
  { value: 'network', label: 'Сеть', hint: 'Явные связи между агентами' },
  { value: 'star', label: 'Координатор', hint: 'Управление через выбранного агента' },
  { value: 'holonic', label: 'Холоны', hint: 'Вложенные группы с собственными целями' },
  { value: 'custom', label: 'Своя архитектура', hint: 'Свободное сочетание правил' },
];
export function architectureIssues(draft: Architecture, count: number): string[] {
  const issues: string[] = [];
  if (
    !Number.isInteger(draft.maxConcurrency) ||
    draft.maxConcurrency < 1 ||
    draft.maxConcurrency > Math.max(1, count)
  )
    issues.push('C должно быть целым числом от 1 до N.');
  if (draft.preset === 'single' && count !== 1)
    issues.push('Для режима «Один агент» нужен ровно один экземпляр.');
  if (draft.coordinatorAgentId && count < 2)
    issues.push('Координатор доступен при двух и более агентах.');
  if (draft.preset === 'star' && !draft.coordinatorAgentId)
    issues.push('Назначьте координатора из существующих агентов.');
  if (draft.memoryEnabled && !draft.memoryConnectionId)
    issues.push('Выберите подключение общей памяти.');
  if (
    !Number.isInteger(draft.messageHopLimit) ||
    draft.messageHopLimit < 1 ||
    draft.messageHopLimit > 32
  )
    issues.push('Предел переходов сообщения: от 1 до 32.');
  if (
    !Number.isInteger(draft.maxMessagesPerRun) ||
    draft.maxMessagesPerRun < 1 ||
    draft.maxMessagesPerRun > 100000
  )
    issues.push('Бюджет сообщений: от 1 до 100 000.');
  const owners = new Set<string>();
  for (const holon of draft.holons) {
    if (!holon.name.trim()) issues.push('У каждого холона должно быть название.');
    const visited = new Set([holon.id]);
    let current = holon.parentId;
    while (current) {
      if (visited.has(current) || visited.size >= 16) {
        issues.push('Владение холонами содержит цикл или больше 16 уровней.');
        break;
      }
      visited.add(current);
      const parent = draft.holons.find((h) => h.id === current);
      if (!parent) {
        issues.push('Не найден родительский холон.');
        break;
      }
      current = parent.parentId;
    }
    for (const member of holon.members) {
      if (owners.has(member)) issues.push('Агент может принадлежать только одному холону.');
      owners.add(member);
    }
    if (holon.coordinatorAgentId && !holon.members.includes(holon.coordinatorAgentId))
      issues.push('Координатор холона должен быть его участником.');
  }
  return [...new Set(issues)];
}
export function ArchitectureEditor({
  snapshot,
  command,
  onClose,
}: {
  snapshot: Snapshot;
  command: Command;
  onClose: () => void;
}) {
  const [baseRevision, setBaseRevision] = useState(snapshot.config.revision);
  const [draft, setDraft] = useState<Architecture>(() =>
    structuredClone(snapshot.config.value.architecture),
  );
  const [section, setSection] = useState<'structure' | 'connections' | 'resources'>('structure');
  const [holonId, setHolonId] = useState<string | null>(null);
  const [memberQuery, setMemberQuery] = useState('');
  const [from, setFrom] = useState<string | null>(null);
  const [to, setTo] = useState<string | null>(null);
  const [bidirectional, setBidirectional] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const agents = snapshot.config.value.registry.agents.filter((a) => a.status !== 'archived');
  const issues = architectureIssues(draft, agents.length);
  const stale = baseRevision !== snapshot.config.revision;
  const dirty = JSON.stringify(draft) !== JSON.stringify(snapshot.config.value.architecture);
  const holon = draft.holons.find((h) => h.id === holonId);
  const patch = (value: Partial<Architecture>) => setDraft((d) => ({ ...d, ...value }));
  const patchHolon = (value: Partial<Holon>) =>
    setDraft((d) => ({
      ...d,
      holons: d.holons.map((h) => (h.id === holonId ? { ...h, ...value } : h)),
    }));
  const close = () => {
    if (
      !busy &&
      (!dirty || window.confirm('Закрыть архитектуру и потерять несохранённые изменения?'))
    )
      onClose();
  };
  async function save() {
    if (issues.length || stale) return;
    setBusy(true);
    setError('');
    try {
      await command('/api/configure', {
        revision: baseRevision,
        command: { type: 'set_architecture', architecture: draft },
      });
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось сохранить архитектуру');
    } finally {
      setBusy(false);
    }
  }
  function addHolon() {
    const id = `holon_${crypto.randomUUID()}`;
    patch({
      holons: [
        ...draft.holons,
        {
          id,
          name: `Холон ${draft.holons.length + 1}`,
          goal: '',
          parentId: null,
          members: [],
          coordinatorAgentId: null,
          inputContract: '',
          outputContract: '',
          maxTasks: 100,
        },
      ],
    });
    setHolonId(id);
  }
  function addEdge() {
    if (!from || !to || from === to) return;
    const edges = [...draft.edges];
    for (const [a, b] of bidirectional
      ? [
          [from, to],
          [to, from],
        ]
      : [[from, to]])
      if (!edges.some((e) => e.from === a && e.to === b)) edges.push({ from: a, to: b });
    patch({ edges });
  }
  return (
    <div className="architecture-editor">
      <header className="modal-heading">
        <div>
          <span className="eyebrow">УСТРОЙСТВО СИСТЕМЫ · РЕВИЗИЯ {baseRevision}</span>
          <h2>Архитектура</h2>
          <p className="muted">
            {agents.length} независимых агентов · {draft.holons.length} холонов · C ={' '}
            {draft.maxConcurrency}
          </p>
        </div>
        <button className="icon-button" aria-label="Закрыть архитектуру" onClick={close}>
          <X size={20} />
        </button>
      </header>
      {stale && (
        <div className="form-error" role="alert">
          Архитектура изменилась в другой форме. Этот черновик нельзя сохранить.
          <button
            onClick={() => {
              if (!dirty || window.confirm('Загрузить текущую архитектуру вместо черновика?')) {
                setDraft(structuredClone(snapshot.config.value.architecture));
                setBaseRevision(snapshot.config.revision);
              }
            }}
          >
            Загрузить текущую
          </button>
        </div>
      )}
      <div className="preset-grid">
        {presets.map((p) => (
          <button
            key={p.value}
            aria-pressed={draft.preset === p.value}
            disabled={p.value === 'single' && agents.length !== 1}
            onClick={() =>
              patch({
                preset: p.value,
                ...(p.value === 'single' ? { coordinatorAgentId: null, maxConcurrency: 1 } : {}),
              })
            }
          >
            <strong>{p.label}</strong>
            <small>{p.hint}</small>
          </button>
        ))}
      </div>
      <div className="architecture-controls">
        <section className="control-card">
          <label className="switch-label">
            <Users size={18} />
            <span>ИИ-координатор</span>
            <input
              type="checkbox"
              checked={!!draft.coordinatorAgentId}
              disabled={agents.length < 2}
              onChange={(e) =>
                patch({ coordinatorAgentId: e.target.checked ? (agents[0]?.id ?? null) : null })
              }
            />
          </label>
          <p className="small muted">
            Существующий агент внутри N. Получает задачи, адресованные системе.
          </p>
          {draft.coordinatorAgentId && (
            <AgentPicker
              label="Агент-координатор"
              agents={agents}
              value={draft.coordinatorAgentId}
              onChange={(id) => patch({ coordinatorAgentId: id })}
              optional={false}
            />
          )}
        </section>
        <section className="control-card">
          <label className="switch-label">
            <Database size={18} />
            <span>Общая память</span>
            <input
              type="checkbox"
              checked={draft.memoryEnabled}
              onChange={(e) => patch({ memoryEnabled: e.target.checked })}
            />
          </label>
          <p className="small muted">Источник знаний через MCP. Память не является агентом.</p>
          {draft.memoryEnabled && (
            <label>
              Подключение памяти
              <select
                value={draft.memoryConnectionId ?? ''}
                onChange={(e) => patch({ memoryConnectionId: e.target.value || null })}
              >
                <option value="">Выберите подключение</option>
                {snapshot.connections.map((c) => (
                  <option key={c.value.id} value={c.value.id}>
                    {c.value.name} ·{' '}
                    {c.value.enabled && c.value.health === 'ready' ? 'готово' : 'нужна настройка'}
                  </option>
                ))}
              </select>
            </label>
          )}
          {draft.memoryEnabled && !snapshot.connections.length && (
            <p className="warning small">Сначала добавьте источник на экране MCP.</p>
          )}
        </section>
      </div>
      <nav className="architecture-tabs" aria-label="Разделы архитектуры">
        {(
          [
            { id: 'structure', title: 'Состав и холоны', Icon: Layers3 },
            { id: 'connections', title: 'Связи', Icon: GitBranch },
            { id: 'resources', title: 'Ресурсы', Icon: Users },
          ] as const
        ).map((t) => (
          <button key={t.id} aria-pressed={section === t.id} onClick={() => setSection(t.id)}>
            <t.Icon size={16} />
            {t.title}
          </button>
        ))}
      </nav>
      <div hidden={section !== 'structure'} className="architecture-body">
        <div className="holon-editor-layout">
          <aside className="holon-list">
            <button className="wide" onClick={addHolon}>
              <Plus size={15} />
              Создать холон
            </button>
            {draft.holons.map((h) => (
              <button
                key={h.id}
                className="wide"
                aria-pressed={h.id === holonId}
                onClick={() => setHolonId(h.id)}
              >
                <Layers3 size={14} />
                <span>{h.name}</span>
                <small>{h.members.length}</small>
              </button>
            ))}
            {!draft.holons.length && (
              <p className="small muted">
                Холон объединяет агентов и вложенные группы с общей целью.
              </p>
            )}
          </aside>
          {holon ? (
            <section className="holon-form">
              <div className="two-fields">
                <label>
                  Название холона
                  <input
                    value={holon.name}
                    maxLength={100}
                    onChange={(e) => patchHolon({ name: e.target.value })}
                  />
                </label>
                <label>
                  Родительский холон
                  <select
                    value={holon.parentId ?? ''}
                    onChange={(e) => patchHolon({ parentId: e.target.value || null })}
                  >
                    <option value="">Верхний уровень</option>
                    {draft.holons
                      .filter((h) => h.id !== holon.id)
                      .map((h) => (
                        <option key={h.id} value={h.id}>
                          {h.name}
                        </option>
                      ))}
                  </select>
                </label>
              </div>
              <label>
                Цель холона
                <textarea
                  rows={2}
                  value={holon.goal}
                  onChange={(e) => patchHolon({ goal: e.target.value })}
                />
              </label>
              <AgentPicker
                label="Координатор холона"
                agents={agents.filter((a) => holon.members.includes(a.id))}
                value={holon.coordinatorAgentId}
                onChange={(id) => patchHolon({ coordinatorAgentId: id })}
              />
              <label>
                Участники · поиск
                <input
                  value={memberQuery}
                  onChange={(e) => setMemberQuery(e.target.value)}
                  placeholder="Имя или ID агента"
                />
              </label>
              <div className="member-list">
                {agents
                  .filter((a) =>
                    `${a.name} ${a.id}`
                      .toLocaleLowerCase()
                      .includes(memberQuery.toLocaleLowerCase()),
                  )
                  .slice(0, 100)
                  .map((a) => {
                    const owner = draft.holons.find(
                      (h) => h.id !== holon.id && h.members.includes(a.id),
                    );
                    return (
                      <label key={a.id}>
                        <input
                          type="checkbox"
                          disabled={!!owner}
                          checked={holon.members.includes(a.id)}
                          onChange={(e) =>
                            patchHolon({
                              members: e.target.checked
                                ? [...holon.members, a.id]
                                : holon.members.filter((id) => id !== a.id),
                              coordinatorAgentId:
                                !e.target.checked && holon.coordinatorAgentId === a.id
                                  ? null
                                  : holon.coordinatorAgentId,
                            })
                          }
                        />
                        <span>{a.name}</span>
                        <small>{owner ? `В ${owner.name}` : a.id.slice(-8)}</small>
                      </label>
                    );
                  })}
              </div>
              {agents.length > 100 && (
                <p className="small muted">
                  В списке до 100 совпадений. Поиск находит любой ID среди {agents.length} агентов.
                </p>
              )}
              <details>
                <summary>Контракты холона</summary>
                <div className="advanced-fields">
                  <label>
                    Входной контракт
                    <textarea
                      rows={2}
                      value={holon.inputContract}
                      onChange={(e) => patchHolon({ inputContract: e.target.value })}
                    />
                  </label>
                  <label>
                    Выходной контракт
                    <textarea
                      rows={2}
                      value={holon.outputContract}
                      onChange={(e) => patchHolon({ outputContract: e.target.value })}
                    />
                  </label>
                  <label>
                    Предел задач холона
                    <input
                      type="number"
                      min={1}
                      max={100000}
                      value={holon.maxTasks}
                      onChange={(e) => patchHolon({ maxTasks: Number(e.target.value) })}
                    />
                  </label>
                </div>
              </details>
              <button
                className="danger-link"
                onClick={() => {
                  patch({
                    holons: draft.holons
                      .filter((h) => h.id !== holon.id)
                      .map((h) => ({
                        ...h,
                        parentId: h.parentId === holon.id ? null : h.parentId,
                      })),
                  });
                  setHolonId(null);
                }}
              >
                <Trash2 size={14} />
                Удалить группу, сохранив агентов
              </button>
            </section>
          ) : (
            <div className="architecture-summary">
              <Layers3 size={32} />
              <h3>{agents.length} отдельных экземпляров</h3>
              <p>
                Выберите или создайте холон для настройки состава. Каждый агент сохраняет
                собственную сессию и рабочую папку.
              </p>
              <p>
                {draft.coordinatorAgentId
                  ? 'Задача системе поступает координатору.'
                  : 'Для общей задачи выберите одного получателя или явно отправьте её всем.'}
              </p>
            </div>
          )}
        </div>
      </div>
      <div hidden={section !== 'connections'} className="architecture-body">
        <p className="muted">
          Направление задаёт право отправить сообщение. Факт передачи отображается отдельно.
        </p>
        {['single', 'independent'].includes(draft.preset) && (
          <p className="warning">
            В выбранном пресете обмен отключён. Сохранённые связи начнут действовать после перехода
            в сетевой режим.
          </p>
        )}
        <div className="edge-create">
          <AgentPicker
            label="Отправитель"
            agents={agents}
            value={from}
            onChange={setFrom}
            optional={false}
          />
          <ArrowRight size={18} />
          <AgentPicker
            label="Получатель"
            agents={agents}
            value={to}
            onChange={setTo}
            optional={false}
          />
          <button
            disabled={!from || !to || from === to || draft.edges.length >= 24999}
            onClick={addEdge}
          >
            <Plus size={15} />
            Связать
          </button>
        </div>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={bidirectional}
            onChange={(e) => setBidirectional(e.target.checked)}
          />
          Добавить обратное направление
        </label>
        <div className="edge-list">
          {draft.edges.map((edge, i) => (
            <div key={`${edge.from}>${edge.to}`}>
              <span>{agents.find((a) => a.id === edge.from)?.name ?? edge.from}</span>
              <ArrowRight size={14} />
              <span>{agents.find((a) => a.id === edge.to)?.name ?? edge.to}</span>
              <button
                className="icon-button"
                aria-label={`Удалить связь ${i + 1}`}
                onClick={() => patch({ edges: draft.edges.filter((_, j) => j !== i) })}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </div>
        <p className="small muted">
          Связи координаторов системы и холонов разрешаются автоматически. Всего явных направлений:{' '}
          {draft.edges.length}.
        </p>
      </div>
      <div hidden={section !== 'resources'} className="architecture-body resource-fields">
        <div className="two-fields">
          <label>
            Одновременно работающих процессов, C
            <input
              type="number"
              min={1}
              max={Math.max(1, agents.length)}
              value={draft.maxConcurrency}
              onChange={(e) => patch({ maxConcurrency: Number(e.target.value) })}
            />
          </label>
          <label>
            Допуск запуска
            <select
              value={draft.admission}
              onChange={(e) => patch({ admission: e.target.value as Architecture['admission'] })}
            >
              <option value="queue_allowed">Очередь: запускать по доступным слотам</option>
              <option value="start_all">Все одновременно: отказ при нехватке слотов</option>
            </select>
          </label>
        </div>
        <p className="small muted">
          На текущем исполнителе настроено {snapshot.runtime.capacity} слотов. N = {agents.length}{' '}
          хранит определения; C ограничивает одновременные процессы. Квота аккаунта дополнительно
          ограничивает работу.
        </p>
        <div className="two-fields">
          <label>
            Предел переходов сообщения
            <input
              type="number"
              min={1}
              max={32}
              value={draft.messageHopLimit}
              onChange={(e) => patch({ messageHopLimit: Number(e.target.value) })}
            />
          </label>
          <label>
            Бюджет сообщений запуска
            <input
              type="number"
              min={1}
              max={100000}
              value={draft.maxMessagesPerRun}
              onChange={(e) => patch({ maxMessagesPerRun: Number(e.target.value) })}
            />
          </label>
        </div>
      </div>
      {issues.length > 0 && (
        <ul className="validation-issues" role="status">
          {issues.map((issue) => (
            <li key={issue}>{issue}</li>
          ))}
        </ul>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <footer className="modal-footer">
        <span className="small muted">
          {dirty ? 'Есть несохранённые изменения' : 'Конфигурация не изменена'} · Текущие запуски
          сохраняют свой снимок
        </span>
        <button disabled={busy} onClick={close}>
          Отмена
        </button>
        <button
          className="primary"
          disabled={busy || stale || issues.length > 0}
          onClick={() => void save()}
        >
          <Save size={15} />
          {busy ? 'Сохранение…' : 'Сохранить архитектуру'}
        </button>
      </footer>
    </div>
  );
}
