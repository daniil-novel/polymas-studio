import { expect, test, vi, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RoleEditor } from '../../apps/desktop/src/RoleEditor';
import type { Snapshot } from '../../apps/desktop/src/types';
afterEach(cleanup);
test('creating instances saves the role first and uses its returned revision and identity', async () => {
  const snapshot = {
    config: {
      revision: 1,
      value: { registry: { roles: [], agents: [] }, architecture: { maxConcurrency: 1 } },
    },
    runtime: { ready: false, capacity: 1, probe: null },
  } as unknown as Snapshot;
  const command = vi
    .fn()
    .mockResolvedValueOnce({ revision: 2, result: { id: 'role-created', version: 1 } })
    .mockResolvedValueOnce({ revision: 3, result: { ids: ['a', 'b', 'c'] } });
  render(<RoleEditor snapshot={snapshot} command={command} />);
  const user = userEvent.setup();
  await user.type(screen.getByLabelText('Название роли'), 'Исследователь');
  await user.type(
    screen.getByLabelText('Ролевая инструкция'),
    'Проверяй факты и сохраняй источники.',
  );
  await user.clear(screen.getByLabelText('Количество экземпляров'));
  await user.type(screen.getByLabelText('Количество экземпляров'), '3');
  await user.click(screen.getByRole('button', { name: 'Создать 3 экземпляра' }));
  expect(await screen.findByText('Создано 3 независимых экземпляра')).toBeTruthy();
  expect(command.mock.calls[0][1].command.type).toBe('create_role');
  expect(command.mock.calls[1][1]).toMatchObject({
    revision: 2,
    command: {
      type: 'create_agents',
      request: { roleId: 'role-created', roleVersion: 1, count: 3 },
    },
  });
});

test('a role changed elsewhere cannot be overwritten by an old open form', async () => {
  const role = {
    id: 'role-a',
    version: 1,
    name: 'Analyst',
    instruction: 'First instruction',
    description: '',
    outputContract: 'markdown',
    acceptanceCriteria: '',
    escalationRule: 'Ask',
    model: null,
    sandbox: 'read-only',
    timeoutSeconds: 600,
    maxTurns: 20,
    maxOutputTokens: null,
    createdAt: '',
  };
  const make = (version: number) =>
    ({
      config: {
        revision: version,
        value: {
          registry: {
            roles: [
              {
                ...role,
                version,
                instruction: version === 1 ? 'First instruction' : 'Changed elsewhere',
              },
            ],
            agents: [],
          },
          architecture: { maxConcurrency: 1 },
        },
      },
      runtime: { ready: false, capacity: 1, probe: null },
    }) as unknown as Snapshot;
  const command = vi.fn();
  const view = render(<RoleEditor snapshot={make(1)} command={command} />);
  const user = userEvent.setup();
  await user.selectOptions(screen.getByLabelText('Выбрать роль'), 'role-a');
  await user.type(screen.getByLabelText('Ролевая инструкция'), ' Local changes');
  view.rerender(<RoleEditor snapshot={make(2)} command={command} />);
  expect(await screen.findByText('Роль изменилась в другом редакторе')).toBeTruthy();
  expect(
    (screen.getByRole('button', { name: 'Сохранить роль' }) as HTMLButtonElement).disabled,
  ).toBe(true);
  expect(command).not.toHaveBeenCalled();
});
