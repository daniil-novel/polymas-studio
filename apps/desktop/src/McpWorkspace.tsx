import { useState } from 'react';
import { Database, Folder, Link, Plus, Shield } from 'lucide-react';
import { McpWizard, recipeNames, type Recipe } from './McpWizard';
import type { Command } from './RoleEditor';
import type { Connection, Snapshot, Stored } from './types';
const icons = { files: Folder, neo4j: Database, custom: Link };
export function McpWorkspace({ snapshot, command }: { snapshot: Snapshot; command: Command }) {
  const [wizard, setWizard] = useState<{
    recipe: Recipe;
    existing?: Stored<Connection>;
    key: string;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const open = (recipe: Recipe, existing?: Stored<Connection>) =>
    setWizard({ recipe, existing, key: crypto.randomUUID() });
  async function toggle(stored: Stored<Connection>) {
    if (!stored.value.enabled && stored.value.health !== 'ready') {
      open(stored.value.recipe as Recipe, stored);
      return;
    }
    setBusy(true);
    setError('');
    try {
      await command(`/api/connections/${encodeURIComponent(stored.value.id)}/toggle`, {
        revision: stored.revision,
        enabled: !stored.value.enabled,
      });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Подключение не изменено');
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className={`mcp-workspace ${wizard ? 'with-wizard' : ''}`}>
      <section className="mcp-catalog">
        <h2>Подключения MCP</h2>
        <p className="muted">Инструменты и знания для ваших агентов</p>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        {snapshot.connections.map((stored) => {
          const c = stored.value;
          const recipe = (c.recipe in icons ? c.recipe : 'custom') as Recipe;
          const Icon = icons[recipe];
          return (
            <div
              key={c.id}
              className={`mcp-card ${wizard?.existing?.value.id === c.id ? 'selected' : ''}`}
            >
              <Icon size={27} />
              <button className="connection-name" onClick={() => open(recipe, stored)}>
                <strong>{c.name}</strong>
                <small>
                  {c.transport.type === 'files'
                    ? 'Файлы · чтение'
                    : c.transport.type === 'stdio'
                      ? 'Локальный сервер'
                      : 'HTTPS MCP'}{' '}
                  · {snapshot.config.value.bindings.filter((b) => b.connectionId === c.id).length}{' '}
                  привязок
                </small>
              </button>
              <span className={`mcp-health ${c.health}`}>
                {c.health !== 'ready'
                  ? c.health === 'unconfigured'
                    ? 'Нужна настройка'
                    : 'Нет связи'
                  : c.enabled
                    ? 'Подключено'
                    : 'Проверено · выключено'}
              </span>
              <label className="switch-label">
                <input
                  type="checkbox"
                  aria-label={`Включить ${c.name}`}
                  checked={c.enabled}
                  disabled={busy}
                  onChange={() => void toggle(stored)}
                />
              </label>
            </div>
          );
        })}
        <h3>Добавить источник</h3>
        {(['files', 'neo4j', 'custom'] as const).map((recipe) => {
          const Icon = icons[recipe];
          return (
            <button
              key={recipe}
              className={`mcp-card catalog-option ${wizard?.recipe === recipe && !wizard.existing ? 'selected' : ''}`}
              onClick={() => open(recipe)}
            >
              <Icon size={27} />
              <span>
                <strong>{recipeNames[recipe]}</strong>
                <small>
                  {recipe === 'files'
                    ? 'Папка знаний без ключа'
                    : recipe === 'neo4j'
                      ? 'Графовая база через MCP'
                      : 'База, файловый сервис или другой MCP'}
                </small>
              </span>
              <Plus size={18} />
            </button>
          );
        })}
        <div className="access-explainer">
          <Shield size={22} />
          <h3>Область доступа</h3>
          <p>
            Каждой роли или экземпляру назначаются инструменты и namespace. Отзыв доступа действует
            на новые вызовы сразу.
          </p>
          <p>
            PostgreSQL и удалённые файловые сервисы подключаются через «Свой MCP» с инструкцией их
            издателя.
          </p>
        </div>
      </section>
      {wizard && (
        <McpWizard
          key={wizard.key}
          recipe={wizard.recipe}
          existing={wizard.existing}
          snapshot={snapshot}
          command={command}
          onClose={() => setWizard(null)}
        />
      )}
    </div>
  );
}
