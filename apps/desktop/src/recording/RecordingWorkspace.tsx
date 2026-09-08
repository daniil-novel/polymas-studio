import { useEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft, Pause, Play, RotateCcw, Search } from 'lucide-react';
import type { ApiClient } from '../api';
import { GraphView } from '../graph/GraphView';
import { IMPULSE_MS } from '../graph/model';
import { stateLabels } from '../types';
import { useRecordedDetail, useRecording } from './data';
import {
  recordingFrame,
  upperBound,
  type RecordedEvent,
  type RecordingManifest,
  type RecordingTimeline,
} from './model';
import './recording.css';

interface Props {
  client: ApiClient;
  runId: string;
  onExit: () => void;
}
export function RecordingWorkspace({ client, runId, onExit }: Props) {
  const [revision, setRevision] = useState(0);
  const { result, error, progress } = useRecording(client, runId, revision);
  return (
    <section className="recording-workspace" aria-label="Запись запуска">
      <header className="recording-header">
        <button onClick={onExit}>
          <ArrowLeft size={16} />
          Вернуться к текущему запуску
        </button>
        <span className="recording-badge">ЗАПИСЬ · ТОЛЬКО ПРОСМОТР</span>
        <button onClick={() => setRevision((n) => n + 1)}>
          <RotateCcw size={14} />
          Обновить запись
        </button>
      </header>
      {error ? (
        <div className="recording-empty">
          <p role="alert" className="form-error">
            {error}
          </p>
        </div>
      ) : result ? (
        <RecordingPlayer
          key={`${runId}:${revision}`}
          client={client}
          manifest={result.manifest}
          timeline={result.timeline}
        />
      ) : (
        <div className="recording-empty" role="status">
          Загрузка записи… {progress.toLocaleString('ru')} событий
        </div>
      )}
    </section>
  );
}
const isMessage = (item: RecordedEvent) => item.event.kind === 'message.delivered_to_context';
const isMemory = (item: RecordedEvent) =>
  item.event.kind === 'knowledge.read_completed' ||
  (item.event.kind === 'mcp.call_completed' && item.event.body.success === true);
const isFailure = (item: RecordedEvent) =>
  item.event.kind.includes('failed') ||
  ['failed', 'unknown', 'rejected'].includes(String(item.event.body.state)) ||
  item.event.body.success === false;

