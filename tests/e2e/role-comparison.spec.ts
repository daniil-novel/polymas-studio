import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`saved role differences remain readable and preserve a local draft at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot(5);
    const original = 'Источники '.repeat(4500) + '🧠Конец';
    state.config.value.registry.roles[0].instruction = original;
    state.config.value.registry.roles.push({
      ...state.config.value.registry.roles[0],
      version: 2,
      instruction: 'Сравни утверждения и сохрани ссылки.',
      sandbox: 'workspace-write',
      maxOutputTokens: 2048,
    });
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
    await page.goto('/');
    await page.getByRole('button', { name: /^Агенты \d/ }).click();
    await page.getByRole('combobox', { name: 'Выбрать роль' }).selectOption('role-research');
    await page
      .getByRole('textbox', { name: 'Ролевая инструкция', exact: true })
      .fill('Несохранённый черновик');
    const trigger = page.getByRole('button', { name: 'Сравнить версии', exact: true });
    await trigger.click();
    const dialog = page.getByRole('dialog', { name: 'Сравнение версий роли' });
    await expect(dialog.getByText('Изменённых полей: 3 из 11')).toBeVisible();
    await dialog.getByRole('checkbox', { name: 'Только изменённые поля' }).check();
    await expect(dialog.getByRole('article')).toHaveCount(3);
    const instruction = dialog.getByRole('article', { name: 'Ролевая инструкция', exact: true });
    const before = instruction.getByRole('region', { name: 'Исходное значение', exact: true });
    const after = instruction.getByRole('region', { name: 'Новое значение', exact: true });
    await instruction.scrollIntoViewIfNeeded();
    const a = (await before.boundingBox())!;
    const b = (await after.boundingBox())!;
    if (width < 800) expect(b.y).toBeGreaterThanOrEqual(a.y + a.height);
    else expect(b.x).toBeGreaterThanOrEqual(a.x + a.width);
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    const box = (await dialog.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(width);
    await page.screenshot({ path: testInfo.outputPath('role-comparison.png'), fullPage: true });
    let joined = (await before.getByRole('textbox').textContent())!;
    await before.getByRole('button', { name: 'Следующая часть' }).click();
    joined += await before.getByRole('textbox').textContent();
    expect(joined).toBe(original);
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(trigger).toBeFocused();
    await expect(
      page.getByRole('textbox', { name: 'Ролевая инструкция', exact: true }),
    ).toHaveValue('Несохранённый черновик');
    expect(errors).toEqual([]);
  });
}
