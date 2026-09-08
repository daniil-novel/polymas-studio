import { expect, test, vi } from 'vitest';
import { GraphLabels } from '../../apps/desktop/src/graph/labels';

test('measured labels cannot overlap even when nodes fall into different grid cells; each frame resets occupancy', () => {
  const fillText = vi.fn();
  const context = {
    measureText: (text: string) => ({ width: text.length * 6 }),
    fillText,
  } as unknown as CanvasRenderingContext2D;
  const labels = new GraphLabels();
  const draw = (label: string, x: number, y: number) =>
    labels.draw(context, { label, x, y, size: 8 }, 'Segoe UI');
  draw('Researcher 05', 20, 30);
  draw('Researcher 03', 60, 32);
  draw('Reviewer', 20, 60);
  expect(fillText.mock.calls.map((c) => c[0])).toEqual(['Researcher 05', 'Reviewer']);
  labels.reset();
  draw('Researcher 03', 60, 32);
  expect(fillText.mock.calls.at(-1)?.[0]).toBe('Researcher 03');
});

test('long labels are bounded and show an ellipsis without changing the graph identity', () => {
  const fillText = vi.fn();
  const context = {
    measureText: (text: string) => ({ width: text.length * 6 }),
    fillText,
  } as unknown as CanvasRenderingContext2D;
  const labels = new GraphLabels();
  const node = { label: 'Длинное имя экземпляра '.repeat(10), x: 0, y: 0, size: 8 };
  labels.draw(context, node, 'Segoe UI');
  const drawn = fillText.mock.calls[0][0] as string;
  expect(drawn.endsWith('…')).toBe(true);
  expect(drawn.length * 6).toBeLessThanOrEqual(220);
  expect(node.label).toHaveLength('Длинное имя экземпляра '.length * 10);
  labels.reset();
  labels.draw(context, { ...node, label: '😀'.repeat(80) }, 'Segoe UI');
  const emoji = fillText.mock.calls.at(-1)?.[0] as string;
  expect([...emoji.slice(0, -1)].every((character) => character === '😀')).toBe(true);
});
