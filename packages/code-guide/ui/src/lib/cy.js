// Cytoscape with the layouts the call graph uses, and its stylesheet. Loaded
// only when a graph is opened, so the details pages do not carry it.
//
// The stylesheet is built from a theme of plain colours (see theme.js): the
// canvas cannot read CSS custom properties, so the design system's tokens are
// resolved first and passed in.

import cytoscape from 'cytoscape';
import dagre from 'cytoscape-dagre';
import fcose from 'cytoscape-fcose';

let registered = false;
function register() {
  if (registered) return;
  cytoscape.use(dagre);
  cytoscape.use(fcose);
  registered = true;
}

/** A Cytoscape instance in `container` (none: headless, for tests). */
export function createCy({ container = null, elements = [], theme, layout = null }) {
  register();
  const cy = cytoscape({
    container,
    headless: !container,
    elements,
    style: styleSheet(theme),
    layout: layout ?? { name: 'preset' },
    // Low enough that a graph of a few hundred functions fits whole.
    minZoom: 0.02,
    maxZoom: 2.5,
    boxSelectionEnabled: false,
    selectionType: 'single',
    autounselectify: true,
  });
  return cy;
}

/**
 * The stylesheet. Nodes are rounded boxes, coloured per crate (`ci`), with the
 * name and, below it, where the function lives. The focus has a thick border,
 * a private function a dashed one. Highlighting fades everything that is not
 * on it (`faded`) or hides it (`hidden`); highlighted edges take the accent
 * colour and show their call-site count.
 */
export function styleSheet(theme) {
  const t = theme ?? FALLBACK_THEME;
  const perCrate = t.categories.map((c, i) => ({
    selector: `node[ci = ${i}]`,
    style: { 'background-color': c.background, 'border-color': c.border, color: c.text },
  }));
  return [
    {
      selector: 'node',
      style: {
        shape: 'round-rectangle',
        label: 'data(display)',
        'text-wrap': 'wrap',
        'text-valign': 'center',
        'text-halign': 'center',
        'font-size': 13,
        'line-height': 1.35,
        'font-family': t.font,
        'border-width': 1,
        'background-color': t.surface,
        'border-color': t.divider,
        color: t.content,
        'transition-property': 'opacity',
        'transition-duration': '120ms',
      },
    },
    // A box (an opened module or crate) takes its size from what is in it.
    {
      selector: 'node[width]',
      style: {
        width: 'data(width)',
        height: 'data(height)',
        'text-max-width': (n) => `${n.data('width') - 12}px`,
      },
    },
    ...perCrate,
    {
      selector: 'node.box',
      style: {
        'text-valign': 'top',
        'text-halign': 'center',
        'text-margin-y': -4,
        'font-weight': 'bold',
        padding: '16px',
        'background-opacity': 0.45,
        'border-width': 2,
      },
    },
    { selector: 'node.private', style: { 'border-style': 'dashed', 'border-width': 1.5 } },
    { selector: 'node.focus', style: { 'border-width': 3, 'border-color': t.accent, 'font-weight': 'bold' } },
    {
      selector: 'edge',
      style: {
        width: 'data(width)',
        'curve-style': 'bezier',
        'line-color': t.edge,
        'target-arrow-color': t.edge,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.9,
        'line-opacity': 0.8,
        'font-size': 10,
        'font-family': t.font,
        color: t.content,
        'text-background-color': t.surface,
        'text-background-opacity': 0.9,
        'text-background-padding': '2px',
        'transition-property': 'opacity, line-color',
        'transition-duration': '120ms',
      },
    },
    { selector: 'edge.in', style: { 'line-color': t.incoming, 'target-arrow-color': t.incoming, 'line-opacity': 1, 'z-index': 8 } },
    { selector: 'edge.out', style: { 'line-color': t.outgoing, 'target-arrow-color': t.outgoing, 'line-opacity': 1, 'z-index': 8 } },
    {
      selector: 'edge.hl',
      style: {
        'line-color': t.accent,
        'target-arrow-color': t.accent,
        'line-opacity': 1,
        width: (e) => e.data('width') + 2,
        label: 'data(count)',
        'z-index': 10,
      },
    },
    { selector: 'node.hl', style: { 'z-index': 10 } },
    { selector: 'node.sel', style: { 'border-width': 4, 'border-color': t.content, 'z-index': 11 } },
    { selector: '.faded', style: { opacity: 0.13 } },
    { selector: '.hidden', style: { display: 'none' } },
  ];
}

/** Colours for a theme that could not be resolved (tests, an old browser). */
export const FALLBACK_THEME = {
  font: 'sans-serif',
  content: '#1f2937',
  secondary: '#4b5563',
  surface: '#ffffff',
  divider: '#d1d5db',
  edge: '#9ca3af',
  accent: '#154273',
  incoming: '#a1520a',
  outgoing: '#1d6f42',
  categories: [{ background: '#eef2f7', border: '#b6c3d4', text: '#1f2937' }],
};
