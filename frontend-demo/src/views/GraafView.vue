<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { VueFlow, MarkerType, Position, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import OrgLogo from '../components/OrgLogo.vue';
import { useDemo } from '../store/demoStore.js';

// The dependency graph of the demo corpus: one node per law, one edge per
// `source.regulation` reference, laid out in layers by dependency depth and
// coloured by the organisation that executes the law. Click a node to see its
// neighbours; double-click to open the law. The canvas is vue-flow, as in the
// editor - the design system has no graph component (documented exception).

const router = useRouter();
const { corpus, profile, portalLaws } = useDemo();
// Same store id as the <VueFlow> below, otherwise fitView talks to a different instance.
const { fitView } = useVueFlow({ id: 'demo-graph' });

const scope = ref('portaal'); // 'portaal' | 'alles'
const selected = ref(null);

function referencesOf(law) {
  const ids = new Set();
  const walk = (node) => {
    if (Array.isArray(node)) node.forEach(walk);
    else if (node && typeof node === 'object') {
      if (typeof node.regulation === 'string') ids.add(node.regulation);
      Object.values(node).forEach(walk);
    }
  };
  walk(law.doc.articles);
  return [...ids].filter((id) => corpus.value.latestById.has(id));
}

/** Laws in scope: the portal's laws plus everything they depend on, or all. */
const lawsInScope = computed(() => {
  if (!corpus.value) return [];
  if (scope.value === 'alles') return [...corpus.value.latestById.values()];
  const wanted = new Map();
  const queue = [...portalLaws.value];
  while (queue.length) {
    const law = queue.shift();
    if (wanted.has(law.id)) continue;
    wanted.set(law.id, law);
    for (const ref of referencesOf(law)) queue.push(corpus.value.lawById(ref));
  }
  return [...wanted.values()];
});

const graph = computed(() => {
  const laws = lawsInScope.value;
  const ids = new Set(laws.map((l) => l.id));
  const edges = [];
  const deps = new Map();
  for (const law of laws) {
    const refs = referencesOf(law).filter((r) => ids.has(r));
    deps.set(law.id, refs);
    for (const ref of refs) edges.push({ from: law.id, to: ref });
  }
  // Depth = longest path to a leaf (a law with no references).
  const depth = new Map();
  const visit = (id, stack = new Set()) => {
    if (depth.has(id)) return depth.get(id);
    if (stack.has(id)) return 0;
    stack.add(id);
    const d = (deps.get(id) ?? []).reduce((m, r) => Math.max(m, visit(r, stack) + 1), 0);
    stack.delete(id);
    depth.set(id, d);
    return d;
  };
  laws.forEach((l) => visit(l.id));
  const maxDepth = Math.max(0, ...depth.values());
  const columns = new Map();
  for (const law of laws) {
    const col = maxDepth - depth.get(law.id);
    if (!columns.has(col)) columns.set(col, []);
    columns.get(col).push(law);
  }
  // Layered layout: one layer per depth, left to right. A layer taller than
  // MAX_ROWS wraps into side-by-side sub-columns, and every layer is centred
  // vertically, so a corpus with many leaf registers stays readable.
  const nodes = [];
  const subColWidth = 260;
  const layerGap = 60;
  const rowHeight = 72;
  const MAX_ROWS = 8;
  const orderedCols = [...columns.keys()].sort((a, b) => a - b);
  const tallest = Math.max(...orderedCols.map((c) => Math.min(columns.get(c).length, MAX_ROWS)));
  let x = 0;
  for (const col of orderedCols) {
    const list = columns.get(col);
    list.sort((a, b) => (a.service ?? '').localeCompare(b.service ?? '') || a.name.localeCompare(b.name));
    const subCols = Math.ceil(list.length / MAX_ROWS);
    const rows = Math.ceil(list.length / subCols);
    const yOffset = ((tallest - rows) * rowHeight) / 2;
    list.forEach((law, i) => {
      const sub = Math.floor(i / rows);
      const row = i % rows;
      nodes.push({
        id: law.id,
        type: 'law',
        position: { x: x + sub * subColWidth + (row % 2) * 24, y: yOffset + row * rowHeight },
        data: { law },
        sourcePosition: Position.Right,
        targetPosition: Position.Left,
      });
    });
    x += subCols * subColWidth + layerGap;
  }
  const neighbours = new Set();
  if (selected.value) {
    neighbours.add(selected.value);
    for (const e of edges) {
      if (e.from === selected.value) neighbours.add(e.to);
      if (e.to === selected.value) neighbours.add(e.from);
    }
  }
  return {
    nodes: nodes.map((n) => ({ ...n, class: selected.value && !neighbours.has(n.id) ? 'graph-dim' : '' })),
    edges: edges.map((e) => ({
      id: `${e.from}->${e.to}`,
      source: e.from,
      target: e.to,
      markerEnd: MarkerType.ArrowClosed,
      animated: selected.value ? e.from === selected.value || e.to === selected.value : false,
      style: selected.value && e.from !== selected.value && e.to !== selected.value ? { opacity: 0.1 } : { opacity: 0.6 },
    })),
  };
});

function onNodeClick({ node }) {
  selected.value = selected.value === node.id ? null : node.id;
}
function onNodeDoubleClick({ node }) {
  router.push(`/wetten/${encodeURIComponent(node.id)}`);
}
function onPaneClick() {
  selected.value = null;
}
const selectedLaw = computed(() => (selected.value ? corpus.value?.lawById(selected.value) : null));
const usedBy = computed(() => (selected.value ? graph.value.edges.filter((e) => e.target === selected.value).map((e) => corpus.value.lawById(e.source)) : []));
const uses = computed(() => (selected.value ? graph.value.edges.filter((e) => e.source === selected.value).map((e) => corpus.value.lawById(e.target)) : []));

function refit() {
  setTimeout(() => fitView({ padding: 0.15 }), 50);
}
// vue-flow's fit-view-on-init runs before the nodes exist (the corpus arrives
// async); refit whenever the node set changes.
watch(() => graph.value.nodes.length, async () => {
  await nextTick();
  refit();
});
</script>

<template>
  <nldd-navigation-split-view inspector-accessible-label="Geselecteerde wet">
    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-title slot="start" text="Graaf" :supporting-text="`${graph.nodes.length} wetten, ${graph.edges.length} verwijzingen`"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end">
              <nldd-segmented-control size="sm" width="fit-content" :value="scope" @change="scope = $event.detail?.value; refit()">
                <nldd-segmented-control-item value="portaal" :text="`Regelingen van ${profile?.name ?? 'dit profiel'}`"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="alles" text="Hele corpus"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="binoculars" text="Passend maken" @click="refit"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <div class="graph-canvas">
          <VueFlow
            id="demo-graph"
            :nodes="graph.nodes"
            :edges="graph.edges"
            :min-zoom="0.2"
            :max-zoom="2"
            fit-view-on-init
            :nodes-draggable="false"
            :nodes-connectable="false"
            @node-click="onNodeClick"
            @node-double-click="onNodeDoubleClick"
            @pane-click="onPaneClick"
            @nodes-initialized="refit"
          >
            <template #node-law="{ data }">
              <div :class="['graph-node', selected === data.law.id ? 'graph-node--selected' : '']" :title="data.law.id">
                <OrgLogo v-if="data.law.service" :service="data.law.service" size="sm" />
                <span>{{ data.law.name }}</span>
              </div>
            </template>
            <Background />
            <Controls :show-interactive="false" />
          </VueFlow>
        </div>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="inspector" :has-content="selectedLaw ? true : undefined" background="tinted">
      <nldd-page v-if="selectedLaw" background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="selectedLaw.name" :supporting-text="selectedLaw.id" dismiss-text="Sluiten" @dismiss="selected = null"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-container layout="row" gap="12" vertical-alignment="center">
            <OrgLogo :service="selectedLaw.service" />
            <nldd-text-cell overline="Uitgevoerd door" :text="corpus.services[selectedLaw.service]?.name ?? selectedLaw.service"></nldd-text-cell>
          </nldd-container>
          <nldd-button variant="secondary" size="sm" start-icon="book" text="Open in Wetten" @click="router.push(`/wetten/${encodeURIComponent(selectedLaw.id)}`)"></nldd-button>
          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Gebruikt gegevens uit"></nldd-text-cell></nldd-container>
<nldd-list variant="box" accessible-label="Gebruikt gegevens uit">
            <nldd-list-item v-if="uses.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen andere wetten"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-for="l in uses" :key="l.id" size="sm" button @click="selected = l.id">
              <nldd-text-cell size="sm" :text="l.name" :supporting-text="l.service"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-container padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" text="Wordt gebruikt door"></nldd-text-cell></nldd-container>
<nldd-list variant="box" accessible-label="Wordt gebruikt door">
            <nldd-list-item v-if="usedBy.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen andere wetten"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-for="l in usedBy" :key="l.id" size="sm" button @click="selected = l.id">
              <nldd-text-cell size="sm" :text="l.name" :supporting-text="l.service"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>

<style>
/* vue-flow node dimming for the selection focus (no design-system hook). */
.vue-flow__node.graph-dim {
  opacity: 0.25;
}
</style>
