import { afterEach, expect, test, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiClient } from '../../apps/desktop/src/api';
import { ConfigurationExportPanel } from '../../apps/desktop/src/ConfigurationExportPanel';
import { exportTextPage, prepareConfigurationExport } from '../../apps/desktop/src/export-data';
import fixture from '../../contracts/export-examples/materialized.json';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test('only a reviewed schema-valid configuration can be saved; private unknown fields are rejected', () => {
  const prepared = prepareConfigurationExport(fixture);
  expect(JSON.parse(prepared.text)).toEqual(fixture);
  expect(prepared.document.instances).toHaveLength(2);
  expect(() => prepareConfigurationExport({ ...fixture, history: ['private'] })).toThrow(/формат/);
  const invalid = structuredClone(fixture);
  Object.assign(invalid.connections[1], { url: 'https://private.invalid' });
  expect(() => prepareConfigurationExport(invalid)).toThrow(/формат/);
  const tooLarge = structuredClone(fixture);
  tooLarge.roleVersions[0].instruction = 'я'.repeat(9 * 1024 * 1024);
  expect(() => prepareConfigurationExport(tooLarge)).toThrow(/16 МиБ/);
  expect(() =>
    prepareConfigurationExport({ ...fixture, sourceRevision: Number.MAX_SAFE_INTEGER + 1 }),
  ).toThrow(/точность/);
});

test('JSON preview pages preserve a Unicode pair at the boundary without duplicate or missing characters', () => {
  const text = 'x'.repeat(32767) + '😀' + 'я'.repeat(32766) + '😀' + 'tail';
  const parts = [0, 1, 2].map((page) => exportTextPage(text, page));
  expect(parts.join('')).toBe(text);
  for (const part of parts)
    expect(new TextDecoder().decode(new TextEncoder().encode(part))).toBe(part);
});

test('preparation is explicit and background revisions never replace the reviewed file', async () => {
  const client = new ApiClient();
  const request = vi.spyOn(client, 'request').mockResolvedValue(structuredClone(fixture));
  const save = vi.fn();
  const view = render(
    <ConfigurationExportPanel client={client} connected currentRevision={4} save={save} />,
  );
  const user = userEvent.setup();
  expect(request).not.toHaveBeenCalled();
  expect(screen.queryByRole('button', { name: 'Скачать JSON' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Подготовить экспорт' }));
  expect(await screen.findByText('Подготовлена ревизия 4')).toBeTruthy();
  expect(request.mock.calls[0][0]).toBe('/api/exports/configuration');
  expect(request.mock.calls[0][1]).toBeUndefined();
  view.rerender(
    <ConfigurationExportPanel client={client} connected currentRevision={5} save={save} />,
  );
  expect(screen.getByText(/Конфигурация уже изменилась/)).toBeTruthy();
  await user.click(screen.getByText('Просмотреть JSON'));
  expect(screen.getByRole('region', { name: 'Содержимое экспорта' }).textContent).toContain(
    'Сравни источники',
  );
  await user.click(screen.getByRole('button', { name: 'Скачать JSON' }));
  expect(save).toHaveBeenCalledTimes(1);
  expect(JSON.parse(save.mock.calls[0][0])).toEqual(fixture);
  expect(save.mock.calls[0][1]).toBe('polymas-configuration-r4.json');
  expect(request).toHaveBeenCalledTimes(1);
});

test('cancel and disconnect discard late replies, a failed refresh cannot download an older file', async () => {
  const client = new ApiClient();
  let resolve!: (value: unknown) => void;
  const request = vi.spyOn(client, 'request').mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const view = render(<ConfigurationExportPanel client={client} connected currentRevision={4} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Подготовить экспорт' }));
  await user.click(screen.getByRole('button', { name: 'Отменить подготовку' }));
  await act(async () => resolve(fixture));
  expect(screen.queryByText('Подготовлена ревизия 4')).toBeNull();
  request.mockResolvedValueOnce(fixture);
  await user.click(screen.getByRole('button', { name: 'Подготовить экспорт' }));
  expect(await screen.findByText('Подготовлена ревизия 4')).toBeTruthy();
  request.mockRejectedValueOnce(new Error('Нет связи'));
  await user.click(screen.getByRole('button', { name: 'Обновить экспорт' }));
  expect(await screen.findByRole('alert')).toHaveProperty('textContent', 'Нет связи');
  expect(screen.queryByRole('button', { name: 'Скачать JSON' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Подготовить экспорт' }));
  view.rerender(<ConfigurationExportPanel client={client} connected={false} currentRevision={4} />);
  await act(async () => resolve(fixture));
  expect(screen.queryByRole('button', { name: 'Скачать JSON' })).toBeNull();
  expect(
    (screen.getByRole('button', { name: 'Подготовить экспорт' }) as HTMLButtonElement).disabled,
  ).toBe(true);
});
