<script setup>
import { computed } from 'vue';
import { formatHash, moduleTree, targetOf } from '../lib/guide.js';
import ModuleRow from './ModuleRow.vue';

const props = defineProps({ g: { type: Object, required: true } });

/** The module tree of the open crate, flattened to rows with a depth. */
const rows = computed(() => {
  if (!props.g.crateView) return [];
  const out = [];
  const walk = (nodes, depth) => {
    for (const n of nodes) {
      out.push({ module: n.module, depth });
      walk(n.children, depth + 1);
    }
  };
  walk(moduleTree(props.g.crateView.modules), 0);
  return out;
});

const layers = computed(() => {
  const c = props.g.crateView;
  if (!c) return [];
  const byPath = new Map(c.modules.map((m) => [m.path, m]));
  return c.layers.map((components, layer) => ({
    layer,
    modules: components.flat().map((p) => byPath.get(p)).filter(Boolean),
  }));
});

// `nldd-dropdown` passes on the native `change` of its `<select>` and then fires
// one of its own from the host, without a `value`. Reading the select itself
// makes both say the same thing; choosing what is already open does nothing.
function onCrate(event) {
  const crate = event.currentTarget.querySelector('select')?.value ?? '';
  if (crate === (props.g.route.crate ?? '')) return;
  globalThis.location.hash = formatHash({ crate: crate || null, view: props.g.route.view });
}
const onSearch = (event) => {
  props.g.query = event.target.value;
};
const onView = (event) => {
  props.g.view = event.target.value;
};
const onPrivate = (event) => {
  props.g.includePrivate = event.target.checked;
};
const hitHref = (r) => props.g.hrefFor(targetOf(r));
</script>

<template>
  <nldd-page sticky-header background="tinted" accessible-label="Crates and modules">
    <nldd-container slot="header" padding="16">
      <nldd-top-title-bar text="Code guide" supporting-text="Calls resolved by rust-analyzer" heading-level="1" />
      <nldd-spacer size="12" />
      <nldd-dropdown accessible-label="Crate" width="100%" @change="onCrate">
        <select>
          <option value="" :selected="!g.route.crate">All crates</option>
          <option v-for="c in g.workspace?.crates ?? []" :key="c.name" :value="c.name" :selected="c.name === g.route.crate">
            {{ c.name }}
          </option>
        </select>
      </nldd-dropdown>
      <template v-if="g.route.crate">
        <nldd-spacer size="8" />
        <nldd-search-field
          accessible-label="Search modules, types, functions and methods"
          placeholder="Search this crate"
          width="100%"
          :value="g.query"
          @input="onSearch"
        />
        <nldd-spacer size="8" />
        <nldd-switch-field label="Include non-public items" :checked="g.includePrivate" @change="onPrivate" />
        <nldd-spacer v-if="!g.query" size="8" />
        <nldd-segmented-control v-if="!g.query" accessible-label="Group modules by" width="100%" :value="g.view" @change="onView">
          <nldd-segmented-control-item value="modules" text="Modules" :selected="g.view === 'modules'" />
          <nldd-segmented-control-item value="layers" text="Reading order" :selected="g.view === 'layers'" />
        </nldd-segmented-control>
      </template>
    </nldd-container>

    <!-- No crate open: the crates, in reading order. -->
    <nldd-simple-section v-if="!g.route.crate">
      <nldd-text>Crates in reading order: layer 0 calls into no other crate, each later layer calls into the ones below.</nldd-text>
      <template v-for="(layer, i) in g.workspace?.layers ?? []" :key="i">
        <nldd-spacer size="16" />
        <nldd-title :size="5" :text="`Layer ${i}`" :heading-level="2" />
        <nldd-list type="list" :accessible-label="`Crates in layer ${i}`">
          <nldd-list-item v-for="name in layer.flat()" :key="name" :href="g.hrefFor({ crate: name })">
            <nldd-text-cell
              :text="name"
              :supporting-text="g.workspace.crates.find((c) => c.name === name)?.description || 'No description'"
            />
          </nldd-list-item>
        </nldd-list>
      </template>
    </nldd-simple-section>

    <nldd-simple-section v-else-if="g.loading && !g.crateView">
      <nldd-text>Loading the crate…</nldd-text>
    </nldd-simple-section>

    <nldd-simple-section v-else-if="g.query">
      <nldd-text v-if="g.results.total === 0">Nothing in {{ g.route.crate }} matches “{{ g.query }}”.</nldd-text>
      <template v-else>
        <nldd-text>
          {{ g.results.total }} {{ g.results.total === 1 ? 'match' : 'matches'
          }}{{ g.results.total > g.results.results.length ? `, showing the first ${g.results.results.length}` : '' }}
        </nldd-text>
        <nldd-list type="navigation" aria-label="Search results">
          <nldd-list-item v-for="r in g.results.results" :key="`${r.kind}/${r.module}/${r.type}/${r.key}/${r.name}`" :href="hitHref(r)">
            <nldd-text-cell :text="r.name" :supporting-text="r.doc || r.module || 'crate root'" />
            <nldd-cell><nldd-tag size="sm" :text="r.kind" /></nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>
    </nldd-simple-section>

    <nldd-simple-section v-else-if="g.view === 'modules'">
      <nldd-list type="navigation" aria-label="Modules">
        <ModuleRow v-for="r in rows" :key="r.module.path" :g="g" :module="r.module" :depth="r.depth" />
      </nldd-list>
    </nldd-simple-section>

    <nldd-simple-section v-else>
      <nldd-text>Layer 0 calls into no other module of this crate. Each later layer calls into the ones below it.</nldd-text>
      <template v-for="l in layers" :key="l.layer">
        <nldd-spacer size="16" />
        <nldd-title :size="5" :text="`Layer ${l.layer}`" :heading-level="2" />
        <nldd-list type="list" :accessible-label="`Layer ${l.layer}`">
          <ModuleRow v-for="m in l.modules" :key="m.path" :g="g" :module="m" />
        </nldd-list>
      </template>
    </nldd-simple-section>
  </nldd-page>
</template>
