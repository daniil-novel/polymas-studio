import { afterEach, expect, test, vi } from 'vitest';
import { createHash, webcrypto } from 'node:crypto';
import { ApiClient } from '../../apps/desktop/src/api';
import {
  checkedResultPage,
  readCompleteResult,
} from '../../apps/desktop/src/approvals/result-data';

afterEach(() => vi.unstubAllGlobals());
const chunks = ['{"ответ":"', 'Тест🧠', '"}'];
const text = chunks.join('');
const metadata = {
  byteLength: Buffer.byteLength(text),
  sha256: createHash('sha256').update(text).digest('hex'),
};
function setup() {
  vi.stubGlobal('crypto', webcrypto);
  const client = new ApiClient();
  let offset = 0;
  const pages = chunks.map((chunk, i) => {
    const start = offset;
    offset += Buffer.byteLength(chunk);
    return { metadata, offset: start, nextOffset: i === chunks.length - 1 ? null : offset, chunk };
  });
  const request = vi.spyOn(client, 'request').mockImplementation(async (path) => {
    const offset = Number(new URL(path, 'http://fixture.invalid').searchParams.get('offset'));
    return pages.find((page) => page.offset === offset);
  });
  return { client, request, pages };
}
test('export reconstructs exact UTF-8 JSON and checks the entire saved response hash', async () => {
  const { client, request, pages } = setup();
  expect(await readCompleteResult(client, 'one', metadata, new AbortController().signal)).toBe(
    text,
  );
  expect(request.mock.calls).toHaveLength(3);
  expect(request.mock.calls.every(([, body]) => body === undefined)).toBe(true);
  const wrongHash = { ...metadata, sha256: 'a'.repeat(64) };
  request.mockImplementation(async (path) => {
    const offset = Number(new URL(path, 'http://fixture.invalid').searchParams.get('offset'));
    return { ...pages.find((page) => page.offset === offset), metadata: wrongHash };
  });
  await expect(
    readCompleteResult(client, 'one', wrongHash, new AbortController().signal),
  ).rejects.toThrow(/контрольная сумма/i);
});
test('malformed pages, changed metadata and cancellation cannot produce an export', async () => {
  const { client, request, pages } = setup();
  const page = pages[0];
  for (const wrong of [
    { ...page, offset: 1 },
    { ...page, nextOffset: 0 },
    { ...page, nextOffset: null },
    { ...page, chunk: 'x'.repeat(32769) },
    { ...page, metadata: { ...metadata, byteLength: 8_000_000 } },
  ])
    expect(() => checkedResultPage(wrong, metadata, 0)).toThrow();
  const abort = new AbortController();
  request.mockImplementation(async () => {
    abort.abort();
    return page;
  });
  await expect(readCompleteResult(client, 'one', metadata, abort.signal)).rejects.toThrow();
  expect(request.mock.calls).toHaveLength(1);
});
