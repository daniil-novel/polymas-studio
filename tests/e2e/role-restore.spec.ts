import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`a reviewed restore creates one new role version without reassigning agents at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot(5);
    const first = state.config.value.registry.roles[0];
    state.config.value.registry.roles.push({
      ...first,
      version: 2,
      instruction: 'Новая инструкция',
      sandbox: 'workspace-write',
    });
    const prior = structuredClone(state.config.value);
    let writes = 0;
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/configure') {
        expect(route.request().postDataJSON()).toEqual({
          revision: 8,
          command: { type: 'restore_role', roleId: first.id, sourceVersion: 1, expectedVersion: 2 },
        });
        writes++;
        const restored = { ...first, version: 3 };
        state.config.value.registry.roles.push(restored);
        state.config.revision++;
        return route.fulfill({
          json: { revision: 9, result: { role: restored, sourceVersion: 1 } },
        });
      }
      expect(route.request().method()).toBe('GET');
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      expect(path).toBe('/api/state');
      return route.fulfill({ json: state });
    });
    const navigate = async () => {
      await page.getByRole('button', { name: /^Агенты \d/ }).click();
      await page.getByRole('combobox', { name: 'Выбрать роль' }).selectOption(first.id);
    };
    await page.goto('/');
    await navigate();
    await page
      .getByRole('textbox', { name: 'Ролевая инструкция', exact: true })
      .fill('Черновик не теряется');
    await page.getByRole('button', { name: 'Сравнить версии', exact: true }).click();
    await page.getByRole('button', { name: 'Восстановить исходную версию' }).click();
    const dialog = page.getByRole('dialog', { name: 'Восстановление версии роли' });
    await expect(dialog.getByText('Будет создана версия v3 из v1')).toBeVisible();
    const box = (await dialog.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(width);
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('role-restore.png'), fullPage: true });
    await dialog.getByRole('button', { name: 'Создать восстановленную версию' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByText('Сохранённая версия роли изменилась', { exact: true })).toBeVisible();
    await expect(
      page.getByRole('textbox', { name: 'Ролевая инструкция', exact: true }),
    ).toHaveValue('Черновик не теряется');
    expect(writes).toBe(1);
    expect(state.config.value).toEqual({
      ...prior,
      registry: { ...prior.registry, roles: [...prior.registry.roles, { ...first, version: 3 }] },
    });
    await page.reload();
    await navigate();
    await expect(
      page.getByRole('textbox', { name: 'Ролевая инструкция', exact: true }),
    ).toHaveValue(first.instruction);
    await page.getByRole('button', { name: 'Экземпляры 5', exact: true }).click();
    await expect(page.getByRole('table')).toContainText('Исследователь · v1');
    expect(errors).toEqual([]);
  });
}
