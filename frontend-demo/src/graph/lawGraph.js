/**
 * The dependency graph as the POC drew it: one box per law with its register
 * sources, its inputs from other laws and its outputs, and an edge from every
 * input to the output of the law that supplies it. Pure functions; the view
 * hands the result to vue-flow.
 */
import { MarkerType, Position } from '@vue-flow/core';

export const ITEM_H = 30;
export const ITEM_GAP = 6;
export const BOX_PAD = 8;
export const BOX_LABEL_H = 26;
export const COL_W = 200;
export const LAW_PAD = 12;
export const HEADER_H = 52;
export const LAW_W = LAW_PAD * 3 + COL_W * 2;
const LAYER_GAP = 96;
const ROW_GAP = 40;
const MAX_PER_COLUMN = 3;

/** Every execution block of a law, in article order. */
function executions(law) {
  return (law.doc?.articles ?? []).map((a) => a.machine_readable?.execution).filter(Boolean);
}

/**
 * What a law reads and produces, deduplicated by name.
 * @returns {{ sources: {name}[], inputs: {name, ref:{regulation, output}}[], outputs: {name}[] }}
 */
export function lawShape(law) {
  const sources = new Map();
  const inputs = new Map();
  const outputs = new Map();
  for (const ex of executions(law)) {
    for (const input of ex.input ?? []) {
      const ref = input.source?.regulation ? { regulation: input.source.regulation, output: input.source.output } : null;
      if (ref) inputs.set(input.name, { name: input.name, ref });
      else sources.set(input.name, { name: input.name });
    }
    for (const output of ex.output ?? []) outputs.set(output.name, { name: output.name });
  }
  return { sources: [...sources.values()], inputs: [...inputs.values()], outputs: [...outputs.values()] };
}

function boxHeight(count) {
  return BOX_LABEL_H + BOX_PAD * 2 + count * ITEM_H + Math.max(0, count - 1) * ITEM_GAP;
}

/** Size of a law box, from what it holds. */
export function lawSize(shape) {
  let left = 0;
  if (shape.sources.length) left += boxHeight(shape.sources.length);
  if (shape.inputs.length) left += (left ? LAW_PAD : 0) + boxHeight(shape.inputs.length);
  const right = shape.outputs.length ? boxHeight(shape.outputs.length) : 0;
  return { width: LAW_W, height: HEADER_H + Math.max(left, right, BOX_LABEL_H) + LAW_PAD };
}

/**
 * Layered positions: a law sits right of every law it reads from. A layer with
 * more than MAX_PER_COLUMN laws wraps into side-by-side sub-columns.
 * @param {Map<string, {size:{width,height}, deps:string[]}>} laws
 * @returns {Map<string, {x:number, y:number}>}
 */
export function layout(laws) {
  const depth = new Map();
  const visit = (id, stack = new Set()) => {
    if (depth.has(id)) return depth.get(id);
    if (stack.has(id)) return 0;
    stack.add(id);
    const d = (laws.get(id)?.deps ?? []).filter((r) => laws.has(r)).reduce((m, r) => Math.max(m, visit(r, stack) + 1), 0);
    stack.delete(id);
    depth.set(id, d);
    return d;
  };
  for (const id of laws.keys()) visit(id);
  const layers = new Map();
  for (const [id, d] of depth) {
    if (!layers.has(d)) layers.set(d, []);
    layers.get(d).push(id);
  }
  const positions = new Map();
  let x = 0;
  for (const d of [...layers.keys()].sort((a, b) => a - b)) {
    const ids = layers.get(d).sort();
    const subCols = Math.ceil(ids.length / MAX_PER_COLUMN);
    const perCol = Math.ceil(ids.length / subCols);
    let widest = 0;
    for (let c = 0; c < subCols; c += 1) {
      let y = 0;
      for (const id of ids.slice(c * perCol, (c + 1) * perCol)) {
        const size = laws.get(id).size;
        positions.set(id, { x: x + c * (LAW_W + LAW_PAD * 2), y });
        y += size.height + ROW_GAP;
        widest = Math.max(widest, c * (LAW_W + LAW_PAD * 2) + size.width);
      }
    }
    x += widest + LAYER_GAP;
  }
  return positions;
}

export const itemId = (lawId, kind, name) => `${lawId}::${kind}::${name}`;

/**
 * The laws to draw for a selection, as the POC did it: the selected laws plus
 * every law directly connected to one of them (what they read from and what
 * reads from them), so the picture is the connected neighbourhood, not one
 * chain.
 * @param {Set<string>} selected
 * @param {object[]} laws   all corpus laws
 * @returns {Set<string>}
 */
