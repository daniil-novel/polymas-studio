import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApprovalWorkspace } from '../../apps/desktop/src/approvals/ApprovalWorkspace';
import { ApiClient, ApiError } from '../../apps/desktop/src/api';
import type { Snapshot } from '../../apps/desktop/src/types';
afterEach(cleanup);
test('manual outcome reconciliation preserves the note across refresh and submits no execution command', async () => {
  const { client, request } = setup();
  const original = request.getMockImplementation()!;
  let saved = false;
  const note = 'Проверен документ и его контрольная сумма в источнике';
  request.mockImplementation(async (path, body, signal) => {
    if (body) {
      saved = true;
      return {};
    }
    if (path === '/api/approvals/one') {
      const current = detail('one');
      return {
        ...current,
        approval: {
          ...current.approval,
          value: { ...current.approval.value, state: saved ? 'succeeded' : 'unknown' },
        },
        canApprove: false,
        canDeny: false,
        reconciliation: saved
          ? {
              outcome: 'succeeded',
              note,
              author: 'local:owner',
              observedAt: '2026-09-08T09:00:00Z',
            }
          : null,
      };
    }
    return original(path, body, signal);
  });
  render(<ApprovalWorkspace client={client} snapshot={snapshot} connected />);
  const outcome = await screen.findByRole('combobox', { name: 'Проверенный исход' });
  expect(screen.getByRole('button', { name: 'Сохранить результат сверки' })).toHaveProperty(
    'disabled',
    true,
  );
  await userEvent.selectOptions(outcome, 'succeeded');
  await userEvent.type(screen.getByRole('textbox', { name: 'Что вы проверили' }), note);
  await userEvent.click(screen.getByRole('button', { name: 'Обновить' }));
  await waitFor(() =>
    expect(screen.getByRole('textbox', { name: 'Что вы проверили' })).toHaveProperty('value', note),
  );
  await userEvent.click(screen.getByRole('button', { name: 'Сохранить результат сверки' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith('/api/approvals/one/reconcile', {
      expectedRevision: 3,
      outcome: 'succeeded',
      note,
    }),
  );
  expect(await screen.findByText('Результат сверки владельца')).toBeTruthy();
  expect(screen.getByText(note)).toBeTruthy();
  expect(request.mock.calls.filter(([, body]) => body)).toHaveLength(1);
});
test('a saved refusal is a normal historical decision, not an actionable error', async () => {
  const { client, request } = setup();
  const original = request.getMockImplementation()!;
  request.mockImplementation(async (path, body, signal) => {
    if (path === '/api/approvals/one') {
      const saved = detail('one');
      return {
        ...saved,
        approval: { ...saved.approval, value: { ...saved.approval.value, state: 'denied' } },
        canApprove: false,
        canDeny: false,
        unavailableReason: 'Разрешение уже решено или истекло',
      };
    }
    return original(path, body, signal);
  });
  render(<ApprovalWorkspace client={client} snapshot={snapshot} connected />);
  expect(await screen.findByText(/Точный текст one/)).toBeTruthy();
  expect(screen.queryByText('Разрешение уже решено или истекло')).toBeNull();
  expect(screen.queryByRole('button', { name: 'Разрешить эту операцию' })).toBeNull();
});
const snapshot = {
  config: { value: { registry: { agents: [{ id: 'agent', name: 'Исследователь 01' }] } } },
  runs: [{ id: 'run', title: 'Проверка источника' }],
} as unknown as Snapshot;
const detail = (id: string) => ({
  approval: {
    revision: 3,
    value: {
      id,
      fingerprint: `digest-${id}`,
      state: 'pending',
      createdAt: 100,
      expiresAt: 9000000000,
      operation: {
        agentId: 'agent',
        taskId: 'task',
        runId: 'run',
        connectionId: 'connection',
        connectionRevision: 7,
        originEpoch: 4,
        epoch: 5,
        namespace: 'reports',
        resourceScopeHash: 'scope',
        tool: 'write_report',
        arguments: { text: `Точный текст ${id}` },
      },
    },
  },
  connection: { name: 'Архив', transport: { type: 'http', url: 'https://fixture.invalid/mcp' } },
  canApprove: true,
  canDeny: true,
  unavailableReason: null,
});
function setup() {
  const client = new ApiClient();
  const request = vi.spyOn(client, 'request').mockImplementation(async (path, body) => {
    if (body) return {};
    if (path.startsWith('/api/approvals?'))
      return {
        items: ['one', 'two'].map((id) => ({
          id,
          agentId: 'agent',
          tool: 'write_report',
          namespace: 'reports',
          state: 'pending',
          expiresAt: 9000000000,
        })),
        nextCursor: null,
      };
    return detail(path.split('/').at(-1)!);
  });
  return { client, request };
}
test('owner approval sends only the reviewed identity and revision, never replacement arguments', async () => {
  const { client, request } = setup();
  render(<ApprovalWorkspace client={client} snapshot={snapshot} connected />);
  expect(await screen.findByText(/Точный текст one/)).toBeTruthy();
  expect(screen.getByText('https://fixture.invalid/mcp')).toBeTruthy();
  expect(request.mock.calls.filter(([, body]) => body)).toHaveLength(0);
  await userEvent.click(screen.getByRole('button', { name: 'Разрешить эту операцию' }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith('/api/approvals/one/decision', {
      expectedRevision: 3,
      fingerprint: 'digest-one',
      allow: true,
    }),
  );
});
test('late detail cannot enable a different operation and a lost decision response requires a fresh review', async () => {
  const { client, request } = setup();
  const original = request.getMockImplementation()!;
  let release: (value: unknown) => void = () => {};
  let first = true;
  let failed = false;
  request.mockImplementation(async (path, body, signal) => {
    if (body) {
      failed = true;
      throw new ApiError('disconnected', 'Связь потеряна', 503);
    }
    if (path === '/api/approvals/one' && first) {
      first = false;
      return new Promise((resolve) => {
        release = resolve;
      });
    }
    if (path === '/api/approvals/two' && failed) throw new Error('Не удалось обновить решение');
    return original(path, body, signal);
  });
  render(<ApprovalWorkspace client={client} snapshot={snapshot} connected />);
  await userEvent.click(await screen.findByRole('button', { name: 'Открыть операцию two' }));
  expect(await screen.findByText(/Точный текст two/)).toBeTruthy();
  release(detail('one'));
  await userEvent.click(screen.getByRole('button', { name: 'Разрешить эту операцию' }));
  expect(await screen.findByText('Не удалось обновить решение')).toBeTruthy();
  expect(screen.queryByRole('button', { name: 'Разрешить эту операцию' })).toBeNull();
  expect(request.mock.calls.filter(([, body]) => body)).toEqual([
    [
      '/api/approvals/two/decision',
      { expectedRevision: 3, fingerprint: 'digest-two', allow: true },
    ],
  ]);
});
test('revoked resources and disconnected services never expose an enabled approval button', async () => {
  const { client, request } = setup();
  const original = request.getMockImplementation()!;
  request.mockImplementation(async (path, body, signal) =>
    path === '/api/approvals/one'
      ? { ...detail('one'), canApprove: false, unavailableReason: 'Подключение изменилось' }
      : original(path, body, signal),
  );
  render(<ApprovalWorkspace client={client} snapshot={snapshot} connected={false} />);
  expect(await screen.findByText('Подключение изменилось')).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Разрешить эту операцию' })).toHaveProperty(
    'disabled',
    true,
  );
  expect(screen.getByRole('button', { name: 'Отклонить операцию' })).toHaveProperty(
    'disabled',
    true,
  );
});
