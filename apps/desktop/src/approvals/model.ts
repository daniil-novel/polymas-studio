import type { Connection, Stored } from '../types';
export type ApprovalState =
  | 'pending'
  | 'approved'
  | 'denied'
  | 'revoked'
  | 'expired'
  | 'executing'
  | 'succeeded'
  | 'failed'
  | 'unknown';
export const approvalLabels: Record<ApprovalState, string> = {
  pending: 'Нужно решение',
  approved: 'Разрешено',
  denied: 'Отклонено',
  revoked: 'Отозвано',
  expired: 'Срок истёк',
  executing: 'Выполняется',
  succeeded: 'Выполнено',
  failed: 'Ошибка операции',
  unknown: 'Нужна сверка',
};
export interface ApprovalSummary {
  id: string;
  revision: number;
  state: ApprovalState;
  createdAt: number;
  expiresAt: number;
  agentId: string;
  taskId: string;
  runId: string;
  connectionId: string;
  namespace: string;
  tool: string;
}
export interface Approval {
  id: string;
  fingerprint: string;
  state: ApprovalState;
  createdAt: number;
  expiresAt: number;
  operation: {
    agentId: string;
    taskId: string;
    runId: string;
    connectionId: string;
    connectionRevision: number;
    originEpoch: number;
    epoch: number;
    namespace: string;
    resourceScopeHash: string;
    tool: string;
    arguments: Record<string, unknown>;
  };
}
export interface ApprovalDetail {
  reconciliation?: {
    outcome: 'succeeded' | 'failed';
    note: string;
    author: string;
    observedAt: string;
  } | null;
  approval: Stored<Approval>;
  connection: { name: string; transport: Connection['transport'] } | null;
  canApprove: boolean;
  canDeny: boolean;
  unavailableReason: string | null;
}
