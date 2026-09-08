import { useState } from 'react';
import { BookOpen, Database, FileText, Plus, RefreshCw, Search } from 'lucide-react';
import type { ApiClient } from '../api';
import type { Snapshot } from '../types';
import { Pager, usePage } from './data';
import {
  knowledgePath,
  verificationLabels,
  type Claim,
  type Source,
  type Verification,
} from './model';
import { ClaimInspector, SourceProvenance } from './ClaimInspector';
import { KnowledgeForm } from './KnowledgeForms';
import './knowledge.css';
export function KnowledgeWorkspace({
  client,
  snapshot,
}: {
  client: ApiClient;
  snapshot: Snapshot;
}) {
  const [namespace, setNamespace] = useState('');
  const [query, setQuery] = useState('');
  const [sourceQuery, setSourceQuery] = useState('');
  const [status, setStatus] = useState<Verification | ''>('');
  const [includeExpired, setIncludeExpired] = useState(false);
  const [source, setSource] = useState<Source | null>(null);
  const [claimId, setClaimId] = useState<string | null>(null);
  const [form, setForm] = useState<'source' | 'claim' | null>(null);
  const [refresh, setRefresh] = useState(0);
  const update = () => setRefresh((n) => n + 1);
  const sources = usePage<Source>(
    client,
    knowledgePath('sources', { namespace, query: sourceQuery }),
    refresh,
  );
  const claims = usePage<Claim>(
    client,
    knowledgePath('claims', { namespace, query, status, includeExpired, sourceId: source?.id }),
    refresh,
  );
  const items = claims.data?.items ?? [];
  function changeScope(value: string) {
    setNamespace(value);
    setSource(null);
    setClaimId(null);
  }
  return (
    <div className="knowledge-workspace">
      <header className="knowledge-header">
        <BookOpen size={25} />
        <div>
          <span className="eyebrow">ПАМЯТЬ И ДОКАЗАТЕЛЬСТВА</span>
          <h1>Знания</h1>
        </div>
        <span className="knowledge-memory">
          <i className="dot violet" />
          {snapshot.config.value.architecture.memoryEnabled
            ? 'Общая память включена'
            : 'Общая память выключена'}
        </span>
        <button onClick={update}>
          <RefreshCw size={14} />
          Обновить
        </button>
        <button className="primary" onClick={() => setForm('source')}>
          <Plus size={15} />
          Добавить источник
        </button>
      </header>
      <div className="knowledge-columns">
        <aside className="knowledge-sources">
          <h2>
            <Database size={16} /> Источники
          </h2>
          <label>
            Область знаний
            <input
              value={namespace}
              maxLength={120}
              placeholder="Все области"
              onChange={(e) => changeScope(e.target.value)}
            />
          </label>
          <label>
            Найти источник
            <input
              value={sourceQuery}
              maxLength={100}
              placeholder="Название…"
              onChange={(e) => setSourceQuery(e.target.value)}
            />
          </label>
          <button
            className={`knowledge-source-card ${source === null ? 'selected' : ''}`}
            onClick={() => {
              setSource(null);
              setClaimId(null);
            }}
          >
            <BookOpen size={16} />
            <span>
              Все источники<small>Утверждения рабочей области</small>
            </span>
          </button>
          {sources.error && (
            <p role="alert" className="form-error">
              {sources.error}
            </p>
          )}
          {sources.loading && (
            <p className="muted" role="status">
              Загрузка источников…
            </p>
          )}
          {sources.data?.items.map(({ value: item }) => (
            <button
              key={item.id}
              className={`knowledge-source-card ${source?.id === item.id ? 'selected' : ''}`}
              onClick={() => {
                setSource(item);
                setClaimId(null);
              }}
            >
              <FileText size={16} />
              <span>
                {item.name}
                <small>
                  {item.namespace} · {item.version}
                </small>
              </span>
            </button>
          ))}
          {sources.data?.items.length === 0 && (
            <p className="small muted">По этим условиям источников нет.</p>
          )}
          <Pager page={sources} label="Страницы источников" />
          <div className="knowledge-note">
            Источники сохраняются при выключении общей памяти. Права агентов задаются отдельно.
          </div>
        </aside>
        <section className="knowledge-claims">
          <div className="knowledge-section-title">
            <div>
              <span className="eyebrow">{source?.name ?? 'ВСЕ ИСТОЧНИКИ'}</span>
              <h2>Утверждения</h2>
            </div>
            <button
              disabled={!source}
              title={source ? 'Добавить утверждение' : 'Сначала выберите источник слева'}
              onClick={() => setForm('claim')}
            >
              <Plus size={14} />
              Добавить утверждение
            </button>
          </div>
          <div className="knowledge-search">
            <Search size={15} />
            <input
              aria-label="Поиск утверждений"
              placeholder="Найти в тексте…"
              maxLength={100}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="knowledge-filters">
            <label>
              Статус
              <select
                value={status}
                onChange={(e) => setStatus(e.target.value as Verification | '')}
              >
                <option value="">Все статусы</option>
                {Object.entries(verificationLabels).map(([key, label]) => (
                  <option key={key} value={key}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <label className="knowledge-checkbox">
              <input
                type="checkbox"
                checked={includeExpired}
                onChange={(e) => setIncludeExpired(e.target.checked)}
              />
              Включая устаревшие
            </label>
          </div>
          {claims.error && (
            <p role="alert" className="form-error">
              {claims.error}
            </p>
          )}
          {claims.loading && (
            <p role="status" className="muted">
              Загрузка утверждений…
            </p>
          )}
          {claims.data && <p className="small muted">На странице: {items.length}</p>}
          {items.map(({ value: claim }) => {
            const expired = claim.expiresAt <= (claims.data?.observedAt ?? 0);
            return (
              <button
                aria-label={`Открыть утверждение ${claim.id}`}
                key={claim.id}
                className={`knowledge-claim-card ${claimId === claim.id ? 'selected' : ''}`}
                onClick={() => setClaimId(claim.id)}
              >
                <span className={`knowledge-status ${expired ? 'expired' : claim.status}`}>
                  {expired ? 'Устарело' : verificationLabels[claim.status]}
                </span>
                <p>{claim.assertion}</p>
                <small>
                  {claim.namespace} · {claim.sourceVersion}
                </small>
              </button>
            );
          })}
          {claims.data && items.length === 0 && (
            <div className="knowledge-empty">
              <BookOpen size={34} />
              <h3>Утверждений пока нет</h3>
              <p>
                Выберите источник и сохраните утверждение с его происхождением или измените фильтры.
              </p>
            </div>
          )}
          <Pager page={claims} label="Страницы утверждений" />
        </section>
        <aside className="knowledge-inspector" aria-label="Происхождение утверждения">
          {claimId ? (
            <ClaimInspector
              key={claimId}
              id={claimId}
              client={client}
              refresh={refresh}
              onSaved={update}
              onSelect={setClaimId}
              candidates={items}
            />
          ) : (
            <SourceProvenance source={source} />
          )}
        </aside>
      </div>
      {form && (
        <KnowledgeForm
          kind={form}
          source={source}
          snapshot={snapshot}
          client={client}
          onSaved={update}
          onClose={() => setForm(null)}
        />
      )}
    </div>
  );
}
