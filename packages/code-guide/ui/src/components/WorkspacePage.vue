<script setup>
import { cycleTag } from '../lib/guide.js';
// All crates, in reading order: layer 0 calls into no other crate of the
// workspace, each later layer calls into the ones below it.
defineProps({ g: { type: Object, required: true } });
const byName = (g, name) => g.workspace.crates.find((c) => c.name === name);
const callsOut = (g, name) => g.workspace.edges.filter((e) => e.from === name);
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" :size="4" text="Crates in reading order" :heading-level="2" />
    <nldd-text>
      Layer 0 calls into no other crate of the workspace; each later layer calls into the ones below it. Only calls
      from production code count, as rust-analyzer resolved them.
    </nldd-text>
  </nldd-simple-section>
  <nldd-simple-section v-for="(layer, i) in g.workspace.layers" :key="i">
    <nldd-title slot="header" :size="5" :text="`Layer ${i}`" :heading-level="3" />
    <nldd-list type="list" :accessible-label="`Crates in layer ${i}`">
      <nldd-list-item v-for="name in layer.flat()" :key="name" :href="g.hrefFor({ crate: name })">
        <nldd-text-cell
          :text="name"
          :supporting-text="byName(g, name).description || 'No description'"
        />
        <nldd-cell v-if="byName(g, name).cycleWith.length">
          <nldd-tag
            size="sm"
            color="warning"
            :text="cycleTag(byName(g, name).cycleWith).text"
            :accessible-label="cycleTag(byName(g, name).cycleWith).label"
          />
        </nldd-cell>
        <nldd-cell>
          <nldd-tag size="sm" :text="`calls into ${callsOut(g, name).length} ${callsOut(g, name).length === 1 ? 'crate' : 'crates'}`" />
        </nldd-cell>
        <nldd-cell><nldd-tag size="sm" :text="`${byName(g, name).functions} functions`" /></nldd-cell>
      </nldd-list-item>
    </nldd-list>
  </nldd-simple-section>
</template>
