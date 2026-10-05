<script setup>
// The map of a process (spec 2): what the runtime uses, folded per law. A
// click on (or Enter on) a law opens it to articles, on another node shows
// its fragment. "Volg een event" lights up the chain of one event. The
// layout is a fixed column per kind (map.js); vue-flow only draws.
import { computed, provide, ref, watch } from 'vue';
import { VueFlow, MarkerType } from '@vue-flow/core';
import '@vue-flow/core/dist/style.css';
import { processApi } from '../api.js';
import { EDGE_TEXT, collapse, columnLayout, edgeKey, followEvent, litShown, nodeText } from '../map.js';
import { fragmentCache } from '../why.js';
import MapNode from '../components/MapNode.vue';
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
const fragments = computed(() => {
  const id = chosen.value;
  return fragmentCache((path) => processApi(id).fragment(path));
});
provide('fragment', (source) => fragments.value(source));

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
const lit = computed(() =>
  map.value ? litShown(map.value, followEvent(map.value, event.value || null), opened.value) : null,
);

// Dutch names for vue-flow's English roles on nodes and edges.
const NODE_ATTRS = { 'aria-roledescription': 'knoop' };
const EDGE_ATTRS = { 'aria-roledescription': 'verbinding' };

const flow = computed(() => {
  if (!shown.value) return { nodes: [], edges: [] };
  const pos = columnLayout(shown.value.nodes);
  const text = new Map(shown.value.nodes.map((n) => [n.id, nodeText(n)]));
  const dim = (on) => (on ? {} : { opacity: 0.2 });
  return {
    nodes: shown.value.nodes.map((n) => ({
      id: n.id,
      position: pos.get(n.id),
      data: n,
      style: dim(!lit.value || lit.value.nodes.has(n.id)),
      domAttributes: NODE_ATTRS,
    })),
    edges: shown.value.edges.map((e) => {
      const key = edgeKey(e.from, e.to, e.kind);
      const on = !lit.value || lit.value.edges.has(key);
      const kind = EDGE_TEXT[e.kind] ?? e.kind;
      return {
        id: key,
        source: e.from,
        target: e.to,
        // A dimmed edge loses its label: the followed chain reads without it.
        label: on ? kind : undefined,
        ariaLabel: `${text.get(e.from)} ${kind} ${text.get(e.to)}`,
        markerEnd: MarkerType.ArrowClosed,
        style: dim(on),
        domAttributes: EDGE_ATTRS,
      };
    }),
  };
});

function activate(n) {
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
  <nldd-inline-dialog
    v-if="!processes.length"
    icon="info"
    text="Geen processen"
    supporting-text="Deze runtime heeft geen processen, dus er is geen opbouw te tonen."
  ></nldd-inline-dialog>
  <template v-else>
    <nldd-rich-text>
      <p>
        Welke configuratie, cellen, events, lexostatussen en wetsartikelen de runtime voor dit proces gebruikt, ook
        van de cellen die het proces bevraagt. Een wet staat ingeklapt; kies hem om de artikelen te zien. Kies een
        andere knoop voor het YAML-fragment dat de cel geladen heeft; een configuratiebestand opent in zijn geheel.
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
      <!-- Remounts per process (map is null while loading), so it fits once
           per process and not on every opened law. The nodes are buttons
           (MapNode); vue-flow's own focus and keyboard selection are off. -->
      <VueFlow
        :nodes="flow.nodes"
        :edges="flow.edges"
        :min-zoom="0.1"
        fit-view-on-init
        :nodes-draggable="false"
        :nodes-connectable="false"
        :nodes-focusable="false"
        :edges-focusable="false"
        :elements-selectable="false"
        :disable-keyboard-a11y="true"
      >
        <template #node-default="{ data }"><MapNode :node="data" @activate="activate" /></template>
      </VueFlow>
    </div>
  </template>
  <MapSheet :node="selected" @close="selected = null" />
</template>

<style scoped>
/* vue-flow draws into its parent and needs a height; its nodes and edge
   labels take the page's font. */
.map {
  height: 75vh;
  font-family: var(--primitives-font-family-body);
}
</style>
