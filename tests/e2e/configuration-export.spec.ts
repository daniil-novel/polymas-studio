import { expect, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import fixture from '../../contracts/export-examples/materialized.json' with { type: 'json' };
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`review and download the exact configuration under CSP at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot();
    state.config.revision = 4;
    const exported = structuredClone(fixture);
    for (let index = 2; index < 5000; index++)
      exported.instances.push({
        ...exported.instances[0],
        id: `agent_extra_${index}`,
        name: `Исполнитель ${index}`,
      });
    exported.roleVersions[0].instruction = 'Сравни источники 😀\n'.repeat(3000);
    let reads = 0;
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/', async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        headers: {
          ...response.headers(),
          'content-security-policy':
            "script-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'none'",
        },
      });
    });
    await page.route('**/api/**', async (route) => {
      expect(route.request().method()).toBe('GET');
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/exports/configuration') {
        reads++;
        return route.fulfill({ json: exported });
      }
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      expect(path).toBe('/api/state');
      return route.fulfill({ json: state });
    });
    await page.goto('/');
    await page.getByRole('button', { name: 'Настройки', exact: true }).first().click();
    expect(reads).toBe(0);
    const panel = page.getByRole('region', { name: 'Экспорт конфигурации' });
    await panel.getByRole('button', { name: 'Подготовить экспорт' }).click();
    await expect(panel.getByText('Подготовлена ревизия 4')).toBeVisible();
    state.config.revision = 5;
    await expect(panel.getByText(/Конфигурация уже изменилась/)).toBeVisible({ timeout: 10000 });
    await panel.getByText('Просмотреть JSON').click();
    const preview = panel.getByRole('region', { name: 'Содержимое экспорта' });
    expect((await preview.textContent())!.length).toBeLessThanOrEqual(32768);
    await panel.getByRole('button', { name: 'Следующая часть' }).click();
    await expect(preview).toContainText('Сравни источники 😀');
    const box = await panel.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await page.screenshot({
      path: testInfo.outputPath('configuration-export.png'),
      fullPage: true,
    });
    const downloadPending = page.waitForEvent('download');
    await panel.getByRole('button', { name: 'Скачать JSON' }).click();
    const download = await downloadPending;
    expect(download.suggestedFilename()).toBe('polymas-configuration-r4.json');
    expect(JSON.parse(await readFile((await download.path())!, 'utf8'))).toEqual(exported);
    expect(reads).toBe(1);
    expect(errors).toEqual([]);
  });
}
