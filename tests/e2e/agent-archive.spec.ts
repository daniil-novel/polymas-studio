import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`a reviewed selection survives filtering and archives exactly once at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot();
    const selected = [
      state.config.value.registry.agents[0],
      state.config.value.registry.agents[4997],
    ].map((agent) => structuredClone(agent));
    const ids = selected.map((agent) => agent.id);
    let writes = 0;
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      if (path === '/api/configure') {
        expect(route.request().postDataJSON()).toEqual({
          revision: 8,
          command: { type: 'archive_agents', agentIds: ids },
        });
        writes++;
        for (const agent of state.config.value.registry.agents)
          if (ids.includes(agent.id)) agent.status = 'archived';
        state.config.revision++;
        return route.fulfill({ json: { revision: 9, result: { ids } } });
      }
      expect(path).toBe('/api/state');
      expect(route.request().method()).toBe('GET');
      return route.fulfill({ json: state });
    });
    const navigate = async () => {
      await page.getByRole('button', { name: /^Агенты \d/ }).click();
      await page.getByRole('button', { name: /Экземпляры/ }).click();
    };
    await page.goto('/');
    await navigate();
    for (const agent of selected) {
      await page.getByLabel('Поиск экземпляров').fill(agent.id);
      await page.getByRole('checkbox', { name: `Выбрать ${agent.name}`, exact: true }).check();
    }
    await expect(page.getByText('Выбрано: 2 · вне фильтра: 1')).toBeVisible();
    await page.getByRole('button', { name: 'Архивировать выбранные' }).click();
    const dialog = page.getByRole('dialog', { name: 'Архивировать экземпляры' });
    await expect(dialog.getByText('Выбрано для архива: 2')).toBeVisible();
    for (const id of ids) await expect(dialog.getByRole('list')).toContainText(id);
    await expect(dialog.getByRole('button', { name: 'Подтвердить архивирование' })).toBeEnabled();
    const box = await dialog.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
      true,
    );
    await page.screenshot({ path: testInfo.outputPath('archive-review.png'), fullPage: true });
    await dialog.getByRole('button', { name: 'Подтвердить архивирование' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByText('Выбранные экземпляры архивированы')).toBeVisible();
    expect(writes).toBe(1);
    for (const agent of selected)
      expect(state.config.value.registry.agents.find((item) => item.id === agent.id)).toEqual({
        ...agent,
        status: 'archived',
      });
    await page.reload();
    await navigate();
    await page.getByLabel('Состояние экземпляров').selectOption('archived');
    for (const agent of selected) await expect(page.getByRole('table')).toContainText(agent.name);
    expect(await page.getByRole('checkbox', { disabled: true }).count()).toBe(3);
    expect(errors).toEqual([]);
  });
}
