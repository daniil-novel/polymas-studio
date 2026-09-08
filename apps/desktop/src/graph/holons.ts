import type { Agent, Holon } from '../types';

export function holonNavigation(holons: Holon[], agents: Agent[]) {
  const active = new Set(agents.filter((a) => a.status !== 'archived').map((a) => a.id));
  const byId = new Map(holons.map((h) => [h.id, h]));
  const children = new Map<string | null, Holon[]>();
  for (const holon of holons) {
    const parent = holon.parentId && byId.has(holon.parentId) ? holon.parentId : null;
    const siblings = children.get(parent) ?? [];
    siblings.push(holon);
    children.set(parent, siblings);
  }
  const rows: { id: string; name: string; depth: number; members: Set<string> }[] = [];
  const seen = new Set<string>();
  const visit = (root: Holon) => {
    const stack = [{ holon: root, depth: 0 }];
    while (stack.length) {
      const { holon, depth } = stack.pop()!;
      if (seen.has(holon.id)) continue;
      seen.add(holon.id);
      rows.push({
        id: holon.id,
        name: holon.name,
        depth,
        members: new Set(holon.members.filter((id) => active.has(id))),
      });
      for (const child of [...(children.get(holon.id) ?? [])].reverse())
        stack.push({ holon: child, depth: depth + 1 });
    }
  };
  for (const root of children.get(null) ?? []) visit(root);
  // Invalid legacy ancestry must not trap navigation in an infinite recursion.
  for (const holon of holons) if (!seen.has(holon.id)) visit(holon);
  const rowById = new Map(rows.map((row) => [row.id, row]));
  for (const row of [...rows].reverse()) {
    const parent = rowById.get(byId.get(row.id)?.parentId ?? '');
    if (parent && parent.depth < row.depth) for (const id of row.members) parent.members.add(id);
  }
  return rows;
}
