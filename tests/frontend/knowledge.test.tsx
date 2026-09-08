import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { KnowledgeWorkspace } from '../../apps/desktop/src/knowledge/KnowledgeWorkspace';
import { ApiClient } from '../../apps/desktop/src/api';
import type { Snapshot } from '../../apps/desktop/src/types';
afterEach(cleanup);
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) {
    this.open = true;
  });
  HTMLDialogElement.prototype.close = vi.fn(function (this: HTMLDialogElement) {
    this.open = false;
  });
});
const source = {
  revision: 1,
  value: {
    id: 'source-1',
    name: 'Правила архитектуры',
    uri: 'https://example.org/spec',
    version: 'fixture-v7',
    namespace: 'shared',
    retentionDays: 30,
    connectionId: null,
    createdAt: '2026-09-08T10:00:00Z',
  },
};
const claim = (id: string) => ({
  revision: 3,
  value: {
    id,
    assertion: `Утверждение ${id}`,
    sourceId: source.value.id,
    sourceVersion: 'fixture-v7',
    sourceUri: source.value.uri,
    namespace: 'shared',
    author: 'agent-fixture',
    createdAt: 100,
    expiresAt: 9000000000,
    status: 'unverified',
    verifiedBy: null,
    verifiedAt: null,
  },
});
const snapshot = {
  connections: [],
  config: { value: { architecture: { memoryEnabled: false }, registry: { agents: [] } } },
} as unknown as Snapshot;
function setup() {
  const client = new ApiClient();
  const request = vi.spyOn(client, 'request').mockImplementation(async (path) => {
    if (path.startsWith('/api/knowledge/claims/'))
      return { claim: claim(path.split('/').at(-1)!), source, expired: false, observedAt: 101 };
    return {
      items: path.startsWith('/api/knowledge/sources')
        ? [source]
        : path.startsWith('/api/knowledge/claims')
          ? [claim('one'), claim('two')]
          : [],
      nextCursor: null,
      observedAt: 101,
    };
  });
  return { client, request };
}
test('knowledge review remains explicit and carries the selected claim revision', async () => {
  const { client, request } = setup();
  render(<KnowledgeWorkspace client={client} snapshot={snapshot} />);
  await userEvent.click(await screen.findByRole('button', { name: 'Открыть утверждение one' }));
  expect(await screen.findByText('agent-fixture')).toBeTruthy();
  expect(screen.getAllByText('Не проверено').length).toBeGreaterThan(0);
  expect(request.mock.calls.filter(([, body]) => body)).toHaveLength(0);
  await userEvent.click(screen.getByRole('button', { name: 'Подтвердить' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      '/api/knowledge/command',
      expect.objectContaining({
        key: expect.any(String),
        command: { action: 'verify', claimId: 'one', revision: 3, status: 'verified' },
      }),
    ),
  );
});
test('a source save with an uncertain outcome retries the exact same command key', async () => {
  const { client, request } = setup();
  let fail = true;
  const original = request.getMockImplementation()!;
  request.mockImplementation(async (path, body, signal) => {
    if (body) {
      if (fail) {
        fail = false;
        throw new Error('Соединение прервано');
      }
      return { result: source.value };
    }
    return original(path, body, signal);
  });
  render(<KnowledgeWorkspace client={client} snapshot={snapshot} />);
  await userEvent.click(screen.getByRole('button', { name: 'Добавить источник' }));
  await userEvent.type(screen.getByLabelText('Название источника'), 'Справочник');
  await userEvent.type(screen.getByLabelText('Адрес источника'), 'https://example.org/guide');
  await userEvent.type(screen.getByLabelText('Версия или hash'), 'v1');
  await userEvent.click(screen.getByRole('button', { name: 'Сохранить источник' }));
  expect((await screen.findByRole('alert')).textContent).toContain('Соединение прервано');
  await userEvent.click(screen.getByRole('button', { name: 'Сохранить источник' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  const writes = request.mock.calls.filter(([, body]) => body);
  expect(writes).toHaveLength(2);
  expect(writes[0][1]).toEqual(writes[1][1]);
});
test('a late provenance response cannot replace the newly selected claim', async () => {
  const { client, request } = setup();
  const original = request.getMockImplementation()!;
  let release: (value: unknown) => void = () => {};
  request.mockImplementation((path, body, signal) =>
    path.endsWith('/one')
      ? new Promise((resolve) => {
          release = resolve;
        })
      : original(path, body, signal),
  );
  render(<KnowledgeWorkspace client={client} snapshot={snapshot} />);
  await userEvent.click(await screen.findByRole('button', { name: 'Открыть утверждение one' }));
  await userEvent.click(screen.getByRole('button', { name: 'Открыть утверждение two' }));
  expect((await screen.findByTestId('knowledge-detail-id')).textContent).toBe('two');
  release({ claim: claim('one'), source, expired: false, observedAt: 101 });
  await waitFor(() => expect(screen.getByTestId('knowledge-detail-id').textContent).toBe('two'));
});

test('claim pagination replaces the page and changing scope discards its old cursor', async () => {
  const { client, request } = setup();
  const original = request.getMockImplementation()!;
  request.mockImplementation(async (path, body, signal) => {
    if (path.startsWith('/api/knowledge/claims?')) {
      const next = new URL(path, 'http://localhost').searchParams.has('cursor');
      return {
        items: [claim(next ? 'two' : 'one')],
        nextCursor: next ? null : 'page-one',
        observedAt: 101,
      };
    }
    return original(path, body, signal);
  });
  render(<KnowledgeWorkspace client={client} snapshot={snapshot} />);
  await screen.findByRole('button', { name: 'Открыть утверждение one' });
  await userEvent.click(
    within(screen.getByRole('navigation', { name: 'Страницы утверждений' })).getByRole('button', {
      name: 'Далее',
    }),
  );
  await screen.findByRole('button', { name: 'Открыть утверждение two' });
  expect(screen.queryByRole('button', { name: 'Открыть утверждение one' })).toBeNull();
  await userEvent.type(screen.getByRole('textbox', { name: 'Область знаний' }), 'new');
  await screen.findByRole('button', { name: 'Открыть утверждение one' });
  const last = request.mock.calls
    .filter(([path]) => path.startsWith('/api/knowledge/claims?'))
    .at(-1)![0];
  expect(new URL(last, 'http://localhost').searchParams.get('namespace')).toBe('new');
  expect(new URL(last, 'http://localhost').searchParams.has('cursor')).toBe(false);
});
