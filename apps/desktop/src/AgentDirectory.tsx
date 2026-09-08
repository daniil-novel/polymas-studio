import { useEffect, useMemo, useRef, useState } from 'react';
import { Copy } from 'lucide-react';
import { stateLabels, type Agent, type Snapshot } from './types';
import type { GraphNode } from './graph/model';
import './instances.css';

interface Row {
  agent: Agent;
  state: string;
  role: string;
  holon: string;
  task: string;
  worker: string;
  pid: number | null;
}
const rowHeight = 60;
const collator = new Intl.Collator('ru', { numeric: true });

export function AgentDirectory({
  snapshot,
  roleId,
  nodes,
}: {
  snapshot: Snapshot;
  roleId: string;
  nodes: GraphNode[];
}) {
  const [query, setQuery] = useState('');
  const [state, setState] = useState('active');
  const [order, setOrder] = useState('name-asc');
  const rows = useMemo(() => {
    const config = snapshot.config.value;
    const states = new Map(nodes.map((node) => [node.id, node.state]));
    const roles = new Map(
      config.registry.roles.map((role) => [`${role.id}@${role.version}`, role]),
    );
    const holons = new Map(
      config.architecture.holons.flatMap((holon) =>
        holon.members.map((id) => [id, holon.name] as const),
      ),
    );
    const activity = new Map(
      (snapshot.agentActivity ?? snapshot.tasks)
        .filter(
          (task) =>
            states.get(task.assignee) === task.state ||
            (states.get(task.assignee) === 'stale' && ['running', 'stopping'].includes(task.state)),
        )
        .map((task) => [task.assignee, task]),
    );
    const search = query.trim().toLocaleLowerCase('ru');
    return config.registry.agents
      .filter((agent) => !roleId || agent.roleId === roleId)
      .map((agent): Row => {
        const task = activity.get(agent.id);
        return {
          agent,
          state: agent.status === 'archived' ? 'archived' : (states.get(agent.id) ?? 'stale'),
          role: `${roles.get(`${agent.roleId}@${agent.roleVersion}`)?.name ?? agent.roleId} · v${agent.roleVersion}`,
          holon: holons.get(agent.id) ?? 'Без холона',
          task: task?.id ?? '—',
          worker: task?.lease?.workerId ?? '—',
          pid: task?.lease?.pid ?? null,
        };
      })
      .filter((row) => `${row.agent.name} ${row.agent.id}`.toLocaleLowerCase('ru').includes(search))
      .filter(
        (row) =>
          state === 'all' || (state === 'active' ? row.state !== 'archived' : row.state === state),
      )
      .sort((a, b) => {
        const result =
          order === 'id'
            ? collator.compare(a.agent.id, b.agent.id)
            : order === 'role'
              ? collator.compare(a.role, b.role)
              : collator.compare(a.agent.name, b.agent.name) * (order === 'name-desc' ? -1 : 1);
        return result || collator.compare(a.agent.id, b.agent.id);
      });
  }, [snapshot, roleId, nodes, query, state, order]);
  return (
    <section className="agent-directory" aria-label="Каталог экземпляров">
      <div className="instance-filters">
        <label>
          Поиск экземпляров
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Имя или полный ID"
          />
        </label>
        <label>
          Состояние экземпляров
          <select value={state} onChange={(event) => setState(event.target.value)}>
            <option value="active">Вне архива</option>
            <option value="all">Все состояния</option>
            {[
              'idle',
              'running',
              'queued',
              'waiting',
              'waiting_user',
              'stopping',
              'unknown',
              'stale',
              'archived',
            ].map((value) => (
              <option key={value} value={value}>
                {stateLabels[value]}
              </option>
            ))}
          </select>
        </label>
        <label>
          Порядок экземпляров
          <select value={order} onChange={(event) => setOrder(event.target.value)}>
            <option value="name-asc">Имя · по возрастанию</option>
            <option value="name-desc">Имя · по убыванию</option>
            <option value="role">Версия роли</option>
            <option value="id">ID</option>
          </select>
        </label>
      </div>
      <InstanceRows key={JSON.stringify([roleId, query, state, order])} rows={rows} />
    </section>
  );
}

