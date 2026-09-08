import { useEffect, useState } from 'react';
import type { ApiClient } from '../api';
import type { RuntimeEvent } from '../types';
import {
  compactEvent,
  recordingTimeline,
  type RecordingManifest,
  type RecordingTimeline,
} from './model';

interface Page {
  events: RuntimeEvent[];
  cursor: number;
  through: number;
  complete: boolean;
}
interface Recording {
  runId: string;
  manifest: RecordingManifest;
  timeline: RecordingTimeline;
}
export function useRecording(client: ApiClient, runId: string, revision: number) {
  const [result, setResult] = useState<Recording | null>(null);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState('');
  useEffect(() => {
    const abort = new AbortController();
    setResult(null);
    setError('');
    setProgress(0);
    const path = `/api/runs/${encodeURIComponent(runId)}/recording`;
    void (async () => {
      try {
        const manifest = await client.request<RecordingManifest>(path, undefined, abort.signal);
        if (abort.signal.aborted) return;
        if (
          manifest.schemaVersion !== 1 ||
          manifest.runId !== runId ||
          !Number.isSafeInteger(manifest.head) ||
          manifest.head < 0
        )
          throw new Error('Несовместимая запись запуска');
        const events: RuntimeEvent[] = [];
        let cursor = 0;
        for (;;) {
          const page = await client.request<Page>(
            `${path}/events?after=${cursor}&through=${manifest.head}&limit=500`,
            undefined,
            abort.signal,
          );
          if (abort.signal.aborted) return;
          if (
            page.through !== manifest.head ||
            page.cursor > manifest.head ||
            page.cursor < cursor ||
            (!page.complete && page.cursor <= cursor) ||
            (page.complete && page.cursor !== manifest.head)
          )
            throw new Error('Некорректная граница записи');
          let last = cursor;
          for (const event of page.events) {
            if (
              event.runId !== runId ||
              !Number.isSafeInteger(event.sequence) ||
              event.sequence <= last ||
              event.sequence > page.cursor
            )
              throw new Error('Нарушен порядок событий записи');
            last = event.sequence;
            events.push(compactEvent(event));
          }
          if (events.length > 100_000)
            throw new Error(
              'Запись превышает текущий предел просмотра: 100 000 событий. Данные сохранены в журнале службы.',
            );
          cursor = page.cursor;
          setProgress(events.length);
          if (page.complete) break;
        }
        setResult({ runId, manifest, timeline: recordingTimeline(events) });
      } catch (reason) {
        if (!abort.signal.aborted)
          setError(reason instanceof Error ? reason.message : 'Запись недоступна');
      }
    })();
    return () => abort.abort();
  }, [client, runId, revision]);
  return { result: result?.runId === runId ? result : null, progress, error };
}
export function useRecordedDetail(client: ApiClient, runId: string, sequence: number | null) {
  const [detail, setDetail] = useState<{
    runId: string;
    sequence: number;
    event?: RuntimeEvent;
    error?: string;
  } | null>(null);
  useEffect(() => {
    if (sequence === null) return;
    const abort = new AbortController();
    void client
      .request<Page>(
        `/api/runs/${encodeURIComponent(runId)}/recording/events?after=${sequence - 1}&through=${sequence}&limit=1`,
        undefined,
        abort.signal,
      )
      .then((page) => {
        if (abort.signal.aborted) return;
        const event = page.events.find((e) => e.sequence === sequence && e.runId === runId);
        setDetail({ runId, sequence, event, error: event ? undefined : 'Событие недоступно' });
      })
      .catch((reason) => {
        if (!abort.signal.aborted)
          setDetail({
            runId,
            sequence,
            error: reason instanceof Error ? reason.message : 'Событие недоступно',
          });
      });
    return () => abort.abort();
  }, [client, runId, sequence]);
  return detail?.runId === runId && detail.sequence === sequence ? detail : null;
}
