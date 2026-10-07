<script setup>
import { computed } from 'vue';
import { Handle, MarkerType, Position, VueFlow } from '@vue-flow/core';
import { Controls } from '@vue-flow/controls';

// A graph of calls at the level that is open: all crates, the modules of a
// crate, the modules around one module, or the non-private methods of a type.
const props = defineProps({ g: { type: Object, required: true } });

const SCOPE_TEXT = { workspace: 'All crates', crate: 'This crate', module: 'Around this module', type: 'This type' };
const scopes = computed(() => {
  const r = props.g.route;
  return [
    'workspace',
    ...(r.crate ? ['crate'] : []),
    ...(r.crate && r.module !== null ? ['module'] : []),
    ...(props.g.page === 'type' ? ['type'] : []),
  ];
});

// Arrows run from a caller to what it calls, and callees sit below, so an edge
// leaves a node at its bottom and enters the next one at its top.
const nodes = computed(() =>
  props.g.graph.nodes.map((n) => ({
    id: n.id,
    type: 'card',
    position: n.position,
    data: n,
    sourcePosition: Position.Bottom,
    targetPosition: Position.Top,
    style: { width: `${props.g.graph.nodeWidth}px` },
  })),
);
// A call through private helpers is dashed and names them, so the arrow does not
// claim a direct call that is not in the source.
const edges = computed(() =>
  props.g.graph.edges.map((e) => ({
    ...e,
    markerEnd: MarkerType.ArrowClosed,
    ...(e.via?.length ? { label: `via ${e.via.join(' → ')}`, style: { strokeDasharray: '6 4' } } : {}),
  })),
);

// A new graph starts over and is fitted, rather than keeping the old zoom.
const canvasKey = computed(() => `${props.g.scope}|${props.g.route.crate}|${props.g.route.module}|${props.g.route.item}`);

const loading = computed(() => {
  const g = props.g;
  if (g.scope === 'workspace') return !g.workspace;
  if (g.scope === 'type') return !g.typeView;
  return !g.crateView;
});
const empty = computed(() => !loading.value && !props.g.graph.nodes.length);

const summary = computed(() => {
  const n = props.g.graph.nodes.length;
  const e = props.g.graph.edges.length;
  const what = { workspace: 'crate', crate: 'module', module: 'module', type: 'node' }[props.g.scope];
  return `${n} ${what}${n === 1 ? '' : 's'}, ${e} ${e === 1 ? 'call edge' : 'call edges'}`;
});

const onScope = (event) => {
  globalThis.location.hash = props.g.hrefHere({ scope: event.target.value });
};

// The cards are web components, so their size is only known once they have
// rendered, and Vue Flow's own fit runs before that. Which of `init` and
// `nodes-initialized` comes last is not something to rely on either, so a fit is
// requested from both and again shortly after; fitting is idempotent.
let flow = null;
const fit = () => flow?.fitView({ padding: 0.12, maxZoom: 1 });
const scheduleFit = () => {
  requestAnimationFrame(() => requestAnimationFrame(fit));
  setTimeout(fit, 150);
  setTimeout(fit, 600);
};
const onInit = (instance) => {
  flow = instance;
  scheduleFit();
};
// Vue Flow turns pointer events off on nodes that are neither selectable nor
// draggable unless something listens for node clicks; this listener is that
// something. It goes to the same address as the card's own link.
const onNodeClick = ({ node }) => {
  globalThis.location.hash = node.data.href;
};

const tagText = (data) => {
  if (data.cycleWith?.length) return `cycle with ${data.cycleWith.join(', ')}`;
  if (data.role === 'both') return 'calls it and is called by it';
  if (data.external) return 'calls this type';
  return null;
};
const unconnectedHref = (key) =>
  props.g.hrefFor({ module: props.g.typeView.module, item: props.g.typeView.name, method: key });
</script>

<template>
  <nldd-simple-section>
    <nldd-segmented-control accessible-label="What the graph shows" :value="g.scope" @change="onScope">
      <nldd-segmented-control-item v-for="s in scopes" :key="s" :value="s" :text="SCOPE_TEXT[s]" :selected="g.scope === s" />
    </nldd-segmented-control>
    <nldd-spacer size="8" />
    <nldd-text v-if="g.scope === 'type'">
      Arrows point from a caller to the method it calls, so the methods others build on are at the bottom.
      Modules outside the type that call it are on top. A dashed arrow goes through private helpers, named on it.
      {{ summary }}.
    </nldd-text>
    <nldd-text v-else>
      Arrows point from a caller to what it calls, so what others build on is at the bottom. Select a node to open it.
      {{ summary }}.
    </nldd-text>
  </nldd-simple-section>

  <nldd-simple-section v-if="loading">
    <nldd-text>Loading the graph…</nldd-text>
  </nldd-simple-section>
  <nldd-simple-section v-else-if="empty">
    <nldd-text>No calls to draw here.</nldd-text>
  </nldd-simple-section>
  <nldd-simple-section v-else>
    <div class="graph-canvas" role="group" :aria-label="`Call graph, ${summary}`">
      <VueFlow
        :key="canvasKey"
        :nodes="nodes"
        :edges="edges"
        :nodes-draggable="false"
        :nodes-connectable="false"
        :elements-selectable="false"
        :min-zoom="0.2"
        :max-zoom="2"
        @init="onInit"
        @nodes-initialized="scheduleFit"
        @node-click="onNodeClick"
      >
        <template #node-card="{ data }">
          <Handle type="target" :position="Position.Top" />
          <Handle type="source" :position="Position.Bottom" />
          <nldd-card :href="data.href" :background="data.selected ? 'tinted' : 'base'" :accessible-label="data.label">
            <nldd-container padding="8">
              <nldd-text weight="bold">{{ data.label }}</nldd-text>
              <nldd-tag v-if="data.selected" size="sm" color="accent" text="open" />
              <nldd-tag v-if="tagText(data)" size="sm" color="warning" :text="tagText(data)" />
            </nldd-container>
          </nldd-card>
        </template>
        <Controls :show-interactive="false" />
      </VueFlow>
    </div>
  </nldd-simple-section>

  <nldd-simple-section v-if="g.scope === 'type' && g.typeView && g.graph.unconnected.length">
    <nldd-title slot="header" :size="6" text="Not connected" :heading-level="3" />
    <nldd-text>
      {{ g.graph.unconnected.length }}
      {{ g.graph.unconnected.length === 1 ? 'method is' : 'methods are' }} neither called by anything in the workspace's
      production code nor {{ g.graph.unconnected.length === 1 ? 'calls' : 'call' }} another non-private method of {{ g.typeView.name }}:
    </nldd-text>
    <nldd-spacer size="8" />
    <nldd-link v-for="key in g.graph.unconnected" :key="key" :href="unconnectedHref(key)" :text="key" />
  </nldd-simple-section>
</template>
