import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { McpWizard } from '../../apps/desktop/src/McpWizard';
import type { Snapshot } from '../../apps/desktop/src/types';
afterEach(cleanup);
const snapshot = {
  config: {
    revision: 1,
    value: { bindings: [], registry: { agents: [], roles: [] }, architecture: { holons: [] } },
  },
  connections: [],
} as unknown as Snapshot;
test('an untested source cannot be enabled and a failed probe keeps the switch unavailable', async () => {
  const command = vi
    .fn()
    .mockImplementation(async (path, body) =>
      path.endsWith('/save')
        ? { revision: 1, value: { ...body.connection, health: 'unconfigured' } }
        : {
            connection: {
              revision: 2,
              value: {
                id: 'c',
                name: 'Files',
                transport: { type: 'files', roots: ['E:\\knowledge'] },
                recipe: 'files',
                health: 'error',
                enabled: false,
                tools: [],
                toolAllowlist: [],
                environment: {},
                secretEnvironment: {},
                authSecretRef: null,
                readOnly: true,
                timeoutSeconds: 30,
                lastError: 'Папка недоступна',
              },
            },
          },
    );
  render(<McpWizard recipe="files" snapshot={snapshot} command={command} onClose={vi.fn()} />);
  expect(
    screen.getByRole('button', { name: 'Включить подключение' }).hasAttribute('disabled'),
  ).toBe(true);
  await userEvent.type(screen.getByLabelText('Разрешённые папки'), 'E:\\knowledge');
  await userEvent.click(screen.getByRole('button', { name: 'Проверить подключение' }));
  expect(await screen.findByText('Папка недоступна')).toBeTruthy();
  expect(
    screen.getByRole('button', { name: 'Включить подключение' }).hasAttribute('disabled'),
  ).toBe(true);
  expect(command.mock.calls.some(([path]) => path.endsWith('/toggle'))).toBe(false);
});
