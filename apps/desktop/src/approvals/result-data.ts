import type { ApiClient } from '../api';

export interface ResultMetadata {
  byteLength: number;
  sha256: string;
}
export interface ResultPage {
  metadata: ResultMetadata;
  offset: number;
  nextOffset: number | null;
  chunk: string;
}
function validMetadata(value: ResultMetadata) {
  return (
    Number.isSafeInteger(value.byteLength) &&
    value.byteLength > 0 &&
    value.byteLength <= 4 * 1024 * 1024 + 8192 &&
    /^[0-9a-f]{64}$/.test(value.sha256)
  );
}
export function checkedResultPage(
  value: unknown,
  metadata: ResultMetadata,
  offset: number,
): ResultPage {
  if (!validMetadata(metadata) || !value || typeof value !== 'object')
    throw new Error('Некорректные данные результата');
  const page = value as Partial<ResultPage>;
  if (
    !page.metadata ||
    page.metadata.byteLength !== metadata.byteLength ||
    page.metadata.sha256 !== metadata.sha256 ||
    page.offset !== offset ||
    typeof page.chunk !== 'string' ||
    !Number.isSafeInteger(offset) ||
    offset < 0
  ) {
    throw new Error('Результат изменился или получен ответ другой операции');
  }
  const bytes = new TextEncoder().encode(page.chunk).byteLength;
  const end = offset + bytes;
  if (
    bytes > 32768 ||
    end > metadata.byteLength ||
    (bytes === 0 && end !== metadata.byteLength) ||
    page.nextOffset !== (end === metadata.byteLength ? null : end)
  )
    throw new Error('Нарушена последовательность частей результата');
  return page as ResultPage;
}
export async function readCompleteResult(
  client: ApiClient,
  id: string,
  metadata: ResultMetadata,
  signal: AbortSignal,
): Promise<string> {
  if (!validMetadata(metadata)) throw new Error('Некорректные данные результата');
  const chunks: string[] = [];
  let offset = 0;
  for (let count = 0; offset < metadata.byteLength; count++) {
    signal.throwIfAborted();
    if (count >= 256) throw new Error('Служба вернула слишком много частей результата');
    const raw = await client.request(
      `/api/approvals/${encodeURIComponent(id)}/result?offset=${offset}`,
      undefined,
      signal,
    );
    signal.throwIfAborted();
    const page = checkedResultPage(raw, metadata, offset);
    chunks.push(page.chunk);
    offset += new TextEncoder().encode(page.chunk).byteLength;
  }
  const text = chunks.join('');
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  signal.throwIfAborted();
  const hash = [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  if (hash !== metadata.sha256)
    throw new Error('Не совпала контрольная сумма результата; файл не сохранён');
  JSON.parse(text);
  return text;
}
