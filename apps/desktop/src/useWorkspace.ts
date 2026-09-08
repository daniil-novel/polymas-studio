import { useCallback, useEffect, useRef, useState } from 'react';
import type { ApiClient } from './api';
import type { RuntimeEvent, Snapshot } from './types';
export function useWorkspace(client: ApiClient) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [events, setEvents] = useState<RuntimeEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const cursor = useRef(0);
  const refresh = useCallback(
    async (signal?: AbortSignal) => {
      const value = await client.request<Snapshot>('/api/state', undefined, signal);
      if (value.config.value.schemaVersion !== 1)
        throw new Error('Обновите приложение: несовместимая версия workspace');
      setSnapshot(value);
      setConnected(true);
      setError(null);
      return value;
    },
    [client],
  );
  useEffect(() => {
    const abort = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    let tick = 0;
    const poll = async () => {
      try {
        if (tick === 0 || tick % 8 === 0) await refresh(abort.signal);
        const page = await client.request<{ events: RuntimeEvent[]; cursor: number; head: number }>(
          `/api/events?after=${cursor.current}&limit=500`,
          undefined,
          abort.signal,
        );
        if (abort.signal.aborted) return;
        if (page.events.length) {
          cursor.current = page.cursor;
          setEvents((old) =>
            [
              ...old,
              ...page.events.filter((e) => !old.some((prior) => prior.sequence === e.sequence)),
            ].slice(-3000),
          );
        }
        setConnected(true);
        tick++;
      } catch (reason) {
        if (abort.signal.aborted) return;
        setConnected(false);
        setError(reason instanceof Error ? reason.message : 'Служба недоступна');
      }
      if (!abort.signal.aborted) timer = setTimeout(poll, 500);
    };
    void poll();
    return () => {
      abort.abort();
      clearTimeout(timer);
    };
  }, [client, refresh]);
  const command = useCallback(
    async <T>(path: string, body: unknown) => {
      try {
        const result = await client.request<T>(path, body);
        await refresh();
        return result;
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : 'Команда не выполнена');
        throw reason;
      }
    },
    [client, refresh],
  );
  return { snapshot, events, error, connected, refresh, command, setError };
}
