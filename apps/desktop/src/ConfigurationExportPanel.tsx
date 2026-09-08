import { useEffect, useMemo, useRef, useState } from 'react';
import { Download, FileJson } from 'lucide-react';
import type { ApiClient } from './api';
import {
  exportTextPage,
  prepareConfigurationExport,
  saveConfigurationExport,
  type PreparedExport,
} from './export-data';
import './export.css';

export function ConfigurationExportPanel({
  client,
  connected,
  currentRevision,
  save = saveConfigurationExport,
}: {
  client: ApiClient;
  connected: boolean;
  currentRevision: number;
  save?: (text: string, filename: string) => void;
}) {
  const activation = useMemo(() => ({ connected, client }), [connected, client]);
  const [prepared, setPrepared] = useState<{ file: PreparedExport; activation: object } | null>(
    null,
  );
  const [working, setWorking] = useState<object | null>(null);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const [page, setPage] = useState(0);
  const pending = useRef<AbortController | null>(null);
  useEffect(
    () => () => {
      pending.current?.abort();
      pending.current = null;
    },
    [activation],
  );
  const file = prepared?.activation === activation && connected ? prepared.file : null;
  async function prepare() {
    if (!connected || (pending.current && !pending.current.signal.aborted)) return;
    const abort = new AbortController();
    pending.current = abort;
    setPrepared(null);
    setError('');
    setSaved(false);
    setWorking(activation);
    setPage(0);
    try {
      const value = await client.request('/api/exports/configuration', undefined, abort.signal);
      abort.signal.throwIfAborted();
      const file = prepareConfigurationExport(value);
      setPrepared({ file, activation });
    } catch (reason) {
      if (!abort.signal.aborted)
        setError(reason instanceof Error ? reason.message : 'Не удалось подготовить экспорт');
    } finally {
      if (pending.current === abort) {
        pending.current = null;
        setWorking(null);
      }
    }
  }
  function cancel() {
    pending.current?.abort();
    pending.current = null;
    setWorking(null);
  }
  const visibleBusy = working === activation && connected;
  return (
    <section className="configuration-export" aria-label="Экспорт конфигурации">
      <h2>
        <FileJson size={20} /> Экспорт конфигурации
      </h2>
      <p className="muted">
        Сохраните роли, экземпляры, холоны, связи и настройки архитектуры в переносимый JSON.
      </p>
      <p className="small muted">
        История задач, сессии и артефакты остаются в рабочей области. Подключения потребуется
        настроить заново.
      </p>
      <div className="export-actions">
        <button disabled={!connected || visibleBusy} onClick={() => void prepare()}>
          {file ? 'Обновить экспорт' : 'Подготовить экспорт'}
        </button>
        {visibleBusy && <button onClick={cancel}>Отменить подготовку</button>}
      </div>
      {visibleBusy && <p role="status">Подготовка конфигурации…</p>}
      {!connected && (
        <p className="form-error">Нет связи со службой. Подключитесь для подготовки файла.</p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {file && (
        <div className="export-review">
          <h3>Подготовлена ревизия {file.document.sourceRevision}</h3>
          <p>{file.document.name}</p>
          {currentRevision > file.document.sourceRevision && (
            <p className="warning">
              Конфигурация уже изменилась. Скачивание сохранит показанную ревизию; для текущей
              версии обновите экспорт.
            </p>
          )}
          <dl className="export-counts">
            <div>
              <dt>Версии ролей</dt>
              <dd>{file.document.roleVersions.length}</dd>
            </div>
            <div>
              <dt>Экземпляры</dt>
              <dd>{file.document.instances.length}</dd>
            </div>
            <div>
              <dt>В архиве</dt>
              <dd>{file.document.instances.filter((agent) => agent.archived).length}</dd>
            </div>
            <div>
              <dt>Холоны</dt>
              <dd>{file.document.architecture.holons.length}</dd>
            </div>
            <div>
              <dt>Подключения</dt>
              <dd>{file.document.connections.length}</dd>
            </div>
            <div>
              <dt>Размер</dt>
              <dd>
                {(file.byteLength / 1024).toLocaleString('ru', { maximumFractionDigits: 1 })} КиБ
              </dd>
            </div>
          </dl>
          <p className="small">
            Тексты ролей, имена, цели и пространства знаний сохраняются дословно. Просмотрите их
            перед передачей файла.
          </p>
          <details>
            <summary>Просмотреть JSON</summary>
            <div
              role="region"
              aria-label="Содержимое экспорта"
              tabIndex={0}
              className="export-json"
            >
              <pre>{exportTextPage(file.text, page)}</pre>
            </div>
            <nav className="export-actions" aria-label="Страницы JSON">
              <button disabled={page === 0} onClick={() => setPage(page - 1)}>
                Предыдущая часть
              </button>
              <span>
                {page + 1} / {Math.ceil(file.text.length / 32768)}
              </span>
              <button
                disabled={(page + 1) * 32768 >= file.text.length}
                onClick={() => setPage(page + 1)}
              >
                Следующая часть
              </button>
            </nav>
          </details>
          <button
            className="primary"
            onClick={() => {
              try {
                save(file.text, `polymas-configuration-r${file.document.sourceRevision}.json`);
                setSaved(true);
                setError('');
              } catch (reason) {
                setError(reason instanceof Error ? reason.message : 'Не удалось сохранить файл');
              }
            }}
          >
            <Download size={16} /> Скачать JSON
          </button>
          {saved && (
            <p className="form-success" role="status">
              Файл передан для сохранения
            </p>
          )}
        </div>
      )}
    </section>
  );
}
