<script setup>
import { computed } from 'vue';
import { moduleLabel } from '../lib/guide.js';
import CratePage from './CratePage.vue';
import FunctionPage from './FunctionPage.vue';
import GraphPane from './GraphPane.vue';
import ModulePage from './ModulePage.vue';
import TypePage from './TypePage.vue';
import WorkspacePage from './WorkspacePage.vue';

// The middle pane: a title bar with a way back up, the index's state when it is
// not fresh, a Details/Graph switch, and the page for what is open.
const props = defineProps({ g: { type: Object, required: true } });

const title = computed(() => {
  const g = props.g;
  const r = g.route;
  if (!r.crate) return 'All crates';
  if (r.module === null) return r.crate;
  const mod = moduleLabel(r.module, r.crate);
  return r.item ? `${r.module === '' ? r.crate : mod}::${r.item}` : mod;
});
const subtitle = computed(() => {
  const g = props.g;
  if (g.page === 'type') return g.typeView?.place.file ?? '';
  if (g.page === 'function') return g.fnView?.place.file ?? '';
  if (g.page === 'module') return g.module?.file ?? 'inline module';
  if (g.page === 'crate') return g.crateView?.dir ?? '';
  return g.workspace ? `${g.workspace.crates.length} crates` : '';
});
/** One level up: item -> module -> crate -> all crates. */
const back = computed(() => {
  const r = props.g.route;
  if (!r.crate) return null;
  if (r.module === null) return { href: props.g.hrefFor({ crate: null }), text: 'Back to all crates' };
  if (!r.item) return { href: props.g.hrefFor({}), text: `Back to ${r.crate}` };
  return { href: props.g.hrefFor({ module: r.module }), text: `Back to ${moduleLabel(r.module, r.crate)}` };
});

const index = computed(() => props.g.status?.index?.state ?? null);
const staleFiles = computed(() => props.g.status?.counts?.staleFiles ?? []);
const ready = computed(() => {
  const g = props.g;
  switch (g.page) {
    case 'workspace':
      return !!g.workspace;
    case 'crate':
      return !!g.crateView;
    case 'module':
      return !!g.module;
    case 'type':
      return !!g.typeView;
    default:
      return !!g.fnView;
  }
});

const onView = (event) => {
  globalThis.location.hash = props.g.hrefHere({ view: event.target.value });
};
</script>

<template>
  <nldd-page sticky-header accessible-label="Details">
    <nldd-container slot="header" padding="16">
      <nldd-top-title-bar :text="title" :supporting-text="subtitle" heading-level="1">
        <!-- The title bar's own back button only shows when the panes stack. -->
        <nldd-button v-if="back" slot="toolbar" :href="back.href" start-icon="back" :text="back.text" />
      </nldd-top-title-bar>
      <nldd-spacer size="8" />
      <nldd-segmented-control accessible-label="How to show it" :value="g.route.view" @change="onView">
        <nldd-segmented-control-item value="details" text="Details" :selected="g.route.view === 'details'" />
        <nldd-segmented-control-item value="graph" text="Graph" :selected="g.route.view === 'graph'" />
      </nldd-segmented-control>
    </nldd-container>

    <nldd-simple-section v-if="index === 'missing'">
      <nldd-banner
        variant="warning"
        text="There is no rust-analyzer index yet"
        :supporting-text="`Run ${g.status.command} to build it. It takes about a minute.`"
      />
    </nldd-simple-section>
    <nldd-simple-section v-else-if="index === 'stale'">
      <nldd-banner
        variant="warning"
        text="The index is older than the source"
        :supporting-text="
          staleFiles.length
            ? `${staleFiles.length} ${staleFiles.length === 1 ? 'file has' : 'files have'} changed since: ${staleFiles.slice(0, 5).join(', ')}${staleFiles.length > 5 ? ', …' : ''}. The guide shows them as they were indexed; run ${g.status.command} to catch up.`
            : `The toolchain or the Cargo files changed. Run ${g.status.command} to rebuild it.`
        "
      />
    </nldd-simple-section>

    <nldd-simple-section v-if="g.error">
      <nldd-banner variant="critical" text="Could not load this" :supporting-text="g.error" />
    </nldd-simple-section>
    <GraphPane v-else-if="g.route.view === 'graph'" :g="g" />
    <nldd-simple-section v-else-if="!ready">
      <nldd-text>Loading…</nldd-text>
    </nldd-simple-section>
    <WorkspacePage v-else-if="g.page === 'workspace'" :g="g" />
    <CratePage v-else-if="g.page === 'crate'" :g="g" />
    <ModulePage v-else-if="g.page === 'module'" :g="g" />
    <TypePage v-else-if="g.page === 'type'" :g="g" />
    <FunctionPage v-else :g="g" />
  </nldd-page>
</template>
