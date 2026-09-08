import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`rename preserves agent identity through save and reload at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot(12);
    const before = structuredClone(state.config.value.registry.agents[0]);
    const writes: unknown[] = [];
    await page.route('**/api/**', async (route) => {
      const request = route.request();
      const path = new URL(request.url()).pathname;
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      if (path === '/api/configure') {
        const body = request.postDataJSON();
        writes.push(body);
        expect(body).toEqual({
          revision: 8,
          command: { type: 'rename_agent', agentId: before.id, name: 'Проверяющий источники' },
        });
        state.config.revision++;
        state.config.value.registry.agents[0].name = body.command.name;
        return route.fulfill({ json: { revision: 9, result: {} } });
      }
      expect(path).toBe('/api/state');
      expect(request.method()).toBe('GET');
      return route.fulfill({ json: state });
    });
    const navigate = async () => {
      await page.getByRole('button', { name: /^Агенты \d/ }).click();
      await page.getByRole('button', { name: /Экземпляры/ }).click();
      await page.getByLabel('Поиск экземпляров').fill(before.id);
    };
    await page.goto('/');
    await navigate();
    await page.getByRole('button', { name: `Переименовать ${before.name}` }).click();
    const modal = page.getByRole('dialog', { name: 'Переименовать экземпляр' });
    await expect(modal.getByLabel('Имя экземпляра')).toHaveValue(before.name);
    expect((await modal.boundingBox())!.width).toBeLessThanOrEqual(560);
    await modal.getByLabel('Имя экземпляра').fill('Проверяющий источники');
    await page.screenshot({ path: testInfo.outputPath('rename.png'), fullPage: true });
    await modal.getByRole('button', { name: 'Сохранить имя' }).click();
    await expect(modal).toHaveCount(0);
    await expect(page.getByRole('table')).toContainText('Проверяющий источники');
    expect(writes).toHaveLength(1);
    expect(state.config.value.registry.agents[0]).toEqual({
      ...before,
      name: 'Проверяющий источники',
    });
    await page.reload();
    await navigate();
    await expect(page.getByRole('table')).toContainText('Проверяющий источники');
    await page.getByRole('button', { name: 'Переименовать Проверяющий источники' }).click();
    await modal.getByLabel('Имя экземпляра').fill('Не сохранять');
    await page.keyboard.press('Escape');
    await expect(modal).toHaveCount(0);
    expect(writes).toHaveLength(1);
  });
}
