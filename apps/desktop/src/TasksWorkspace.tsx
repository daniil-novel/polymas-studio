import { useEffect, useRef, useState } from 'react';
import { Box, Plus, Square, History } from 'lucide-react';
import type { ApiClient } from './api';
import type { Command } from './RoleEditor';
import { TaskComposer } from './TaskComposer';
import { TaskDetail } from './TaskDetail';
import { Pager, useCursorPage, useRemote } from './knowledge/data';
import { stateLabels, type Run, type RuntimeEvent, type Snapshot, type TaskSummary } from './types';

type HistoryRun = Omit<Run, 'taskIds'> & { taskCount: number };
type HistoryTask = Pick<
  TaskSummary,
  'id' | 'runId' | 'assignee' | 'state' | 'attempt' | 'createdAt' | 'completedAt'
>;
type WorkspaceProps = {
  snapshot: Snapshot;
  events: RuntimeEvent[];
  client: ApiClient;
  command: Command;
  onContinue: (id: string) => void;
  onReplay: (id: string) => void;
  initialRunId?: string | null;
  active?: boolean;
};

export function TasksWorkspace(props: WorkspaceProps) {
  const { snapshot, client, command, initialRunId, active = true } = props;
  const [selectedRunId, setSelectedRunId] = useState<string | null>(initialRunId ?? null);
  const [composing, setComposing] = useState(!initialRunId);
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState('all');
  const [refresh, setRefresh] = useState(0);
  const [poll, setPoll] = useState(0);
  useEffect(() => {
    if (!active) return;
    const timer = window.setInterval(() => setPoll((n) => n + 1), 5000);
    return () => window.clearInterval(timer);
  }, [active]);
  const runs = useCursorPage<HistoryRun>(
    client,
    `/api/runs?${new URLSearchParams({ query, status, limit: '30' })}`,
    refresh,
    poll,
    active,
  );
  const open = (id: string) => {
    setSelectedRunId(id);
    setComposing(false);
  };
  return (
    <div className="tasks-workspace">
      <aside className="runs-sidebar" aria-label="История запусков">
        <button className="primary wide" onClick={() => setComposing(true)}>
          <Plus size={15} />
          Новая задача
        </button>
        <h3>История запусков</h3>
        <div className="history-filters">
          <label>
            Поиск запусков
            <input
              type="search"
              value={query}
              maxLength={200}
              placeholder="Название или ID"
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
          <label>
            Состояние запуска
            <select value={status} onChange={(e) => setStatus(e.target.value)}>
              <option value="all">Все запуски</option>
              <option value="open">Приёмка не завершена</option>
              <option value="accepted">Принятые</option>
              <option value="stopped">Остановленные</option>
            </select>
          </label>
          <button onClick={() => setRefresh((n) => n + 1)} disabled={runs.loading}>
            Обновить историю
          </button>
        </div>
        {runs.error && (
          <p className="form-error" role="alert">
            {runs.error}
          </p>
        )}
        {!runs.data && runs.loading && (
          <p className="small muted" role="status">
            Загрузка истории…
          </p>
        )}
        {runs.data?.items.map((run) => (
          <button
            key={run.id}
            className={`run-card ${selectedRunId === run.id && !composing ? 'selected' : ''}`}
            aria-pressed={selectedRunId === run.id && !composing}
            onClick={() => open(run.id)}
          >
            <Box size={16} />
            <span>
              <strong>{run.title}</strong>
              <small>
                {new Date(run.createdAt).toLocaleString('ru')} · {run.taskCount} задач
              </small>
              <small>
                {run.accepted ? 'Принят' : run.stopped ? 'Остановлен' : 'Приёмка не завершена'}
              </small>
            </span>
          </button>
        ))}
        {runs.data?.items.length === 0 && !runs.error && (
          <p className="small muted">Запуски не найдены</p>
        )}
        <Pager page={runs} label="Страницы запусков" />
      </aside>
      <section className="tasks-main">
        <div hidden={!composing}>
          <TaskComposer
            snapshot={snapshot}
            command={command}
            onStarted={(id) => {
              open(id);
              setRefresh((n) => n + 1);
            }}
          />
        </div>
        {!composing && selectedRunId && (
          <RunReview
            key={selectedRunId}
            {...props}
            active={active}
            runId={selectedRunId}
            poll={poll}
          />
        )}
      </section>
    </div>
  );
}

