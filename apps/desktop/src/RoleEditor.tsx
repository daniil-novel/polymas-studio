import { useMemo, useState } from 'react';
import { Check, ChevronRight, FileText, Plus, Save, Shield, Terminal, Users } from 'lucide-react';
import type { Role, RoleDraft, Snapshot } from './types';
import type { GraphNode } from './graph/model';
import { AgentDirectory } from './AgentDirectory';
import { RoleComparisonDialog, type RoleComparison } from './RoleComparisonDialog';
import { RoleRestoreDialog } from './RoleRestoreDialog';
import { RoleDuplicationActions } from './RoleDuplicationActions';
import type { RefreshWorkspace } from './AgentNameEditor';
import './roles.css';
export type Command = (path: string, body: unknown) => Promise<unknown>;
const blank: RoleDraft = {
  name: '',
  description: '',
  instruction: '',
  outputContract: 'Markdown: результат, источники, ограничения.',
  acceptanceCriteria: '',
  escalationRule: 'При нехватке данных сообщи об ограничениях оператору.',
  model: null,
  sandbox: 'read-only',
  timeoutSeconds: 600,
  maxTurns: 20,
  maxOutputTokens: null,
};
function formatName(pattern: string, n: number) {
  return pattern.replace(/\{n(?::0([1-8]))?\}/g, (_match, width: string | undefined) =>
    String(n).padStart(Number(width ?? 0), '0'),
  );
}
export function RoleEditor({
  snapshot,
  command,
  nodes = [],
  refresh,
  connected = false,
}: {
  snapshot: Snapshot;
  command: Command;
  nodes?: GraphNode[];
  refresh?: RefreshWorkspace;
  connected?: boolean;
}) {
  const [roleId, setRoleId] = useState('');
  const [baseVersion, setBaseVersion] = useState(0);
  const [draft, setDraft] = useState<RoleDraft>({ ...blank });
  const [count, setCount] = useState(1);
  const [pattern, setPattern] = useState('Агент · {n:02}');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [batchKey, setBatchKey] = useState(() => crypto.randomUUID());
  const [tab, setTab] = useState<'editor' | 'instances'>('editor');
  const [pendingRole, setPendingRole] = useState<string | null>(null);
  const [comparison, setComparison] = useState<RoleComparison | null>(null);
  const [restoring, setRestoring] = useState<{ roleId: string; sourceVersion: number } | null>(
    null,
  );
  const [duplicating, setDuplicating] = useState<{
    roleId: string;
    sourceVersion: number;
    workspaceId: string;
  } | null>(null);
  const latest = useMemo(
    () =>
      Array.from(
        new Map(
          [...snapshot.config.value.registry.roles]
            .sort((a, b) => a.version - b.version)
            .map((r) => [r.id, r]),
        ).values(),
      ),
    [snapshot.config.value.registry.roles],
  );
  const current = latest.find((r) => r.id === roleId);
  const stale = Boolean(current && baseVersion !== current.version);
  const draftOf = (role: Role | undefined): RoleDraft =>
    role
      ? (Object.fromEntries(
          Object.keys(blank).map((key) => [key, role[key as keyof RoleDraft]]),
        ) as unknown as RoleDraft)
      : { ...blank };
  const dirty = JSON.stringify(draft) !== JSON.stringify(draftOf(current));
  const available =
    5000 - snapshot.config.value.registry.agents.filter((a) => a.status !== 'archived').length;
  const patternValid =
    pattern.trim().length > 0 && !/[{}]/.test(pattern.replace(/\{n(?::0([1-8]))?\}/g, ''));
  const valid =
    !!draft.name.trim() &&
    !!draft.instruction.trim() &&
    Number.isInteger(count) &&
    count >= 1 &&
    count <= available &&
    patternValid;
  const plural =
    count === 1
      ? 'экземпляр'
      : count % 10 >= 2 && count % 10 <= 4 && (count % 100 < 12 || count % 100 > 14)
        ? 'экземпляра'
        : 'экземпляров';
  const update = <K extends keyof RoleDraft>(key: K, value: RoleDraft[K]) => {
    setDraft((old) => ({ ...old, [key]: value }));
    setSuccess('');
  };
  function choose(id: string) {
    const role = latest.find((r) => r.id === id);
    setRoleId(id);
    setBaseVersion(role?.version ?? 0);
    setDraft(draftOf(role));
    setPattern(`${role?.name ?? 'Агент'} · {n:02}`);
    setError('');
    setSuccess('');
    setBatchKey(crypto.randomUUID());
  }
  async function save(create: boolean) {
    if (stale) {
      setError('Сохранённая версия роли изменилась');
      return;
    }
    setBusy(true);
    setError('');
    setSuccess('');
    try {
      let revision = snapshot.config.revision;
      let savedRole = current;
      if (!current || dirty) {
        const result = (await command('/api/configure', {
          revision,
          command: current
            ? { type: 'revise_role', roleId: current.id, expectedVersion: current.version, draft }
            : { type: 'create_role', draft },
        })) as { revision: number; result: Role };
        savedRole = result.result;
        revision = result.revision;
        setRoleId(savedRole.id);
        setBaseVersion(savedRole.version);
      }
      if (!savedRole) throw new Error('Сохранённая роль не найдена');
      if (create) {
        await command('/api/configure', {
          revision,
          command: {
            type: 'create_agents',
            key: batchKey,
            request: {
              roleId: savedRole.id,
              roleVersion: savedRole.version,
              count,
              namePattern: pattern,
            },
          },
        });
        setSuccess(
          count === 1 ? 'Создан 1 независимый экземпляр' : `Создано ${count} независимых ${plural}`,
        );
        setBatchKey(crypto.randomUUID());
      } else setSuccess(`Сохранена версия роли v${savedRole.version}`);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось сохранить роль');
    } finally {
      setBusy(false);
    }
  }
  const owned = snapshot.config.value.registry.agents.filter((a) => !roleId || a.roleId === roleId);
  return (
    <div className="roles-workbench">
      <div className="section-heading">
        <div>
          <h2>Роль и независимые экземпляры</h2>
          <p className="muted">
            Одна инструкция. У каждого агента — собственная история и рабочая папка.
          </p>
        </div>
        <button onClick={() => (dirty ? setPendingRole('') : choose(''))}>
          <Plus size={15} />
          Новая роль
        </button>
      </div>
      <div className="role-navigation">
        <button className={tab === 'editor' ? 'active' : ''} onClick={() => setTab('editor')}>
          <FileText size={14} />
          Роль и создание
        </button>
        <button className={tab === 'instances' ? 'active' : ''} onClick={() => setTab('instances')}>
          <Users size={14} />
          Экземпляры <span>{owned.length}</span>
        </button>
        <select
          aria-label="Выбрать роль"
          value={roleId}
          onChange={(e) => (dirty ? setPendingRole(e.target.value) : choose(e.target.value))}
        >
          <option value="">{tab === 'instances' ? 'Все роли' : 'Новая роль'}</option>
          {latest.map((r) => (
            <option value={r.id} key={r.id}>
              {r.name} · v{r.version}
            </option>
          ))}
        </select>
      </div>
      {current && (
        <button
          className="role-compare-button"
          onClick={() =>
            setComparison({
              revision: snapshot.config.revision,
              versions: snapshot.config.value.registry.roles
                .filter((role) => role.id === current.id)
                .map((role) => ({ ...role }))
                .sort((a, b) => b.version - a.version),
            })
          }
        >
          Сравнить версии
        </button>
      )}
      {refresh && (
        <RoleDuplicationActions
          key={`role-duplication:${snapshot.config.value.id}`}
          source={duplicating?.workspaceId === snapshot.config.value.id ? duplicating : null}
          snapshot={snapshot}
          connected={connected}
          refresh={refresh}
          command={command}
          onClose={() => setDuplicating(null)}
          onSaved={(role) =>
            setSuccess(
              `Создана роль «${role.name}» · v1. ID: ${role.id}. Назначения экземпляров сохранены.`,
            )
          }
        />
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {stale && (
        <div className="form-error">
          <p>Сохранённая версия роли изменилась</p>
          <button onClick={() => setPendingRole(roleId)}>Загрузить новую версию</button>
          <p className="small">
            Черновик сохранён в форме. Скопируйте нужные изменения перед обновлением.
          </p>
        </div>
      )}
      {success && (
        <p className="form-success" role="status">
          <Check size={15} />
          {success}
        </p>
      )}
      {tab === 'editor' ? (
        <>
          <div className="role-columns">
            <section className="form-panel">
              <h3>
                <FileText size={17} />
                Ролевая инструкция
              </h3>
              <label>
                Название роли
                <input
                  value={draft.name}
                  onChange={(e) => {
                    update('name', e.target.value);
                    if (!roleId) setPattern(`${e.target.value || 'Агент'} · {n:02}`);
                  }}
                  maxLength={100}
                />
              </label>
              <label>
                Ролевая инструкция
                <textarea
                  value={draft.instruction}
                  onChange={(e) => update('instruction', e.target.value)}
                  rows={9}
                  placeholder={
                    'Опишите назначение агента, способ работы\nи требования к результату…'
                  }
                />
              </label>
              <div className="property-row">
                <span>Исполнитель</span>
                <span className="readonly-field">
                  <Terminal size={14} />
                  Codex CLI
                </span>
              </div>
              <div className="property-row">
                <span>Вход</span>
                <span className="readonly-field">
                  {snapshot.runtime.probe?.authMethod === 'chatgpt'
                    ? 'Подписка ChatGPT'
                    : snapshot.runtime.probe?.authMethod === 'api_key'
                      ? 'API key'
                      : 'Настроить в разделе «Настройки»'}
                </span>
              </div>
              <label className="property-row">
                Модель
                <input
                  value={draft.model ?? ''}
                  onChange={(e) => update('model', e.target.value || null)}
                  placeholder="По умолчанию в Codex"
                />
              </label>
              <details>
                <summary>Формат результата, критерии и ограничения</summary>
                <div className="advanced-fields">
                  <label>
                    Формат результата
                    <textarea
                      rows={2}
                      value={draft.outputContract}
                      onChange={(e) => update('outputContract', e.target.value)}
                    />
                  </label>
                  <label>
                    Критерии приёмки
                    <textarea
                      rows={2}
                      value={draft.acceptanceCriteria}
                      onChange={(e) => update('acceptanceCriteria', e.target.value)}
                      placeholder="По каким признакам результат можно принять?"
                    />
                  </label>
                  <label>
                    Описание
                    <input
                      value={draft.description}
                      onChange={(e) => update('description', e.target.value)}
                    />
                  </label>
                  <label>
                    Правило эскалации
                    <textarea
                      rows={2}
                      value={draft.escalationRule}
                      onChange={(e) => update('escalationRule', e.target.value)}
                    />
                  </label>
                  <label>
                    Доступ к рабочей папке
                    <select
                      value={draft.sandbox}
                      onChange={(e) => update('sandbox', e.target.value as RoleDraft['sandbox'])}
                    >
                      <option value="read-only">Только чтение</option>
                      <option value="workspace-write">Запись в рабочую папку агента</option>
                    </select>
                  </label>
                  <div className="two-fields">
                    <label>
                      Таймаут, секунд
                      <input
                        type="number"
                        min={5}
                        max={86400}
                        value={draft.timeoutSeconds}
                        onChange={(e) => update('timeoutSeconds', Number(e.target.value))}
                      />
                    </label>
                    <label>
                      Максимум попыток
                      <input
                        type="number"
                        min={1}
                        max={1000}
                        value={draft.maxTurns}
                        onChange={(e) => update('maxTurns', Number(e.target.value))}
                      />
                    </label>
                  </div>
                </div>
              </details>
              <p className="policy-note">
                <Shield size={15} />
                Права инструментов задаются отдельно через подключения MCP.
              </p>
            </section>
            <section className="form-panel instances-panel">
              <h3>
                <Users size={18} />
                Экземпляры
              </h3>
              <label>Количество экземпляров</label>
              <div className="population-control">
                <button
                  aria-label="Уменьшить количество"
                  onClick={() => setCount(Math.max(1, count - 1))}
                >
                  −
                </button>
                <input
                  aria-label="Количество экземпляров"
                  type="number"
                  min={1}
                  max={available}
                  value={count || ''}
                  onChange={(e) => setCount(Number(e.target.value))}
                />
                <button
                  aria-label="Увеличить количество"
                  onClick={() => setCount(Math.min(available, count + 1))}
                >
                  +
                </button>
                <div className="population-slider">
                  <input
                    aria-label="Масштаб количества"
                    type="range"
                    min={0}
                    max={1000}
                    value={(Math.log(Math.max(1, count)) / Math.log(5000)) * 1000}
                    onChange={(e) =>
                      setCount(
                        Math.min(
                          available,
                          Math.max(1, Math.round(5000 ** (Number(e.target.value) / 1000))),
                        ),
                      )
                    }
                  />
                  <div>
                    <span>1</span>
                    <span>10</span>
                    <span>100</span>
                    <span>1000</span>
                    <span>5000</span>
                  </div>
                </div>
              </div>
              <p className="small muted">У каждого — свои ID, сессия и процесс при запуске.</p>
              <div className="count-presets">
                {[1, 10, 100, 1000].map((n) => (
                  <button
                    disabled={n > available}
                    className={count === n ? 'selected' : ''}
                    key={n}
                    onClick={() => setCount(n)}
                  >
                    {n}
                  </button>
                ))}
              </div>
              <label>
                Шаблон имени
                <input
                  value={pattern}
                  onChange={(e) => {
                    setPattern(e.target.value);
                    setBatchKey(crypto.randomUUID());
                  }}
                />
              </label>
              {!patternValid && (
                <p className="form-error">
                  Используйте {'{n}'} или {'{n:02}'} для номера.
                </p>
              )}
              <div className="capacity-note">
                <span>Параллелизм системы</span>
                <strong>{snapshot.config.value.architecture.maxConcurrency}</strong>
                <p className="small muted">
                  Сколько экземпляров могут работать одновременно. Меняется в архитектуре системы.
                </p>
              </div>
              <h4>Предпросмотр экземпляров</h4>
              <div className="preview-layout">
                <div>
                  <div className="instance-preview">
                    {Array.from({ length: Math.min(4, Math.max(0, count)) }, (_, i) => (
                      <div key={i}>
                        <i className="dot idle" />
                        <span>{formatName(pattern, i + 1)}</span>
                        <small>Новый ID</small>
                      </div>
                    ))}
                  </div>
                  {count > 4 && (
                    <p className="small muted">+ ещё {(count - 4).toLocaleString('ru')}</p>
                  )}
                </div>
                <div className="independence-illustration" aria-hidden="true">
                  <div className="role-origin">
                    <FileText size={14} />
                    {draft.name || 'Роль'}
                  </div>
                  <div className="instance-branches">
                    {[0, 1, 2, 3].slice(0, Math.min(4, count)).map((n) => (
                      <span key={n}>
                        <Users size={20} />
                        <small>{String(n + 1).padStart(2, '0')}</small>
                      </span>
                    ))}
                  </div>
                </div>
              </div>
              <p className="small muted">
                Будут созданы определения агентов. Запросы к Codex начнутся после отправки задачи.
              </p>
            </section>
          </div>
          <footer className="form-action-bar">
            <span>
              <strong>{count}</strong> {plural} ·{' '}
              {current ? `роль v${current.version}${dirty ? ' → новая версия' : ''}` : 'новая роль'}
              {dirty && <span className="unsaved-dot"> · Не сохранено</span>}
            </span>
            <button
              disabled={stale || busy || !draft.name.trim() || !draft.instruction.trim()}
              onClick={() => void save(false)}
            >
              <Save size={15} />
              Сохранить роль
            </button>
            <button
              className="primary"
              disabled={stale || !valid || busy}
              onClick={() => void save(true)}
            >
              {busy ? 'Сохранение…' : `Создать ${count} ${plural}`}
              <ChevronRight size={16} />
            </button>
          </footer>
        </>
      ) : (
        <AgentDirectory
          key={snapshot.config.value.id}
          snapshot={snapshot}
          roleId={roleId}
          nodes={nodes}
          command={command}
          refresh={refresh}
          connected={connected}
        />
      )}
      {pendingRole !== null && (
        <div className="dialog-backdrop">
          <div className="dialog" role="dialog" aria-modal="true" aria-label="Несохранённая роль">
            <h2>Есть несохранённые изменения</h2>
            <p>Переключение роли заменит текущий черновик.</p>
            <div className="dialog-actions">
              <button autoFocus onClick={() => setPendingRole(null)}>
                Продолжить редактирование
              </button>
              <button
                onClick={() => {
                  choose(pendingRole);
                  setPendingRole(null);
                }}
              >
                Отбросить изменения
              </button>
            </div>
          </div>
        </div>
      )}
      {comparison && (
        <RoleComparisonDialog
          comparison={comparison}
          currentRevision={snapshot.config.revision}
          connected={connected}
          onClose={() => setComparison(null)}
          onDuplicate={
            refresh
              ? (source) => {
                  setComparison(null);
                  setDuplicating({
                    roleId: source.id,
                    sourceVersion: source.version,
                    workspaceId: snapshot.config.value.id,
                  });
                }
              : undefined
          }
          onRestore={
            refresh
              ? (source) => {
                  setComparison(null);
                  setRestoring({ roleId: source.id, sourceVersion: source.version });
                }
              : undefined
          }
        />
      )}
      {restoring && refresh && (
        <RoleRestoreDialog
          {...restoring}
          snapshot={snapshot}
          connected={connected}
          refresh={refresh}
          command={command}
          onClose={() => setRestoring(null)}
          onSaved={(version) => {
            setRestoring(null);
            setSuccess(
              `Восстановленная версия v${version} создана. Назначения экземпляров сохранены.`,
            );
          }}
        />
      )}
    </div>
  );
}
