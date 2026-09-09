<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { VueFlow, useVueFlow } from '@vue-flow/core';
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';
import '@vue-flow/controls/dist/style.css';
import GraphLawNode from '../components/graph/GraphLawNode.vue';
import GraphBoxNode from '../components/graph/GraphBoxNode.vue';
import GraphItemNode from '../components/graph/GraphItemNode.vue';
import OrgLogo from '../components/OrgLogo.vue';
import LawGroupTree from '../components/LawGroupTree.vue';
import { buildGraph, lawShape, neighbourhood } from '../graph/lawGraph.js';
import { fieldSpec, formatValue } from '../data/format.js';
import { lineageFromTrace } from '../data/lineage.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { useDemo } from '../store/demoStore.js';

// The dependency graph as the POC drew it: every selected law and its direct
// neighbours as a box with its register sources, its inputs from other laws
// and its outputs (the whole corpus is laid out, so switching to "Alles" adds
// laws around the ones already in view without moving them), an edge
// from each input to the output that supplies it, and on every item the value
// the engine found for the active persona. The profile chooses the laws that
// tell its story (`graph_laws`); the sidebar lets the presenter add or drop
// laws. The canvas is vue-flow, as in the editor: the design system has no
// graph component (documented exception).

const router = useRouter();
const demo = useDemo();
const { corpus, profile, portalLaws, dataVersion } = demo;
// Same store id as the <VueFlow> below, otherwise fitView talks to a different instance.
const { fitView } = useVueFlow({ id: 'demo-graph' });
// The law list is a sheet, closed until asked for; the presets live in the toolbar.
const splitView = ref(null);

const allLaws = computed(() => (corpus.value ? [...corpus.value.latestById.values()].sort((a, b) => a.name.localeCompare(b.name)) : []));
const groups = computed(() => {
  const byService = new Map();
  for (const law of allLaws.value) {
    if (!byService.has(law.service)) byService.set(law.service, []);
    byService.get(law.service).push(law);
  }
  return [...byService.entries()].sort((a, b) => a[0].localeCompare(b[0])).map(([service, laws]) => ({ service, info: serviceInfo(corpus.value, service), laws }));
});

function referencesOf(law) {
  return lawShape(law).inputs.map((i) => i.ref.regulation).filter((id) => corpus.value.latestById.has(id));
}
/** A set of laws plus everything they read from, transitively. */
function withDependencies(laws) {
  const wanted = new Set();
  const queue = [...laws];
  while (queue.length) {
    const law = queue.shift();
    if (wanted.has(law.id)) continue;
    wanted.add(law.id);
    for (const ref of referencesOf(law)) queue.push(corpus.value.lawById(ref));
  }
  return wanted;
}

// ---- selection ----------------------------------------------------------------
const selected = ref(new Set());
const preset = ref('verhaal'); // 'verhaal' | 'portaal' | 'alles' | '' (hand-picked)

function applyPreset(name) {
  if (!corpus.value || !profile.value) return;
  preset.value = name;
  if (name === 'alles') {
    selected.value = new Set(allLaws.value.map((l) => l.id));
  } else if (name === 'portaal') {
    selected.value = withDependencies(portalLaws.value);
  } else {
    const story = (profile.value.graph_laws ?? []).filter((id) => corpus.value.latestById.has(id));
    if (story.length) selected.value = new Set(story);
    else {
      const d = profile.value.default_law;
      const law = d ? corpus.value.lawByPath(d.law_path, d.service) : null;
      selected.value = law ? withDependencies([law]) : new Set();
    }
  }
}
watch([profile, corpus], () => applyPreset('verhaal'), { immediate: true });

function toggle(lawId) {
  const next = new Set(selected.value);
  if (next.has(lawId)) next.delete(lawId);
  else next.add(lawId);
  selected.value = next;
  preset.value = '';
}
function only(lawId) {
  selected.value = new Set([lawId]);
  preset.value = '';
}
// As in the POC: the selected laws and everything directly connected to them.
const shownIds = computed(() => neighbourhood(selected.value, allLaws.value));
const shownLaws = computed(() => allLaws.value.filter((l) => shownIds.value.has(l.id)));

// ---- the persona's values ----------------------------------------------------
// Every shown law is evaluated for the active persona; the outputs go on the
// output items, the register values the trace resolved on the source items,
// and an input shows the value of the output it reads.
const values = computed(() => {
  void dataVersion.value;
  const outputs = {};
  const sources = {};
  const refs = new Map(); // `${law}#${output}` -> raw value, from cross-law references in traces
  const fmt = (lawId, name, v) => formatValue(v, fieldSpec(corpus.value.lawById(lawId)?.doc, name));
  for (const law of shownLaws.value) {
    let evaluation;
    try {
      evaluation = demo.evaluate(law);
    } catch {
      continue;
    }
    if (!evaluation?.ok) continue;
    outputs[law.id] = Object.fromEntries(Object.entries(evaluation.outputs ?? {}).map(([k, v]) => [k, fmt(law.id, k, v)]));
    const walk = (nodes) => {
      for (const n of nodes) {
        if (n.kind === 'value') {
          sources[n.law] ??= {};
          sources[n.law][n.name] = fmt(n.law, n.name, n.value);
        } else {
          refs.set(`${n.law}#${n.name}`, n.value);
          walk(n.children ?? []);
        }
      }
    };
    if (evaluation.trace) walk(lineageFromTrace(evaluation.trace, law.id, demo.personaParams()));
  }
  const inputs = {};
  for (const law of shownLaws.value) {
    for (const input of lawShape(law).inputs) {
      const key = `${input.ref.regulation}#${input.ref.output}`;
      const fromOutputs = outputs[input.ref.regulation]?.[input.ref.output];
      const v = fromOutputs ?? (refs.has(key) ? fmt(input.ref.regulation, input.ref.output, refs.get(key)) : undefined);
      if (v !== undefined) {
        inputs[law.id] ??= {};
        inputs[law.id][input.name] = v;
      }
    }
  }
  return { outputs, sources, inputs };
});

