import type { Stored } from '../types';
export type Verification = 'unverified' | 'verified' | 'disputed' | 'rejected';
export const verificationLabels: Record<Verification, string> = {
  unverified: 'Не проверено',
  verified: 'Подтверждено',
  disputed: 'Оспорено',
  rejected: 'Отклонено',
};
export type Relation = 'supports' | 'contradicts' | 'derived_from';
export const relationLabels: Record<Relation, string> = {
  supports: 'Поддерживает',
  contradicts: 'Противоречит',
  derived_from: 'Выведено из',
};
export interface Source {
  id: string;
  name: string;
  uri: string;
  version: string;
  namespace: string;
  connectionId: string | null;
  retentionDays: number;
  createdAt: string;
}
export interface Claim {
  id: string;
  assertion: string;
  sourceId: string;
  sourceVersion: string;
  sourceUri: string;
  namespace: string;
  author: string;
  createdAt: number;
  expiresAt: number;
  status: Verification;
  verifiedBy: string | null;
  verifiedAt: number | null;
}
export interface KnowledgeLink {
  id: string;
  from: string;
  to: string;
  relation: Relation;
  namespace: string;
  createdAt: number;
}
export interface Page<T> {
  items: Stored<T>[];
  nextCursor: string | null;
  observedAt: number;
}
export interface ClaimDetail {
  claim: Stored<Claim>;
  source: Stored<Source> | null;
  expired: boolean;
  observedAt: number;
}
export function knowledgePath(
  view: string,
  values: Record<string, string | boolean | undefined> = {},
) {
  const query = new URLSearchParams({ limit: '30' });
  for (const [key, value] of Object.entries(values))
    if (value !== undefined && value !== '') query.set(key, String(value));
  return `/api/knowledge/${view}?${query}`;
}
export function timestamp(value: number | null) {
  return value === null ? '—' : new Date(value * 1000).toLocaleString('ru');
}
