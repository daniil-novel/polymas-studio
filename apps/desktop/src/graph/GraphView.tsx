import { useEffect, useMemo, useRef, useState } from 'react';
import { Focus, List, Minus, Network, Plus } from 'lucide-react';
import { stateLabels } from '../types';
import type { GraphProjection, Positions } from './model';
import type { GraphRenderer } from './renderer';
import './graph.css';

interface Props {
  projection: GraphProjection;
  pinned: Positions;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  query: string;
  groupLabels: Record<string, string>;
  onMove?: (id: string, position: [number, number]) => void;
  onUnpin?: (id: string) => void;
  scope?: Set<string>;
}
export function GraphView(props: Props) {
  const { projection, pinned, selectedId, onSelect, query, groupLabels, onMove, onUnpin, scope } =
    props;
  const [mode, setMode] = useState<'graph' | 'list'>('graph');
  const [failure, setFailure] = useState('');
  const [activeOnly, setActiveOnly] = useState(false);
  const [neighbors, setNeighbors] = useState(false);
  const [scrollTop, setScrollTop] = useState(0);
  const container = useRef<HTMLDivElement>(null);
  const renderer = useRef<GraphRenderer | null>(null);
  const latest = useRef(props);
  useEffect(() => {
    latest.current = props;
  });
  const visibleNodes = useMemo(() => {
    const adjacent = new Set([selectedId]);
    if (neighbors && selectedId)
      for (const edge of projection.edges) {
        if (edge.from === selectedId) adjacent.add(edge.to);
        if (edge.to === selectedId) adjacent.add(edge.from);
      }
    const term = query.trim().toLocaleLowerCase();
    return projection.nodes.filter(
      (n) =>
        (!scope || n.kind === 'memory' || scope.has(n.id)) &&
        (!activeOnly || n.active) &&
        (!neighbors || !selectedId || adjacent.has(n.id)) &&
        (!term || `${n.label} ${n.id}`.toLocaleLowerCase().includes(term)),
    );
  }, [projection, activeOnly, neighbors, selectedId, query, scope]);
  const visible = useMemo(() => new Set(visibleNodes.map((n) => n.id)), [visibleNodes]);
  const visibleRef = useRef(visible);
  useEffect(() => {
    visibleRef.current = visible;
  });
  useEffect(() => {
    if (mode !== 'graph' || !container.current) return;
    let cancelled = false;
    const host = container.current;
    if (typeof Worker === 'undefined' || typeof WebGLRenderingContext === 'undefined') {
      setFailure('WebGL недоступен. Используйте список агентов.');
      return;
    }
    void import('./renderer')
      .then(({ GraphRenderer: Renderer }) => {
        if (cancelled) return;
        renderer.current = new Renderer(
          host,
          (id) => latest.current.onSelect(id),
          (id, position) => latest.current.onMove?.(id, position),
        );
        const current = latest.current;
        renderer.current.update(
          current.projection,
          current.pinned,
          current.selectedId,
          visibleRef.current,
          current.groupLabels,
          !!current.onMove,
        );
      })
      .catch(() => {
        if (!cancelled) setFailure('WebGL недоступен. Используйте список агентов.');
      });
    return () => {
      cancelled = true;
      renderer.current?.dispose();
      renderer.current = null;
    };
  }, [mode]);
  useEffect(() => {
    renderer.current?.update(projection, pinned, selectedId, visible, groupLabels, !!onMove);
  }, [projection, pinned, selectedId, visible, groupLabels, onMove]);
  const list = visibleNodes.filter((n) => n.kind === 'agent');
  const start = Math.min(Math.max(0, Math.floor(scrollTop / 54) - 4), Math.max(0, list.length - 1));
  return (
    <div
      className="graph-workbench"
      onKeyDown={(e) => {
        if (e.key === 'Escape') onSelect(null);
      }}
    >
      <div className="graph-controls">
        <div className="segmented" aria-label="Представление агентов">
          <button aria-pressed={mode === 'graph'} onClick={() => setMode('graph')}>
            <Network size={14} />
            Граф
          </button>
          <button aria-pressed={mode === 'list'} onClick={() => setMode('list')}>
            <List size={14} />
            Список
          </button>
        </div>
        <button aria-pressed={activeOnly} onClick={() => setActiveOnly(!activeOnly)}>
          Активные
        </button>
        <button
          aria-pressed={neighbors}
          disabled={!selectedId}
          onClick={() => setNeighbors(!neighbors)}
        >
          Соседи
        </button>
        <span className="small muted">
          {list.length} из {projection.nodes.filter((n) => n.kind === 'agent').length}
        </span>
        {selectedId && pinned[selectedId] && onUnpin && (
          <button onClick={() => onUnpin(selectedId)}>Снять закрепление</button>
        )}
      </div>
      {mode === 'graph' ? (
        <>
          <div
            className="sigma-host"
            ref={container}
            role="img"
            aria-label={`Граф ${list.length} агентов. Для клавиатуры доступен режим Список.`}
          />
          {failure && (
            <div className="graph-fallback" role="status">
              {failure}
              <button onClick={() => setMode('list')}>Открыть список</button>
            </div>
          )}
          {list.length === 0 && (
            <p className="graph-no-results">Нет агентов по выбранным фильтрам</p>
          )}
          <div className="graph-camera">
            <button aria-label="Вписать граф" onClick={() => renderer.current?.zoom(0)}>
              <Focus size={16} />
            </button>
            <button aria-label="Уменьшить граф" onClick={() => renderer.current?.zoom(-1)}>
              <Minus size={16} />
            </button>
            <button aria-label="Увеличить граф" onClick={() => renderer.current?.zoom(1)}>
              <Plus size={16} />
            </button>
          </div>
        </>
      ) : (
        <div
          className="graph-agent-list"
          aria-label="Список узлов"
          onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
        >
          {list.length === 0 ? (
            <p className="muted">Нет агентов по выбранным фильтрам</p>
          ) : (
            <div style={{ height: list.length * 54, position: 'relative' }}>
              {list.slice(start, start + 32).map((node, i) => (
                <button
                  className={`graph-list-row ${node.id === selectedId ? 'selected' : ''}`}
                  key={node.id}
                  style={{ top: (start + i) * 54 }}
                  onClick={() => onSelect(node.id)}
                  aria-pressed={node.id === selectedId}
                >
                  <i className={`dot ${node.active ? '' : 'idle'}`} />
                  <strong>{node.label}</strong>
                  <span>{stateLabels[node.state] ?? node.state}</span>
                  <code>{node.id.slice(-12)}</code>
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
