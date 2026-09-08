import { useState } from 'react';
import { Check, Info, LockKeyhole, Plus, Shield, X } from 'lucide-react';
import { AgentPicker } from './AgentPicker';
import type { Command } from './RoleEditor';
import type { Binding, Connection, Snapshot, Stored } from './types';
import './mcp.css';

export type Recipe = 'files' | 'neo4j' | 'custom';
export const recipeNames: Record<Recipe, string> = {
  files: 'Локальные файлы',
  neo4j: 'Neo4j',
  custom: 'Свой MCP-сервер',
};
function createConnection(recipe: Recipe): Connection {
  return {
    id: `connection_${crypto.randomUUID()}`,
    name: recipeNames[recipe],
    recipe,
    transport:
      recipe === 'files'
        ? { type: 'files', roots: [] }
        : {
            type: 'stdio',
            command: '',
            args: recipe === 'neo4j' ? ['-m', 'neo4j_mcp_server'] : [],
          },
    enabled: false,
    health: 'unconfigured',
    checkedAt: null,
    lastError: null,
    authSecretRef: null,
    environment:
      recipe === 'neo4j'
        ? {
            NEO4J_URI: '',
            NEO4J_DATABASE: 'neo4j',
            NEO4J_READ_ONLY: 'true',
            NEO4J_TELEMETRY: 'false',
          }
        : {},
    secretEnvironment: {},
    tools: [],
    toolAllowlist: [],
    readOnly: true,
    timeoutSeconds: 30,
  };
}
function parseEnvironment(value: string): Record<string, string> {
  const parsed: unknown = JSON.parse(value);
  if (
    !parsed ||
    Array.isArray(parsed) ||
    typeof parsed !== 'object' ||
    Object.values(parsed).some((v) => typeof v !== 'string')
  )
    throw new Error('Параметры окружения: JSON-объект со строковыми значениями');
  return parsed as Record<string, string>;
}
export function McpWizard({
  recipe,
  existing,
  snapshot,
  command,
  onClose,
}: {
  recipe: Recipe;
  existing?: Stored<Connection>;
  snapshot: Snapshot;
  command: Command;
  onClose: () => void;
}) {
  const [connection, setConnection] = useState<Connection>(() =>
    structuredClone(existing?.value ?? createConnection(recipe)),
  );
  const [revision, setRevision] = useState(existing?.revision ?? 0);
  const [dirty, setDirty] = useState(!existing);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [stage, setStage] = useState(1);
  const [roots, setRoots] = useState(
    existing?.value.transport.type === 'files' ? existing.value.transport.roots.join('\n') : '',
  );
  const [args, setArgs] = useState(
    JSON.stringify(connection.transport.type === 'stdio' ? connection.transport.args : [], null, 2),
  );
  const [environment, setEnvironment] = useState(JSON.stringify(connection.environment, null, 2));
  const [bearer, setBearer] = useState('');
  const [secretValues, setSecretValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      [
        ...new Set([
          ...Object.keys(connection.secretEnvironment),
          ...(recipe === 'neo4j' ? ['NEO4J_USERNAME', 'NEO4J_PASSWORD'] : []),
        ]),
      ].map((name) => [name, '']),
    ),
  );
  const [secretName, setSecretName] = useState('');
  const [selectedTools, setSelectedTools] = useState(connection.toolAllowlist);
  const [subjectType, setSubjectType] = useState<'none' | 'role' | 'agent' | 'holon'>('none');
  const [subjectId, setSubjectId] = useState('');
  const [namespace, setNamespace] = useState('shared');
  const [required, setRequired] = useState(false);
  const [allowWrite, setAllowWrite] = useState(false);
  const stale =
    !busy &&
    (snapshot.connections.find((c) => c.value.id === connection.id)?.revision ?? revision) >
      revision;
  const set = (patch: Partial<Connection>) => {
    setConnection((c) => ({ ...c, ...patch }));
    setDirty(true);
  };
  const latestRoles = [
    ...new Map(snapshot.config.value.registry.roles.map((r) => [r.id, r])).values(),
  ];
  const bindings = snapshot.config.value.bindings.filter((b) => b.connectionId === connection.id);
  const transport = connection.transport;
  async function probe() {
    setBusy(true);
    setError('');
    try {
      let current = connection;
      let currentRevision = revision;
      if (dirty || !currentRevision) {
        const candidate = { ...connection, secretEnvironment: { ...connection.secretEnvironment } };
        if (transport.type === 'files')
          candidate.transport = {
            type: 'files',
            roots: roots
              .split('\n')
              .map((s) => s.trim())
              .filter(Boolean),
          };
        if (transport.type === 'stdio') {
          const parsed: unknown = JSON.parse(args);
          if (!Array.isArray(parsed) || parsed.some((v) => typeof v !== 'string'))
            throw new Error('Аргументы должны быть JSON-массивом строк');
          candidate.transport = { ...transport, args: parsed as string[] };
          candidate.environment =
            recipe === 'neo4j'
              ? { ...connection.environment, NEO4J_READ_ONLY: String(connection.readOnly) }
              : parseEnvironment(environment);
        }
        if (bearer) {
          const saved = (await command('/api/secrets/save', { value: bearer })) as {
            reference: string;
          };
          candidate.authSecretRef = saved.reference;
          setBearer('');
        }
        for (const [name, value] of Object.entries(secretValues))
          if (value) {
            if (!/^[A-Z_][A-Z0-9_]*$/.test(name))
              throw new Error('Имя защищённой переменной: латинские заглавные буквы, цифры, _');
            const saved = (await command('/api/secrets/save', { value })) as { reference: string };
            candidate.secretEnvironment[name] = saved.reference;
            setSecretValues((old) => ({ ...old, [name]: '' }));
          }
        setConnection(candidate);
        const saved = (await command('/api/connections/save', {
          revision: currentRevision,
          connection: candidate,
        })) as Stored<Connection>;
        current = saved.value;
        currentRevision = saved.revision;
        setConnection(current);
        setRevision(currentRevision);
        setDirty(false);
      }
      const result = (await command(`/api/connections/${encodeURIComponent(current.id)}/probe`, {
        revision: currentRevision,
      })) as { connection: Stored<Connection>; protocol?: string };
      setConnection(result.connection.value);
      setRevision(result.connection.revision);
      setStage(3);
      setSelectedTools(
        result.connection.value.toolAllowlist.length
          ? result.connection.value.toolAllowlist
          : result.connection.value.tools.filter((t) => t.readOnly).map((t) => t.name),
      );
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Подключение не проверено');
    } finally {
      setBusy(false);
    }
  }
  async function enable() {
    setBusy(true);
    setError('');
    try {
      const saved = (await command(`/api/connections/${encodeURIComponent(connection.id)}/tools`, {
        revision,
        tools: selectedTools,
      })) as Stored<Connection>;
      setConnection(saved.value);
      setRevision(saved.revision);
      if (subjectType !== 'none') {
        if (!subjectId) throw new Error('Выберите участника для доступа');
        const subjects: Binding['subject'][] =
          subjectType === 'holon'
            ? (
                snapshot.config.value.architecture.holons.find((h) => h.id === subjectId)
                  ?.members ?? []
              ).map((id) => ({ type: 'agent' as const, id }))
            : [{ type: subjectType, id: subjectId }];
        if (!subjects.length) throw new Error('В выбранном холоне нет непосредственных участников');
        const updated = snapshot.config.value.bindings.filter(
          (b) =>
            !(
              b.connectionId === connection.id &&
              subjects.some((s) => s.type === b.subject.type && s.id === b.subject.id)
            ),
        );
        updated.push(
          ...subjects.map((subject) => ({
            id: `binding_${crypto.randomUUID()}`,
            connectionId: connection.id,
            subject,
            tools: selectedTools,
            allowWrite,
            required,
            namespace,
          })),
        );
        await command('/api/configure', {
          revision: snapshot.config.revision,
          command: { type: 'set_bindings', bindings: updated },
        });
      }
      const enabled = (await command(
        `/api/connections/${encodeURIComponent(connection.id)}/toggle`,
        { revision: saved.revision, enabled: true },
      )) as Stored<Connection>;
      setConnection(enabled.value);
      setRevision(enabled.revision);
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Не удалось включить подключение');
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="mcp-wizard" aria-label="Мастер подключения MCP">
      <header className="modal-heading">
        <div>
          <h2>
            {existing ? 'Настроить' : 'Подключить'} {recipeNames[recipe]}
          </h2>
          <p className="muted">
            {recipe === 'neo4j'
              ? 'Графовая память и инструменты базы'
              : recipe === 'files'
                ? 'Материалы в разрешённых папках'
                : 'Инструменты внешнего сервера'}
          </p>
        </div>
        <button
          className="icon-button"
          aria-label="Закрыть мастер MCP"
          disabled={busy}
          onClick={onClose}
        >
          <X size={19} />
        </button>
      </header>
      <nav className="wizard-steps" aria-label="Шаги настройки">
        {['Сервер', 'Доступ', 'Проверка'].map((label, i) => (
          <button
            key={label}
            aria-current={stage === i + 1 ? 'step' : undefined}
            onClick={() => setStage(i + 1)}
          >
            <span>{i + 1}</span>
            {label}
          </button>
        ))}
      </nav>
      <div className="setup-guide">
        <Info size={18} />
        <div>
          <strong>Как подготовить подключение</strong>
          {recipe === 'files' ? (
            <ol>
              <li>Создайте отдельную папку с материалами для агентов.</li>
              <li>Скопируйте её полный путь из проводника. Ключ не нужен.</li>
              <li>Проверьте папку, выберите инструменты и назначьте участников.</li>
            </ol>
          ) : recipe === 'neo4j' ? (
            <>
              <ol>
                <li>В Neo4j Console откройте Instances и сведения выбранной базы.</li>
                <li>
                  Для HTTPS возьмите MCP endpoint. Для локального сервера укажите Python с
                  установленным neo4j-mcp-server и адрес базы.
                </li>
                <li>
                  Создайте отдельную учётную запись с правами чтения. Значения доступа внесите в
                  защищённые поля.
                </li>
              </ol>
              <a
                href="https://neo4j.com/docs/mcp/current/quickstart/"
                target="_blank"
                rel="noreferrer"
              >
                Официальная инструкция Neo4j ↗
              </a>
            </>
          ) : (
            <ol>
              <li>Выберите MCP-сервер у его издателя и установите проверенную версию.</li>
              <li>Укажите установленный executable с аргументами или HTTPS endpoint.</li>
              <li>Секреты добавьте через защищённые поля; затем проверьте инструменты.</li>
            </ol>
          )}
        </div>
      </div>
      <div hidden={stage !== 1} className="wizard-fields">
        <label>
          Название подключения
          <input
            value={connection.name}
            maxLength={100}
            onChange={(e) => set({ name: e.target.value })}
          />
        </label>
        {recipe !== 'files' && (
          <div className="segmented transport-tabs">
            <button
              aria-pressed={transport.type === 'stdio'}
              onClick={() =>
                set({
                  transport: {
                    type: 'stdio',
                    command: '',
                    args: recipe === 'neo4j' ? ['-m', 'neo4j_mcp_server'] : [],
                  },
                })
              }
            >
              Локальный сервер
            </button>
            <button
              aria-pressed={transport.type === 'http'}
              onClick={() => set({ transport: { type: 'http', url: '' } })}
            >
              HTTPS MCP
            </button>
          </div>
        )}
        {transport.type === 'files' && (
          <label>
            Разрешённые папки
            <textarea
              aria-label="Разрешённые папки"
              rows={4}
              value={roots}
              onChange={(e) => {
                setRoots(e.target.value);
                setDirty(true);
              }}
              placeholder="Полный путь, каждая папка с новой строки"
            />
            <small>Чтение UTF-8 до 4 МиБ. Выход за выбранные папки запрещён.</small>
          </label>
        )}
        {transport.type === 'stdio' && (
          <>
            <label>
              {recipe === 'neo4j' ? 'Полный путь к Python' : 'Полный путь к исполняемому файлу'}
              <input
                value={transport.command}
                onChange={(e) => set({ transport: { ...transport, command: e.target.value } })}
                placeholder="C:\…\python.exe или /…/bin/python"
              />
              <small>Установленная программа. Загрузка пакетов при проверке не выполняется.</small>
            </label>
            <label>
              Аргументы запуска · JSON
              <textarea
                rows={2}
                value={args}
                onChange={(e) => {
                  setArgs(e.target.value);
                  setDirty(true);
                }}
              />
            </label>
          </>
        )}
        {transport.type === 'http' && (
          <label>
            HTTPS MCP endpoint
            <input
              value={transport.url}
              onChange={(e) => set({ transport: { ...transport, url: e.target.value } })}
              placeholder="https://…/mcp"
            />
            <small>Адрес MCP-сервера. Строка подключения самой базы сюда не подходит.</small>
          </label>
        )}
        <label>
          Таймаут подключения, секунд
          <input
            type="number"
            min={1}
            max={300}
            value={connection.timeoutSeconds}
            onChange={(e) => set({ timeoutSeconds: Number(e.target.value) })}
          />
        </label>
      </div>
      <div hidden={stage !== 2} className="wizard-fields">
        {recipe === 'neo4j' && transport.type === 'stdio' && (
          <>
            <label>
              Адрес базы Neo4j
              <input
                value={connection.environment.NEO4J_URI ?? ''}
                onChange={(e) =>
                  set({ environment: { ...connection.environment, NEO4J_URI: e.target.value } })
                }
                placeholder="neo4j+s://… или bolt://localhost:7687"
              />
            </label>
            <label>
              Имя базы
              <input
                value={connection.environment.NEO4J_DATABASE ?? 'neo4j'}
                onChange={(e) =>
                  set({
                    environment: { ...connection.environment, NEO4J_DATABASE: e.target.value },
                  })
                }
              />
            </label>
          </>
        )}
        {transport.type === 'http' && (
          <label>
            Bearer token
            <input
              type="password"
              autoComplete="new-password"
              value={bearer}
              onChange={(e) => {
                setBearer(e.target.value);
                setDirty(true);
              }}
              placeholder={
                connection.authSecretRef
                  ? 'Сохранён в защищённом хранилище'
                  : 'Если сервер требует bearer авторизацию'
              }
            />
            <small>
              Выдаётся MCP-сервером. Пароль базы не подставляется как token. OAuth настраивается
              отдельным потоком.
            </small>
          </label>
        )}
        {transport.type === 'stdio' && (
          <>
            {recipe !== 'neo4j' && (
              <label>
                Параметры окружения · JSON
                <textarea
                  rows={3}
                  value={environment}
                  onChange={(e) => {
                    setEnvironment(e.target.value);
                    setDirty(true);
                  }}
                />
                <small>Только несекретные значения. Ключи и пароли — ниже.</small>
              </label>
            )}
            {Object.entries(secretValues).map(([name, value]) => (
              <label key={name}>
                {name === 'NEO4J_USERNAME'
                  ? 'Имя пользователя'
                  : name === 'NEO4J_PASSWORD'
                    ? 'Пароль'
                    : name}
                <input
                  type="password"
                  autoComplete="new-password"
                  value={value}
                  onChange={(e) => {
                    setSecretValues((old) => ({ ...old, [name]: e.target.value }));
                    setDirty(true);
                  }}
                  placeholder={
                    connection.secretEnvironment[name]
                      ? 'Сохранён в защищённом хранилище'
                      : 'Защищённое значение'
                  }
                />
              </label>
            ))}
            {recipe === 'custom' && (
              <div className="secret-add">
                <input
                  aria-label="Имя защищённой переменной"
                  value={secretName}
                  onChange={(e) => setSecretName(e.target.value)}
                  placeholder="API_KEY"
                />
                <button
                  disabled={!/^[A-Z_][A-Z0-9_]*$/.test(secretName)}
                  onClick={() => {
                    setSecretValues((old) => ({ ...old, [secretName]: '' }));
                    setSecretName('');
                  }}
                >
                  <Plus size={14} />
                  Поле секрета
                </button>
              </div>
            )}
          </>
        )}
        <div className="vault-note">
          <LockKeyhole size={14} />
          Секреты сохраняются в системном хранилище при проверке.
        </div>
        <label className="switch-label">
          <Shield size={17} />
          <span>Только чтение</span>
          <input
            type="checkbox"
            checked={connection.readOnly}
            disabled={recipe === 'files'}
            onChange={(e) => set({ readOnly: e.target.checked })}
          />
        </label>
        <p className="small muted">
          Права базы и инструменты проверяются отдельно. Неизвестные инструменты требуют разрешения
          на запись и подтверждения операции.
        </p>
      </div>
      <div hidden={stage !== 3} className="wizard-fields">
        <div className={`probe-result ${connection.health === 'ready' && !dirty ? 'ready' : ''}`}>
          <Check size={17} />
          <div>
            <strong>
              {dirty
                ? 'Настройки изменены — проверьте подключение'
                : connection.health === 'ready'
                  ? 'Проверка успешна'
                  : 'Проверка ещё не пройдена'}
            </strong>
            {connection.checkedAt && (
              <small>{new Date(connection.checkedAt).toLocaleString('ru')}</small>
            )}
          </div>
        </div>
        {connection.lastError && (
          <p className="form-error" role="alert">
            {connection.lastError}
          </p>
        )}
        <div className="tool-checklist">
          {connection.tools.map((tool) => (
            <label key={tool.name}>
              <input
                type="checkbox"
                checked={selectedTools.includes(tool.name)}
                disabled={connection.readOnly && !tool.readOnly}
                onChange={(e) =>
                  setSelectedTools((old) =>
                    e.target.checked ? [...old, tool.name] : old.filter((n) => n !== tool.name),
                  )
                }
              />
              <span>
                <strong>{tool.name}</strong>
                <small>{tool.description}</small>
              </span>
              <small>{tool.readOnly ? 'Чтение' : 'Запрос разрешения'}</small>
            </label>
          ))}
        </div>
        <h3>Доступ участников</h3>
        <div className="two-fields">
          <label>
            Кому выдать доступ
            <select
              value={subjectType}
              onChange={(e) => {
                setSubjectType(e.target.value as typeof subjectType);
                setSubjectId('');
              }}
            >
              <option value="none">Пока никому</option>
              <option value="role">Всем экземплярам роли</option>
              <option value="agent">Отдельному агенту</option>
              <option value="holon">Текущим участникам холона</option>
            </select>
          </label>
          <label>
            Namespace
            <input
              value={namespace}
              maxLength={120}
              onChange={(e) => setNamespace(e.target.value)}
            />
          </label>
        </div>
        {subjectType === 'role' && (
          <label>
            Роль с доступом
            <select value={subjectId} onChange={(e) => setSubjectId(e.target.value)}>
              <option value="">Выберите роль</option>
              {latestRoles.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {subjectType === 'agent' && (
          <AgentPicker
            label="Агент с доступом"
            agents={snapshot.config.value.registry.agents.filter((a) => a.status !== 'archived')}
            value={subjectId}
            onChange={(id) => setSubjectId(id ?? '')}
          />
        )}
        {subjectType === 'holon' && (
          <label>
            Холон с доступом
            <select value={subjectId} onChange={(e) => setSubjectId(e.target.value)}>
              <option value="">Выберите холон</option>
              {snapshot.config.value.architecture.holons.map((h) => (
                <option key={h.id} value={h.id}>
                  {h.name}
                </option>
              ))}
            </select>
            <small>
              Привязки создаются для текущего состава. Новые участники не получают доступ
              автоматически.
            </small>
          </label>
        )}
        {subjectType !== 'none' && (
          <>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={required}
                onChange={(e) => setRequired(e.target.checked)}
              />
              Обязательно для запуска этих агентов
            </label>
            {!connection.readOnly && (
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={allowWrite}
                  onChange={(e) => setAllowWrite(e.target.checked)}
                />
                Разрешить запрашивать операции записи
              </label>
            )}
          </>
        )}
        <p className="small muted">
          Действующих привязок: {bindings.length}. Включение источника само по себе не выдаёт доступ
          агентам. Новые права применяются к новым запускам.
        </p>
      </div>
      {stale && (
        <p className="form-error" role="alert">
          Подключение изменилось. Закройте мастер и откройте текущую версию.
        </p>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <footer className="wizard-footer">
        <button disabled={busy || stale || !connection.name.trim()} onClick={() => void probe()}>
          {busy ? 'Выполняется…' : 'Проверить подключение'}
        </button>
        <button
          className="primary"
          disabled={
            busy ||
            stale ||
            dirty ||
            connection.health !== 'ready' ||
            selectedTools.length === 0 ||
            (subjectType !== 'none' && !subjectId)
          }
          onClick={() => void enable()}
        >
          Включить подключение
        </button>
      </footer>
    </section>
  );
}
