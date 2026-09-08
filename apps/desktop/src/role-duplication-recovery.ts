export interface DuplicationRequest {
  revision: number;
  command: {
    type: 'duplicate_role';
    key: string;
    roleId: string;
    sourceVersion: number;
    name: string;
  };
}
export interface PendingDuplication {
  version: 1;
  workspaceId: string;
  request: DuplicationRequest;
}
const duplicationPrefix = (id: string) => `polymas:role-duplication:v1:${id}:`;
export const duplicationStorageKey = (id: string, key: string) => `${duplicationPrefix(id)}${key}`;
export function saveDuplication(value: PendingDuplication) {
  window.localStorage.setItem(
    duplicationStorageKey(value.workspaceId, value.request.command.key),
    JSON.stringify(value),
  );
}
export function clearDuplication(value: PendingDuplication) {
  window.localStorage.removeItem(
    duplicationStorageKey(value.workspaceId, value.request.command.key),
  );
}
export function validCopyName(name: string) {
  return !!name.trim() && new TextEncoder().encode(name).length <= 200;
}
export function suggestedCopyName(name: string) {
  const suffix = ' · копия';
  let prefix = '';
  for (const char of name) {
    if (!validCopyName(prefix + char + suffix)) break;
    prefix += char;
  }
  return prefix + suffix;
}
function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
function keys(value: Record<string, unknown>, expected: string[]) {
  return Object.keys(value).length === expected.length && expected.every((key) => key in value);
}
export function readDuplication(workspaceId: string): {
  pending: PendingDuplication | null;
  error: string;
} {
  try {
    // Each operation owns a separate entry. Another window cannot replace its recovery key.
    const entries: string[] = [];
    for (let i = 0; i < window.localStorage.length; i++) {
      const key = window.localStorage.key(i);
      if (key?.startsWith(duplicationPrefix(workspaceId))) entries.push(key);
    }
    let saved: { key: string; text: string } | undefined;
    for (const key of entries.sort()) {
      const text = window.localStorage.getItem(key);
      if (text !== null) {
        saved = { key, text };
        break;
      }
    }
    if (!saved) return { pending: null, error: '' };
    const text = saved.text;
    if (text.length > 4096) throw new Error('Invalid pending request size');
    const value: unknown = JSON.parse(text);
    if (
      !record(value) ||
      !keys(value, ['version', 'workspaceId', 'request']) ||
      value.version !== 1 ||
      value.workspaceId !== workspaceId ||
      !record(value.request)
    )
      throw new Error('Invalid pending workspace');
    const request = value.request;
    if (
      !keys(request, ['revision', 'command']) ||
      !Number.isSafeInteger(request.revision) ||
      (request.revision as number) < 1 ||
      !record(request.command)
    )
      throw new Error('Invalid request');
    const command = request.command;
    if (
      !keys(command, ['type', 'key', 'roleId', 'sourceVersion', 'name']) ||
      command.type !== 'duplicate_role' ||
      typeof command.key !== 'string' ||
      !/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(command.key) ||
      typeof command.roleId !== 'string' ||
      !command.roleId ||
      command.roleId.length > 200 ||
      !Number.isInteger(command.sourceVersion) ||
      (command.sourceVersion as number) < 1 ||
      (command.sourceVersion as number) > 4294967295 ||
      typeof command.name !== 'string' ||
      !validCopyName(command.name)
    )
      throw new Error('Invalid duplicate parameters');
    if (saved.key !== duplicationStorageKey(workspaceId, command.key))
      throw new Error('Mismatched operation key');
    return { pending: value as unknown as PendingDuplication, error: '' };
  } catch {
    return {
      pending: null,
      error:
        'Не удалось прочитать незавершённый запрос. Новое дублирование недоступно; восстановите локальное хранилище и перечитайте запись.',
    };
  }
}
