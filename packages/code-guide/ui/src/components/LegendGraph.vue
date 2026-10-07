<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';

// The legend: a small graph drawn with the same stylesheet as the real one, so
// every mark it explains looks exactly as it does there. It cannot be moved or
// clicked. `description` says the same in words, for a screen reader.
const props = defineProps({
  /** Whether to show a box (only module and crate graphs have them). */
  boxes: { type: Boolean, default: false },
  /** What a box is, by level: "module" or "crate". */
  boxNoun: { type: String, default: 'module' },
  /** What the graph is centred on, as the page names it ("this method"). */
  focusName: { type: String, default: 'this page' },
  description: { type: String, required: true },
});

const container = ref(null);
let cy = null;
let scheme = null;
let lib = null;
let themeObserver = null;
let resizer = null;

const node = (id, x, y, text, extra = {}) => ({
  group: 'nodes',
  data: { id, display: text, width: 112, height: 30, ...extra.data },
  position: { x, y },
  classes: extra.classes ?? '',
});
const edge = (source, target, text, extra = {}) => ({
  group: 'edges',
  data: { id: `${source}>${target}`, source, target, width: 1.2, text, ...extra.data },
  classes: extra.classes ?? '',
});

function elements() {
  // Rows from top to bottom; x in three columns.
  const [l, m, r] = [60, 186, 312];
  let y = 20;
  const rows = [];
  // The first row is placed by width: the centre's name ("this crate's
  // modules") can be longer than the other two.
  const first = [
    ['open', props.focusName, 'focus'],
    ['private', 'private function', 'private'],
    ['selected', 'selected', 'sel'],
  ];
  let x = 4;
  for (const [id, text, classes] of first) {
    const width = Math.max(96, text.length * 7 + 24);
    rows.push(node(id, x + width / 2, y, text, { classes, data: { width } }));
    x += width + 14;
  }
  y += props.boxes ? 78 : 58;
  if (props.boxes) {
    rows.push(
      { group: 'nodes', data: { id: 'box', display: `a ${props.boxNoun}` }, classes: 'box' },
      node('added', m, y, 'added function', { data: { parent: 'box' } }),
    );
    y += 62;
  }
  rows.push(node('a1', l, y, 'caller'), node('a2', r, y, 'callee'), edge('a1', 'a2', 'a call'));
  y += 50;
  rows.push(
    node('b1', l, y, 'caller'),
    node('b2', r, y, 'callee'),
    edge('b1', 'b2', 'thicker: 4 calls', { data: { width: 2.8 } }),
  );
  y += 50;
  // Narrower nodes here, so the two arrows have room for their labels.
  const narrow = { data: { width: 64 } };
  rows.push(
    node('c1', l - 24, y, 'caller', narrow),
    node('c2', m, y, 'hovered', { data: { width: 80 } }),
    node('c3', r + 24, y, 'callee', narrow),
    edge('c1', 'c2', 'into it', { classes: 'in' }),
    edge('c2', 'c3', 'out of it', { classes: 'out' }),
  );
  y += 50;
  rows.push(
    node('d1', l, y, 'caller', { classes: 'hl' }),
    node('d2', r, y, 'callee', { classes: 'hl' }),
    edge('d1', 'd2', 'highlighted path', { classes: 'hl' }),
  );
  return rows;
}

/** The legend's own additions to the stylesheet: every edge says what it is. */
const LEGEND_STYLE = [
  { selector: 'edge[text]', style: { label: 'data(text)', 'text-margin-y': -9, 'font-size': 11 } },
];

function draw() {
  if (!cy || !lib) return;
  cy.style([...lib.styleSheet(lib.resolveTheme(container.value)), ...LEGEND_STYLE]);
  cy.elements().remove();
  cy.add(elements());
  cy.fit(cy.elements(), 8);
  if (cy.zoom() > 1) {
    cy.zoom(1);
    cy.center();
  }
}

onMounted(async () => {
  try {
    const [cyMod, themeMod] = await Promise.all([import('../lib/cy.js'), import('../lib/theme.js')]);
    lib = { ...cyMod, ...themeMod };
    if (!container.value) return;
    cy = lib.createCy({ container: container.value, theme: lib.resolveTheme(container.value) });
    cy.userZoomingEnabled(false);
    cy.userPanningEnabled(false);
    cy.autoungrabify(true);
    cy.autounselectify(true);
    draw();
    // Like the graph: redrawn for a colour scheme set by the system or by the
    // page, and fitted again when the panel changes size.
    scheme = globalThis.matchMedia?.('(prefers-color-scheme: dark)');
    scheme?.addEventListener?.('change', draw);
    if (globalThis.MutationObserver) {
      themeObserver = new MutationObserver(draw);
      themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    }
    if (globalThis.ResizeObserver) {
      resizer = new ResizeObserver(() => {
        cy?.resize();
        draw();
      });
      resizer.observe(container.value);
    }
  } catch {
    // Without a canvas the example cannot be drawn; its description still
    // says everything it shows, and the graph beside it reports the problem.
    cy = null;
  }
});
onBeforeUnmount(() => {
  scheme?.removeEventListener?.('change', draw);
  themeObserver?.disconnect();
  resizer?.disconnect();
  cy?.destroy();
  cy = null;
});
watch(() => [props.boxes, props.boxNoun, props.focusName], draw);
</script>

<template>
  <div ref="container" :class="['graph-legend', { 'with-box': boxes }]" role="img" :aria-label="description"></div>
</template>
