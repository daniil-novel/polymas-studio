import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ArchitectureEditor } from '../../apps/desktop/src/ArchitectureEditor';
import type { Snapshot } from '../../apps/desktop/src/types';
afterEach(cleanup);
function fixture(): Snapshot {
  return {
    config: {
      revision: 8,
      value: {
        registry: {
          agents: [
            { id: 'a1', name: 'Первый', status: 'idle' },
            { id: 'a2', name: 'Второй', status: 'idle' },
          ],
          roles: [],
        },
        architecture: {
          preset: 'network',
          maxConcurrency: 1,
          admission: 'queue_allowed',
          coordinatorAgentId: null,
          memoryEnabled: false,
          memoryConnectionId: null,
          holons: [],
          edges: [],
          positions: {},
          messageHopLimit: 8,
          maxMessagesPerRun: 1000,
          paused: false,
        },
      },
    },
    connections: [
      { revision: 1, value: { id: 'db', name: 'База знаний', enabled: true, health: 'ready' } },
    ],
    runtime: { capacity: 1 },
  } as unknown as Snapshot;
}
test('memory can be enabled without an AI coordinator and saves against the opening revision', async () => {
  const command = vi.fn().mockResolvedValue({ revision: 9 });
  render(<ArchitectureEditor snapshot={fixture()} command={command} onClose={vi.fn()} />);
  const user = userEvent.setup();
  await user.click(screen.getByLabelText('Общая память'));
  await user.selectOptions(screen.getByLabelText('Подключение памяти'), 'db');
  await user.click(screen.getByRole('button', { name: 'Сохранить архитектуру' }));
  expect(command).toHaveBeenCalledWith(
    '/api/configure',
    expect.objectContaining({
      revision: 8,
      command: {
        type: 'set_architecture',
        architecture: expect.objectContaining({
          memoryEnabled: true,
          memoryConnectionId: 'db',
          coordinatorAgentId: null,
        }),
      },
    }),
  );
});
test('C above N and an externally changed revision cannot be saved from the old draft', async () => {
  const snapshot = fixture();
  const command = vi.fn();
  const view = render(
    <ArchitectureEditor snapshot={snapshot} command={command} onClose={vi.fn()} />,
  );
  const user = userEvent.setup();
  await user.clear(screen.getByLabelText('Одновременно работающих процессов, C'));
  await user.type(screen.getByLabelText('Одновременно работающих процессов, C'), '3');
  expect(
    screen.getByRole('button', { name: 'Сохранить архитектуру' }).hasAttribute('disabled'),
  ).toBe(true);
  view.rerender(
    <ArchitectureEditor
      snapshot={{ ...snapshot, config: { ...snapshot.config, revision: 9 } }}
      command={command}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByText(/Архитектура изменилась/)).toBeTruthy();
  expect(command).not.toHaveBeenCalled();
});
