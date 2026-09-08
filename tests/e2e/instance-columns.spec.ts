import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`column preferences persist without changing selection or virtual position at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot();
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/api/**', async (route) => {
      expect(route.request().method()).toBe('GET');
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      expect(path).toBe('/api/state');
      return route.fulfill({ json: state });
    });
    const navigate = async () => {
      await page.getByRole('button', { name: /^Агенты \d/ }).click();
      await page.getByRole('button', { name: /Экземпляры/ }).click();
    };
    await page.goto('/');
    await navigate();
    const viewport = page.getByRole('region', { name: 'Прокрутка экземпляров' });
    const table = page.getByRole('table');
    await viewport.focus();
    await page.keyboard.press('Control+End');
    await page.getByRole('checkbox', { name: 'Выбрать Исследователь · 4999', exact: true }).check();
    const top = await viewport.evaluate((el) => el.scrollTop);
    await page.getByText('Колонки', { exact: true }).click();
    for (const name of [
      'Показывать ID',
      'Показывать холон',
      'Показывать задачу',
      'Показывать исполнителя',
    ])
      await page.getByRole('checkbox', { name, exact: true }).uncheck();
    await expect(table).toHaveAttribute('aria-colcount', '5');
    expect(await viewport.evaluate((el) => el.scrollTop)).toBe(top);
    expect(await table.getByRole('row').count()).toBeLessThan(40);
    await expect(
      page.getByRole('checkbox', { name: 'Выбрать Исследователь · 4999', exact: true }),
    ).toBeChecked();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    await page.screenshot({ path: testInfo.outputPath('instance-columns.png'), fullPage: true });
    await page.reload();
    await navigate();
    await expect(table).toHaveAttribute('aria-colcount', '5');
    await page.getByLabel('Поиск экземпляров').fill('agent-directory-4998');
    await expect(table).toContainText('Исследователь · 4999');
    await expect(table.getByRole('columnheader', { name: 'Состояние', exact: true })).toBeVisible();
    await page.getByText('Колонки', { exact: true }).click();
    await page.getByRole('button', { name: 'Стандартные колонки' }).click();
    await expect(table).toHaveAttribute('aria-colcount', '9');
    await expect(table).toContainText('agent-directory-4998');
    expect(errors).toEqual([]);
  });
}
