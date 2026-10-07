<script setup>
import { moduleLabel, refTarget } from '../lib/guide.js';

// Calls as a list of links: either functions (a method's callers or callees),
// or modules (what a module or crate calls into, or is called from). Every row
// links to the thing at the other end of the call.
const props = defineProps({
  g: { type: Object, required: true },
  label: { type: String, required: true },
  functions: { type: Array, default: null },
  modules: { type: Array, default: null },
});

const fnText = (r) => (r.type ? `${r.type}::${r.name}` : r.name);
const where = (crate, module) => `${crate}::${moduleLabel(module, crate)}`;
const count = (n) => (n === 1 ? '1 call' : `${n} calls`);
const fnHref = (r) => props.g.hrefFor(refTarget(r));
const moduleHref = (l) => props.g.hrefFor({ crate: l.crate, module: l.module });
</script>

<template>
  <nldd-list type="list" :accessible-label="label">
    <template v-if="functions">
      <nldd-list-item v-for="r in functions" :key="`${r.crate}/${r.module}/${r.type}/${r.key}`" size="sm" :href="fnHref(r)">
        <nldd-text-cell size="sm" :text="fnText(r)" :supporting-text="where(r.crate, r.module)" />
        <nldd-cell v-if="r.sites > 1"><nldd-tag size="sm" :text="count(r.sites)" /></nldd-cell>
      </nldd-list-item>
    </template>
    <template v-else>
      <nldd-list-item v-for="l in modules" :key="`${l.crate}/${l.module}`" size="sm" :href="moduleHref(l)">
        <nldd-text-cell size="sm" :text="where(l.crate, l.module)" />
        <nldd-cell><nldd-tag size="sm" :text="count(l.calls)" /></nldd-cell>
      </nldd-list-item>
    </template>
  </nldd-list>
</template>
