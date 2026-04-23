<script setup>
import { computed, ref, toRef, markRaw, watch } from 'vue';
import { VueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MiniMap } from '@vue-flow/minimap';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import '@vue-flow/minimap/dist/style.css';
import './graph/graph-styles.css';

import LawNode from './graph/LawNode.vue';
import LeafNode from './graph/LeafNode.vue';
import { useLawGraph } from '../composables/useLawGraph.js';

const props = defineProps({
  lawId: { type: String, default: null },
});

async function fetchLawYaml(lawId) {
  const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}`);
  if (!res.ok) throw new Error(`Law '${lawId}' niet gevonden (${res.status})`);
  return await res.text();
}

const nodeTypes = markRaw({
  law: LawNode,
  leaf: LeafNode,
});

const { nodes, edges, loading, error } = useLawGraph({
  rootLawId: toRef(props, 'lawId'),
  fetchLawYaml,
});

// --- Local UI state -----------------------------------------------------

// Which root law's edges are currently highlighted (inbound/outbound).
// null = no highlight. Toggles off when the same law is clicked twice.
const selectedRoot = ref(null);

// Laws the user hid via the close button (visible but excluded).
const hiddenLaws = ref(new Set());

function rootOfId(nodeOrEdgeId) {
  const i = nodeOrEdgeId.indexOf('-');
  return i === -1 ? nodeOrEdgeId : nodeOrEdgeId.substring(0, i);
}

// Reset local UI state when the underlying law changes, otherwise the
// hidden set / selection carries over to an unrelated graph.
watch(() => props.lawId, () => {
  selectedRoot.value = null;
  hiddenLaws.value = new Set();
});

const visibleNodes = computed(() => {
  const hidden = hiddenLaws.value;
  return nodes.value.map((n) => ({
    ...n,
    hidden: hidden.has(rootOfId(n.id)),
  }));
});

const visibleEdges = computed(() => {
  const hidden = hiddenLaws.value;
  const highlight = selectedRoot.value;
  return edges.value.map((e) => {
    const sourceRoot = rootOfId(e.source);
    const targetRoot = rootOfId(e.target);
    const edgeHidden = hidden.has(sourceRoot) || hidden.has(targetRoot);
    let cls = '';
    if (highlight) {
      if (sourceRoot === highlight) cls = 'inbound';
      else if (targetRoot === highlight) cls = 'outbound';
    }
    return { ...e, hidden: edgeHidden, class: cls || undefined };
  });
});

function handleNodeClick({ node, event }) {
  // A11Y LIMITATION (tracked for PR3 polish): the close button on a root
  // law node is detected here by DOM sniffing the click target. Keyboard
  // activation (Enter/Space on a focused button) dispatches a synthetic
  // `click` that bubbles normally, so this path works for keyboards too —
  // but the close button has no independent activation path if Vue Flow
  // ever stops forwarding inner clicks via `node-click`. Wiring a proper
  // data-callback from the custom node component is deferred to PR3.
  const target = event?.target;
  if (target && target.closest && target.closest('.close')) {
    hiddenLaws.value = new Set([...hiddenLaws.value, node.id]);
    if (selectedRoot.value === node.id) selectedRoot.value = null;
    return;
  }
  const isRoot = typeof node.class === 'string' && node.class.includes('root');
  if (!isRoot) return;
  selectedRoot.value = selectedRoot.value === node.id ? null : node.id;
}

function miniMapNodeColor(node) {
  return node.class?.includes('root') && !node.hidden ? '#ccc' : 'transparent';
}
</script>

<template>
  <div class="law-graph-view">
    <div v-if="error" class="law-graph-error">
      <nldd-inline-dialog variant="alert" text="Kon de wettengraaf niet opbouwen" :supporting-text="error"></nldd-inline-dialog>
    </div>

    <div v-else-if="!lawId" class="law-graph-empty">
      <nldd-inline-dialog text="Open een wet om de graaf te zien."></nldd-inline-dialog>
    </div>

    <div v-else class="law-graph-container">
      <div v-if="loading" class="law-graph-loading">Bezig met laden…</div>
      <VueFlow
        :nodes="visibleNodes"
        :edges="visibleEdges"
        :node-types="nodeTypes"
        :nodes-connectable="false"
        :min-zoom="0.1"
        fit-view-on-init
        @node-click="handleNodeClick"
      >
        <Controls :show-lock="false" />
        <Background variant="dots" />
        <MiniMap zoomable pannable :node-color="miniMapNodeColor" />
      </VueFlow>
    </div>
  </div>
</template>

<style scoped>
.law-graph-view {
  /* Fill the pane. ndd-page gives us a flex body; claim the viewport
   * minus toolbar + tab-bar chrome (mirrors the YAML textarea).
   * 180px = primary toolbar (~48px) + document tab bar (~56px) +
   * right-pane title bar (~48px) + spacer (~28px). Update this if the
   * chrome layout changes (e.g. a new toolbar row is added). */
  height: calc(100vh - 180px);
  display: flex;
  flex-direction: column;
  min-height: 400px;
}

.law-graph-container {
  flex: 1;
  position: relative;
  min-height: 0;
}

.law-graph-loading {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 5;
  background: var(--semantics-surfaces-tinted-background-color, #f5f5f5);
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #666);
}

.law-graph-empty,
.law-graph-error {
  padding: 16px;
}
</style>
