import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`a lost duplication response recovers across reload without another role at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot(5);
    const source = state.config.value.registry.roles[0];
    state.config.value.registry.roles.push({
      ...source,
      version: 2,
      instruction: 'Latest saved instruction',
    });
    const before = structuredClone(state.config.value);
    const copy = { ...source, id: 'role-copy-fixture', name: 'Копия источника 🧠', version: 1 };
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
      if (
        message.type() === 'error' &&
        !(
          message.text().includes('net::ERR_FAILED') &&
          message.location().url.includes('/api/configure')
        )
      )
        errors.push(message.text());
    });
    let posts = 0;
    let operationKey = '';
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/configure') {
        const body = route.request().postDataJSON();
        if (!operationKey) operationKey = body.command.key;
        expect(body).toEqual({
          revision: posts === 0 ? 8 : 9,
          command: {
            type: 'duplicate_role',
            key: operationKey,
            roleId: source.id,
            sourceVersion: 1,
            name: copy.name,
          },
        });
        posts++;
        if (posts === 1) {
          state.config.value.registry.roles.push(copy);
          state.config.revision++;
          return route.abort('failed');
        }
        return route.fulfill({
          json: { revision: 9, result: { role: copy, sourceRoleId: source.id, sourceVersion: 1 } },
        });
      }
      expect(route.request().method()).toBe('GET');
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      expect(path).toBe('/api/state');
      return route.fulfill({ json: state });
    });
    await page.goto('/');
    await page.getByRole('button', { name: /^Агенты \d/ }).click();
    await page.getByRole('combobox', { name: 'Выбрать роль' }).selectOption(source.id);
    await page
      .getByRole('textbox', { name: 'Ролевая инструкция', exact: true })
      .fill('Сохранить локальный черновик');
    await page.getByRole('button', { name: 'Сравнить версии', exact: true }).click();
    await page.getByRole('button', { name: 'Дублировать исходную версию' }).click();
    const dialog = page.getByRole('dialog', { name: 'Дублирование роли' });
    await dialog.getByRole('textbox', { name: 'Название новой роли', exact: true }).fill(copy.name);
    await expect(dialog.getByText('Новая независимая роль · v1')).toBeVisible();
    const bounds = (await dialog.boundingBox())!;
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(width);
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('role-duplication.png'), fullPage: true });
    await dialog.getByRole('button', { name: 'Создать копию роли' }).click();
    await expect(dialog.getByRole('alert')).toContainText('Обновите проверку');
    await expect(
      dialog.getByRole('textbox', { name: 'Название новой роли', exact: true }),
    ).toBeDisabled();
    await dialog.getByRole('button', { name: 'Закрыть дублирование' }).click();
    await expect(
      page.getByRole('textbox', { name: 'Ролевая инструкция', exact: true }),
    ).toHaveValue('Сохранить локальный черновик');
    await page.reload();
    await page.getByRole('button', { name: /^Агенты \d/ }).click();
    await page.getByRole('button', { name: 'Продолжить дублирование' }).click();
    await expect(
      dialog.getByRole('textbox', { name: 'Название новой роли', exact: true }),
    ).toHaveValue(copy.name);
    const retry = dialog.getByRole('button', { name: 'Повторить сохранённый запрос' });
    await expect(retry).toBeEnabled();
    expect(posts).toBe(1);
    await retry.click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByText(/Создана роль «Копия источника 🧠» · v1/)).toBeVisible();
    await expect(page.getByRole('button', { name: 'Продолжить дублирование' })).toHaveCount(0);
    expect(posts).toBe(2);
    expect(state.config.value).toEqual({
      ...before,
      registry: { ...before.registry, roles: [...before.registry.roles, copy] },
    });
    await page.getByRole('combobox', { name: 'Выбрать роль' }).selectOption(copy.id);
    await expect(
      page.getByRole('textbox', { name: 'Ролевая инструкция', exact: true }),
    ).toHaveValue(source.instruction);
    await expect(page.getByRole('button', { name: 'Экземпляры 0', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Экземпляры 0', exact: true }).click();
    await expect(page.getByRole('table')).toBeVisible();
    expect(errors).toEqual([]);
  });
}
