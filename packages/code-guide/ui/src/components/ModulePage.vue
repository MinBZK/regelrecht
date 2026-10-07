<script setup>
import { computed } from 'vue';
import { visibleItems } from '../lib/guide.js';
import { renderDocs } from '../lib/markdown.js';
import CallList from './CallList.vue';
import ItemRow from './ItemRow.vue';

// One module: its //! comment, the modules it calls into and is called from
// (across crates), and its types and free functions.
const props = defineProps({ g: { type: Object, required: true } });
const m = computed(() => props.g.module);
const docsHtml = computed(() => renderDocs(m.value.docs ?? m.value.doc));
const types = computed(() => visibleItems(m.value.types, props.g.includePrivate));
const fns = computed(() => visibleItems(m.value.functions, props.g.includePrivate));
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" :size="4" text="About this module" :heading-level="2" />
    <!-- The module's //! comment, rendered and sanitised by renderDocs. -->
    <nldd-rich-text v-if="docsHtml" v-html="docsHtml" />
    <nldd-banner v-else variant="warning" text="This module has no //! doc comment" supporting-text="Add one at the top of its file and it appears here." />
    <nldd-spacer size="12" />
    <nldd-tag :text="`layer ${m.layer}`" />
    <nldd-tag :text="m.vis" />
    <nldd-tag v-if="m.cycleWith.length" color="warning" :text="`calls in a cycle with ${m.cycleWith.join(', ')}`" />
  </nldd-simple-section>
  <nldd-simple-section v-if="m.callsInto.length || m.calledFrom.length">
    <nldd-title slot="header" :size="4" text="How it connects" :heading-level="2" />
    <template v-if="m.callsInto.length">
      <nldd-title :size="6" text="Calls into" :heading-level="3" />
      <CallList :g="g" label="Modules this module calls into" :modules="m.callsInto" />
    </template>
    <template v-if="m.calledFrom.length">
      <nldd-spacer size="12" />
      <nldd-title :size="6" text="Called from" :heading-level="3" />
      <CallList :g="g" label="Modules that call this module" :modules="m.calledFrom" />
    </template>
  </nldd-simple-section>
  <nldd-simple-section v-if="types.length">
    <nldd-title slot="header" :size="4" text="Types" :heading-level="2" />
    <nldd-list type="list" accessible-label="Types">
      <ItemRow v-for="t in types" :key="t.name" :g="g" :module="m" :item="t" :kind="t.kind" />
    </nldd-list>
  </nldd-simple-section>
  <nldd-simple-section v-if="fns.length">
    <nldd-title slot="header" :size="4" text="Functions" :heading-level="2" />
    <nldd-list type="list" accessible-label="Functions">
      <ItemRow v-for="f in fns" :key="f.key" :g="g" :module="m" :item="f" kind="fn" />
    </nldd-list>
  </nldd-simple-section>
  <nldd-simple-section v-if="!types.length && !fns.length">
    <nldd-text>This module declares no {{ g.includePrivate ? '' : 'public ' }}types or functions of its own.</nldd-text>
  </nldd-simple-section>
</template>
