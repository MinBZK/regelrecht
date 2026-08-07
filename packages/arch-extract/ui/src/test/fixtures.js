/**
 * Shared synthetic model for the unit tests.
 *
 * Small enough to reason about by hand, but it exercises every case the rollup
 * has: a cross-container relation, two relations that aggregate into one, a
 * container-internal one, an `impl`, a relation pointing at an own ancestor,
 * and root-level `depends-on`.
 *
 *   crate:a                       (container)
 *     mod:a::m1                   (component)  → type T1, type T2   (code)
 *     mod:a::m2                   (component)  → type U1            (code)
 *   crate:b                       (container)  → type V1            (code)
 *   crate:c                       (container)  → type W1            (code)
 */
export function makeModel() {
  const node = (id, kind, level, parent) => ({
    id,
    kind,
    level,
    lang: 'rust',
    name: id.split('::').pop(),
    path: 'x',
    ...(parent ? { parent } : {}),
  });
  return {
    schemaVersion: '1',
    nodes: [
      node('crate:a', 'crate', 'container'),
      node('crate:b', 'crate', 'container'),
      node('crate:c', 'crate', 'container'),
      node('mod:a::m1', 'module', 'component', 'crate:a'),
      node('mod:a::m2', 'module', 'component', 'crate:a'),
      node('type:a::m1::T1', 'struct', 'code', 'mod:a::m1'),
      node('type:a::m1::T2', 'struct', 'code', 'mod:a::m1'),
      node('type:a::m2::U1', 'struct', 'code', 'mod:a::m2'),
      node('type:b::V1', 'struct', 'code', 'crate:b'),
      node('type:c::W1', 'struct', 'code', 'crate:c'),
    ],
    edges: [
      { from: 'type:a::m1::T1', to: 'type:b::V1', kind: 'uses' }, // E1 cross-crate
      { from: 'type:a::m1::T2', to: 'type:b::V1', kind: 'uses' }, // E2 cross-crate (aggregates with E1)
      { from: 'type:a::m1::T1', to: 'type:a::m2::U1', kind: 'uses' }, // E3 crate-internal / cross-module
      { from: 'type:a::m1::T1', to: 'type:c::W1', kind: 'impl' }, // E4 cross-crate impl
      { from: 'type:a::m1::T1', to: 'crate:a', kind: 'uses' }, // E5 points at own ancestor
      { from: 'crate:a', to: 'crate:b', kind: 'depends-on' }, // E6 root depends-on
      { from: 'crate:a', to: 'crate:c', kind: 'depends-on' }, // E7 root depends-on
    ],
  };
}

/**
 * A model with enough units that fitting it into the shared world box shrinks
 * the geometry hard — the regime the real model is in, and the one `makeModel`
 * is far too small to reach.
 *
 * One container, `units` modules inside it, and one of them a hub that a tenth
 * of the others point at. Used to check that a layout still tells the hub apart
 * from a module with no relations *after* the fit.
 */
export function makeWideModel(units = 300) {
  const nodes = [
    { id: 'crate:w', kind: 'crate', level: 'container', lang: 'rust', name: 'w', path: 'x' },
  ];
  const edges = [];
  for (let i = 0; i < units; i += 1) {
    nodes.push({
      id: `mod:w::m${i}`,
      kind: 'module',
      level: 'component',
      lang: 'rust',
      name: `m${i}`,
      path: 'x',
      parent: 'crate:w',
    });
  }
  for (let i = 1; i <= Math.floor(units / 10); i += 1) {
    edges.push({ from: `mod:w::m${i}`, to: 'mod:w::m0', kind: 'uses' });
  }
  return { schemaVersion: '1', nodes, edges };
}

/** Every relation kind the model produces. */
export const ALL_KINDS = new Set(['depends-on', 'impl', 'uses']);
