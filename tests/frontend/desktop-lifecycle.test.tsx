import { afterEach, expect, test, vi, beforeEach } from 'vitest';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { DesktopLifecycle } from '../../apps/desktop/src/DesktopLifecycle';
import { App } from '../../apps/desktop/src/App';

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
beforeEach(() => {
  vi.resetAllMocks();
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) {
    this.open = true;
  });
  HTMLDialogElement.prototype.close = vi.fn(function (this: HTMLDialogElement) {
    this.open = false;
  });
  vi.mocked(listen).mockResolvedValue(vi.fn());
  vi.mocked(invoke).mockResolvedValue({
    phase: 'ready',
    closeBehavior: null,
    closeRequested: false,
    warnings: [],
  });
});
afterEach(cleanup);

test('a native startup failure offers reconnect without a manual control-token field', async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === 'desktop_status')
      return {
        phase: 'failed',
        error: 'Служба не запустилась',
        closeRequested: false,
        warnings: [],
      };
    throw { code: 'disconnected', message: 'Нет связи', status: 503 };
  });
  render(<App />);
  expect(await screen.findByRole('button', { name: 'Повторить подключение' })).toBeTruthy();
  await screen.findByText('Нет связи');
  expect(screen.queryByLabelText('Токен локального подключения')).toBeNull();
});

test('a close request waits for a deliberate choice and sends the remember flag', async () => {
  render(<DesktopLifecycle />);
  await waitFor(() => expect(listen).toHaveBeenCalled());
  expect(invoke).not.toHaveBeenCalledWith('desktop_close', expect.anything());
  const handler = vi.mocked(listen).mock.calls.find((c) => c[0] === 'desktop-close-requested')![1];
  await act(async () => handler({ event: 'desktop-close-requested', id: 1, payload: null }));
  expect(screen.getByRole('dialog', { name: 'Закрыть PolyMAS Studio' })).toBeTruthy();
  await userEvent.click(screen.getByRole('button', { name: 'Оставить в фоне' }));
  expect(invoke).toHaveBeenCalledWith('desktop_close', { behavior: 'background', remember: true });
});

test('failed shutdown keeps the close dialog and displays the service error', async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === 'desktop_close')
      throw { code: 'timeout', message: 'Служба ещё завершает задачи', status: 503 };
    return { phase: 'ready', closeBehavior: null, closeRequested: true, warnings: [] };
  });
  render(<DesktopLifecycle />);
  await userEvent.click(await screen.findByRole('button', { name: 'Остановить и выйти' }));
  expect(await screen.findByRole('alert')).toHaveProperty(
    'textContent',
    'Служба ещё завершает задачи',
  );
  expect(screen.getByRole('dialog', { name: 'Закрыть PolyMAS Studio' })).toBeTruthy();
});
