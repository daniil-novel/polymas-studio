import { beforeEach, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { ApiClient, ApiError, createClient } from '../../apps/desktop/src/api';

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn() }));
beforeEach(() => {
  vi.resetAllMocks();
  sessionStorage.clear();
  history.replaceState(null, '', '/');
});

it('preserves native conflict errors so forms can refresh rather than silently retry', async () => {
  vi.mocked(invoke).mockRejectedValue({
    code: 'conflict',
    message: 'Ревизия изменилась',
    status: 409,
  });
  const error = await new ApiClient().request('/api/configure', { revision: 1 }).catch((e) => e);
  expect(error).toBeInstanceOf(ApiError);
  expect(error).toMatchObject({ code: 'conflict', message: 'Ревизия изменилась', status: 409 });
});

it('does not dispatch a command whose caller already aborted', async () => {
  const controller = new AbortController();
  controller.abort();
  await expect(
    new ApiClient().request('/api/control', { action: 'pause' }, controller.signal),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(invoke).not.toHaveBeenCalled();
});

it('uses native IPC without importing a browser credential into desktop storage', async () => {
  history.replaceState(null, '', '/#connect=fixture-browser-credential');
  vi.mocked(invoke).mockResolvedValue({ ready: true });
  await expect(createClient().request('/api/state')).resolves.toEqual({ ready: true });
  expect(invoke).toHaveBeenCalledWith('api_request', { path: '/api/state', body: null });
  expect(sessionStorage.getItem('polymas-control')).toBeNull();
});
