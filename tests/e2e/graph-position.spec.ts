import { expect, test, type Page } from '@playwright/test';

async function fixture(page: Page, conflict = false) {
  const state = {
    config: {
      revision: 3,
      value: {
        schemaVersion: 1,
        id: 'layout',
        name: 'Layout fixture',
        bindings: [],
        registry: {
          roles: [],
          agents: [
            {
              id: 'agent-layout',
              name: 'Layout agent',
              roleId: 'role',
              roleVersion: 1,
              status: 'idle',
            },
          ],
        },
        architecture: {
          preset: 'network',
          maxConcurrency: 1,
          holons: [],
          edges: [],
          positions: {} as Record<string, [number, number]>,
          memoryEnabled: false,
        },
      },
    },
    connections: [],
    cursor: 0,
    runs: [],
    tasks: [],
    taskCount: 0,
    agentRuntime: [],
    paused: true,
    runtime: { ready: false, capacity: 1, probe: null, error: null },
  };
  const commands: unknown[] = [];
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/configure') {
      const body = route.request().postDataJSON();
      commands.push(body);
      expect(body.revision).toBe(state.config.revision);
      expect(body.command.type).toBe('set_node_position');
      expect(body.command.nodeId).toBe('agent-layout');
      expect(Object.keys(body.command).sort()).toEqual(['nodeId', 'position', 'type']);
      if (conflict)
        return route.fulfill({
          status: 409,
          json: { code: 'conflict', message: 'Конфигурация изменилась' },
        });
      if (body.command.position === null)
        delete state.config.value.architecture.positions['agent-layout'];
      else state.config.value.architecture.positions['agent-layout'] = body.command.position;
      state.config.revision++;
      return route.fulfill({ json: { result: {}, revision: state.config.revision } });
    }
    expect(route.request().method()).toBe('GET');
    if (path === '/api/state') return route.fulfill({ json: state });
    if (path === '/api/events') return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
    throw new Error(`Unexpected request: ${path}`);
  });
  return { state, commands };
}

async function dragNode(page: Page, cancel = false) {
  // The authored single-node fixture is centered. Read canvas pixels only to wait
  // for the real worker/Sigma renderer, without reaching into application state.
  const minimap = page.locator('canvas.graph-minimap');
  await expect(minimap).toBeVisible();
  await expect
    .poll(() =>
      minimap.evaluate((canvas: HTMLCanvasElement) =>
        canvas
          .getContext('2d')!
          .getImageData(0, 0, canvas.width, canvas.height)
          .data.some((v, i) => i % 4 === 3 && v > 0),
      ),
    )
    .toBe(true);
  const box = (await page.getByRole('img', { name: /Граф 1 агентов/ }).boundingBox())!;
  const x = box.x + box.width / 2,
    y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 90, y - 55, { steps: 12 });
  if (cancel) await page.keyboard.press('Escape');
  await page.mouse.up();
}

test('drag saves one node, survives reload, and can release its pin', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  const { state, commands } = await fixture(page);
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await dragNode(page, true);
  expect(commands).toHaveLength(0);
  await dragNode(page);
  await expect.poll(() => commands.length).toBe(1);
  const saved = state.config.value.architecture.positions['agent-layout'];
  expect(saved.every(Number.isFinite)).toBe(true);
  expect(Math.hypot(...saved)).toBeGreaterThan(0);
  await page.reload();
  await page.getByRole('button', { name: 'Список', exact: true }).click();
  await page.getByRole('button', { name: /Layout agent/ }).click();
  await expect(page.getByText('Положение закреплено', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Снять закрепление', exact: true }).click();
  await expect.poll(() => commands.length).toBe(2);
  await expect(page.getByText('Автоматическое положение', { exact: true })).toBeVisible();
  expect(state.config.value.architecture.positions).toEqual({});
  expect(errors).toEqual([]);
});

test('a failed drag is not silently retried or shown as a saved pin', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  const { commands } = await fixture(page, true);
  await page.goto('/');
  await dragNode(page);
  await expect.poll(() => commands.length).toBe(1);
  await expect(
    page.getByRole('button', { name: 'Обновить положение узлов', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Обновить положение узлов', exact: true }).click();
  await page.getByRole('button', { name: 'Список', exact: true }).click();
  await page.getByRole('button', { name: /Layout agent/ }).click();
  await expect(page.getByText('Автоматическое положение', { exact: true })).toBeVisible();
  expect(commands).toHaveLength(1);
});

test('pin controls remain reachable inside a compact graph', async ({ page }) => {
  await page.setViewportSize({ width: 736, height: 800 });
  const { commands } = await fixture(page);
  await page.goto('/');
  await dragNode(page);
  await expect.poll(() => commands.length).toBe(1);
  const reset = page.getByRole('button', { name: 'Снять закрепление', exact: true });
  await expect(reset).toBeVisible();
  const bounds = (await reset.boundingBox())!;
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(736);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(736);
  await reset.click();
  await expect.poll(() => commands.length).toBe(2);
});
