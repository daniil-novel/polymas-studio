// Authored read-only visual fixture; no provider or workspace data.
const createdAt = '2026-09-08T12:00:00Z';
const runs = Array.from({ length: 35 }, (_, i) => ({
  id: `run-${String(i).padStart(4, '0')}`,
  title: `Анализ архитектуры · запуск ${i + 1}`,
  createdAt,
  configRevision: 8,
  taskCount: 1017,
  accepted: false,
  stopped: true,
  completion: { type: 'all_required' },
}));
export const taskId = (i: number) =>
  `task_${String(i).padStart(4, '0')}_b5b84700-8706-4a83-b77c-4f35a1cc5368`;

export function historyResponse(url: URL): unknown {
  const path = url.pathname;
  let json: unknown;
  if (path === '/api/state')
    json = {
      config: {
        revision: 8,
        value: {
          schemaVersion: 1,
          id: 'history-visual-fixture',
          name: 'Лаборатория архитектуры',
          bindings: [],
          registry: {
            roles: [],
            agents: [
              {
                id: 'a1',
                name: 'Исследователь распределённых систем',
                roleId: 'r1',
                roleVersion: 1,
                status: 'idle',
              },
            ],
          },
          architecture: {
            preset: 'network',
            maxConcurrency: 1,
            coordinatorAgentId: null,
            memoryEnabled: false,
            edges: [],
            positions: {},
            holons: [],
          },
        },
      },
      connections: [],
      cursor: 0,
      runs: [],
      tasks: [],
      agentActivity: [],
      taskCount: 1017,
      agentRuntime: [],
      paused: true,
      runtime: { ready: false, capacity: 1, probe: null, error: null },
    };
  else if (path === '/api/events') json = { events: [], cursor: 0, head: 0 };
  else if (path === '/api/runs') {
    const filtered = runs.filter((run) => run.title.includes(url.searchParams.get('query') ?? ''));
    const offset = Number(url.searchParams.get('cursor') ?? 0);
    json = {
      items: filtered.slice(offset, offset + 30),
      nextCursor: offset + 30 < filtered.length ? String(offset + 30) : null,
    };
  } else if (path === '/api/runs/run-0030') json = runs[30];
  else if (path === '/api/runs/run-0030/tasks') {
    const offset = Number(url.searchParams.get('cursor') ?? 0);
    json = {
      items: Array.from({ length: Math.max(0, Math.min(30, 1017 - offset)) }, (_, j) => ({
        id: taskId(1016 - offset - j),
        runId: 'run-0030',
        assignee: 'a1',
        state: 'accepted',
        attempt: 1,
        createdAt,
        completedAt: createdAt,
      })),
      nextCursor: offset + 30 < 1017 ? String(offset + 30) : null,
    };
  } else if (path.startsWith('/api/tasks/')) {
    const id = path.split('/').at(-1)!;
    json = {
      id,
      runId: 'run-0030',
      assignee: 'a1',
      state: 'accepted',
      attempt: 1,
      lease: null,
      createdAt,
      completedAt: createdAt,
      artifactIds: [],
      error: null,
      dependencies: [],
      newSession: false,
      prompt: 'Проверь независимость экземпляров и условия восстановления после перезапуска.',
      criteria: 'Подтверди вывод проверяемыми фактами.',
      role: { name: 'Исследователь', version: 1 },
      result: `HISTORY-RESULT ${id}\nКаждый экземпляр сохраняет собственную сессию. Проверены границы доступа и порядок продолжения.`,
    };
  } else throw new Error(`Unexpected history fixture request ${path}`);
  return json;
}
