import { useState } from 'react';
import { X } from 'lucide-react';
import { Modal } from '../Modal';
import type { ApiClient } from '../api';
import type { Snapshot } from '../types';
import { useKnowledgeCommand } from './data';
import type { Source } from './model';
export function KnowledgeForm({
  kind,
  source,
  snapshot,
  client,
  onSaved,
  onClose,
}: {
  kind: 'source' | 'claim';
  source: Source | null;
  snapshot: Snapshot;
  client: ApiClient;
  onSaved: () => void;
  onClose: () => void;
}) {
  const [name, setName] = useState('');
  const [uri, setUri] = useState('');
  const [version, setVersion] = useState('');
  const [namespace, setNamespace] = useState('shared');
  const [connectionId, setConnectionId] = useState('');
  const [retentionDays, setRetentionDays] = useState('30');
  const [assertion, setAssertion] = useState('');
  const operation = useKnowledgeCommand(client, () => {
    onSaved();
    onClose();
  });
  const dirty = Boolean(
    name ||
    uri ||
    version ||
    assertion ||
    namespace !== 'shared' ||
    connectionId ||
    retentionDays !== '30',
  );
  function close() {
    if (
      !operation.busy &&
      (!dirty || window.confirm('Закрыть форму и потерять несохранённые изменения?'))
    )
      onClose();
  }
  return (
    <Modal label={kind === 'source' ? 'Добавление источника' : 'Новое утверждение'} onClose={close}>
      <form
        className="knowledge-form"
        onSubmit={(e) => {
          e.preventDefault();
          void operation.save(
            kind === 'source'
              ? {
                  action: 'add_source',
                  draft: {
                    name,
                    uri,
                    version,
                    namespace,
                    connectionId: connectionId || null,
                    retentionDays: Number(retentionDays),
                  },
                }
              : {
                  action: 'publish',
                  draft: { assertion, sourceId: source?.id, namespace: source?.namespace },
                },
          );
        }}
      >
        <header>
          <div>
            <span className="eyebrow">БАЗА ЗНАНИЙ</span>
            <h2>{kind === 'source' ? 'Новый источник' : 'Новое утверждение'}</h2>
          </div>
          <button
            type="button"
            className="icon-button"
            aria-label="Закрыть форму знаний"
            disabled={operation.busy}
            onClick={close}
          >
            <X size={19} />
          </button>
        </header>
        <fieldset disabled={operation.busy}>
          {kind === 'source' ? (
            <>
              <p className="muted">
                Сохраните адрес и версию для проверки происхождения. Если это подключение выбрано
                общей памятью, его утверждения смогут читать агенты с доступом к указанной области.
              </p>
              <label>
                Название источника
                <input
                  required
                  maxLength={200}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </label>
              <label>
                Адрес источника
                <input
                  required
                  maxLength={4096}
                  placeholder="https://… или file:///…"
                  value={uri}
                  onChange={(e) => setUri(e.target.value)}
                />
              </label>
              <label>
                Версия или hash
                <input
                  required
                  maxLength={512}
                  placeholder="v1 или sha256:…"
                  value={version}
                  onChange={(e) => setVersion(e.target.value)}
                />
              </label>
              <div className="knowledge-form-grid">
                <label>
                  Область знаний
                  <input
                    required
                    maxLength={120}
                    value={namespace}
                    onChange={(e) => setNamespace(e.target.value)}
                  />
                </label>
                <label>
                  Срок актуальности, дней
                  <input
                    required
                    type="number"
                    min={1}
                    max={3650}
                    value={retentionDays}
                    onChange={(e) => setRetentionDays(e.target.value)}
                  />
                </label>
              </div>
              <label>
                Подключение MCP
                <select value={connectionId} onChange={(e) => setConnectionId(e.target.value)}>
                  <option value="">Самостоятельная запись источника</option>
                  {snapshot.connections.map((c) => (
                    <option key={c.value.id} value={c.value.id}>
                      {c.value.name}
                    </option>
                  ))}
                </select>
              </label>
            </>
          ) : (
            <>
              <p className="knowledge-source-caption">
                {source?.name} <span>· {source?.version}</span>
              </p>
              <label>
                Текст утверждения
                <textarea
                  required
                  maxLength={32000}
                  rows={8}
                  value={assertion}
                  onChange={(e) => setAssertion(e.target.value)}
                />
              </label>
              <p className="muted">
                Автор: вы. Начальный статус — «Не проверено». Источник и его версия сохранятся
                вместе с утверждением.
              </p>
            </>
          )}
        </fieldset>
        {operation.error && (
          <p role="alert" className="form-error">
            {operation.error}
          </p>
        )}
        <footer>
          <button type="button" onClick={close} disabled={operation.busy}>
            Отмена
          </button>
          <button className="primary" disabled={operation.busy || (kind === 'claim' && !source)}>
            {operation.busy
              ? 'Сохранение…'
              : kind === 'source'
                ? 'Сохранить источник'
                : 'Сохранить утверждение'}
          </button>
        </footer>
      </form>
    </Modal>
  );
}