function InstanceRows({ rows }: { rows: Row[] }) {
  const viewport = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(540);
  const [copyError, setCopyError] = useState('');
  const [copied, setCopied] = useState('');
  useEffect(() => {
    const element = viewport.current;
    if (!element || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => setHeight(element.clientHeight || 540));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const visibleCount = Math.max(1, Math.ceil(height / rowHeight));
  const first = Math.min(
    Math.floor(scrollTop / rowHeight),
    Math.max(0, rows.length - visibleCount),
  );
  const start = Math.max(0, first - 4);
  const end = Math.min(rows.length, first + visibleCount + 4);
  function move(direction: number) {
    const target = Math.max(
      0,
      Math.min(rows.length - visibleCount, first + direction * visibleCount),
    );
    if (viewport.current) viewport.current.scrollTop = target * rowHeight;
    setScrollTop(target * rowHeight);
  }
  async function copy(agent: Agent) {
    setCopyError('');
    setCopied('');
    try {
      await navigator.clipboard.writeText(agent.id);
      setCopied(`ID скопирован: ${agent.name}`);
    } catch {
      setCopyError('Не удалось скопировать ID. Выделите его в строке таблицы.');
    }
  }
  return (
    <>
      <div className="instance-summary">
        <span>Найдено: {rows.length.toLocaleString('ru')}</span>
        <span className="muted">Каждая строка — независимый экземпляр</span>
      </div>
      <div
        ref={viewport}
        className="instance-viewport"
        role="region"
        aria-label="Прокрутка экземпляров"
        tabIndex={0}
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      >
        <table
          className="instance-directory-table"
          aria-label="Экземпляры агентов"
          aria-rowcount={rows.length + 1}
        >
          <colgroup>
            {[230, 220, 205, 180, 185, 200, 170, 55].map((width, i) => (
              <col key={i} style={{ width }} />
            ))}
          </colgroup>
          <thead>
            <tr aria-rowindex={1}>
              {[
                'Экземпляр',
                'ID',
                'Версия роли',
                'Холон',
                'Состояние',
                'Задача',
                'Исполнитель',
                'ID',
              ].map((name, i) => (
                <th scope="col" key={i}>
                  {name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {start > 0 && (
              <tr aria-hidden="true" className="instance-spacer">
                <td colSpan={8} style={{ height: start * rowHeight }} />
              </tr>
            )}
            {rows.slice(start, end).map((row, index) => (
              <tr key={row.agent.id} aria-rowindex={start + index + 2}>
                <td title={row.agent.name}>
                  <span className="instance-name">
                    <i className={`dot ${row.state}`} />
                    {row.agent.name}
                  </span>
                </td>
                <td title={row.agent.id}>
                  <code>{row.agent.id}</code>
                </td>
                <td title={row.role}>{row.role}</td>
                <td title={row.holon}>{row.holon}</td>
                <td>{stateLabels[row.state] ?? row.state}</td>
                <td title={row.task}>
                  <code>{row.task}</code>
                </td>
                <td title={row.worker}>
                  <span>{row.worker}</span>
                  <small>{row.pid === null ? 'Codex CLI' : `Codex CLI · PID ${row.pid}`}</small>
                </td>
                <td>
                  <button
                    className="icon-button"
                    aria-label={`Скопировать ID ${row.agent.name}`}
                    onClick={() => void copy(row.agent)}
                  >
                    <Copy size={14} />
                  </button>
                </td>
              </tr>
            ))}
            {end < rows.length && (
              <tr aria-hidden="true" className="instance-spacer">
                <td colSpan={8} style={{ height: (rows.length - end) * rowHeight }} />
              </tr>
            )}
          </tbody>
        </table>
        {rows.length === 0 && <p className="empty-state">Экземпляры не найдены</p>}
      </div>
      <nav className="instance-pager" aria-label="Строки экземпляров">
        <button disabled={first === 0} onClick={() => move(-1)}>
          Предыдущие строки
        </button>
        <span>
          {rows.length ? first + 1 : 0}–{Math.min(rows.length, first + visibleCount)} из{' '}
          {rows.length}
        </span>
        <button disabled={first + visibleCount >= rows.length} onClick={() => move(1)}>
          Следующие строки
        </button>
      </nav>
      {copyError && (
        <p role="alert" className="form-error">
          {copyError}
        </p>
      )}
      {copied && (
        <p role="status" className="small muted">
          {copied}
        </p>
      )}
    </>
  );
}
