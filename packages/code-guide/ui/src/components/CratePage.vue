<script setup>
import { computed } from 'vue';
import { renderDocs } from '../lib/markdown.js';
import CallList from './CallList.vue';

// One crate: what it says about itself, and which other crates' modules it
// calls into and is called from.
const props = defineProps({ g: { type: Object, required: true } });
const c = computed(() => props.g.crateView);
const docsHtml = computed(() => renderDocs(c.value.docs ?? c.value.description));
const functions = computed(() => c.value.modules.reduce((n, m) => n + m.functions.length, 0));
const types = computed(() => c.value.modules.reduce((n, m) => n + m.types.length, 0));
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" :size="4" text="About this crate" :heading-level="2" />
    <!-- Rendered from lib.rs's //! comment (or the Cargo description), sanitised by renderDocs. -->
    <nldd-rich-text v-if="docsHtml" v-html="docsHtml" />
    <nldd-banner v-else variant="warning" text="This crate has no //! comment and no description" />
    <nldd-spacer size="12" />
    <nldd-tag :text="`${c.modules.length} modules`" />
    <nldd-tag :text="`${types} types`" />
    <nldd-tag :text="`${functions} free functions`" />
    <nldd-tag :text="`${c.layers.length} layers`" />
    <nldd-spacer size="12" />
    <nldd-text>Pick a module on the left, or switch to “Reading order” to read the crate from the modules others build on.</nldd-text>
  </nldd-simple-section>
  <nldd-simple-section v-if="c.callsInto.length">
    <nldd-title slot="header" :size="4" text="Calls into other crates" :heading-level="2" />
    <CallList :g="g" label="Modules of other crates this crate calls into" :modules="c.callsInto" />
  </nldd-simple-section>
  <nldd-simple-section v-if="c.calledFrom.length">
    <nldd-title slot="header" :size="4" text="Called from other crates" :heading-level="2" />
    <CallList :g="g" label="Modules of other crates that call this crate" :modules="c.calledFrom" />
  </nldd-simple-section>
</template>
