import { useEffect, useRef, useState } from 'react';
import type { ApiClient } from '../api';
import type { Page } from './model';
export function useRemote<T>(client: ApiClient, path: string, refresh: number) {
  const [state, setState] = useState<{ path: string; refresh: number; data?: T; error?: string }>();
  useEffect(() => {
    const abort = new AbortController();
    client
      .request<T>(path, undefined, abort.signal)
      .then((data) => {
        if (!abort.signal.aborted) setState({ path, refresh, data });
      })
      .catch((error: unknown) => {
        if (!abort.signal.aborted)
          setState({
            path,
            refresh,
            error: error instanceof Error ? error.message : 'Не удалось загрузить знания',
          });
      });
    return () => abort.abort();
  }, [client, path, refresh]);
  const current = state?.path === path && state.refresh === refresh ? state : null;
  return { data: current?.data, error: current?.error, loading: !current };
}
export function usePage<T>(client: ApiClient, base: string, refresh: number) {
  const [navigation, setNavigation] = useState<{ base: string; cursors: string[] }>({
    base,
    cursors: [],
  });
  const cursors = navigation.base === base ? navigation.cursors : [];
  const cursor = cursors.at(-1);
  const remote = useRemote<Page<T>>(
    client,
    `${base}${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ''}`,
    refresh,
  );
  return {
    ...remote,
    number: cursors.length + 1,
    previous: () => setNavigation({ base, cursors: cursors.slice(0, -1) }),
    next: () => {
      if (remote.data?.nextCursor)
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
export function Pager({ page, label }: { page: ReturnType<typeof usePage>; label: string }) {
  return (
    <nav className="knowledge-pager" aria-label={label}>
      <button disabled={page.loading || page.number === 1} onClick={page.previous}>
        Назад
      </button>
      <span>Стр. {page.number}</span>
      <button disabled={page.loading || !page.data?.nextCursor} onClick={page.next}>
        Далее
      </button>
    </nav>
  );
}
