import { expect, test } from '@playwright/test';

const operation = {
  id: 'approval-layout-fixture',
  fingerprint: 'f'.repeat(64),
  state: 'revoked',
  createdAt: 100,
  expiresAt: 200,
  operation: {
    agentId: 'agent-layout-fixture',
    taskId: 'task-layout-fixture',
    runId: 'run-layout-fixture',
    connectionId: 'connection-layout-fixture',
    connectionRevision: 1,
    originEpoch: 1,
    epoch: 2,
    namespace: 'fixture',
    resourceScopeHash: 'a'.repeat(64),
    tool: 'write_report',
    arguments: { text: 'Каждый агент сохраняет собственную сессию.' },
  },
};

for (const width of [736, 1280, 1600]) {
  test(`approval review remains readable without horizontal scrolling at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 800 });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    // Isolated authored responses: this test never contacts a live daemon or provider.
    await page.route('**/api/**', async (route) => {
      expect(route.request().method()).toBe('GET');
      const path = new URL(route.request().url()).pathname;
      let json: unknown;
      if (path === '/api/state')
        json = {
          config: {
            revision: 1,
            value: {
              schemaVersion: 1,
              id: 'layout-fixture',
              name: 'Проверка интерфейса',
              registry: { roles: [], agents: [] },
              bindings: [],
              architecture: {
                maxConcurrency: 1,
                holons: [],
                edges: [],
                positions: {},
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
      else if (path === '/api/events') json = { events: [], cursor: 0, head: 0 };
      else if (path === '/api/approvals')
        json = {
          items: [
            {
              id: operation.id,
              revision: 3,
              state: operation.state,
              agentId: operation.operation.agentId,
              namespace: operation.operation.namespace,
              tool: operation.operation.tool,
              expiresAt: operation.expiresAt,
            },
          ],
          nextCursor: null,
        };
      else if (path === `/api/approvals/${operation.id}`)
        json = {
          approval: { revision: 3, value: operation },
          connection: {
            name: 'Архив отчётов',
            transport: { type: 'http', url: 'https://fixture.invalid/mcp' },
          },
          canApprove: false,
          canDeny: false,
          unavailableReason: null,
        };
      else throw new Error(`Unexpected fixture request ${path}`);
      await route.fulfill({ json });
    });
    await page.goto('/');
    await page.getByRole('button', { name: 'Подтверждения', exact: true }).click();
    const card = page.getByRole('region', { name: 'Точная операция' });
    await expect(card).toBeVisible();
    await card.getByText('Данные операции', { exact: true }).click();
    await expect(card.getByText(operation.fingerprint, { exact: true })).toBeVisible();
    const box = await card.boundingBox();
    expect(box!.width).toBeGreaterThanOrEqual(400);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= document.documentElement.clientWidth,
      ),
    ).toBe(true);
    expect(errors).toEqual([]);
  });
}
