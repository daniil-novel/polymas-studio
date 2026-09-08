import { afterEach, expect, test, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiClient } from '../../apps/desktop/src/api';
import { OperationResultViewer } from '../../apps/desktop/src/approvals/OperationResultViewer';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const metadata = { byteLength: 12, sha256: 'a'.repeat(64) };
test('switching operations cancels an in-flight download before it can create a file', async () => {
  class TestURL extends URL {
    static createObjectURL = vi.fn(() => 'blob:fixture');
  }
  vi.stubGlobal('URL', TestURL);
  const client = new ApiClient();
  let release: (value: unknown) => void = () => {};
  vi.spyOn(client, 'request').mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const view = render(
    <OperationResultViewer client={client} id="one" metadata={metadata} connected />,
  );
  await userEvent.click(screen.getByRole('button', { name: 'Скачать ответ JSON' }));
  expect(screen.getByRole('button', { name: 'Отменить загрузку' })).toBeTruthy();
  view.rerender(<OperationResultViewer client={client} id="two" metadata={metadata} connected />);
  await act(async () => release({ metadata, offset: 0, nextOffset: null, chunk: '{"x":"done"}' }));
  expect(TestURL.createObjectURL).not.toHaveBeenCalled();
  expect(screen.queryByRole('alert')).toBeNull();
  expect(screen.getByRole('button', { name: 'Скачать ответ JSON' })).toHaveProperty(
    'disabled',
    false,
  );
});
test('result viewer reads on demand, pages forward/back and ignores a late previous operation', async () => {
  const client = new ApiClient();
  let release: (value: unknown) => void = () => {};
  const request = vi.spyOn(client, 'request').mockImplementation(async (path) => {
    if (path.includes('/two/'))
      return new Promise((resolve) => {
        release = resolve;
      });
    return path.includes('offset=0')
      ? { metadata, offset: 0, nextOffset: 6, chunk: '{"x":"' }
      : { metadata, offset: 6, nextOffset: null, chunk: 'done"}' };
  });
  const view = render(
    <OperationResultViewer client={client} id="one" metadata={metadata} connected />,
  );
  expect(request.mock.calls).toHaveLength(0);
  await userEvent.click(screen.getByRole('button', { name: 'Показать ответ' }));
  expect(await screen.findByText('{"x":"')).toBeTruthy();
  await userEvent.click(screen.getByRole('button', { name: 'Следующая часть ответа' }));
  expect(await screen.findByText('done"}')).toBeTruthy();
  await userEvent.click(screen.getByRole('button', { name: 'Предыдущая часть ответа' }));
  expect(await screen.findByText('{"x":"')).toBeTruthy();
  view.rerender(<OperationResultViewer client={client} id="two" metadata={metadata} connected />);
  await userEvent.click(screen.getByRole('button', { name: 'Показать ответ' }));
  view.rerender(<OperationResultViewer client={client} id="three" metadata={metadata} connected />);
  await act(async () => release({ metadata, offset: 0, nextOffset: 6, chunk: 'SECRET' }));
  expect(screen.queryByText('SECRET')).toBeNull();
  expect(screen.queryByText('{"x":"')).toBeNull();
  expect(request.mock.calls.every(([, body]) => body === undefined)).toBe(true);
});
test('a failed result read offers a retry and never a write or approval action', async () => {
  const client = new ApiClient();
  vi.spyOn(client, 'request').mockRejectedValue(new Error('Ответ недоступен'));
  const view = render(
    <OperationResultViewer client={client} id="one" metadata={metadata} connected />,
  );
  await userEvent.click(screen.getByRole('button', { name: 'Показать ответ' }));
  expect(await screen.findByText('Ответ недоступен')).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Обновить ответ' })).toBeTruthy();
  view.rerender(
    <OperationResultViewer client={client} id="one" metadata={metadata} connected={false} />,
  );
  expect(screen.getByRole('button', { name: 'Скачать ответ JSON' })).toHaveProperty(
    'disabled',
    true,
  );
  expect(screen.queryByRole('button', { name: /Разрешить/ })).toBeNull();
});
