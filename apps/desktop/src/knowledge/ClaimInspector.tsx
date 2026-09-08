import { useState } from 'react';
import { BookOpen, Check, CircleHelp, Link2 } from 'lucide-react';
import type { ApiClient } from '../api';
import type { Stored } from '../types';
import { Pager, useKnowledgeCommand, usePage, useRemote } from './data';
import {
  knowledgePath,
  relationLabels,
  timestamp,
  verificationLabels,
  type Claim,
  type ClaimDetail,
  type KnowledgeLink,
  type Relation,
  type Source,
  type Verification,
} from './model';
export function SourceProvenance({ source }: { source: Source | null }) {
  if (!source)
    return (
      <div className="knowledge-empty">
        <BookOpen size={27} />
        <h3>Происхождение знаний</h3>
        <p>Выберите утверждение, чтобы увидеть источник, автора и решение проверки.</p>
      </div>
    );
  return (
    <div className="knowledge-provenance">
      <span className="eyebrow">ИСТОЧНИК</span>
      <h2>{source.name}</h2>
      <dl>
        <dt>Версия</dt>
        <dd>{source.version}</dd>
        <dt>Адрес</dt>
        <dd>{source.uri}</dd>
        <dt>Область</dt>
        <dd>{source.namespace}</dd>
        <dt>Срок актуальности</dt>
        <dd>{source.retentionDays} дней</dd>
        <dt>Зарегистрирован</dt>
        <dd>{new Date(source.createdAt).toLocaleString('ru')}</dd>
      </dl>
      <p className="small muted">
        Регистрация источника сохраняет происхождение. Содержимое доступно агентам по разрешениям
        подключения.
      </p>
    </div>
  );
}
export function ClaimInspector({
  id,
  client,
  refresh,
  onSaved,
  onSelect,
  candidates,
}: {
  id: string;
  client: ApiClient;
  refresh: number;
  onSaved: () => void;
  onSelect: (id: string) => void;
  candidates: Stored<Claim>[];
}) {
  const remote = useRemote<ClaimDetail>(
    client,
    `/api/knowledge/claims/${encodeURIComponent(id)}`,
    refresh,
  );
  const links = usePage<KnowledgeLink>(
    client,
    knowledgePath('links', { claimId: id, includeExpired: true }),
    refresh,
  );
  const operation = useKnowledgeCommand(client, onSaved);
  const [target, setTarget] = useState('');
  const [relation, setRelation] = useState<Relation>('supports');
  const data = remote.data;
  if (remote.error)
    return (
      <p className="form-error" role="alert">
        {remote.error}
      </p>
    );
  if (!data)
    return (
      <p className="muted" role="status">
        Загрузка происхождения…
      </p>
    );
  const claim = data.claim.value;
  const eligible = candidates.filter(
    (c) =>
      c.value.id !== id &&
      c.value.namespace === claim.namespace &&
      c.value.expiresAt > data.observedAt,
  );
  const disabled = operation.busy || data.expired;
  async function review(status: Verification) {
    await operation.save({ action: 'verify', claimId: id, revision: data!.claim.revision, status });
  }
  return (
    <div className="knowledge-provenance">
      <span className="eyebrow">ПРОИСХОЖДЕНИЕ</span>
      <h2>Карточка утверждения</h2>
      <span className={`knowledge-status ${data.expired ? 'expired' : claim.status}`}>
        {data.expired ? 'Устарело' : verificationLabels[claim.status]}
      </span>
      <p className="knowledge-assertion">{claim.assertion}</p>
      <dl>
        <dt>Источник</dt>
        <dd>{data.source?.value.name ?? 'Запись источника отсутствует'}</dd>
        <dt>Версия источника</dt>
        <dd>{claim.sourceVersion}</dd>
        <dt>Адрес</dt>
        <dd>{claim.sourceUri}</dd>
        <dt>Автор</dt>
        <dd>{claim.author === 'human:owner' ? 'Вы' : claim.author}</dd>
        <dt>Создано</dt>
        <dd>{timestamp(claim.createdAt)}</dd>
        <dt>Актуально до</dt>
        <dd>{timestamp(claim.expiresAt)}</dd>
        <dt>Проверяющий</dt>
        <dd>
          {claim.verifiedBy === 'human:owner' ? 'Вы' : (claim.verifiedBy ?? 'Ещё не назначен')}
        </dd>
        <dt>Дата проверки</dt>
        <dd>{timestamp(claim.verifiedAt)}</dd>
      </dl>
      <h3>
        <Check size={15} /> Решение проверки
      </h3>
      <div className="knowledge-review">
        <button disabled={disabled} onClick={() => void review('verified')}>
          Подтвердить
        </button>
        <button disabled={disabled} onClick={() => void review('disputed')}>
          Оспорить
        </button>
        <button disabled={disabled} onClick={() => void review('rejected')}>
          Отклонить
        </button>
      </div>
      {data.expired && (
        <p className="small muted">
          Срок актуальности истёк. Создайте утверждение на основе актуальной версии источника.
        </p>
      )}
      {operation.error && (
        <p role="alert" className="form-error">
          {operation.error}
        </p>
      )}
      <h3>
        <Link2 size={15} /> Связи
      </h3>
      {links.error && (
        <p role="alert" className="form-error">
          {links.error}
        </p>
      )}
      {links.data?.items.length === 0 && <p className="small muted">Связей пока нет.</p>}
      {links.data?.items.map(({ value: link }) => (
        <button
          key={link.id}
          className="knowledge-link"
          onClick={() => onSelect(link.from === id ? link.to : link.from)}
        >
          <span>
            {link.from === id ? '→' : '←'} {relationLabels[link.relation]}
          </span>
          <code>{link.from === id ? link.to : link.from}</code>
        </button>
      ))}
      <Pager page={links} label="Страницы связей" />
      <form
        className="knowledge-relation"
        onSubmit={(e) => {
          e.preventDefault();
          if (eligible.some((c) => c.value.id === target))
            void operation.save({ action: 'relate', from: id, to: target, relation });
        }}
      >
        <label>
          Связать с утверждением
          <select
            required
            disabled={disabled}
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          >
            <option value="">Выберите на текущей странице</option>
            {eligible.map((c) => (
              <option key={c.value.id} value={c.value.id}>
                {c.value.assertion.slice(0, 80)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Тип связи
          <select
            disabled={disabled}
            value={relation}
            onChange={(e) => setRelation(e.target.value as Relation)}
          >
            {Object.entries(relationLabels).map(([key, label]) => (
              <option key={key} value={key}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <button disabled={disabled || !eligible.some((c) => c.value.id === target)}>
          Сохранить связь
        </button>
      </form>
      <details className="knowledge-identifiers">
        <summary>
          <CircleHelp size={13} /> Идентификаторы
        </summary>
        <code data-testid="knowledge-detail-id">{claim.id}</code>
        <span>
          Ревизия {data.claim.revision} · {claim.namespace}
        </span>
      </details>
    </div>
  );
}
