import { useRef, useState } from 'react';
import type { Snapshot } from '../types';
import type { Command } from '../RoleEditor';

export function useGraphPositions(
  snapshot: Snapshot | null,
  connected: boolean,
  command: Command,
  refresh: () => Promise<unknown>,
) {
  const [pending, setPending] = useState<{ id: string; position: [number, number] | null } | null>(
    null,
  );
  const [error, setError] = useState('');
  const [refreshing, setRefreshing] = useState(false);
  const lock = useRef(false);
  const pins = { ...snapshot?.config.value.architecture.positions };
  if (pending?.position) pins[pending.id] = pending.position;
  else if (pending) delete pins[pending.id];
  const canEdit = connected && !!snapshot && !pending && !refreshing && !error;
  const move = async (id: string, position: [number, number] | null) => {
    if (!canEdit || lock.current || !snapshot) return;
    lock.current = true;
    setPending({ id, position });
    try {
      await command('/api/configure', {
        revision: snapshot.config.revision,
        command: { type: 'set_node_position', nodeId: id, position },
      });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось сохранить положение');
    } finally {
      lock.current = false;
      setPending(null);
    }
  };
  const reconcile = async () => {
    if (lock.current) return;
    lock.current = true;
    setRefreshing(true);
    try {
      await refresh();
      setError('');
    } catch {
      setError('Не удалось обновить положение узлов. Проверьте связь со службой.');
    } finally {
      lock.current = false;
      setRefreshing(false);
    }
  };
  return { pins, pending, refreshing, error, canEdit, move, reconcile };
}
