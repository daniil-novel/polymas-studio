import { useState } from 'react';
import type { Role, Snapshot } from './types';
import type { Command } from './RoleEditor';
import type { RefreshWorkspace } from './AgentNameEditor';
import { RoleDuplicationDialog } from './RoleDuplicationDialog';
import {
  saveDuplication,
  clearDuplication,
  readDuplication,
  type PendingDuplication,
} from './role-duplication-recovery';

export interface DuplicationSource {
  roleId: string;
  sourceVersion: number;
}
export function RoleDuplicationActions({
  source,
  snapshot,
  connected,
  refresh,
  command,
  onClose,
  onSaved,
}: {
  source: DuplicationSource | null;
  snapshot: Snapshot;
  connected: boolean;
  refresh: RefreshWorkspace;
  command: Command;
  onClose: () => void;
  onSaved: (role: Role) => void;
}) {
  const workspaceId = snapshot.config.value.id;
  const [recovery, setRecovery] = useState(() => readDuplication(workspaceId));
  const [resuming, setResuming] = useState(false);
  function close() {
    setResuming(false);
    onClose();
  }
  function persist(value: PendingDuplication) {
    try {
      saveDuplication(value);
    } catch {
      throw new Error('Запрос не отправлен: не удалось сохранить параметры для восстановления.');
    }
    setRecovery({ pending: value, error: '' });
  }
  function clear(value: PendingDuplication) {
    clearDuplication(value);
    setRecovery(readDuplication(workspaceId));
  }
  return (
    <>
      {(recovery.pending || recovery.error) && (
        <div className="role-duplication-recovery">
          <button onClick={() => setResuming(true)}>Продолжить дублирование</button>
          {recovery.error && (
            <button onClick={() => setRecovery(readDuplication(workspaceId))}>
              Перечитать локальную запись
            </button>
          )}
        </div>
      )}
      {(source || resuming) && (
        <RoleDuplicationDialog
          source={source}
          snapshot={snapshot}
          connected={connected}
          refresh={refresh}
          command={command}
          pendingRequest={recovery.pending}
          recoveryError={recovery.error}
          persist={persist}
          clear={clear}
          onClose={close}
          onSaved={(role) => {
            close();
            onSaved(role);
          }}
        />
      )}
    </>
  );
}
