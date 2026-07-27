<script setup>
import { computed, markRaw, provide, ref, watch } from 'vue';
import { VueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MiniMap } from '@vue-flow/minimap';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import '@vue-flow/minimap/dist/style.css';

import ArchNode from './ArchNode.vue';
import DetailPanel from './DetailPanel.vue';
import { useArchGraph } from '../composables/useArchGraph.js';
import { useColorScheme } from '../composables/useColorScheme.js';

const props = defineProps({
  model: { type: Object, required: true },
});

const nodeTypes = markRaw({ arch: ArchNode });

const { model, nodes, edges, setModel, toggle, expandSubtree, collapseAll } = useArchGraph();

watch(
  () => props.model,
  (m) => {
    if (m) setModel(m);
  },
  { immediate: true },
);

// --- Selection / detail panel ---------------------------------------------
const selectedId = ref(null);
const selectedNode = computed(() => {
  if (!selectedId.value || !model.value) return null;
  return model.value.nodes.find((n) => n.id === selectedId.value) || null;
});

function selectNode(id) {
  selectedId.value = id;
}

// Custom nodes reach expand/select through injection (avoids DOM-sniffing the
// Vue Flow node-click event, as the editor's LawGraphView had to).
provide('toggleExpand', toggle);
provide('expandSubtree', expandSubtree);
provide('selectNode', selectNode);

// --- Theme ----------------------------------------------------------------
const { colorScheme, cycleColorScheme } = useColorScheme();
const themeLabel = computed(
  () => ({ auto: 'Auto', light: 'Licht', dark: 'Donker' })[colorScheme.value] || 'Auto',
);

// MiniMap needs a flat colour, not a CSS variable — resolve per kind.
function miniMapNodeColor(node) {
  if (node.class?.includes('kind-crate')) return '#6366f1';
  if (node.class?.includes('level-component')) return '#0ea5e9';
  if (node.class?.includes('level-code')) return '#94a3b8';
  return '#cbd5e1';
}

const nodeCount = computed(() => (model.value ? model.value.nodes.length : 0));
const edgeCount = computed(() => (model.value ? model.value.edges.length : 0));
</script>

<template>
  <div class="arch-explorer">
    <header class="arch-toolbar">
      <strong class="arch-toolbar__title">Architectuurverkenner</strong>
      <span class="arch-toolbar__stat">{{ nodeCount }} nodes · {{ edgeCount }} edges</span>

      <div class="arch-toolbar__spacer"></div>

      <div class="arch-legend" aria-hidden="true">
        <span class="arch-legend__item"><i class="lg lg-depends"></i>depends-on</span>
        <span class="arch-legend__item"><i class="lg lg-impl"></i>impl</span>
        <span class="arch-legend__item"><i class="lg lg-uses"></i>uses</span>
      </div>

      <button type="button" class="arch-btn" @click="collapseAll">Alles inklappen</button>
      <button type="button" class="arch-btn" @click="cycleColorScheme" :title="`Thema: ${themeLabel}`">
        Thema: {{ themeLabel }}
      </button>
    </header>

    <div class="arch-canvas">
      <VueFlow
        :nodes="nodes"
        :edges="edges"
        :node-types="nodeTypes"
        :nodes-connectable="false"
        :min-zoom="0.05"
        :max-zoom="4"
        fit-view-on-init
        @node-double-click="({ node }) => expandSubtree(node.id)"
        @pane-click="selectedId = null"
      >
        <Background variant="dots" :gap="24" />
        <Controls :show-lock="false" />
        <MiniMap pannable zoomable :node-color="miniMapNodeColor" />
      </VueFlow>

      <DetailPanel :node="selectedNode" @close="selectedId = null" />
    </div>
  </div>
</template>