// ---- the graph ------------------------------------------------------------------
const focus = ref(null);
// Every law is laid out so the picture never shifts when "Alles" comes on;
// only the neighbourhood of the selection is visible.
const graph = computed(() => buildGraph(allLaws.value, values.value, focus.value, shownIds.value));

function onNodeClick({ node }) {
  if (node.type === 'law') focus.value = focus.value === node.id ? null : node.id;
  else if (node.type === 'item' && node.data.ref && selected.value.has(node.data.ref.regulation)) focus.value = node.data.ref.regulation;
}
function onNodeDoubleClick({ node }) {
  if (node.type === 'law') router.push(`/wetten/${encodeURIComponent(node.id)}`);
}
function onPaneClick() {
  focus.value = null;
}
function refit() {
  setTimeout(() => fitView({ padding: 0.1, nodes: [...shownIds.value] }), 50);
}
// vue-flow's fit-view-on-init runs before the nodes exist (the corpus arrives
// async); refit whenever the set of laws changes.
watch(() => [shownLaws.value.map((l) => l.id).join(), !!focus.value], async () => {
  await nextTick();
  refit();
});

const focusLaw = computed(() => (focus.value ? corpus.value?.lawById(focus.value) : null));
const uses = computed(() => (focus.value ? graph.value.edges.filter((e) => e.data.from === focus.value).map((e) => corpus.value.lawById(e.data.to)) : []));
const usedBy = computed(() => (focus.value ? graph.value.edges.filter((e) => e.data.to === focus.value).map((e) => corpus.value.lawById(e.data.from)) : []));
function unique(laws) {
  return [...new Map(laws.map((l) => [l.id, l])).values()];
}
</script>

<template>
  <nldd-navigation-split-view ref="splitView" primary-sidebar-as-sheet primary-sidebar-accessible-label="Wetten in de graaf" inspector-accessible-label="Geselecteerde wet">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar text="Wetten in de graaf" :supporting-text="`${selected.size} gekozen, ${shownLaws.length} in beeld`"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <LawGroupTree :groups="groups" mode="check" :checked="selected" :supporting-text="(law) => (!selected.has(law.id) && shownIds.has(law.id) ? 'in beeld als buur' : undefined)" @toggle="toggle" />
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="books" text="Wetten" :supporting-text="`${selected.size}`" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-title slot="start" text="Graaf" :supporting-text="`${shownLaws.length} wetten, ${graph.edges.filter((e) => !e.hidden).length} verwijzingen · waarden voor ${profile?.name ?? 'de persona'}`" max-width="480px"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end">
              <nldd-segmented-control size="sm" width="fit-content" :value="preset" @change="applyPreset($event.detail?.value)">
                <nldd-segmented-control-item value="verhaal" text="Verhaal"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="portaal" :text="profile?.name ?? 'Portaal'"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="alles" text="Alles"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="binoculars" text="Passend maken" @click="refit"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-simple-section v-if="!shownLaws.length" height="60vh">
          <nldd-inline-dialog icon="centralized-network" text="Geen wetten gekozen" supporting-text="Kies een preset of vink wetten aan.">
            <nldd-button slot="actions" variant="primary" size="sm" text="Wetten" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <div v-else class="graph-canvas">
          <VueFlow
            id="demo-graph"
            :nodes="graph.nodes"
            :edges="graph.edges"
            :min-zoom="0.1"
            :max-zoom="2"
            fit-view-on-init
            :nodes-connectable="false"
            :elevate-edges-on-select="false"
            @node-click="onNodeClick"
            @node-double-click="onNodeDoubleClick"
            @pane-click="onPaneClick"
            @nodes-initialized="refit"
          >
            <template #node-law="{ data }"><GraphLawNode :data="data" /></template>
            <template #node-box="{ data }"><GraphBoxNode :data="data" /></template>
            <template #node-item="{ data }"><GraphItemNode :data="data" /></template>
            <Background />
            <Controls :show-interactive="false" />
          </VueFlow>
        </div>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane v-if="focusLaw" slot="inspector" has-content background="tinted">
      <nldd-page background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="focusLaw.name" :supporting-text="serviceInfo(corpus, focusLaw.service).name" dismiss-text="Sluiten" @dismiss="focus = null"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-button-group orientation="horizontal" size="sm">
            <nldd-button variant="secondary" size="sm" start-icon="book" text="Open in Wetten" @click="router.push(`/wetten/${encodeURIComponent(focusLaw.id)}`)"></nldd-button>
            <nldd-button variant="neutral-tinted" size="sm" text="Alleen deze" @click="only(focusLaw.id)"></nldd-button>
          </nldd-button-group>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">Leest uit</nldd-text></nldd-container>
            <nldd-list variant="box-base" accessible-label="Leest uit">
              <nldd-list-item v-if="uses.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen andere wet in beeld"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="l in unique(uses)" :key="l.id" size="sm" button @click="focus = l.id">
                <nldd-text-cell size="sm" :text="l.name" :supporting-text="serviceInfo(corpus, l.service).name"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">Wordt gelezen door</nldd-text></nldd-container>
            <nldd-list variant="box-base" accessible-label="Wordt gelezen door">
              <nldd-list-item v-if="usedBy.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" text="Geen andere wet in beeld"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="l in unique(usedBy)" :key="l.id" size="sm" button @click="focus = l.id">
                <nldd-text-cell size="sm" :text="l.name" :supporting-text="serviceInfo(corpus, l.service).name"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
