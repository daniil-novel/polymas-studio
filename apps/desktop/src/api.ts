import { invoke, isTauri } from '@tauri-apps/api/core';
export class ApiError extends Error {
  constructor(
    public code: string,
    message: string,
    public status: number,
  ) {
    super(message);
  }
}
export class ApiClient {
  constructor(private token = '') {}
  async request<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
    if (!path.startsWith('/api/')) throw new Error('Некорректный путь команды');
    if (isTauri()) return invoke<T>('api_request', { path, body: body ?? null });
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : 'POST',
      headers: { Authorization: `Bearer ${this.token}`, 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
    const result = await response.json();
    if (!response.ok)
      throw new ApiError(
        result.code ?? 'request_failed',
        result.message ?? `Ошибка подключения (${response.status})`,
        response.status,
      );
    return result as T;
  }
}
export function createClient(): ApiClient {
  const params = new URLSearchParams(location.hash.slice(1));
  const token = params.get('connect');
  if (token) {
    sessionStorage.setItem('polymas-control', token);
    history.replaceState(null, '', location.pathname);
  }
  return new ApiClient(sessionStorage.getItem('polymas-control') ?? '');
}
