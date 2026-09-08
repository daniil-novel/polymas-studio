import { useState } from 'react';
import type { Agent } from './types';
export function AgentPicker({
  label,
  agents,
  value,
  onChange,
  optional = true,
}: {
  label: string;
  agents: Agent[];
  value: string | null;
  onChange: (id: string | null) => void;
  optional?: boolean;
}) {
  const [query, setQuery] = useState('');
  const matching = agents.filter((a) =>
    `${a.name} ${a.id}`.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
  );
  const items = matching.slice(0, 100);
  const selected = agents.find((a) => a.id === value);
  if (selected && !items.some((a) => a.id === value)) items.unshift(selected);
  return (
    <div className="agent-picker">
      {agents.length > 100 && (
        <label className="small">
          Поиск: {label}
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Имя или ID"
          />
        </label>
      )}
      <label>
        {label}
        <select value={value ?? ''} onChange={(e) => onChange(e.target.value || null)}>
          <option value="">{optional ? 'Не назначен' : 'Выберите агента'}</option>
          {items.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name} · {a.id.slice(-6)}
            </option>
          ))}
        </select>
      </label>
      {matching.length > 100 && (
        <small className="muted">Первые 100 из {matching.length}. Уточните поиск.</small>
      )}
    </div>
  );
}
