import { describe, expect, it } from 'vitest';
import single from '../../contracts/examples/single-agent.json';
import thousand from '../../contracts/examples/thousand-agents.json';

describe('accepted architecture fixtures', () => {
  it('does not hide a coordinator in direct mode', () => {
    expect(single.topology.coordinatorPopulationId).toBeNull();
    expect(single.populations.reduce((n, p) => n + p.count, 0)).toBe(1);
  });
  it('preserves the explicit requirement for 1000 simultaneous processes', () => {
    expect(thousand.populations.reduce((n, p) => n + p.count, 0)).toBe(1000);
    expect(thousand.execution.maxConcurrency).toBe(1000);
    expect(thousand.execution.admission).toBe('start_all');
  });
});
