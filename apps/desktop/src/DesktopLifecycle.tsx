import { useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Modal } from './Modal';

interface DesktopStatus {
  phase: 'starting' | 'ready' | 'failed' | 'stopping';
  error?: string | null;
  closeBehavior: 'background' | 'stop' | null;
  closeRequested: boolean;
  warnings: string[];
}
function message(error: unknown): string {
  return typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
    ? error.message
    : 'Не удалось выполнить действие настольной службы.';
}
export function DesktopLifecycle() {
  const [status, setStatus] = useState<DesktopStatus | null>(null);
  const [open, setOpen] = useState(false);
  const [remember, setRemember] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const stops: (() => void)[] = [];
    const update = (value: DesktopStatus) => {
      if (!disposed) {
        setStatus(value);
        setOpen(value.closeRequested);
      }
    };
    void (async () => {
      for (const subscription of [
        listen('desktop-close-requested', () => {
          if (!disposed) setOpen(true);
        }),
        listen<DesktopStatus>('desktop-status-changed', (event) => update(event.payload)),
      ]) {
        const stop = await subscription;
        if (disposed) stop();
        else stops.push(stop);
      }
      update(await invoke<DesktopStatus>('desktop_status'));
    })().catch((error) => {
      if (!disposed) setError(message(error));
    });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);
  if (!isTauri()) return null;
  const close = async (behavior?: 'background' | 'stop') => {
    if (busy) return;
    setBusy(true);
    setError('');
    try {
      if (behavior) await invoke('desktop_close', { behavior, remember });
      else await invoke('desktop_cancel_close');
      setOpen(false);
    } catch (error) {
      setError(message(error));
    } finally {
      setBusy(false);
    }
  };
  const retry = async () => {
    setBusy(true);
    setError('');
    try {
      await invoke('desktop_reconnect');
    } catch (error) {
      setError(message(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      {!open &&
        (status?.phase === 'starting' ||
          status?.phase === 'stopping' ||
          status?.phase === 'failed' ||
          error) && (
          <aside className="desktop-notice" aria-live="polite">
            <strong>
              {status?.phase === 'starting'
                ? 'Подключаем рабочую область…'
                : status?.phase === 'stopping'
                  ? 'Останавливаем задачи…'
                  : 'Служба требует внимания'}
            </strong>
            {(error || status?.error) && <p role="alert">{error || status?.error}</p>}
            {status?.phase === 'failed' && (
              <button disabled={busy} onClick={() => void retry()}>
                Повторить подключение
              </button>
            )}
          </aside>
        )}
      {!open &&
        status?.warnings.map((warning) => (
          <aside className="desktop-notice warning" key={warning}>
            {warning}
          </aside>
        ))}
      {open && (
        <Modal label="Закрыть PolyMAS Studio" onClose={() => void close()}>
          <section className="desktop-close">
            <span className="eyebrow">POLYMAS STUDIO</span>
            <h2>Что делать с задачами?</h2>
            <p className="muted">
              В фоне агенты продолжат работу. Вернуться в редактор можно через значок в области
              уведомлений.
            </p>
            <p className="muted">
              Остановка отменит незавершённые задачи и закроет службу. Сохранённые результаты
              останутся в рабочей области.
            </p>
            <label className="check-label">
              <input
                type="checkbox"
                checked={remember}
                disabled={busy}
                onChange={(e) => setRemember(e.target.checked)}
              />
              Запомнить выбор
            </label>
            {error && (
              <p role="alert" className="error-text">
                {error}
              </p>
            )}
            <footer>
              <button disabled={busy} onClick={() => void close()}>
                Отмена
              </button>
              <button
                disabled={busy || status?.phase === 'starting'}
                onClick={() => void close('stop')}
              >
                Остановить и выйти
              </button>
              <button className="primary" disabled={busy} onClick={() => void close('background')}>
                Оставить в фоне
              </button>
            </footer>
            {busy && (
              <p className="muted" role="status">
                Дожидаемся завершения действия…
              </p>
            )}
          </section>
        </Modal>
      )}
    </>
  );
}
