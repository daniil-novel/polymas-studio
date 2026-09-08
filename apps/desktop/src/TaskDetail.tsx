import { useEffect, useState } from 'react';
import { Check, Copy, Download, RotateCcw, Square, X } from 'lucide-react';
import type { ApiClient } from './api';
import type { Command } from './RoleEditor';
import { stateLabels, type RuntimeEvent, type Task } from './types';

const eventNames: Record<string, string> = {
  'runtime.started': 'Процесс запущен',
  'runtime.input_delivered': 'Задача передана процессу',
  'runtime.session': 'Сессия подключена',
  'runtime.turn_started': 'Исполнитель начал работу',
  'runtime.turn_completed': 'Ответ получен',
  'runtime.heartbeat': 'Исполнитель на связи',
  'runtime.item_started': 'Начато действие',
  'runtime.item_completed': 'Действие завершено',
  'runtime.item_updated': 'Действие обновлено',
  'runtime.error': 'Ошибка исполнителя',
  'task.state_changed': 'Состояние задачи изменено',
  'task.result_saved': 'Результат сохранён',
  'task.wait_requested': 'Агент запросил ожидание',
  'task.woken': 'Задача продолжена после ожидания',
};
export function TaskDetail({
  taskId,
  client,
  command,
  events,
  agentName,
  onContinue,
}: {
  taskId: string;
  client: ApiClient;
  command: Command;
  events: RuntimeEvent[];
  agentName: string;
  onContinue?: (agentId: string) => void;
}) {
  const [task, setTask] = useState<Task | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState('');
  const [refresh, setRefresh] = useState(0);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    const abort = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    setTask(null);
    setError('');
    setNote('');
    async function read() {
      try {
        const value = await client.request<Task>(
          `/api/tasks/${encodeURIComponent(taskId)}`,
          undefined,
          abort.signal,
        );
        if (abort.signal.aborted) return;
        setTask(value);
        setError('');
        if (['queued', 'waiting', 'waiting_user', 'running', 'stopping'].includes(value.state))
          timer = setTimeout(() => void read(), 2000);
      } catch (reason) {
        if (!abort.signal.aborted)
          setError(reason instanceof Error ? reason.message : 'Не удалось прочитать задачу');
      }
    }
    void read();
    return () => {
      abort.abort();
      clearTimeout(timer);
    };
  }, [client, taskId, refresh]);
  const current = task?.id === taskId ? task : null;
  async function act(action: string) {
    setBusy(true);
    setError('');
    try {
      await command(`/api/tasks/${encodeURIComponent(taskId)}/action`, {
        action,
        ...(['retry', 'reconcile'].includes(action) ? { note } : {}),
      });
      setRefresh((n) => n + 1);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Команда не выполнена');
    } finally {
      setBusy(false);
    }
  }
  function download() {
    if (!current?.result) return;
    const blob = new Blob([current.result], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `result-${current.id}.md`;
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  const taskEvents = events.filter((e) => e.body.taskId === taskId).slice(-100);
  return (
    <section className="task-detail" aria-label="Подробности задачи">
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {!current ? (
        <p className="muted">{error ? 'Данные задачи недоступны' : 'Загрузка задачи…'}</p>
      ) : (
        <>
          <div className="task-detail-heading">
            <div>
              <span className="eyebrow">ОТДЕЛЬНЫЙ ЭКЗЕМПЛЯР</span>
              <h2>{agentName}</h2>
            </div>
            <span className={`badge task-state-${current.state}`}>
              {stateLabels[current.state]}
            </span>
          </div>
          <div className="task-metadata">
            <span>Попытка {current.attempt}</span>
            <span>
              Роль {current.role.name} · v{current.role.version}
            </span>
            {current.lease?.pid && <span>PID {current.lease.pid}</span>}
            <code>{current.id}</code>
          </div>
          <div className="conversation-message user-message">
            <span className="message-author">Задача пользователя</span>
            <p>{current.prompt}</p>
          </div>
          {current.criteria && (
            <div className="acceptance-criteria">
              <strong>Критерии приёмки</strong>
              <p>{current.criteria}</p>
            </div>
          )}
          {current.error && <p className="form-error">{current.error}</p>}
          {current.state === 'waiting_user' && (
            <div className="task-waiting">
              <strong>Нужно подтверждение операции</strong>
              <p>
                Откройте раздел «Подтверждения», чтобы проверить запрос. Слот исполнителя свободен;
                после решения продолжится эта же сессия.
              </p>
            </div>
          )}
          {current.state === 'waiting' && current.waiting && (
            <div className="wait-panel" role="status">
              <strong>Ожидание ответа</strong>
              <p>
                Слот исполнителя свободен. Задача продолжится после подходящего сообщения или по
                истечении срока.
              </p>
              <dl>
                <dt>Отправитель</dt>
                <dd>{current.waiting.sender ?? 'Любой разрешённый агент'}</dd>
                <dt>Срок ожидания</dt>
                <dd>
                  <time dateTime={new Date(current.waiting.deadlineAt * 1000).toISOString()}>
                    {new Date(current.waiting.deadlineAt * 1000).toLocaleString('ru')}
                  </time>
                </dd>
                {current.waiting.correlationId && (
                  <>
                    <dt>Цепочка сообщений</dt>
                    <dd>{current.waiting.correlationId}</dd>
                  </>
                )}
              </dl>
            </div>
          )}
          {current.result ? (
            <div className="conversation-message assistant-message">
              <div className="result-heading">
                <span className="message-author">
                  {['waiting', 'waiting_user'].includes(current.state)
                    ? 'Промежуточная сводка'
                    : 'Результат агента'}
                </span>
                <button
                  className="icon-button"
                  aria-label="Скопировать результат"
                  onClick={() => {
                    void navigator.clipboard
                      .writeText(current.result ?? '')
                      .then(() => setCopied(true))
                      .catch(() => setError('Буфер обмена недоступен'));
                  }}
                >
                  <Copy size={14} />
                </button>
                <button
                  className="icon-button"
                  aria-label="Скачать результат Markdown"
                  onClick={download}
                >
                  <Download size={14} />
                </button>
              </div>
              <pre>{current.result}</pre>
              {copied && <span className="small muted">Скопировано</span>}
            </div>
          ) : (
            <div className="execution-placeholder">
              <span className="dot idle" />
              <p>
                {current.state === 'queued'
                  ? 'Задача в очереди. Ответ появится после выполнения.'
                  : current.state === 'unknown'
                    ? 'Исход попытки неизвестен. Проверьте процесс и внешние действия перед повтором.'
                    : current.state === 'running'
                      ? 'Агент выполняет задачу. Ниже — наблюдаемые действия исполнителя.'
                      : 'Сохранённого результата нет.'}
              </p>
            </div>
          )}
          {current.artifactIds.length > 0 && (
            <details>
              <summary>Сохранённые артефакты · {current.artifactIds.length}</summary>
              <div className="artifact-ids">
                {current.artifactIds.map((id) => (
                  <code key={id}>{id}</code>
                ))}
              </div>
            </details>
          )}
          <div className="task-actions">
            {current.state === 'awaiting_acceptance' && (
              <>
                <button className="primary" disabled={busy} onClick={() => void act('accept')}>
                  <Check size={15} />
                  Принять результат
                </button>
                <button disabled={busy} onClick={() => void act('reject')}>
                  <X size={15} />
                  Отклонить
                </button>
              </>
            )}
            {['queued', 'waiting', 'waiting_user', 'running', 'stopping'].includes(
              current.state,
            ) && (
              <button
                disabled={busy || current.state === 'stopping'}
                onClick={() => void act('cancel')}
              >
                <Square size={14} />
                {current.state === 'stopping' ? 'Остановка…' : 'Отменить задачу'}
              </button>
            )}
            {onContinue &&
              !['queued', 'waiting', 'waiting_user', 'running', 'stopping', 'unknown'].includes(
                current.state,
              ) && (
                <button disabled={busy} onClick={() => onContinue(current.assignee)}>
                  Продолжить диалог
                </button>
              )}
          </div>
          {['unknown', 'failed', 'cancelled', 'rejected'].includes(current.state) && (
            <div className="recovery-panel">
              <h3>Сверка перед повтором</h3>
              <p className="small muted">
                Укажите, что проверено и почему повтор безопасен. Неизвестный исход внешней записи
                требует отдельной проверки.
              </p>
              <label>
                Основание для повтора или завершения
                <textarea rows={2} value={note} onChange={(e) => setNote(e.target.value)} />
              </label>
              <div className="task-actions">
                <button disabled={busy || !note.trim()} onClick={() => void act('retry')}>
                  <RotateCcw size={15} />
                  Повторить задачу
                </button>
                {current.state === 'unknown' && (
                  <button disabled={busy || !note.trim()} onClick={() => void act('reconcile')}>
                    Завершить без повтора
                  </button>
                )}
              </div>
            </div>
          )}
          <details open={!current.result && current.state === 'running'}>
            <summary>Наблюдаемые действия · {taskEvents.length}</summary>
            <div className="task-event-list">
              {taskEvents.map((event) => (
                <div key={event.sequence}>
                  <time>{new Date(event.observedAt).toLocaleTimeString('ru')}</time>
                  <span>{eventNames[event.kind] ?? event.kind}</span>
                  <small>#{event.sequence}</small>
                </div>
              ))}
            </div>
            <p className="small muted">
              События процесса, инструментов и сообщений. Внутренние рассуждения модели не
              отображаются.
            </p>
          </details>
        </>
      )}
    </section>
  );
}
