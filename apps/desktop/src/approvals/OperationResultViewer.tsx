import { useEffect, useRef, useState } from 'react';
import { Download, FileText } from 'lucide-react';
import type { ApiClient } from '../api';
import { useRemote } from '../knowledge/data';
import {
  checkedResultPage,
  readCompleteResult,
  type ResultMetadata,
  type ResultPage,
} from './result-data';

interface Props {
  client: ApiClient;
  id: string;
  metadata: ResultMetadata;
  connected: boolean;
}
export function OperationResultViewer(props: Props) {
  return <ResultViewer key={`${props.id}:${props.metadata.sha256}`} {...props} />;
}
function ResultViewer({ client, id, metadata, connected }: Props) {
  const [open, setOpen] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState('');
  const transfer = useRef<AbortController | null>(null);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      transfer.current?.abort();
    };
  }, [client]);
  useEffect(() => {
    if (!connected) transfer.current?.abort();
  }, [connected]);
  async function download() {
    if (transfer.current || !connected) return;
    const abort = new AbortController();
    transfer.current = abort;
    setDownloading(true);
    setError('');
    try {
      const text = await readCompleteResult(client, id, metadata, abort.signal);
      abort.signal.throwIfAborted();
      const url = URL.createObjectURL(new Blob([text], { type: 'application/json;charset=utf-8' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = `operation-${id.replace(/[^a-zA-Z0-9_-]/g, '_').slice(0, 100)}.json`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (reason) {
      if (!abort.signal.aborted && mounted.current)
        setError(reason instanceof Error ? reason.message : 'Не удалось получить файл');
    } finally {
      if (transfer.current === abort) transfer.current = null;
      if (mounted.current) setDownloading(false);
    }
  }
  return (
    <section className="operation-result" aria-label="Сохранённый ответ инструмента">
      <h3>
        <FileText size={16} /> Сохранённый ответ
      </h3>
      <p className="muted">
        Ответ инструмента после вызова ·{' '}
        {(metadata.byteLength / 1024).toLocaleString('ru', { maximumFractionDigits: 1 })} КиБ
      </p>
      <div className="operation-result-actions">
        <button aria-expanded={open} onClick={() => setOpen((value) => !value)}>
          {open ? 'Скрыть ответ' : 'Показать ответ'}
        </button>
        <button disabled={!connected || downloading} onClick={() => void download()}>
          <Download size={15} /> {downloading ? 'Подготовка файла…' : 'Скачать ответ JSON'}
        </button>
        {downloading && (
          <button onClick={() => transfer.current?.abort()}>Отменить загрузку</button>
        )}
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {open && <ResultPages client={client} id={id} metadata={metadata} connected={connected} />}
    </section>
  );
}
function ResultPages({ client, id, metadata, connected }: Props) {
  const [offsets, setOffsets] = useState([0]);
  const [refresh, setRefresh] = useState(0);
  const offset = offsets.at(-1)!;
  const remote = useRemote<unknown>(
    client,
    `/api/approvals/${encodeURIComponent(id)}/result?offset=${offset}`,
    refresh,
  );
  let page: ResultPage | undefined;
  let error = remote.error;
  if (remote.data !== undefined) {
    try {
      page = checkedResultPage(remote.data, metadata, offset);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : 'Невозможно прочитать ответ';
    }
  }
  let text = page?.chunk;
  if (page?.offset === 0 && page.nextOffset === null) {
    try {
      text = JSON.stringify(JSON.parse(page.chunk), null, 2);
    } catch {
      /* Show exact source text. */
    }
  }
  return (
    <div className="operation-result-pages">
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {remote.loading && (
        <p role="status" className="muted">
          Загрузка ответа…
        </p>
      )}
      {page && <pre className="operation-result-text">{text}</pre>}
      <nav aria-label="Части ответа инструмента">
        <button
          aria-label="Предыдущая часть ответа"
          disabled={!connected || remote.loading || offsets.length === 1}
          onClick={() => setOffsets((list) => list.slice(0, -1))}
        >
          Назад
        </button>
        <span>Часть {offsets.length}</span>
        <button
          aria-label="Следующая часть ответа"
          disabled={!connected || remote.loading || !page?.nextOffset}
          onClick={() => setOffsets((list) => [...list, page!.nextOffset!])}
        >
          Далее
        </button>
        <button
          disabled={!connected || remote.loading}
          onClick={() => setRefresh((value) => value + 1)}
        >
          Обновить ответ
        </button>
      </nav>
    </div>
  );
}
