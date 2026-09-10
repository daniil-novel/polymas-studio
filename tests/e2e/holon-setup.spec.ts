import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`empty holon setup keeps the task draft at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    const snapshot = instanceSnapshot(3);
    snapshot.config.value.architecture.holons = [];
    snapshot.agentActivity = [];
    snapshot.runtime.ready = true;
    snapshot.taskCount = 0;
    const writes: string[] = [];
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (route.request().method() === 'POST') {
        writes.push(path);
        expect(path).toBe('/api/configure');
        const body = route.request().postDataJSON();
        expect(body.revision).toBe(snapshot.config.revision);
        expect(body.command.type).toBe('set_architecture');
        snapshot.config.value.architecture = body.command.architecture;
        snapshot.config.revision++;
        return route.fulfill({ json: snapshot.config });
      }
      if (path === '/api/state') return route.fulfill({ json: snapshot });
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      expect(path).toBe('/api/runs');
      return route.fulfill({ json: { items: [], nextCursor: null } });
    });
    await page.goto('/');
    await page.getByRole('button', { name: 'Задачи', exact: true }).click();
    await page.getByRole('combobox', { name: 'Адресация', exact: true }).selectOption('holon');
    await expect(page.getByText(/Холоны пока не созданы/)).toBeVisible();
    await page
      .getByRole('textbox', { name: 'Задача или сообщение', exact: true })
      .fill('Придумайте совместную шутку');
    await expect(page.getByRole('button', { name: 'Начать', exact: true })).toBeDisabled();
    await page.getByRole('button', { name: 'Настроить холоны', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Архитектура системы', exact: true });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Создать холон', exact: true }).click();
    await dialog.getByLabel('Название холона', { exact: true }).fill('Мастерская юмора');
    await dialog.getByRole('checkbox', { name: /Исследователь · 0001/ }).check();
    await dialog.getByRole('checkbox', { name: /Исследователь · 0002/ }).check();
    await dialog
      .getByRole('combobox', { name: 'Координатор холона', exact: true })
      .selectOption('agent-directory-0000');
    await dialog.getByRole('button', { name: 'Сохранить архитектуру', exact: true }).click();
    await expect(dialog).toBeHidden();
    await expect(
      page.getByRole('textbox', { name: 'Задача или сообщение', exact: true }),
    ).toHaveValue('Придумайте совместную шутку');
    await expect(page.getByRole('button', { name: 'Начать', exact: true })).toBeDisabled();
    const holon = snapshot.config.value.architecture.holons[0];
    await page
      .getByRole('combobox', { name: 'Холон-получатель', exact: true })
      .selectOption(holon.id);
    await expect(
      page.getByRole('textbox', { name: 'Задача или сообщение', exact: true }),
    ).toHaveValue('Придумайте совместную шутку');
    await expect(page.getByRole('button', { name: 'Начать', exact: true })).toBeEnabled();
    await expect(page.getByText(/общий ответ требует сценария обсуждения/)).toBeVisible();
    expect(holon.members).toEqual(['agent-directory-0000', 'agent-directory-0001']);
    expect(writes).toEqual(['/api/configure']);
    await page.getByRole('button', { name: 'Начать', exact: true }).click({ trial: true });
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
    await page.screenshot({ path: `tmp/holon-setup-${width}.png`, fullPage: true });
  });
}
