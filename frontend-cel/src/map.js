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
export const ROW_H = 64;
// The edges between articles that a followed event drags along.
const ARTICLE_EDGES = new Set(['decides_on', 'prefill', 'source']);

// The id of the node that stands for a law when it is folded.
export const lawId = (regulation) => `law:${regulation}`;

/** Articles of a law that is not in `opened` become one node `law:<regulation>`. */
export function collapse(map, opened) {
  const into = new Map();
  const laws = new Map();
  const nodes = [];
  for (const n of map.nodes) {
    if (n.kind === 'article' && !opened.has(n.regulation)) {
      const id = lawId(n.regulation);
      into.set(n.id, id);
      const law = laws.get(id) ?? { id, kind: 'law', label: n.regulation, regulation: n.regulation, count: 0 };
      law.count += 1;
      laws.set(id, law);
    } else {
      nodes.push(n);
    }
  }
  const seen = new Set();
  const edges = [];
  for (const e of map.edges) {
    const from = into.get(e.from) ?? e.from;
    const to = into.get(e.to) ?? e.to;
    const key = `${from}|${to}|${e.kind}`;
    if (from === to || seen.has(key)) continue;
    seen.add(key);
    edges.push({ from, to, kind: e.kind });
  }
  return { process: map.process, nodes: [...nodes, ...laws.values()], edges };
}

/**
 * The chain of one event: `{nodes: Set<id>, edges: Set<index in map.edges>}`,
 * or null for no filter. From the event every edge one step in either
 * direction (process, stream, the lexostatus that reads it, articles); from
 * the articles reached only article-to-article edges, transitively. So the
 * article that decides on an established article comes along, but not the
 * other events of that deciding article.
 */
export function followEvent(map, event) {
  if (!event) return null;
  const nodes = new Set([event]);
  const edges = new Set();
  const article = (id) => id.startsWith('article:');
  map.edges.forEach((e, i) => {
    if (e.from === event || e.to === event) {
      nodes.add(e.from);
      nodes.add(e.to);
      edges.add(i);
    }
  });
  let grew = true;
  while (grew) {
    grew = false;
    map.edges.forEach((e, i) => {
      if (edges.has(i) || !ARTICLE_EDGES.has(e.kind) || !article(e.from) || !article(e.to)) return;
      if (nodes.has(e.from) || nodes.has(e.to)) {
        nodes.add(e.from);
        nodes.add(e.to);
        edges.add(i);
        grew = true;
      }
    });
  }
  return { nodes, edges };
}

/**
 * Position per node id: column by kind, rows by id within a column. Laws and
 * articles sort by regulation first, so an opened law's articles stand where
 * the folded law stood.
 */
export function columnLayout(nodes) {
  const columns = new Map();
  for (const n of nodes) {
    const c = COLUMN[n.kind] ?? 2;
    if (!columns.has(c)) columns.set(c, []);
    columns.get(c).push(n);
  }
  const key = (n) => (n.regulation ? `${n.regulation}\u0000${n.id}` : n.id);
  const pos = new Map();
  for (const [c, column] of columns) {
    column
      .map((n) => [key(n), n.id])
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .forEach(([, id], row) => pos.set(id, { x: c * COL_W, y: row * ROW_H }));
  }
  return pos;
}

// What a kind of node is called on the page.
export const KIND_TEXT = {
  process: 'proces', channel: 'kanaal', role: 'rol', action: 'handeling',
  cell: 'cel', stream: 'stroom', register: 'register', source_cell: 'broncel',
  event: 'event', lexostatus: 'lexostatus', article: 'artikel', law: 'wet',
};

/** The text of a node in the graph: its kind and its name; a law with its count. */
export function nodeText(n) {
  if (n.kind === 'law') return `wet ${n.label} (${n.count})`;
  if (n.kind === 'article') return `${n.regulation} art. ${n.label}`;
  return `${KIND_TEXT[n.kind] ?? n.kind}: ${n.label}`;
}
