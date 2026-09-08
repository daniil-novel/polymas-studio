import { useState } from 'react';
import { Box, Plus, Square, History } from 'lucide-react';
import type { ApiClient } from './api';
import type { Command } from './RoleEditor';
import { TaskComposer } from './TaskComposer';
import { TaskDetail } from './TaskDetail';
import { stateLabels, type RuntimeEvent, type Snapshot } from './types';

export function TasksWorkspace({
  snapshot,
  events,
  client,
  command,
  onContinue,
  onReplay,
  initialRunId,
}: {
  snapshot: Snapshot;
  events: RuntimeEvent[];
  client: ApiClient;
  command: Command;
  onContinue: (id: string) => void;
  onReplay: (id: string) => void;
  initialRunId?: string | null;
}) {
  const [selectedRunId, setSelectedRunId] = useState<string | null>(initialRunId ?? null);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const [composing, setComposing] = useState(!initialRunId);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const selectedRun = snapshot.runs.find((r) => r.id === selectedRunId);
  const currentId =
    selectedTaskId && selectedRun?.taskIds.includes(selectedTaskId)
      ? selectedTaskId
      : selectedRun?.taskIds[0];
  const summary = snapshot.tasks.find((t) => t.id === currentId);
  async function act(action: string) {
    if (!selectedRun) return;
    setBusy(true);
    setError('');
    try {
      await command(`/api/runs/${encodeURIComponent(selectedRun.id)}/action`, { action });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось изменить запуск');
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="tasks-workspace">
      <aside className="runs-sidebar">
        <button className="primary wide" onClick={() => setComposing(true)}>
          <Plus size={15} />
          Новая задача
        </button>
        <h3>Последние запуски</h3>
        <p className="small muted">
          Показано {snapshot.runs.length} запусков. Всего задач: {snapshot.taskCount}.
        </p>
        {snapshot.runs.map((run) => (
          <button
            key={run.id}
            className={`run-card ${selectedRunId === run.id && !composing ? 'selected' : ''}`}
            onClick={() => {
              setSelectedRunId(run.id);
              setSelectedTaskId(null);
              setComposing(false);
            }}
          >
            <Box size={16} />
            <span>
              <strong>{run.title}</strong>
              <small>
                {new Date(run.createdAt).toLocaleString('ru')} · {run.taskIds.length} задач
              </small>
              <small>
                {run.accepted ? 'Принят' : run.stopped ? 'Остановлен' : 'Приёмка не завершена'}
              </small>
            </span>
          </button>
        ))}
        {!snapshot.runs.length && (
          <p className="small muted">
            После отправки здесь появятся задача, её состояние и результат.
          </p>
        )}
      </aside>
      <section className="tasks-main">
        <div hidden={!composing}>
          <TaskComposer
            snapshot={snapshot}
            command={command}
            onStarted={(id) => {
              setSelectedRunId(id);
              setSelectedTaskId(null);
              setComposing(false);
            }}
          />
        </div>
        {!composing && selectedRun && (
          <>
            <header className="run-heading">
              <h2>{selectedRun.title}</h2>
              <p className="small muted">Снимок архитектуры: v{selectedRun.configRevision}</p>
              <div className="task-actions">
                <button onClick={() => onReplay(selectedRun.id)}>
                  <History size={14} />
                  Посмотреть запись
                </button>
                <button
                  disabled={busy || selectedRun.stopped || selectedRun.accepted}
                  onClick={() => void act('cancel')}
                >
                  <Square size={14} />
                  Остановить запуск
                </button>
                {selectedRun.completion?.type === 'manual' && (
                  <button
                    disabled={busy || selectedRun.accepted || selectedRun.stopped}
                    onClick={() => void act('accept')}
                  >
                    Принять запуск вручную
                  </button>
                )}
              </div>
            </header>
            {error && (
              <p className="form-error" role="alert">
                {error}
              </p>
            )}
            <label className="task-selector">
              Задача исполнителя
              <select value={currentId ?? ''} onChange={(e) => setSelectedTaskId(e.target.value)}>
                {selectedRun.taskIds.map((id) => {
                  const t = snapshot.tasks.find((task) => task.id === id);
                  const agent = snapshot.config.value.registry.agents.find(
                    (a) => a.id === t?.assignee,
                  );
                  return (
                    <option key={id} value={id}>
                      {agent?.name ?? id} · {t ? stateLabels[t.state] : 'Загрузить состояние'}
                    </option>
                  );
                })}
              </select>
            </label>
            {currentId && (
              <TaskDetail
                key={currentId}
                taskId={currentId}
                client={client}
                command={command}
                events={events}
                agentName={
                  snapshot.config.value.registry.agents.find((a) => a.id === summary?.assignee)
                    ?.name ?? 'Исполнитель'
                }
                onContinue={onContinue}
              />
            )}
          </>
        )}
      </section>
    </div>
  );
}