export function neighbourhood(selected, laws) {
  const shown = new Set(selected);
  const known = new Set(laws.map((l) => l.id));
  for (const law of laws) {
    const refs = lawShape(law).inputs.map((i) => i.ref.regulation).filter((id) => known.has(id));
    if (selected.has(law.id)) refs.forEach((id) => shown.add(id));
    else if (refs.some((id) => selected.has(id))) shown.add(law.id);
  }
  return shown;
}

/** Seven Rijkshuisstijl colours; a service keeps its colour by first appearance. */
export const COLOUR_COUNT = 7;
export function colourIndex(services) {
  const order = [...new Set(services)];
  return (service) => order.indexOf(service) % COLOUR_COUNT;
}

/**
 * Nodes and edges for vue-flow.
 * @param {object[]} laws          corpus law entries to draw
 * @param {object} [values]        { outputs: {lawId: {name: text}}, sources: {lawId: {name: text}}, inputs: {lawId: {name: text}} }
 * @param {string|null} [focus]    law id whose edges stay bright
 */
export function buildGraph(laws, values = {}, focus = null) {
  const byId = new Map(laws.map((l) => [l.id, l]));
  const shapes = new Map(laws.map((l) => [l.id, lawShape(l)]));
  const sized = new Map();
  for (const law of laws) {
    const shape = shapes.get(law.id);
    sized.set(law.id, { size: lawSize(shape), deps: shape.inputs.map((i) => i.ref.regulation) });
  }
  const positions = layout(sized);

  const colour = colourIndex(laws.map((l) => l.service));
  const nodes = [];
  const edges = [];
  const dimmed = (lawId) => !!focus && lawId !== focus && !edges.some((e) => (e.data.from === focus && e.data.to === lawId) || (e.data.to === focus && e.data.from === lawId));

  // Edges first: the dimming of a law depends on them.
  for (const law of laws) {
    for (const input of shapes.get(law.id).inputs) {
      const supplier = byId.get(input.ref.regulation);
      if (!supplier || !shapes.get(supplier.id).outputs.some((o) => o.name === input.ref.output)) continue;
      const bright = !focus || law.id === focus || supplier.id === focus;
      edges.push({
        id: `${itemId(law.id, 'in', input.name)}->${itemId(supplier.id, 'out', input.ref.output)}`,
        source: itemId(law.id, 'in', input.name),
        target: itemId(supplier.id, 'out', input.ref.output),
        data: { from: law.id, to: supplier.id },
        markerEnd: MarkerType.ArrowClosed,
        animated: !!focus && bright,
        style: { opacity: bright ? 0.85 : 0.12 },
        zIndex: 5,
      });
    }
  }

  for (const law of laws) {
    const shape = shapes.get(law.id);
    const { size } = sized.get(law.id);
    const dim = dimmed(law.id);
    nodes.push({
      id: law.id,
      type: 'law',
      position: positions.get(law.id),
      style: { width: `${size.width}px`, height: `${size.height}px` },
      data: { law, selected: focus === law.id },
      class: `graph-c${colour(law.service)}${dim ? ' graph-dim' : ''}`,
      selectable: false,
    });
    const box = (kind, label, items, x, y) => {
      if (!items.length) return 0;
      const h = boxHeight(items.length);
      const boxId = `${law.id}::box::${kind}`;
      nodes.push({
        id: boxId,
        type: 'box',
        parentNode: law.id,
        position: { x, y },
        style: { width: `${COL_W}px`, height: `${h}px` },
        data: { label, kind },
        class: `graph-c${colour(law.service)}`,
        draggable: false,
        selectable: false,
      });
      items.forEach((item, i) => {
        nodes.push({
          id: itemId(law.id, kind === 'sources' ? 'src' : kind === 'inputs' ? 'in' : 'out', item.name),
          type: 'item',
          parentNode: boxId,
          position: { x: BOX_PAD, y: BOX_LABEL_H + BOX_PAD + i * (ITEM_H + ITEM_GAP) },
          style: { width: `${COL_W - BOX_PAD * 2}px`, height: `${ITEM_H}px` },
          data: { name: item.name, kind, value: values[kind]?.[law.id]?.[item.name], ref: item.ref ?? null },
          sourcePosition: Position.Left,
          targetPosition: Position.Right,
          draggable: false,
          selectable: false,
        });
      });
      return h;
    };
    let y = HEADER_H;
    const sourcesH = box('sources', 'Bronnen', shape.sources, LAW_PAD, y);
    if (sourcesH) y += sourcesH + LAW_PAD;
    box('inputs', 'Invoer', shape.inputs, LAW_PAD, y);
    box('outputs', 'Uitvoer', shape.outputs, LAW_PAD * 2 + COL_W, HEADER_H);
  }
  return { nodes, edges };
}
