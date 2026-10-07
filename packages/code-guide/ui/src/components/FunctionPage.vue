<script setup>
import { computed } from 'vue';
import { renderDocs } from '../lib/markdown.js';
import CallList from './CallList.vue';

// One free function: its signature, its doc comment, and the calls into and
// out of it.
const props = defineProps({ g: { type: Object, required: true } });
const f = computed(() => props.g.fnView);
const docsHtml = computed(() => renderDocs(f.value.docs));
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" :size="4" :text="`fn ${f.name}`" :heading-level="2" />
    <nldd-tag :text="f.vis" />
    <nldd-tag v-if="f.stale" color="warning" text="changed since the index" />
    <nldd-spacer size="12" />
    <nldd-code-viewer language="rust" no-copy wrap>{{ f.signature }}</nldd-code-viewer>
    <nldd-spacer size="12" />
    <!-- Rendered from the doc comment and sanitised by DOMPurify in renderDocs. -->
    <nldd-rich-text v-if="docsHtml" v-html="docsHtml" />
    <nldd-banner v-else variant="warning" :text="`${f.name} has no /// doc comment`" supporting-text="Add one above it in the source and it appears here." />
  </nldd-simple-section>
  <nldd-simple-section>
    <nldd-title slot="header" :size="4" :text="`Called from (${f.callers.length})`" :heading-level="2" />
    <CallList v-if="f.callers.length" :g="g" :label="`Functions that call ${f.name}`" :functions="f.callers" />
    <nldd-text v-else>Nothing in the workspace's production code calls {{ f.name }}.</nldd-text>
  </nldd-simple-section>
  <nldd-simple-section v-if="f.callees.length">
    <nldd-title slot="header" :size="4" :text="`Calls (${f.callees.length})`" :heading-level="2" />
    <CallList :g="g" :label="`Functions ${f.name} calls`" :functions="f.callees" />
  </nldd-simple-section>
</template>
