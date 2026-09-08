import { expect, test } from '@playwright/test';
import { historyResponse, taskId } from './fixtures/history';

for (const width of [736, 1280, 1600]) {
  test(`old runs and paged task results remain readable at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/api/**', async (route) => {
      expect(route.request().method()).toBe('GET');
      await route.fulfill({ json: historyResponse(new URL(route.request().url())) });
    });
    await page.goto('/');
    await page.getByRole('button', { name: 'Задачи', exact: true }).click();
    const sidebar = page.getByRole('complementary', { name: 'История запусков' });
    const runPager = page.getByRole('navigation', { name: 'Страницы запусков' });
    await runPager.getByRole('button', { name: 'Далее' }).click();
    await sidebar.getByRole('button', { name: /Анализ архитектуры · запуск 31 / }).click();
    const detail = page.getByRole('region', { name: 'Подробности задачи' });
    await expect(detail).toContainText(`HISTORY-RESULT ${taskId(1016)}`);
    await page
      .getByRole('navigation', { name: 'Страницы задач запуска' })
      .getByRole('button', { name: 'Далее' })
      .click();
    await expect(detail).toContainText(`HISTORY-RESULT ${taskId(986)}`);
    await expect(page.getByLabel('Задача исполнителя').locator('option')).toHaveCount(30);
    await page.getByLabel('Поиск запусков').fill('Не существующий запуск');
    await expect(sidebar).toContainText('Запуски не найдены');
    await expect(detail).toContainText(`HISTORY-RESULT ${taskId(986)}`);
    const detailBox = await detail.boundingBox();
    expect(detailBox!.width).toBeGreaterThanOrEqual(400);
    expect(detailBox!.x + detailBox!.width).toBeLessThanOrEqual(width);
    if (width < 850) expect((await sidebar.boundingBox())!.height).toBeGreaterThanOrEqual(320);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
    expect(errors).toEqual([]);
    await page.getByLabel('Поиск запусков').fill('');
    await sidebar.getByRole('button', { name: /Анализ архитектуры · запуск 1 / }).waitFor();
    await page.screenshot({ path: testInfo.outputPath('history.png'), fullPage: true });
  });
}
