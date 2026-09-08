import { useEffect, useState } from 'react';
import {
  Activity,
  Box,
  BookOpen,
  Braces,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  Hexagon,
  Layers3,
  MessageCircle,
  Network,
  Pause,
  Play,
  Plus,
  Plug,
  Search,
  Settings2,
  Square,
  Terminal,
  Users,
  WifiOff,
  X,
} from 'lucide-react';
import { ApiClient, createClient } from './api';
import { isTauri } from '@tauri-apps/api/core';
import { DesktopLifecycle } from './DesktopLifecycle';
import { useWorkspace } from './useWorkspace';
import { RoleEditor } from './RoleEditor';
import { ArchitectureEditor } from './ArchitectureEditor';
import { Modal } from './Modal';
import { TaskComposer } from './TaskComposer';
import { TasksWorkspace } from './TasksWorkspace';
import { McpWorkspace } from './McpWorkspace';
import { KnowledgeWorkspace } from './knowledge/KnowledgeWorkspace';
import { agentState, stateLabels } from './types';
import { GraphView } from './graph/GraphView';
import { projectGraph } from './graph/model';
import './styles.css';

const defaultClient = createClient();
type Page = 'graph' | 'agents' | 'tasks' | 'knowledge' | 'mcp' | 'events' | 'settings';
export function App({ client = defaultClient }: { client?: ApiClient }) {
  const data = useWorkspace(client);
  const { snapshot, events, connected, error } = data;
  const [page, setPage] = useState<Page>('graph');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [token, setToken] = useState('');
  const [architectureOpen, setArchitectureOpen] = useState(false);
  const [chatAgentId, setChatAgentId] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const config = snapshot?.config.value;
  const agents = config?.registry.agents.filter((a) => a.status !== 'archived') ?? [];
  const selected = agents.find((a) => a.id === selectedId);
  const running = agents.filter(
    (a) => agentState(a, snapshot?.tasks ?? [], connected) === 'running',
  ).length;
  const projection = config
    ? projectGraph(agents, config.architecture, snapshot?.tasks ?? [], events, connected, now)
    : null;
  const act = async (action: string) => {
    setBusy(true);
    try {
      await data.command('/api/control', { action });
    } catch {
      /* The persistent error banner owns the error. */
    } finally {
      setBusy(false);
    }
  };
  const nav = [
    { id: 'graph' as const, label: 'Граф', Icon: Network },
    { id: 'agents' as const, label: 'Агенты', Icon: Users },
    { id: 'tasks' as const, label: 'Задачи', Icon: Box },
    { id: 'knowledge' as const, label: 'Знания', Icon: BookOpen },
    { id: 'mcp' as const, label: 'MCP', Icon: Plug },
    { id: 'events' as const, label: 'События', Icon: Activity },
    { id: 'settings' as const, label: 'Настройки', Icon: Settings2 },
  ];
  return (
    <div className="app-shell">
      <DesktopLifecycle />
      <header className="topbar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            setPage('graph');
          }}
        >
          <Hexagon size={26} />
          <span>
            PolyMAS<span className="brand-studio">STUDIO</span>
          </span>
        </a>
        <div className="workspace-title">
          <span>Рабочая область</span>
          <ChevronRight size={14} />
          <strong>{config?.name ?? 'Подключение…'}</strong>
          <ChevronDown size={14} />
        </div>
        <span className={`provider-pill ${snapshot?.runtime.ready ? 'ready' : ''}`}>
          <Terminal size={14} />
          Codex CLI
        </span>
        <div className="live-counts">
          <span data-testid="agent-count">
            <i className="dot" />
            {agents.length.toLocaleString('ru')} агентов
          </span>
          <span data-testid="running-count">{running} работают</span>
        </div>
        <button
          disabled={!connected || busy}
          onClick={() => void act(snapshot?.paused ? 'resume' : 'pause')}
        >
          {snapshot?.paused ? <Play size={14} /> : <Pause size={14} />}{' '}
          {snapshot?.paused ? 'Продолжить' : 'Пауза'}
        </button>
        <button disabled={!connected || busy || running === 0} onClick={() => void act('stop_all')}>
          <Square size={13} />
          Остановить
        </button>
        <button className="icon-button" aria-label="Настройки" onClick={() => setPage('settings')}>
          <Settings2 size={18} />
        </button>
        <span className="avatar">Я</span>
      </header>
      <aside className="sidebar">
        <div className="sidebar-label">РАБОЧАЯ ОБЛАСТЬ</div>
        <nav aria-label="Основная навигация">
          {nav.map(({ id, label, Icon }) => (
            <button key={id} className={page === id ? 'selected' : ''} onClick={() => setPage(id)}>
              <Icon size={18} />
              {label}
              {id === 'agents' && <span className="nav-count">{agents.length}</span>}
            </button>
          ))}
        </nav>
        <div className="sidebar-divider" />
        <div className="sidebar-label">
          ХОЛОНЫ <Layers3 size={13} />
        </div>
        <div className="holon-tree">
          {config?.architecture.holons.map((h) => (
            <button
              key={h.id}
              onClick={() => {
                setQuery(h.members[0] ?? '');
                setPage('agents');
              }}
            >
              <ChevronDown size={13} />
              {h.name}
              <span>{h.members.length}</span>
            </button>
          ))}
          {!config?.architecture.holons.length && (
            <p className="muted small">Группы пока не созданы</p>
          )}
        </div>
        <div className="sidebar-bottom">
          <div className={`connection-status ${connected ? '' : 'offline'}`}>
            {connected ? <i className="dot" /> : <WifiOff size={13} />}{' '}
            {connected ? 'Служба подключена' : 'Нет связи со службой'}
          </div>
          <button className="outline-primary wide" onClick={() => setPage('agents')}>
            <Plus size={17} />
            Создать агента
          </button>
        </div>
      </aside>
      <main className="workspace-main">
        {error && (
          <div className="error-banner" role="alert">
            <WifiOff size={16} />
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="Скрыть сообщение"
              onClick={() => data.setError(null)}
            >
              <X size={16} />
            </button>
          </div>
        )}
        {!snapshot ? (
          <div className="connection-gate">
            <Hexagon size={40} />
            <h1>PolyMAS Studio</h1>
            <p>Подключение к локальной службе редактора</p>
            {error && !isTauri() && (
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  sessionStorage.setItem('polymas-control', token);
                  location.reload();
                }}
              >
                <label>
                  Токен локального подключения
                  <input
                    type="password"
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    autoComplete="off"
                  />
                </label>
                <button className="primary">Подключиться</button>
                <p className="small muted">
                  В desktop-приложении подключение устанавливается автоматически.
                </p>
              </form>
            )}
          </div>
        ) : (
          <>
            <div
              className="view-toolbar"
              hidden={page === 'agents' || page === 'mcp' || page === 'knowledge'}
            >
              <div className="view-heading">
                <span className="eyebrow">
                  {page === 'graph' ? 'ЖИВАЯ АРХИТЕКТУРА' : 'РАБОЧАЯ ОБЛАСТЬ'}
                </span>
                <h1>{nav.find((n) => n.id === page)?.label}</h1>
              </div>
              <div className="search-field">
                <Search size={15} />
                <input
                  aria-label="Поиск агентов"
                  placeholder="Найти агента…"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
                <kbd>⌕</kbd>
              </div>
              <span className="version-chip">v{snapshot.config.revision}</span>
              {page === 'graph' && (
                <button onClick={() => setArchitectureOpen(true)}>
                  <Settings2 size={16} />
                  Архитектура
                </button>
              )}
            </div>
            <div className={`content-row ${page === 'graph' ? 'graph-layout' : ''}`}>
              <section className={page === 'graph' ? 'graph-stage' : 'page-content'}>
                {page === 'graph' && (
                  <>
                    <div className="graph-tabs">
                      <button className="selected">
                        <Network size={14} />
                        Агенты
                      </button>
                      <span className="small muted">
                        N = {agents.length} · C = {config?.architecture.maxConcurrency}
                      </span>
                    </div>
                    {agents.length === 0 ? (
                      <div className="empty-state">
                        <div className="empty-orbit">
                          <Hexagon size={40} />
                        </div>
                        <h2>Создайте первого агента</h2>
                        <p>
                          Задайте роль, добавьте независимые экземпляры
                          <br />и объедините их в свою архитектуру.
                        </p>
                        <button className="primary" onClick={() => setPage('agents')}>
                          <Plus size={16} />
                          Настроить агентов
                        </button>
                      </div>
                    ) : (
                      projection && (
                        <GraphView
                          projection={projection}
                          pinned={snapshot.config.value.architecture.positions}
                          selectedId={selectedId}
                          onSelect={setSelectedId}
                          query={query}
                          groupLabels={Object.fromEntries([
                            ...snapshot.config.value.registry.roles.map((r) => [r.id, r.name]),
                            ...snapshot.config.value.architecture.holons.map((h) => [h.id, h.name]),
                          ])}
                        />
                      )
                    )}
                    <div className="graph-legend">
                      <span>
                        <i className="dot" />
                        Работает
                      </span>
                      <span>
                        <i className="dot idle" />
                        Готов
                      </span>
                      <span>
                        <i className="dot violet" />
                        Общая память
                      </span>
                    </div>
                  </>
                )}
                <div hidden={page !== 'agents'}>
                  <RoleEditor snapshot={snapshot} command={data.command} />
                </div>
                <div hidden={page !== 'mcp'}>
                  <McpWorkspace snapshot={snapshot} command={data.command} />
                </div>
                {page === 'knowledge' && <KnowledgeWorkspace client={client} snapshot={snapshot} />}
                <div hidden={page !== 'tasks'}>
                  <TasksWorkspace
                    snapshot={snapshot}
                    events={events}
                    client={client}
                    command={data.command}
                    onContinue={setChatAgentId}
                  />
                </div>
                {page === 'events' && (
                  <>
                    <h2>Журнал событий</h2>
                    <p className="muted">
                      Наблюдаемые действия исполнителей и изменения рабочей области.
                    </p>
                    <div className="event-list">
                      {[...events].reverse().map((e) => (
                        <div className="event-row" key={e.sequence}>
                          <time>{new Date(e.observedAt).toLocaleTimeString('ru')}</time>
                          <span className="event-tag">{e.kind.split('.')[0]}</span>
                          <span>{e.kind}</span>
                          <small>#{e.sequence}</small>
                        </div>
                      ))}
                    </div>
                  </>
                )}
                {page === 'settings' && (
                  <>
                    <h2>Исполнители</h2>
                    <div className="settings-card">
                      <Terminal />
                      <div>
                        <h3>Codex CLI</h3>
                        <p>
                          {snapshot.runtime.probe?.version ?? 'Не обнаружен'} ·{' '}
                          {snapshot.runtime.probe?.authMethod === 'chatgpt'
                            ? 'Подписка ChatGPT'
                            : (snapshot.runtime.probe?.authMethod ?? 'Требуется вход')}
                        </p>
                      </div>
                      <span className={`badge ${snapshot.runtime.ready ? 'ready' : ''}`}>
                        {snapshot.runtime.ready ? 'Готов' : 'Требует настройки'}
                      </span>
                    </div>
                    {snapshot.runtime.error && <p className="warning">{snapshot.runtime.error}</p>}
                    <p className="muted">
                      Доступно процессов на этом worker: {snapshot.runtime.capacity}. Число
                      экземпляров и число одновременно работающих процессов задаются отдельно.
                    </p>
                    <div className="settings-card muted">
                      <Braces />
                      <div>
                        <h3>Claude Code / API</h3>
                        <p>Будущие адаптеры</p>
                      </div>
                      <span className="badge">Запланировано</span>
                    </div>
                  </>
                )}
              </section>
              {page === 'graph' && (
                <aside className="inspector">
                  {selected ? (
                    <>
                      <div className="inspector-title">
                        <h2>{selected.name}</h2>
                        <span
                          className={`status-text ${agentState(selected, snapshot.tasks, connected)}`}
                        >
                          <i className="dot" />
                          {stateLabels[agentState(selected, snapshot.tasks, connected)]}
                        </span>
                        <code>{selected.id.slice(-12)}</code>
                      </div>
                      <dl>
                        <dt>Роль</dt>
                        <dd>
                          {
                            config?.registry.roles.find(
                              (r) => r.id === selected.roleId && r.version === selected.roleVersion,
                            )?.name
                          }
                        </dd>
                        <dt>Версия роли</dt>
                        <dd>v{selected.roleVersion}</dd>
                        <dt>Исполнитель</dt>
                        <dd>Codex CLI</dd>
                        <dt>Идентичность</dt>
                        <dd>Отдельный экземпляр</dd>
                      </dl>
                      <h3>Последние действия</h3>
                      <button
                        className="outline-primary wide"
                        onClick={() => setChatAgentId(selected.id)}
                      >
                        <MessageCircle size={16} />
                        Открыть диалог
                      </button>
                      {events
                        .filter((e) => e.agentId === selected.id)
                        .slice(-6)
                        .reverse()
                        .map((e) => (
                          <div className="inspector-event" key={e.sequence}>
                            <Activity size={14} />
                            <span>{e.kind}</span>
                            <time>{new Date(e.observedAt).toLocaleTimeString('ru')}</time>
                          </div>
                        ))}
                    </>
                  ) : (
                    <div className="inspector-empty">
                      <CircleHelp size={24} />
                      <h3>Инспектор агента</h3>
                      <p>
                        Выберите узел, чтобы увидеть роль, состояние, разрешения и историю работы.
                      </p>
                    </div>
                  )}
                  <div className="inspector-foot">
                    <span className="small muted">Ревизия {snapshot.config.revision}</span>
                    <span className="small muted">
                      {connected ? 'Синхронизировано' : 'Связь потеряна'}
                    </span>
                  </div>
                </aside>
              )}
            </div>
            <section
              className="timeline"
              hidden={page === 'agents' || page === 'mcp' || page === 'knowledge'}
            >
              <div className="timeline-header">
                <h3>Ход выполнения</h3>
                <span className={`live-indicator ${connected ? '' : 'offline'}`}>
                  <i className="dot" />
                  {connected ? 'LIVE' : 'НЕТ СВЯЗИ'}
                </span>
                <span className="small muted">
                  События #{events.at(-1)?.sequence ?? snapshot.cursor}
                </span>
              </div>
              <div className="timeline-body">
                {events.length === 0 ? (
                  <p className="muted">
                    Пока нет событий. Активность появится после реального действия.
                  </p>
                ) : (
                  events
                    .slice(-3)
                    .reverse()
                    .map((e) => (
                      <div className="event-row" key={e.sequence}>
                        <time>{new Date(e.observedAt).toLocaleTimeString('ru')}</time>
                        <Activity size={14} />
                        <span className="event-tag">{e.kind.split('.')[0]}</span>
                        <span>{e.kind}</span>
                        <small>#{e.sequence}</small>
                      </div>
                    ))
                )}
              </div>
            </section>
          </>
        )}
      </main>
      {snapshot && chatAgentId && (
        <Modal label="Диалог с агентом" onClose={() => setChatAgentId(null)}>
          <div className="chat-modal">
            <button
              className="icon-button chat-close"
              aria-label="Закрыть диалог"
              onClick={() => setChatAgentId(null)}
            >
              <X size={18} />
            </button>
            <TaskComposer
              snapshot={snapshot}
              command={data.command}
              initialAgentId={chatAgentId}
              onStarted={() => {
                setChatAgentId(null);
                setPage('tasks');
              }}
            />
          </div>
        </Modal>
      )}
      {snapshot && architectureOpen && (
        <Modal
          label="Архитектура системы"
          onClose={() => {
            if (
              window.confirm('Закрыть форму архитектуры? Несохранённые изменения будут потеряны.')
            )
              setArchitectureOpen(false);
          }}
        >
          <ArchitectureEditor
            snapshot={snapshot}
            command={data.command}
            onClose={() => setArchitectureOpen(false)}
          />
        </Modal>
      )}
    </div>
  );
}
