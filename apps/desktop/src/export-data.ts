import validate from './gen/configuration-export-validator.mjs';
import type { Agent, Architecture, Binding, Role } from './types';

interface ExportDocument {
  format: 'polymas-configuration';
  schemaVersion: '0.1.0';
  migrationVersion: 1;
  sourceRevision: number;
  name: string;
  runtime: { adapter: 'codex-exec'; setupRequired: true };
  roleVersions: Omit<Role, 'createdAt'>[];
  instances: (Pick<Agent, 'id' | 'name' | 'roleId' | 'roleVersion'> & { archived: boolean })[];
  architecture: Architecture;
  connections: {
    id: string;
    name: string;
    recipe: string;
    transport: string;
    readOnly: boolean;
    timeoutSeconds: number;
    toolAllowlist: string[];
    desiredEnabled: boolean;
    setupRequired: true;
    setupFields: string[];
    rootCount: number;
  }[];
  bindings: Binding[];
}
export interface PreparedExport {
  document: ExportDocument;
  text: string;
  byteLength: number;
}
export function prepareConfigurationExport(value: unknown): PreparedExport {
  if (!validate(value)) throw new Error('Служба вернула несовместимый формат экспорта');
  function exactNumbers(item: unknown): boolean {
    if (typeof item === 'number')
      return Number.isFinite(item) && (!Number.isInteger(item) || Number.isSafeInteger(item));
    if (item && typeof item === 'object') return Object.values(item).every(exactNumbers);
    return true;
  }
  if (!exactNumbers(value))
    throw new Error('Числа превышают точность этого интерфейса; файл не создан');
  const text = JSON.stringify(value, null, 2);
  const byteLength = new TextEncoder().encode(text).byteLength;
  if (byteLength > 16 * 1024 * 1024) throw new Error('Экспорт превышает 16 МиБ; файл не создан');
  // Detach the review from mutable responses: the saved text and summary are one version.
  return { document: JSON.parse(text) as ExportDocument, text, byteLength };
}
export function saveConfigurationExport(text: string, filename: string) {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/json;charset=utf-8' }));
  const anchor = document.createElement('a');
  try {
    anchor.href = url;
    anchor.download = filename;
    document.body.append(anchor);
    anchor.click();
  } finally {
    anchor.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
}

export function exportTextPage(text: string, page: number): string {
  let start = page * 32768;
  let end = (page + 1) * 32768;
  const low = (index: number) =>
    text.charCodeAt(index) >= 0xdc00 && text.charCodeAt(index) <= 0xdfff;
  if (start > 0 && low(start)) start--;
  if (end < text.length && low(end)) end--;
  return text.slice(start, end);
}
