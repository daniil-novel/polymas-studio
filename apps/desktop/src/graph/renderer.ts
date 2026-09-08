import { DirectedGraph } from 'graphology';
import Sigma from 'sigma';
import { IMPULSE_MS, type GraphProjection, type Positions } from './model';

const colors: Record<string, string> = {
  running: '#6DE0B3',
  queued: '#A7B3C4',
  stale: '#6F7D93',
  unknown: '#F2C46D',
  stopping: '#F2C46D',
  failed: '#FF8585',
};
export class GraphRenderer {
  private graph = new DirectedGraph();
  private sigma: Sigma;
  private worker: Worker;
  private version = 0;
  private layoutKey = '';
  private positions: Positions = {};
  private projection: GraphProjection = { nodes: [], edges: [], impulses: [] };
  private selected: string | null = null;
  private visible = new Set<string>();
  private groupLabels: Record<string, string> = {};
  private animation = 0;
  private reduced = window.matchMedia('(prefers-reduced-motion: reduce)');
  private disposed = false;
  private overlay: HTMLCanvasElement;
  private minimap: HTMLCanvasElement;
  constructor(
    container: HTMLElement,
    private onSelect: (id: string | null) => void,
  ) {
    this.sigma = new Sigma(this.graph, container, {
      labelColor: { color: '#E7EDF5' },
      labelFont: 'Inter, Segoe UI, sans-serif',
      labelSize: 12,
      labelWeight: '400',
      labelDensity: 0.5,
      labelRenderedSizeThreshold: 5,
      stagePadding: 65,
      defaultNodeColor: '#6F7D93',
      defaultEdgeColor: '#35413F',
      minCameraRatio: 0.03,
      maxCameraRatio: 5,
      renderEdgeLabels: false,
      zIndex: true,
    });
    this.overlay = document.createElement('canvas');
    this.overlay.className = 'graph-effects';
    this.overlay.setAttribute('aria-hidden', 'true');
    container.appendChild(this.overlay);
    this.minimap = document.createElement('canvas');
    this.minimap.className = 'graph-minimap';
    this.minimap.setAttribute('aria-hidden', 'true');
    container.appendChild(this.minimap);
    this.worker = new Worker(new URL('./layout.worker.ts', import.meta.url), { type: 'module' });
    this.worker.onmessage = (event: MessageEvent<{ version: number; positions: Positions }>) => {
      if (this.disposed || event.data.version !== this.version) return;
      this.positions = event.data.positions;
      this.apply();
    };
    this.sigma.on('clickNode', ({ node }) => this.onSelect(node));
    this.sigma.on('clickStage', () => this.onSelect(null));
    this.sigma.on('afterRender', () => this.draw());
    this.tick();
  }
  update(
    projection: GraphProjection,
    pinned: Positions,
    selected: string | null,
    visible: Set<string>,
    groupLabels: Record<string, string>,
  ) {
    this.projection = projection;
    this.selected = selected;
    this.visible = visible;
    this.groupLabels = groupLabels;
    const key = JSON.stringify([projection.nodes.map((n) => [n.id, n.group]), pinned]);
    if (key !== this.layoutKey) {
      this.layoutKey = key;
      this.worker.postMessage({
        version: ++this.version,
        nodes: projection.nodes,
        pinned: { ...this.positions, ...pinned },
      });
    } else this.apply();
  }
  private apply() {
    const ids = new Set(this.projection.nodes.map((n) => n.id));
    for (const node of this.graph.nodes()) if (!ids.has(node)) this.graph.dropNode(node);
    for (const node of this.projection.nodes) {
      const position = this.positions[node.id];
      if (!position) continue;
      const isSelected = node.id === this.selected;
      const attrs = {
        x: position[0],
        y: position[1],
        label: node.label,
        size:
          node.kind === 'memory'
            ? 13
            : this.projection.nodes.length > 500
              ? 3.5
              : node.coordinator
                ? 10
                : 8,
        color: node.kind === 'memory' ? '#A699F0' : (colors[node.state] ?? '#6F7D93'),
        hidden: !this.visible.has(node.id),
        forceLabel: isSelected || this.projection.nodes.length <= 50,
        highlighted: isSelected,
        zIndex: isSelected ? 2 : node.active ? 1 : 0,
      };
      if (this.graph.hasNode(node.id)) this.graph.mergeNodeAttributes(node.id, attrs);
      else this.graph.addNode(node.id, attrs);
    }
    const edgeIds = new Set(this.projection.edges.map((e) => e.id));
    for (const id of this.graph.edges()) if (!edgeIds.has(id)) this.graph.dropEdge(id);
    for (const edge of this.projection.edges)
      if (this.graph.hasNode(edge.from) && this.graph.hasNode(edge.to)) {
        // The canvas layer draws directional dashed permission edges once per render.
        if (!this.graph.hasEdge(edge.id))
          this.graph.addDirectedEdgeWithKey(edge.id, edge.from, edge.to, { hidden: true });
      }
    this.sigma.refresh();
  }
  private surface(canvas: HTMLCanvasElement, width: number, height: number) {
    const ratio = devicePixelRatio || 1;
    if (
      canvas.width !== Math.round(width * ratio) ||
      canvas.height !== Math.round(height * ratio)
    ) {
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) return null;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    return ctx;
  }
  private draw() {
    if (this.disposed) return;
    const { width, height } = this.sigma.getDimensions();
    const ctx = this.surface(this.overlay, width, height);
    if (!ctx) return;
    const points = new Map<string, { x: number; y: number; size: number }>();
    const groups = new Map<string, { x: number; y: number; size: number }[]>();
    for (const node of this.projection.nodes) {
      if (!this.visible.has(node.id) || !this.graph.hasNode(node.id)) continue;
      const data = this.sigma.getNodeDisplayData(node.id);
      if (!data) continue;
      const point = {
        ...this.sigma.graphToViewport(
          this.graph.getNodeAttributes(node.id) as { x: number; y: number },
        ),
        size: this.sigma.scaleSize(data.size),
      };
      points.set(node.id, point);
      if (node.kind === 'agent') {
        const list = groups.get(node.group) ?? [];
        list.push(point);
        groups.set(node.group, list);
      }
    }
    ctx.lineWidth = 1;
    for (const [group, list] of groups)
      if (list.length > 1) {
        let left = Infinity,
          right = -Infinity,
          top = Infinity,
          bottom = -Infinity;
        for (const p of list) {
          left = Math.min(left, p.x);
          right = Math.max(right, p.x);
          top = Math.min(top, p.y);
          bottom = Math.max(bottom, p.y);
        }
        ctx.strokeStyle = '#45524D';
        ctx.setLineDash([2, 5]);
        ctx.beginPath();
        ctx.ellipse(
          (left + right) / 2,
          (top + bottom) / 2,
          (right - left) / 2 + 45,
          (bottom - top) / 2 + 45,
          0,
          0,
          2 * Math.PI,
        );
        ctx.stroke();
        ctx.fillStyle = '#A7B3C4';
        ctx.font = '12px Segoe UI';
        ctx.textAlign = 'center';
        ctx.fillText(
          `${this.groupLabels[group] ?? 'Агенты'} · ${list.length}`,
          (left + right) / 2,
          top - 55,
        );
      }
    ctx.strokeStyle = '#42554E';
    ctx.setLineDash([3, 5]);
    ctx.lineWidth = 0.8;
    for (const edge of this.projection.edges) {
      const a = points.get(edge.from),
        b = points.get(edge.to);
      if (!a || !b) continue;
      const dx = b.x - a.x,
        dy = b.y - a.y,
        distance = Math.hypot(dx, dy);
      if (distance < 1) continue;
      const ux = dx / distance,
        uy = dy / distance;
      const end = { x: b.x - ux * (b.size + 5), y: b.y - uy * (b.size + 5) };
      ctx.beginPath();
      ctx.moveTo(a.x + ux * (a.size + 4), a.y + uy * (a.size + 4));
      ctx.lineTo(end.x, end.y);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.beginPath();
      ctx.moveTo(end.x - ux * 5 - uy * 3, end.y - uy * 5 + ux * 3);
      ctx.lineTo(end.x, end.y);
      ctx.lineTo(end.x - ux * 5 + uy * 3, end.y - uy * 5 - ux * 3);
      ctx.stroke();
      ctx.setLineDash([3, 5]);
    }
    ctx.setLineDash([]);
    const now = Date.now();
    for (const node of this.projection.nodes) {
      const p = points.get(node.id);
      if (!p) continue;
      if (node.kind === 'memory') {
        ctx.fillStyle = '#211E31';
        ctx.strokeStyle = '#A699F0';
        ctx.beginPath();
        for (let i = 0; i < 6; i++) {
          const a = (i / 6) * Math.PI * 2;
          const x = p.x + Math.cos(a) * (p.size + 3),
            y = p.y + Math.sin(a) * (p.size + 3);
          if (i) ctx.lineTo(x, y);
          else ctx.moveTo(x, y);
        }
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
      }
      if (node.active || node.id === this.selected) {
        ctx.strokeStyle = node.active ? '#6DE0B3' : '#D3DFEA';
        ctx.globalAlpha =
          node.active && !this.reduced.matches
            ? 0.85 + Math.sin((now / 1800) * Math.PI * 2) * 0.15
            : 1;
        ctx.shadowColor = '#6DE0B3';
        ctx.shadowBlur = node.active ? 12 : 0;
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size + 5, 0, Math.PI * 2);
        ctx.stroke();
        if (node.id === this.selected) {
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.size + 9, 0, Math.PI * 2);
          ctx.stroke();
        }
        ctx.shadowBlur = 0;
        ctx.globalAlpha = 1;
      }
    }
    for (const impulse of this.projection.impulses) {
      const a = points.get(impulse.from),
        b = points.get(impulse.to),
        progress = (now - impulse.time) / IMPULSE_MS;
      if (!a || !b || progress < 0 || progress > 1) continue;
      ctx.strokeStyle = '#6DE0B3';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
      if (!this.reduced.matches) {
        ctx.fillStyle = '#B2F4DB';
        ctx.shadowColor = '#6DE0B3';
        ctx.shadowBlur = 12;
        ctx.beginPath();
        ctx.arc(a.x + (b.x - a.x) * progress, a.y + (b.y - a.y) * progress, 3, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;
      }
    }
    this.drawMinimap();
  }
  private drawMinimap() {
    const ctx = this.surface(this.minimap, 144, 88);
    if (!ctx) return;
    const nodes = this.projection.nodes.filter((n) => this.positions[n.id]);
    if (!nodes.length) return;
    let minX = Infinity,
      maxX = -Infinity,
      minY = Infinity,
      maxY = -Infinity;
    for (const n of nodes) {
      const [x, y] = this.positions[n.id];
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    }
    const scale = Math.min(124 / Math.max(1, maxX - minX), 68 / Math.max(1, maxY - minY));
    for (const n of nodes) {
      const [x, y] = this.positions[n.id];
      ctx.fillStyle = n.id === this.selected ? '#E7EDF5' : n.active ? '#6DE0B3' : '#6F7D93';
      ctx.beginPath();
      ctx.arc(
        10 + (x - minX) * scale,
        78 - (y - minY) * scale,
        n.id === this.selected ? 3 : 1.5,
        0,
        Math.PI * 2,
      );
      ctx.fill();
    }
  }
  private tick = () => {
    if (this.disposed) return;
    if (
      !document.hidden &&
      !this.reduced.matches &&
      (this.projection.nodes.some((n) => n.active) ||
        this.projection.impulses.some((e) => Date.now() - e.time < IMPULSE_MS))
    )
      this.draw();
    this.animation = requestAnimationFrame(this.tick);
  };
  zoom(direction: number) {
    if (direction === 0)
      void this.sigma.getCamera().animatedReset({ duration: this.reduced.matches ? 0 : 180 });
    else if (direction > 0) void this.sigma.getCamera().animatedZoom({ duration: 180 });
    else void this.sigma.getCamera().animatedUnzoom({ duration: 180 });
  }
  focus(id: string) {
    const data = this.sigma.getNodeDisplayData(id);
    if (data)
      void this.sigma
        .getCamera()
        .animate(
          { x: data.x, y: data.y, ratio: Math.min(this.sigma.getCamera().ratio, 0.5) },
          { duration: this.reduced.matches ? 0 : 200 },
        );
  }
  dispose() {
    this.disposed = true;
    cancelAnimationFrame(this.animation);
    this.worker.terminate();
    this.sigma.kill();
    this.overlay.remove();
    this.minimap.remove();
  }
}
