import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { GraphView } from '../../apps/desktop/src/graph/GraphView';
import type { GraphProjection } from '../../apps/desktop/src/graph/model';

afterEach(cleanup);
test('the accessible list remains usable without WebGL and selects exact agent identity', async () => {
  const model: GraphProjection = {
    nodes: Array.from({ length: 5000 }, (_, i) => ({
      id: `a${i}`,
      label: `Агент ${i}`,
      kind: 'agent',
      group: 'r',
      state: 'idle',
      active: false,
      coordinator: false,
    })),
    edges: [],
    impulses: [],
  };
  const select = vi.fn();
  render(
    <GraphView
      projection={model}
      pinned={{}}
      selectedId={null}
      onSelect={select}
      query="Агент 4999"
      groupLabels={{}}
    />,
  );
  await userEvent.click(screen.getByRole('button', { name: 'Список' }));
  await userEvent.click(screen.getByRole('button', { name: /Агент 4999/ }));
  expect(select).toHaveBeenCalledWith('a4999');
  expect(screen.queryByRole('button', { name: /Агент 4998/ })).toBeNull();
});
