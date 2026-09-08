import { useEffect, useMemo, useRef, useState } from 'react';
import type { ApiClient } from '../api';
import type { Page } from './model';
export function useRemote<T>(
  client: ApiClient,
  path: string,
  refresh: number,
  backgroundRefresh = 0,
  enabled = true,
) {
  const activation = useMemo(() => ({ enabled }), [enabled]);
  const [state, setState] = useState<{
    client: ApiClient;
    path: string;
    refresh: number;
    backgroundRefresh: number;
    activation: object;
    data?: T;
    error?: string;
  }>();
  useEffect(() => {
    if (!enabled) return;
    const abort = new AbortController();
    client
      .request<T>(path, undefined, abort.signal)
      .then((data) => {
        if (!abort.signal.aborted)
          setState({ client, path, refresh, backgroundRefresh, activation, data });
      })
      .catch((error: unknown) => {
        if (!abort.signal.aborted)
          setState((previous) => ({
            client,
            path,
            refresh,
            backgroundRefresh,
            activation,
            data:
              previous?.client === client && previous.path === path && previous.refresh === refresh
                ? previous.data
                : undefined,
            error: error instanceof Error ? error.message : 'Не удалось загрузить знания',
          }));
      });
    return () => abort.abort();
  }, [client, path, refresh, backgroundRefresh, enabled, activation]);
  // Explicit refresh/path/client changes invalidate review; polling preserves its DOM only.
  const current =
    state?.client === client && state.path === path && state.refresh === refresh ? state : null;
  return {
    data: current?.data,
    error: current?.error,
    loading:
      !current ||
      current.backgroundRefresh !== backgroundRefresh ||
      current.activation !== activation,
  };
}
export function usePage<T>(client: ApiClient, base: string, refresh: number) {
  return useCursorPage<Page<T>['items'][number], Page<T>>(client, base, refresh);
}
export function useCursorPage<
  T,
  P extends { items: T[]; nextCursor: string | null } = { items: T[]; nextCursor: string | null },
>(client: ApiClient, base: string, refresh: number, backgroundRefresh = 0, enabled = true) {
  const [navigation, setNavigation] = useState<{ base: string; cursors: string[] }>({
    base,
    cursors: [],
  });
  useEffect(() => {
    if (navigation.base !== base) setNavigation({ base, cursors: [] });
  }, [base, navigation.base]);
  const cursors = navigation.base === base ? navigation.cursors : [];
  const cursor = cursors.at(-1);
  const remote = useRemote<P>(
    client,
    `${base}${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ''}`,
    refresh,
    backgroundRefresh,
    enabled,
  );
  return {
    ...remote,
    number: cursors.length + 1,
    previous: () => setNavigation({ base, cursors: cursors.slice(0, -1) }),
    next: () => {
      if (!remote.loading && !remote.error && remote.data?.nextCursor)
        setNavigation({ base, cursors: [...cursors, remote.data.nextCursor] });
    },
  };
}
export function useKnowledgeCommand(client: ApiClient, onSaved: () => void) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const pending = useRef<{ fingerprint: string; body: { key: string; command: unknown } } | null>(
    null,
  );
  const inFlight = useRef(false);
  async function save(command: unknown) {
    if (inFlight.current) return false;
    inFlight.current = true;
    setBusy(true);
    setError('');
    const fingerprint = JSON.stringify(command);
    if (pending.current?.fingerprint !== fingerprint)
      pending.current = { fingerprint, body: { key: crypto.randomUUID(), command } };
    try {
      await client.request('/api/knowledge/command', pending.current.body);
      pending.current = null;
      onSaved();
      return true;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Изменение не сохранено');
      return false;
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  return { busy, error, save };
}
export function Pager({ page, label }: { page: ReturnType<typeof useCursorPage>; label: string }) {
  return (
    <nav className="knowledge-pager" aria-label={label}>
      <button disabled={page.loading || !!page.error || page.number === 1} onClick={page.previous}>
        Назад
      </button>
      <span>Стр. {page.number}</span>
      <button disabled={page.loading || !!page.error || !page.data?.nextCursor} onClick={page.next}>
        Далее
      </button>
    </nav>
  );
}
