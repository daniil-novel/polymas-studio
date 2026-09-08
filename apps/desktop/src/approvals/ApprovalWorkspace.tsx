import { useEffect, useRef, useState } from 'react';
import { Check, RefreshCw, ShieldCheck, X } from 'lucide-react';
import type { ApiClient } from '../api';
import type { Snapshot } from '../types';
import { useRemote } from '../knowledge/data';
import {
  approvalLabels,
  type ApprovalDetail,
  type ApprovalState,
  type ApprovalSummary,
} from './model';
import './approvals.css';
import { ReconciliationForm, type ReconciliationDraft } from './ReconciliationForm';
import { OperationResultViewer } from './OperationResultViewer';
const emptyDraft: ReconciliationDraft = { outcome: '', note: '' };

const date = (seconds: number) => new Date(seconds * 1000).toLocaleString('ru');
export function ApprovalWorkspace({
  client,
  snapshot,
  connected,
}: {
  client: ApiClient;
  snapshot: Snapshot;
  connected: boolean;
}) {
  const [filter, setFilter] = useState<ApprovalState | ''>('pending');
  const [cursors, setCursors] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [poll, setPoll] = useState(0);
  const [busy, setBusy] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, ReconciliationDraft>>({});
  const inFlight = useRef(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [clock, setClock] = useState(Date.now());
  useEffect(() => {
    const timer = setInterval(() => setClock(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const page = useRemote<{ items: ApprovalSummary[]; nextCursor: string | null }>(
    client,
    `/api/approvals?limit=30${filter ? `&state=${filter}` : ''}${cursors.length ? `&after=${encodeURIComponent(cursors.at(-1)!)}` : ''}`,
    refresh,
    poll,
  );
  const currentId = selected ?? page.data?.items[0]?.id ?? null;
  const draft = currentId ? (drafts[currentId] ?? emptyDraft) : emptyDraft;
  const editing = Boolean(draft.note || draft.outcome);
  useEffect(() => {
    if (busy || !connected || editing) return;
    const timer = setInterval(() => {
      if (document.visibilityState !== 'hidden') setPoll((n) => n + 1);
    }, 5000);
    return () => clearInterval(timer);
  }, [busy, connected, editing]);
  const agentName = (id: string) =>
    snapshot.config.value.registry.agents.find((a) => a.id === id)?.name ?? id;
  function resetSelection() {
    setSelected(null);
    setNotice('');
    setError('');
  }
  function changed() {
    setRefresh((n) => n + 1);
  }
  function decide(detail: ApprovalDetail, allow: boolean) {
    return save(
      detail,
      'decision',
      {
        expectedRevision: detail.approval.revision,
        fingerprint: detail.approval.value.fingerprint,
        allow,
      },
      allow
        ? 'Разрешение сохранено. Агент продолжит работу, когда будет доступен слот.'
        : 'Отказ сохранён. Агент получит решение в продолжении задачи.',
    );
  }
  function reconcile(detail: ApprovalDetail, draft: ReconciliationDraft) {
    return save(
      detail,
      'reconcile',
      { expectedRevision: detail.approval.revision, outcome: draft.outcome, note: draft.note },
      'Сверка сохранена. Вернитесь к задаче, чтобы проверить результат или продолжить работу.',
    );
  }
  async function save(
    detail: ApprovalDetail,
    action: 'decision' | 'reconcile',
    body: unknown,
    message: string,
  ) {
    if (inFlight.current || !connected) return;
    inFlight.current = true;
    setBusy(true);
    setError('');
    setNotice('');
    const { value } = detail.approval;
    try {
      await client.request(`/api/approvals/${encodeURIComponent(value.id)}/${action}`, body);
      setNotice(message);
      setDrafts((current) => {
        const next = { ...current };
        delete next[value.id];
        return next;
      });
    } catch (reason) {
      setError(
        `${reason instanceof Error ? reason.message : 'Ответ службы не получен'}. Обновите карточку, чтобы проверить сохранённое решение.`,
      );
    } finally {
      // A lost response must never leave the old actionable card on screen.
      changed();
      inFlight.current = false;
      setBusy(false);
    }
  }
  return (
    <div className="approval-workspace">
      <header className="approval-heading">
        <div>
          <span className="eyebrow">КОНТРОЛЬ ДЕЙСТВИЙ</span>
          <h1>
            <ShieldCheck size={25} /> Подтверждения
          </h1>
          <p>Проверьте ресурс и параметры перед внешней записью.</p>
        </div>
        <button disabled={busy} onClick={changed}>
          <RefreshCw size={15} /> Обновить
        </button>
      </header>
      {!connected && (
        <p className="form-error" role="status">
          Нет связи со службой. Решения временно недоступны.
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {notice && (
        <p className="approval-notice" role="status">
          {notice}
        </p>
      )}
      <div className="approval-columns">
        <aside className="approval-list" aria-label="Список операций">
          <label>
            Состояние
            <select
              disabled={busy}
              value={filter}
              onChange={(e) => {
                setFilter(e.target.value as ApprovalState | '');
                setCursors([]);
                resetSelection();
              }}
            >
              <option value="">Все операции</option>
              {Object.entries(approvalLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          {page.error && (
            <p role="alert" className="form-error">
              {page.error}
            </p>
          )}
          {page.loading && !page.data && <p className="muted">Загрузка операций…</p>}
          {page.data?.items.length === 0 && (
            <div className="approval-empty">
              <ShieldCheck size={30} />
              <h3>Операций с этим состоянием нет</h3>
              <p>Запросы агентов появятся здесь.</p>
            </div>
          )}
          <div className="approval-items">
            {page.data?.items.map((item) => (
              <button
                key={item.id}
                disabled={busy}
                className={`approval-item ${currentId === item.id ? 'selected' : ''}`}
                aria-label={`Открыть операцию ${item.id}`}
                aria-pressed={currentId === item.id}
                onClick={() => {
                  setSelected(item.id);
                  setNotice('');
                  setError('');
                }}
              >
                <span className={`approval-state ${item.state}`}>{approvalLabels[item.state]}</span>
                <strong>{agentName(item.agentId)}</strong>
                <code>{item.tool}</code>
                <span className="muted">{item.namespace}</span>
              </button>
            ))}
          </div>
          <nav className="approval-pager" aria-label="Страницы операций">
            <button
              disabled={busy || !cursors.length || page.loading}
              onClick={() => {
                setCursors((c) => c.slice(0, -1));
                resetSelection();
              }}
            >
              Назад
            </button>
            <span>{cursors.length + 1}</span>
            <button
              disabled={busy || page.loading || !page.data?.nextCursor}
              onClick={() => {
                setCursors((c) => [...c, page.data!.nextCursor!]);
                resetSelection();
              }}
            >
              Далее
            </button>
          </nav>
        </aside>
        {currentId ? (
          <ApprovalCard
            key={currentId}
            id={currentId}
            client={client}
            snapshot={snapshot}
            refresh={refresh}
            poll={poll}
            busy={busy}
            connected={connected}
            clock={clock}
            onDecision={decide}
            onReconcile={reconcile}
            draft={draft}
            onDraft={(value) => setDrafts((current) => ({ ...current, [currentId]: value }))}
          />
        ) : (
          <section className="approval-card approval-empty">
            <ShieldCheck size={36} />
            <h2>Выберите операцию</h2>
            <p>Здесь появятся точный запрос и доступные решения.</p>
          </section>
        )}
      </div>
    </div>
  );
}

function ApprovalCard({
  id,
  client,
  snapshot,
  refresh,
  poll,
  busy,
  connected,
  clock,
  onDecision,
  onReconcile,
  draft,
  onDraft,
}: {
  id: string;
  client: ApiClient;
  snapshot: Snapshot;
  refresh: number;
  poll: number;
  busy: boolean;
  connected: boolean;
  clock: number;
  onDecision: (detail: ApprovalDetail, allow: boolean) => Promise<void>;
  onReconcile: (detail: ApprovalDetail, draft: ReconciliationDraft) => Promise<void>;
  draft: ReconciliationDraft;
  onDraft: (draft: ReconciliationDraft) => void;
}) {
  const remote = useRemote<ApprovalDetail>(
    client,
    `/api/approvals/${encodeURIComponent(id)}`,
    refresh,
    poll,
  );
  const detail = remote.data;
  if (!detail)
    return (
      <section className="approval-card">
        <p
          role={remote.error ? 'alert' : 'status'}
          className={remote.error ? 'form-error' : 'muted'}
        >
          {remote.error ?? 'Загрузка точного запроса…'}
        </p>
      </section>
    );
  const { value } = detail.approval;
  const op = value.operation;
  const expired = clock >= value.expiresAt * 1000;
  const transport = detail.connection?.transport;
  const agent = snapshot.config.value.registry.agents.find((a) => a.id === op.agentId);
  const run = snapshot.runs.find((r) => r.id === op.runId);
  const unavailable = busy || !connected || remote.loading || Boolean(remote.error);
  return (
    <section className="approval-card" aria-label="Точная операция">
      <header>
        <div>
          <span className={`approval-state ${value.state}`}>{approvalLabels[value.state]}</span>
          <h2>{agent?.name ?? op.agentId}</h2>
          <p>{run?.title ?? op.runId}</p>
        </div>
        <ShieldCheck size={32} />
      </header>
      <div className="approval-scope">
        <div>
          <span>Подключение</span>
          <strong>{detail.connection?.name ?? op.connectionId}</strong>
        </div>
        <div>
          <span>Область</span>
          <strong>{op.namespace}</strong>
        </div>
        <div>
          <span>Инструмент</span>
          <code>{op.tool}</code>
        </div>
        <div>
          <span>Разрешение действует до</span>
          <strong>{date(value.expiresAt)}</strong>
        </div>
      </div>
      <h3>Ресурс на момент запроса</h3>
      <div className="approval-resource">
        {transport?.type === 'http' ? (
          <code>{transport.url}</code>
        ) : transport?.type === 'stdio' ? (
          <>
            <code>{transport.command}</code>
            <pre>{JSON.stringify(transport.args, null, 2)}</pre>
          </>
        ) : transport?.type === 'files' ? (
          transport.roots.map((root) => <code key={root}>{root}</code>)
        ) : (
          <p className="muted">Описание ресурса отсутствует в старой записи.</p>
        )}
      </div>
      <h3>Точные параметры операции</h3>
      <pre className="approval-arguments">{JSON.stringify(op.arguments, null, 2)}</pre>
      <p className="muted">
        Разрешение относится к одной показанной операции. Во время ожидания агент освобождает слот
        исполнителя.
      </p>
      {remote.error && (
        <p className="form-error" role="alert">
          {remote.error}
        </p>
      )}
      {value.state === 'pending' && detail.unavailableReason && (
        <p className="form-error">{detail.unavailableReason}</p>
      )}
      {expired && value.state === 'pending' && (
        <p className="form-error">
          Срок запроса истёк. Агент получит это состояние при продолжении.
        </p>
      )}
      {value.state === 'pending' && (
        <div className="approval-actions">
          <button
            disabled={unavailable || !detail.canDeny || expired}
            onClick={() => void onDecision(detail, false)}
          >
            <X size={16} /> Отклонить операцию
          </button>
          <button
            className="primary"
            disabled={unavailable || !detail.canApprove || expired}
            onClick={() => void onDecision(detail, true)}
          >
            <Check size={16} /> Разрешить эту операцию
          </button>
        </div>
      )}
      {detail.resultMetadata && (
        <OperationResultViewer
          client={client}
          id={value.id}
          metadata={detail.resultMetadata}
          connected={connected}
        />
      )}
      {value.state === 'unknown' && (
        <ReconciliationForm
          draft={draft}
          disabled={unavailable}
          onChange={onDraft}
          onSubmit={() => void onReconcile(detail, draft)}
        />
      )}
      {value.state === 'revoked' && (
        <p className="muted">
          Попытка завершена или отменена. Это разрешение больше не допускает внешний вызов.
        </p>
      )}
      {detail.reconciliation && (
        <div className="approval-reconciliation-record">
          <h3>Результат сверки владельца</h3>
          <p>{detail.reconciliation.note}</p>
          <time>{new Date(detail.reconciliation.observedAt).toLocaleString('ru')}</time>
        </div>
      )}
      <details className="approval-identifiers">
        <summary>Данные операции</summary>
        <dl>
          <dt>Запрос</dt>
          <dd>{value.id}</dd>
          <dt>Задача</dt>
          <dd>{op.taskId}</dd>
          <dt>Ревизия подключения</dt>
          <dd>{op.connectionRevision}</dd>
          <dt>Попытки: запрос → исполнение</dt>
          <dd>
            {op.originEpoch} → {op.epoch}
          </dd>
          <dt>Отпечаток операции</dt>
          <dd>{value.fingerprint}</dd>
        </dl>
      </details>
    </section>
  );
}