function RecordingPlayer({
  client,
  manifest,
  timeline,
}: {
  client: ApiClient;
  manifest: RecordingManifest;
  timeline: RecordingTimeline;
}) {
  const start = timeline.entries[0]?.time ?? Date.parse(manifest.createdAt);
  const end = Math.max(start, (timeline.entries.at(-1)?.time ?? start) + IMPULSE_MS);
  const [position, setPosition] = useState(start);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedEvent, setSelectedEvent] = useState<number | null>(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [scroll, setScroll] = useState(0);
  const listHost = useRef<HTMLDivElement>(null);
  const detail = useRecordedDetail(client, manifest.runId, selectedEvent);
  const projection = useMemo(
    () => recordingFrame(manifest, timeline, position),
    [manifest, timeline, position],
  );
  const selected = projection.nodes.find((n) => n.id === selectedId);
  const cursor = timeline.entries[upperBound(timeline.entries, position) - 1]?.event.sequence ?? 0;
  const labels = useMemo(
    () =>
      Object.fromEntries([
        ...manifest.roles.map((r) => [r.id, r.name]),
        ...manifest.architecture.holons.map((h) => [h.id, h.name]),
      ]),
    [manifest],
  );
  const filtered = useMemo(() => {
    const term = query.trim().toLocaleLowerCase();
    return timeline.entries.filter(
      (item) =>
        (filter === 'all' ||
          (filter === 'messages' && isMessage(item)) ||
          (filter === 'memory' && isMemory(item)) ||
          (filter === 'errors' && isFailure(item))) &&
        (!term ||
          `${item.event.kind} ${item.event.sequence} ${item.event.agentId ?? ''} ${item.event.body.taskId ?? ''}`
            .toLocaleLowerCase()
            .includes(term)),
    );
  }, [timeline, query, filter]);
  useEffect(() => {
    const host = listHost.current;
    if (!host) return;
    const index = filtered.findIndex((item) => item.event.sequence === selectedEvent);
    host.scrollTop = index < 0 ? 0 : Math.max(0, index * 42 - host.clientHeight / 2);
    setScroll(host.scrollTop);
  }, [selectedEvent, filtered]);
  useEffect(() => {
    if (!playing) return;
    let previous = performance.now();
    const interval = setInterval(() => {
      const next = performance.now();
      const delta = Math.min(250, next - previous) * speed;
      previous = next;
      setPosition((p) => Math.min(end, p + delta));
    }, 50);
    const hide = () => {
      if (document.hidden) setPlaying(false);
    };
    document.addEventListener('visibilitychange', hide);
    return () => {
      clearInterval(interval);
      document.removeEventListener('visibilitychange', hide);
    };
  }, [playing, speed, end]);
  useEffect(() => {
    if (position >= end) setPlaying(false);
  }, [position, end]);
  function selectEvent(item: RecordedEvent) {
    setPlaying(false);
    setPosition(item.time);
    setSelectedEvent(item.event.sequence);
    setSelectedId(item.event.agentId);
  }
  function nextEvent(predicate: (item: RecordedEvent) => boolean) {
    return timeline.entries.find((item) => item.time > position && predicate(item));
  }
  const listStart = Math.min(
    Math.max(0, Math.floor(scroll / 42) - 3),
    Math.max(0, filtered.length - 1),
  );
  return (
    <>
      <div className="recording-title">
        <div>
          <span className="eyebrow">
            {manifest.workspaceName} · АРХИТЕКТУРА v{manifest.configRevision}
          </span>
          <h1>{manifest.title}</h1>
        </div>
        <span className="small muted">
          До события #{manifest.head} · {timeline.entries.length.toLocaleString('ru')} событий
        </span>
      </div>
      <div className="recording-body">
        <div className="recording-canvas">
          <GraphView
            projection={projection}
            pinned={manifest.architecture.positions}
            selectedId={selectedId}
            onSelect={setSelectedId}
            query=""
            groupLabels={labels}
          />
          <p className="recording-legend">
            Состояния по журналу запуска · фиолетовый — чтение памяти · зелёный — доставка сообщения
          </p>
        </div>
        <aside className="recording-inspector">
          <span className="eyebrow">В ВЫБРАННЫЙ МОМЕНТ</span>
          <h2>{selected?.label ?? 'Наблюдаемые события'}</h2>
          {selected && (
            <p className={`status-text ${selected.state}`}>
              {stateLabels[selected.state] ?? selected.state}
            </p>
          )}
          <p className="small muted">
            Это сохранённые состояния задач. Запись не подтверждает текущую работу процессов и не
            раскрывает скрытые рассуждения.
          </p>
          <label className="recording-search">
            <Search size={14} />
            <input
              aria-label="Поиск в записи"
              placeholder="Тип события, ID…"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setScroll(0);
              }}
            />
          </label>
          <label>
            События
            <select
              aria-label="Фильтр событий записи"
              value={filter}
              onChange={(e) => {
                setFilter(e.target.value);
                setScroll(0);
              }}
            >
              <option value="all">Все события</option>
              <option value="messages">Доставка сообщений</option>
              <option value="memory">Чтение памяти / MCP</option>
              <option value="errors">Ошибки и отклонения</option>
            </select>
          </label>
          <span className="small muted">Найдено {filtered.length.toLocaleString('ru')}</span>
          <div
            className="recording-event-list"
            ref={listHost}
            style={{ height: Math.min(252, Math.max(84, filtered.length * 42)) }}
            aria-label="События записи"
            onScroll={(e) => setScroll(e.currentTarget.scrollTop)}
          >
            <div style={{ height: filtered.length * 42, position: 'relative' }}>
              {filtered.slice(listStart, listStart + 24).map((item, i) => (
                <button
                  key={item.event.sequence}
                  style={{ top: (listStart + i) * 42 }}
                  className={selectedEvent === item.event.sequence ? 'selected' : ''}
                  onClick={() => selectEvent(item)}
                  title={`#${item.event.sequence} ${item.event.kind}`}
                >
                  <small>
                    #{item.event.sequence} ·{' '}
                    {new Date(item.event.observedAt).toLocaleTimeString('ru')}
                  </small>
                  <span>{item.event.kind}</span>
                </button>
              ))}
            </div>
            {!filtered.length && <p className="small muted">Нет подходящих событий</p>}
          </div>
          {selectedEvent !== null && (
            <div className="recording-detail" aria-label="Сохранённое событие">
              <h3>Событие #{selectedEvent}</h3>
              {detail?.error ? (
                <p role="alert">{detail.error}</p>
              ) : detail?.event ? (
                <>
                  <time className="small muted">{detail.event.observedAt}</time>
                  <pre>{JSON.stringify(detail.event.body, null, 2)}</pre>
                </>
              ) : (
                <p role="status">Загрузка события…</p>
              )}
            </div>
          )}
        </aside>
      </div>
      <footer className="recording-transport">
        <div className="recording-buttons">
          <button
            className="primary"
            disabled={!timeline.entries.length}
            onClick={() => {
              if (position >= end) setPosition(start);
              setPlaying((p) => !p);
            }}
          >
            {playing ? <Pause size={15} /> : <Play size={15} />}
            {playing ? 'Приостановить запись' : 'Воспроизвести запись'}
          </button>
          <select
            aria-label="Скорость записи"
            value={speed}
            onChange={(e) => setSpeed(Number(e.target.value))}
          >
            {[0.5, 1, 2, 4].map((value) => (
              <option key={value} value={value}>
                {value}×
              </option>
            ))}
          </select>
          {[
            [isMessage, 'Следующее сообщение'],
            [isMemory, 'Следующее чтение'],
            [isFailure, 'Следующая ошибка'],
          ].map(([predicate, title]) => {
            const item = nextEvent(predicate as (item: RecordedEvent) => boolean);
            return (
              <button
                key={String(title)}
                disabled={!item}
                onClick={() => item && selectEvent(item)}
              >
                {String(title)}
              </button>
            );
          })}
          <time>
            {new Date(position).toLocaleString('ru')} · #{cursor}
          </time>
        </div>
        <input
          type="range"
          aria-label="Позиция записи"
          min={start}
          max={end}
          step="1"
          value={position}
          onChange={(e) => {
            setPlaying(false);
            setPosition(Number(e.target.value));
            setSelectedEvent(null);
          }}
        />
        <div className="recording-range">
          <span>{new Date(start).toLocaleTimeString('ru')}</span>
          <span>{new Date(end).toLocaleTimeString('ru')}</span>
        </div>
      </footer>
    </>
  );
}
