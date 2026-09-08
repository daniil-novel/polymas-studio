// Authored instance directory fixture. No provider process, credentials or user data.
import type { Snapshot } from '../../../apps/desktop/src/types';

export function instanceSnapshot(count = 5000): Snapshot {
  const time = '2026-09-08T12:00:00Z';
  const role = {
    id: 'role-research',
    version: 1,
    createdAt: time,
    name: 'Исследователь',
    description: '',
    instruction: 'Проверять источники.',
    outputContract: 'Markdown',
    acceptanceCriteria: 'Сохранены источники',
    escalationRule: 'Обратиться к оператору',
    model: null,
    sandbox: 'read-only' as const,
    timeoutSeconds: 600,
    maxTurns: 20,
    maxOutputTokens: null,
  };
  const agents = Array.from({ length: count }, (_, i) => ({
    id: `agent-directory-${String(i).padStart(4, '0')}`,
    name: `Исследователь · ${String(i + 1).padStart(4, '0')}`,
    roleId: role.id,
    roleVersion: 1,
    workspaceKey: `workspace-${i}`,
    sessionId: null,
    owningHolonId: null,
    status: i === count - 1 ? 'archived' : 'idle',
    createdAt: time,
    forkedFrom: null,
  }));
  return {
    config: {
      revision: 8,
      value: {
        schemaVersion: 1,
        id: 'instance-directory-fixture',
        name: 'Лаборатория агентов',
        createdAt: time,
        registry: { roles: [role], agents },
        bindings: [],
        architecture: {
          preset: 'network',
          maxConcurrency: 1,
          admission: 'queue_allowed',
          paused: true,
          coordinatorAgentId: null,
          memoryEnabled: false,
          memoryConnectionId: null,
          holons: [
            {
              id: 'holon-review',
              name: 'Проверка источников',
              goal: '',
              parentId: null,
              members: [agents[count - 2].id],
              coordinatorAgentId: null,
              inputContract: '',
              outputContract: '',
              maxTasks: 10,
            },
          ],
          edges: [],
          positions: {},
          messageHopLimit: 4,
          maxMessagesPerRun: 100,
        },
      },
    },
    connections: [],
    cursor: 0,
    runs: [],
    tasks: [],
    taskCount: 1001,
    paused: true,
    runtime: { ready: false, capacity: 1, probe: null, error: null },
    agentRuntime: [],
    agentActivity: [
      {
        id: 'task-old-active',
        assignee: agents[count - 2].id,
        state: 'running',
        lease: {
          epoch: 1,
          workerId: 'fixture-worker',
          pid: 1234,
          acquiredAt: 0,
          heartbeatAt: Math.floor(Date.now() / 1000),
        },
      },
    ],
  };
}
