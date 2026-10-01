// The map of a process (GET /api/map) as the page draws it: articles folded
// per law unless the law is opened, the chain of one event lit, and a fixed
// column per kind (configuration left, the law right). Pure functions; the
// view hands the result to vue-flow.

// The process stands alone in the first column, so its edges to channels,
// roles and actions run to the right like every other edge.
export const COLUMN = {
  process: 0,
  channel: 1, role: 1, action: 1,
  cell: 2, stream: 2, register: 2, source_cell: 2,
  event: 3,
  lexostatus: 4,
  article: 5, law: 5,
};
export const COL_W = 300;
export const ROW_H = 72;

// What a kind of node is called on the page.
export const KIND_TEXT = {
  process: 'proces', channel: 'kanaal', role: 'rol', action: 'handeling',
  cell: 'cel', stream: 'stroom', register: 'register', source_cell: 'broncel',
  event: 'event', lexostatus: 'lexostatus', article: 'artikel', law: 'wet',
};

// What a kind of edge is called on the page.
export const EDGE_TEXT = {
  channel: 'kanaal', role: 'rol', portal: 'portaal', action: 'handeling',
  legal_basis: 'grondslag', records: 'legt vast', establishes: 'vestigt',
  hook: 'haakt in', extends: 'vult aan', origin: 'herkomst',
  decides_on: 'besluit op', prefill: 'vult voor', source: 'bron',
  reads: 'leest', executes: 'voert uit', synthesis: 'synthese',
  rows: 'rijen', register: 'register',
};

/** The text of a node in the graph: its kind and its name; a law with its count. */
export function nodeText(n) {
  if (n.kind === 'law') return `wet ${n.label} (${n.count})`;
  if (n.kind === 'article') return `${n.regulation} art. ${n.label}`;
  return `${KIND_TEXT[n.kind] ?? n.kind}: ${n.label}`;
}

// The id of the node that stands for a law when it is folded.
export const lawId = (regulation) => `law:${regulation}`;

// The identity of an edge as it is shown: two edges of one kind between the
// same shown nodes are one.
export const edgeKey = (from, to, kind) => `${from}|${to}|${kind}`;

// The id a node is shown under: itself, or the folded law of its article.
function shownIdOf(map, opened) {
  const regulation = new Map(map.nodes.filter((n) => n.kind === 'article').map((n) => [n.id, n.regulation]));
  return (id) => {
    const r = regulation.get(id);
    return r !== undefined && !opened.has(r) ? lawId(r) : id;
  };
}

/** Articles of a law that is not in `opened` become one node `law:<regulation>`. */
export function collapse(map, opened) {
  const shown = shownIdOf(map, opened);
  const laws = new Map();
  const nodes = [];
  for (const n of map.nodes) {
    const id = shown(n.id);
    if (id === n.id) {
      nodes.push(n);
      continue;
    }
    const law = laws.get(id) ?? { id, kind: 'law', label: n.regulation, regulation: n.regulation, count: 0 };
    law.count += 1;
    laws.set(id, law);
  }
  const seen = new Set();
  const edges = [];
  for (const e of map.edges) {
    const from = shown(e.from);
    const to = shown(e.to);
    const key = edgeKey(from, to, e.kind);
    if (from === to || seen.has(key)) continue;
    seen.add(key);
    edges.push({ from, to, kind: e.kind });
  }
  return { process: map.process, nodes: [...nodes, ...laws.values()], edges };
}

/**
 * The chain of one event: `{nodes: Set<id>, edges: Set<index in map.edges>}`,
 * or null for no filter.
 *
 * 1. Every edge of the event itself, in either direction (process, stream,
 *    the lexostatuses that read it, the articles it establishes, hooks,
 *    extends, its origin).
 * 2. From the articles of step 1, `decides_on` transitively, each direction
 *    on its own: who decides on such an article (Wpp 107 on 102), and what
 *    such an article decides on. Never back and forth, so two decisions on
 *    one article do not light each other's other subjects.
 * 3. From the articles of steps 1 and 2, `source` and `prefill` one step
 *    outward (the article is `from`). A shared source is lit but never
 *    followed further, so it does not bridge to another article that reads
 *    it.
 */
export function followEvent(map, event) {
  if (!event) return null;
  const nodes = new Set([event]);
  const edges = new Set();
  const light = (i) => {
    nodes.add(map.edges[i].from);
    nodes.add(map.edges[i].to);
    edges.add(i);
  };
  map.edges.forEach((e, i) => {
    if (e.from === event || e.to === event) light(i);
  });
  const touched = [...nodes].filter((id) => id.startsWith('article:'));
  const decisions = new Set(touched);
  for (const [near, far] of [['to', 'from'], ['from', 'to']]) {
    const reached = new Set(touched);
    let grew = true;
    while (grew) {
      grew = false;
      map.edges.forEach((e, i) => {
        if (e.kind !== 'decides_on' || !reached.has(e[near]) || reached.has(e[far])) return;
        reached.add(e[far]);
        decisions.add(e[far]);
        light(i);
        grew = true;
      });
    }
    // An edge back into the chain (a cycle) is lit too, without growing it.
    map.edges.forEach((e, i) => {
      if (e.kind === 'decides_on' && reached.has(e[near]) && reached.has(e[far])) light(i);
    });
  }
  map.edges.forEach((e, i) => {
    if ((e.kind === 'source' || e.kind === 'prefill') && decisions.has(e.from)) light(i);
  });
  return { nodes, edges };
}

/**
 * A lit chain (followEvent) as it is shown with `opened` laws: the shown node
 * ids, and the keys (edgeKey) of the shown edges. An edge that folds into one
 * law is dropped, as collapse drops it; edges that fold together are one.
 */
export function litShown(map, lit, opened) {
  if (!lit) return null;
  const shown = shownIdOf(map, opened);
  const edges = new Set();
  for (const i of lit.edges) {
    const e = map.edges[i];
    const from = shown(e.from);
    const to = shown(e.to);
    if (from !== to) edges.add(edgeKey(from, to, e.kind));
  }
  return { nodes: new Set([...lit.nodes].map(shown)), edges };
}

/**
 * Position per node id: column by kind, rows by id within a column (numbers
 * in ids in their numeric order). Laws and articles sort by regulation first,
 * so an opened law's articles stand where the folded law stood.
 */
export function columnLayout(nodes) {
  const columns = new Map();
  for (const n of nodes) {
    const c = COLUMN[n.kind] ?? 2;
    if (!columns.has(c)) columns.set(c, []);
    columns.get(c).push(n);
  }
  const key = (n) => (n.regulation ? `${n.regulation} ${n.kind === 'law' ? '' : n.label}` : n.id);
  const pos = new Map();
  for (const [c, column] of columns) {
    column
      .map((n) => [key(n), n.id])
      .sort(([a], [b]) => a.localeCompare(b, 'nl', { numeric: true }))
      .forEach(([, id], row) => pos.set(id, { x: c * COL_W, y: row * ROW_H }));
  }
  return pos;
}
