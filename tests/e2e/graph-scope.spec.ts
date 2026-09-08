import { expect, test } from '@playwright/test';

test('selected holon fits its visible nodes without changing saved positions', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.route('**/api/**', async (route) => {
    expect(route.request().method()).toBe('GET');
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/events') return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
    expect(path).toBe('/api/state');
    return route.fulfill({
      json: {
        config: {
          revision: 1,
          value: {
            schemaVersion: 1,
            id: 'camera-fixture',
            name: 'Camera fixture',
            bindings: [],
            registry: {
              roles: [],
              agents: ['Alpha', 'Bravo', 'Outlier'].map((name, i) => ({
                id: `a${i}`,
                name,
                roleId: 'r',
                roleVersion: 1,
                status: 'idle',
              })),
            },
            architecture: {
              preset: 'network',
              maxConcurrency: 1,
              memoryEnabled: false,
              edges: [],
              positions: { a0: [0, 0], a1: [50, 50], a2: [10000, 10000] },
              holons: [
                {
                  id: 'focus',
                  name: 'Focus group',
                  parentId: null,
                  members: ['a0', 'a1'],
                  coordinatorAgentId: null,
                },
              ],
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
      },
    });
  });
  await page.goto('/');
  const labels = page.locator('canvas.sigma-labels');
  await expect(labels).toBeVisible();
  const labelSpread = () =>
    labels.evaluate((canvas: HTMLCanvasElement) => {
      const pixels = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data;
      let min = canvas.height,
        max = -1;
      for (let y = 0; y < canvas.height; y++)
        for (let x = 0; x < canvas.width; x++) {
          if (pixels[(y * canvas.width + x) * 4 + 3] > 0) {
            min = Math.min(min, y);
            max = Math.max(max, y);
            break;
          }
        }
      return max - min;
    });
  // Measure only rendered text pixels. Two distant visible nodes must remain
  // separable after the hidden outlier stops determining the camera extent.
  await expect.poll(labelSpread).toBeGreaterThan(100);
  await page.getByRole('button', { name: 'Focus group 2', exact: true }).click();
  await expect(page.getByRole('img', { name: /Граф 2 агентов/ })).toBeVisible();
  await expect.poll(labelSpread).toBeGreaterThan(100);
  await page.getByRole('button', { name: 'Вписать граф', exact: true }).click();
  await expect.poll(labelSpread).toBeGreaterThan(100);
  await page.getByRole('button', { name: 'Все агенты', exact: true }).click();
  await expect(page.getByRole('img', { name: /Граф 3 агентов/ })).toBeVisible();
  await expect.poll(labelSpread).toBeGreaterThan(100);
});
