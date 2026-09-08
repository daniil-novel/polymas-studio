export type Stored<T> = { revision: number; value: T };
export type TaskState =
  | 'queued'
  | 'waiting'
  | 'running'
  | 'stopping'
  | 'awaiting_acceptance'
  | 'accepted'
  | 'rejected'
  | 'failed'
  | 'cancelled'
  | 'unknown';
export interface RoleDraft {
  name: string;
  description: string;
  instruction: string;
  outputContract: string;
  acceptanceCriteria: string;
  escalationRule: string;
  model: string | null;
  sandbox: 'read-only' | 'workspace-write';
  timeoutSeconds: number;
  maxTurns: number;
  maxOutputTokens: number | null;
}
export interface Role extends RoleDraft {
  id: string;
  version: number;
  createdAt: string;
}
export interface Agent {
  id: string;
  name: string;
  roleId: string;
  roleVersion: number;
  workspaceKey: string;
  sessionId: string | null;
  owningHolonId: string | null;
  status: string;
  createdAt: string;
  forkedFrom: string | null;
}
export interface Holon {
  id: string;
  name: string;
  goal: string;
  parentId: string | null;
  members: string[];
  coordinatorAgentId: string | null;
  inputContract: string;
  outputContract: string;
  maxTasks: number;
}
export interface Architecture {
  preset: 'single' | 'independent' | 'network' | 'star' | 'holonic' | 'custom';
  maxConcurrency: number;
  admission: 'queue_allowed' | 'start_all';
  coordinatorAgentId: string | null;
  memoryEnabled: boolean;
  memoryConnectionId: string | null;
  holons: Holon[];
  edges: { from: string; to: string }[];
  positions: Record<string, [number, number]>;
  messageHopLimit: number;
  maxMessagesPerRun: number;
  paused: boolean;
}
export interface Binding {
  id: string;
  connectionId: string;
  subject: { type: 'agent' | 'role'; id: string };
  tools: string[];
  allowWrite: boolean;
  required: boolean;
  namespace: string;
}
export interface Configuration {
  schemaVersion: number;
  id: string;
  name: string;
  createdAt: string;
  registry: { roles: Role[]; agents: Agent[] };
  architecture: Architecture;
  bindings: Binding[];
}
export interface Connection {
  id: string;
  name: string;
  recipe: string;
  transport:
    | { type: 'files'; roots: string[] }
    | { type: 'stdio'; command: string; args: string[] }
    | { type: 'http'; url: string };
  enabled: boolean;
  health: 'unconfigured' | 'ready' | 'degraded' | 'error';
  checkedAt: string | null;
  lastError: string | null;
  authSecretRef: string | null;
  environment: Record<string, string>;
  secretEnvironment: Record<string, string>;
  tools: Tool[];
  toolAllowlist: string[];
  readOnly: boolean;
  timeoutSeconds: number;
}
export interface Tool {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
  readOnly: boolean;
}
export interface RuntimeStatus {
  ready: boolean;
  capacity: number;
  error: string | null;
  probe: {
    path: string;
    version: string;
    compatible: boolean;
    authenticated: boolean;
    authMethod: string;
  } | null;
}
export interface TaskSummary {
  id: string;
  runId: string;
  assignee: string;
  state: TaskState;
  attempt: number;
  lease: {
    epoch: number;
    workerId: string;
    acquiredAt: number;
    heartbeatAt: number;
    pid: number | null;
  } | null;
  createdAt: string;
  completedAt: string | null;
  artifactIds: string[];
  error: string | null;
}
export interface Task extends TaskSummary {
  waiting?: { sender: string | null; correlationId: string | null; deadlineAt: number } | null;
  resumeNote?: string | null;
  prompt: string;
  criteria: string;
  role: Role;
  result: string | null;
  dependencies: string[];
  newSession: boolean;
}
export interface Run {
  completion:
    { type: 'all_required' | 'manual' | 'first_accepted' } | { type: 'quorum'; required: number };
  id: string;
  title: string;
  createdAt: string;
  accepted: boolean;
  stopped: boolean;
  taskIds: string[];
  configRevision: number;
}
export interface AgentRuntime {
  agentId: string;
  sessionId: string | null;
  pid: number | null;
  taskId: string | null;
  usage: Record<string, number> | null;
}
export interface Snapshot {
  config: Stored<Configuration>;
  connections: Stored<Connection>[];
  cursor: number;
  paused: boolean;
  runtime: RuntimeStatus;
  agentRuntime: Stored<AgentRuntime>[];
  runs: Run[];
  tasks: TaskSummary[];
  taskCount: number;
}
export interface RuntimeEvent {
  sequence: number;
  eventId: string;
  observedAt: string;
  kind: string;
  agentId: string | null;
  runId: string | null;
  body: Record<string, unknown>;
}
export const stateLabels: Record<string, string> = {
  idle: 'Готов',
  queued: 'В очереди',
  running: 'Выполняет задачу',
  stopping: 'Останавливается',
  awaiting_acceptance: 'Ожидает приёмки',
  accepted: 'Принят',
  rejected: 'Отклонён',
  failed: 'Ошибка',
  cancelled: 'Отменён',
  unknown: 'Нужна сверка',
  archived: 'Архив',
  waiting: 'Ожидает сообщения',
  stale: 'Нет связи',
};
export function agentState(agent: Agent, tasks: TaskSummary[], connected = true): string {
  if (agent.status === 'archived') return 'archived';
  const current = tasks.find(
    (t) => t.assignee === agent.id && ['running', 'stopping', 'unknown'].includes(t.state),
  );
  if (current) return connected ? current.state : 'stale';
  if (tasks.some((t) => t.assignee === agent.id && t.state === 'queued')) return 'queued';
  if (tasks.some((t) => t.assignee === agent.id && t.state === 'waiting')) return 'waiting';
  return 'idle';
}
