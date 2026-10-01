<script setup>
// The map of a process (spec 2): what the runtime uses, folded per law. A
// click on a law opens it to articles, a click on another node shows its
// fragment. "Volg een event" lights up the chain of one event. The layout is
// a fixed column per kind (map.js); vue-flow only draws.
import { computed, provide, ref, watch } from 'vue';
import { VueFlow, MarkerType, useVueFlow } from '@vue-flow/core';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import { processApi } from '../api.js';
import { collapse, columnLayout, followEvent, lawId, nodeText } from '../map.js';
import { fragmentCache } from '../why.js';
import MapSheet from '../components/MapSheet.vue';

const props = defineProps({ processes: { type: Array, required: true } });
const chosen = ref(props.processes.find((p) => p.portal)?.id ?? props.processes[0]?.id ?? null);
const map = ref(null);
const error = ref('');
const opened = ref(new Set());
const event = ref('');
const selected = ref(null);

// The fragments behind the nodes, once each, per chosen process (MapView is
// not under a ProcessView, which provides this for the "waarom?").
const fragments = computed(() => fragmentCache((path) => processApi(chosen.value).fragment(path)));
provide('fragment', (source) => fragments.value(source));

const { fitView } = useVueFlow({ id: 'map' });

async function load() {
  map.value = null;
  error.value = '';
  opened.value = new Set();
  event.value = '';
  selected.value = null;
  if (!chosen.value) return;
  try {
    map.value = await processApi(chosen.value).map();
  } catch (e) {
    error.value = e.message;
  }
}
watch(chosen, load, { immediate: true });

const events = computed(() => (map.value?.nodes ?? []).filter((n) => n.kind === 'event'));
const shown = computed(() => (map.value ? collapse(map.value, opened.value) : null));
const lit = computed(() => (map.value ? followEvent(map.value, event.value || null) : null));
const regulationOf = computed(() => new Map((map.value?.nodes ?? []).map((n) => [n.id, n.regulation])));
// A lit node as it is shown: itself, or the folded law of an article.
const shownId = (id) => {
  const r = regulationOf.value.get(id);
  return r && !opened.value.has(r) ? lawId(r) : id;
};
const litNodes = computed(() => (lit.value ? new Set([...lit.value.nodes].map(shownId)) : null));
const litEdges = computed(() =>
  lit.value
    ? new Set(
        [...lit.value.edges]
          .map((i) => map.value.edges[i])
          .map((e) => `${shownId(e.from)}|${shownId(e.to)}|${e.kind}`),
      )
    : null,
);

const flow = computed(() => {
  if (!shown.value) return { nodes: [], edges: [] };
  const pos = columnLayout(shown.value.nodes);
  const dim = (on) => (litNodes.value && !on ? { opacity: 0.2 } : {});
  return {
    nodes: shown.value.nodes.map((n) => ({
      id: n.id,
      position: pos.get(n.id),
      // vue-flow's default node shows `data.label`; the node itself rides along.
      data: { node: n, label: nodeText(n) },
      label: nodeText(n),
      sourcePosition: 'right',
      targetPosition: 'left',
      style: dim(litNodes.value?.has(n.id)),
    })),
    edges: shown.value.edges.map((e) => {
      const key = `${e.from}|${e.to}|${e.kind}`;
      const on = !litEdges.value || litEdges.value.has(key);
      return {
        id: key,
        source: e.from,
        target: e.to,
        // A dimmed edge loses its label: the followed chain reads without it.
        label: on ? e.kind : undefined,
        markerEnd: MarkerType.ArrowClosed,
        style: dim(on),
      };
    }),
  };
});

function click({ node }) {
  const n = node.data.node;
  if (n.kind === 'law') {
    opened.value = new Set([...opened.value, n.regulation]);
  } else {
    selected.value = n;
  }
}
function fold(regulation) {
  const s = new Set(opened.value);
  s.delete(regulation);
  opened.value = s;
}
</script>

<template>
  <nldd-title size="2"><h1>Opbouw</h1></nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-rich-text>
    <p>
      Welke configuratie, events, lexostatussen en wetsartikelen de runtime voor dit proces gebruikt. Een wet staat
      ingeklapt; klik erop om de artikelen te zien. Klik op een andere knoop voor het YAML-fragment dat de cel geladen
      heeft.
    </p>
  </nldd-rich-text>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-container layout="row" gap="16">
    <nldd-form-field v-if="processes.length > 1" label="Proces">
      <nldd-dropdown accessible-label="Proces">
        <select :value="chosen" @change="chosen = $event.target.value">
          <option v-for="p in processes" :key="p.id" :value="p.id">{{ p.id }}</option>
        </select>
      </nldd-dropdown>
    </nldd-form-field>
    <nldd-form-field label="Volg een event">
      <nldd-dropdown accessible-label="Volg een event">
        <select :value="event" @change="event = $event.target.value">
          <option value="">Alles tonen</option>
          <option v-for="e in events" :key="e.id" :value="e.id">{{ e.id.slice('event:'.length) }}</option>
        </select>
      </nldd-dropdown>
    </nldd-form-field>
  </nldd-container>
  <template v-if="opened.size">
    <nldd-spacer size="12"></nldd-spacer>
    <nldd-container layout="row" gap="8">
      <nldd-token v-for="r in opened" :key="r" control="dismiss" :dismiss-text="`Klap ${r} in`" @dismiss="fold(r)">{{ r }}</nldd-token>
    </nldd-container>
  </template>
  <nldd-spacer size="16"></nldd-spacer>
  <nldd-inline-dialog v-if="error" variant="alert" text="Het schema is niet te laden" :supporting-text="error"></nldd-inline-dialog>
  <div v-else-if="map" class="map">
    <VueFlow
      id="map"
      :key="chosen"
      :nodes="flow.nodes"
      :edges="flow.edges"
      :min-zoom="0.1"
      :nodes-draggable="false"
      :nodes-connectable="false"
      @node-click="click"
      @nodes-initialized="fitView()"
    />
  </div>
  <MapSheet :node="selected" @close="selected = null" />
</template>

<style scoped>
/* vue-flow draws into its parent and needs a height. */
.map {
  height: 75vh;
}
</style>