function RunReview({
  runId,
  poll,
  snapshot,
  client,
  command,
  events,
  onContinue,
  onReplay,
  active = true,
}: WorkspaceProps & { runId: string; poll: number }) {
  const [refresh, setRefresh] = useState(0);
  const [state, setState] = useState('');
  const [selection, setSelection] = useState<{ page: string; task: HistoryTask }>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const inFlight = useRef(false);
  const remote = useRemote<HistoryRun>(
    client,
    `/api/runs/${encodeURIComponent(runId)}`,
    refresh,
    poll,
    active,
  );
  const tasks = useCursorPage<HistoryTask>(
    client,
    `/api/runs/${encodeURIComponent(runId)}/tasks?${new URLSearchParams({ limit: '30', ...(state ? { state } : {}) })}`,
    refresh,
    poll,
    active,
  );
  const selectedRun = remote.data;
  const page = `${state}:${tasks.number}`;
  const first = tasks.data?.items[0];
  useEffect(() => {
    if (first && selection?.page !== page) setSelection({ page, task: first });
  }, [first, page, selection?.page]);
  const summary =
    selection?.page === page
      ? (tasks.data?.items.find((t) => t.id === selection.task.id) ?? selection.task)
      : first;
  const ready = active && !remote.loading && !remote.error && !!selectedRun && !busy;
  async function actOnRun(action: string) {
    if (!ready || inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    try {
      await command(`/api/runs/${encodeURIComponent(runId)}/action`, { action });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось изменить запуск');
    } finally {
      // An uncertain response requires a fresh read, never an automatic POST retry.
      setRefresh((n) => n + 1);
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <>
      {remote.error && (
        <p className="form-error" role="alert">
          {remote.error}
        </p>
      )}
      {!selectedRun && (
        <p className="muted">{remote.error ? 'Данные запуска недоступны' : 'Загрузка запуска…'}</p>
      )}
      {selectedRun && (
        <header className="run-heading">
          <h2>{selectedRun.title}</h2>
          <p className="small muted">
            Снимок архитектуры: v{selectedRun.configRevision} · {selectedRun.taskCount} задач
          </p>
          <div className="task-actions">
            <button onClick={() => onReplay(runId)}>
              <History size={14} />
              Посмотреть запись
            </button>
            <button
              disabled={!ready || selectedRun.stopped || selectedRun.accepted}
              onClick={() => void actOnRun('cancel')}
            >
              <Square size={14} />
              Остановить запуск
            </button>
            {selectedRun.completion?.type === 'manual' && (
              <button
                disabled={!ready || selectedRun.accepted || selectedRun.stopped}
                onClick={() => void actOnRun('accept')}
              >
                Принять запуск вручную
              </button>
            )}
          </div>
        </header>
      )}
      <button
        className="history-refresh"
        disabled={remote.loading || tasks.loading}
        onClick={() => setRefresh((n) => n + 1)}
      >
        Обновить запуск
      </button>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <div className="run-task-browser">
        <label>
          Состояние задач запуска
          <select value={state} onChange={(e) => setState(e.target.value)}>
            <option value="">Все состояния</option>
            {[
              'queued',
              'running',
              'waiting',
              'waiting_user',
              'stopping',
              'awaiting_acceptance',
              'accepted',
              'rejected',
              'failed',
              'cancelled',
              'unknown',
            ].map((value) => (
              <option key={value} value={value}>
                {stateLabels[value] ?? value}
              </option>
            ))}
          </select>
        </label>
        {tasks.error && (
          <p className="form-error" role="alert">
            {tasks.error}
          </p>
        )}
        {!tasks.data && tasks.loading && (
          <p className="small muted" role="status">
            Загрузка задач…
          </p>
        )}
        {tasks.data?.items.length === 0 && !tasks.error && (
          <p className="small muted">Задачи не найдены</p>
        )}
        {!!tasks.data?.items.length && (
          <label className="task-selector">
            Задача исполнителя
            <select
              value={summary?.id ?? ''}
              onChange={(e) => {
                const task = tasks.data?.items.find((t) => t.id === e.target.value);
                if (task) setSelection({ page, task });
              }}
            >
              {summary && !tasks.data.items.some((t) => t.id === summary.id) && (
                <option value={summary.id}>Открытая задача · {summary.id}</option>
              )}
              {tasks.data.items.map((task) => (
                <option key={task.id} value={task.id}>
                  {snapshot.config.value.registry.agents.find((a) => a.id === task.assignee)
                    ?.name ?? task.assignee}
                  {' · '}
                  {stateLabels[task.state]}
                  {' · '}
                  {task.id}
                </option>
              ))}
            </select>
          </label>
        )}
        <Pager page={tasks} label="Страницы задач запуска" />
      </div>
      {summary && (
        <TaskDetail
          key={summary.id}
          taskId={summary.id}
          client={client}
          command={command}
          events={events}
          agentName={
            snapshot.config.value.registry.agents.find((a) => a.id === summary.assignee)?.name ??
            summary.assignee
          }
          onContinue={onContinue}
        />
      )}
    </>
  );
}
