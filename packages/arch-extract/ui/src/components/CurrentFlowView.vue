<script setup>
/**
 * CurrentFlowView — the explorer's original view, unchanged.
 *
 * This is the click-driven expand/collapse graph on a hand-written nested grid
 * (`useArchGraph.js` + Vue Flow). It was lifted out of `ArchExplorer.vue` as-is
 * so the three schema prototypes have something to be compared *against* while
 * the comparison runs. The follow-up ticket that develops the winner removes it
 * again — nothing new should be built on it.
 */
import { markRaw, nextTick, provide, watch } from 'vue';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MiniMap } from '@vue-flow/minimap';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import '@vue-flow/minimap/dist/style.css';

import ArchNode from './ArchNode.vue';
import ArchEdge from './ArchEdge.vue';
import { useArchGraph } from '../composables/useArchGraph.js';

const props = defineProps({
  model: { type: Object, required: true },
});
const emit = defineEmits(['select', 'stats']);

const nodeTypes = markRaw({ arch: ArchNode });
const edgeTypes = markRaw({ arch: ArchEdge });

const { nodes, edges, stats, setModel, toggle, expandSubtree, collapseAll, revealEdge } = useArchGraph();

const { fitView } = useVueFlow();

watch(
  () => props.model,
  (m) => {
    if (m) setModel(m);
  },
  { immediate: true },
);
watch(stats, (s) => emit('stats', s), { immediate: true });

// Custom nodes reach expand/select through injection (avoids DOM-sniffing the
// Vue Flow node-click event, as the editor's LawGraphView had to).
provide('toggleExpand', toggle);
provide('expandSubtree', expandSubtree);
provide('selectNode', (id) => emit('select', id));

// Clicking a rolled-up line's badge reveals its underlying relations, then the
// canvas eases to the involved nodes. Same injection pattern as above.
provide('revealEdge', (data) => {
  const fitIds = revealEdge(data);
  nextTick(() => {
    fitView({ nodes: fitIds, duration: 500, padding: 0.3 });
  });
});

// MiniMap needs a flat colour, not a CSS variable — resolve per kind.
function miniMapNodeColor(node) {
  if (node.class?.includes('kind-crate')) return '#6366f1';
  if (node.class?.includes('kind-app')) return '#16a34a';
  if (node.class?.includes('level-component')) return '#0ea5e9';
  if (node.class?.includes('level-code')) return '#94a3b8';
  return '#cbd5e1';
}
</script>

<template>
  <VueFlow
    :nodes="nodes"
    :edges="edges"
    :node-types="nodeTypes"
    :edge-types="edgeTypes"
    :nodes-connectable="false"
    :min-zoom="0.05"
    :max-zoom="4"
    fit-view-on-init
    @node-double-click="({ node }) => expandSubtree(node.id)"
    @pane-click="emit('select', null)"
  >
    <Background variant="dots" :gap="24" />
    <Controls :show-lock="false" />
    <MiniMap pannable zoomable :node-color="miniMapNodeColor" />
    <button type="button" class="arch-btn arch-flow-collapse" @click="collapseAll()">
      Alles inklappen
    </button>
  </VueFlow>
</template>
