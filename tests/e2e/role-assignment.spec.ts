import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1600]) {
  test(`a selected role version is saved without changing instance identity at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const state = instanceSnapshot();
    state.config.value.registry.roles.push({
      ...state.config.value.registry.roles[0],
      version: 2,
      instruction: 'Сравни два источника. Сохрани результат для новых задач.',
    });
    const selected = [0, 4998].map((index) =>
      structuredClone(state.config.value.registry.agents[index]),
    );
    const ids = selected.map((agent) => agent.id);
    let writes = 0;
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/configure') {
        expect(route.request().postDataJSON()).toEqual({
          revision: 8,
          command: { type: 'pin_role', agentIds: ids, roleId: 'role-research', version: 2 },
        });
        writes++;
        for (const agent of state.config.value.registry.agents)
          if (ids.includes(agent.id)) agent.roleVersion = 2;
        state.config.revision = 9;
        return route.fulfill({ json: { revision: 9, result: {} } });
      }
      expect(route.request().method()).toBe('GET');
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
    for (const agent of selected) {
      await page.getByLabel('Поиск экземпляров').fill(agent.id);
      await page.getByRole('checkbox', { name: `Выбрать ${agent.name}`, exact: true }).check();
    }
    await page.getByRole('button', { name: 'Назначить версию роли', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Назначить версию роли' });
    await dialog
      .getByRole('combobox', { name: 'Версия роли', exact: true })
      .selectOption(JSON.stringify(['role-research', 2]));
    await expect(dialog.getByText(/Экземпляров с незавершёнными задачами: 1/)).toBeVisible();
    for (const id of ids)
      await expect(dialog.getByRole('list', { name: 'Выбранные экземпляры' })).toContainText(id);
    const box = await dialog.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(await dialog.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('role-assignment.png'), fullPage: true });
    await dialog.getByRole('button', { name: 'Применить к новым задачам' }).click();
    await expect(dialog).toHaveCount(0);
    expect(writes).toBe(1);
    for (const agent of selected)
      expect(state.config.value.registry.agents.find((a) => a.id === agent.id)).toEqual({
        ...agent,
        roleVersion: 2,
      });
    await page.reload();
    await navigate();
    await page.getByLabel('Поиск экземпляров').fill(ids[1]);
    await expect(page.getByRole('table')).toContainText('Исследователь · v2');
    await expect(page.getByRole('table')).toContainText('task-old-active');
    expect(errors).toEqual([]);
  });
}
