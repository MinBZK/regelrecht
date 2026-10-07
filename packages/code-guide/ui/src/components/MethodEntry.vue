<script setup>
import { computed } from 'vue';
import { renderDocs } from '../lib/markdown.js';
import CallList from './CallList.vue';

// One method of a type: its name as a link (selecting it shows its source and
// its calls), its signature as rust-analyzer prints it, its doc comment, and how
// many places call it and how many functions it calls. The selected method lists
// those calls, each a link.
const props = defineProps({
  g: { type: Object, required: true },
  owner: { type: Object, required: true },
  method: { type: Object, required: true },
  resolve: { type: Function, required: true },
});

const href = computed(() =>
  props.g.hrefFor({ module: props.owner.module, item: props.owner.name, method: props.method.key }),
);
const selected = computed(() => props.g.route.method === props.method.key);
const docsHtml = computed(() => renderDocs(props.method.docs, { resolve: props.resolve }));
const n = (list) => list.length;
</script>

<template>
  <!-- scroll-margin keeps a method that is scrolled to clear of the sticky header. -->
  <nldd-box
    :id="`method-${method.key}`"
    :background="selected ? 'tinted' : 'base'"
    style="scroll-margin-top: 10rem"
  >
    <nldd-container padding="16">
      <nldd-link :href="href" :text="method.key" :accessible-label="`${method.key}, show its source and calls`" />
      <nldd-tag v-if="selected" size="sm" color="accent" text="shown in source" />
      <nldd-tag v-if="method.vis !== 'pub'" size="sm" :text="method.vis === 'trait' ? `implements ${method.trait}` : method.vis" />
      <nldd-tag v-if="!method.docs" size="sm" color="warning" text="undocumented" />
      <nldd-tag
        size="sm"
        :color="n(method.callers) ? 'neutral' : 'warning'"
        :text="n(method.callers) ? `called from ${n(method.callers)}` : 'not called'"
      />
      <nldd-tag size="sm" :text="`calls ${n(method.callees)}`" />
      <nldd-tag v-if="method.stale" size="sm" color="warning" text="changed since the index" />
      <nldd-spacer size="8" />
      <nldd-code-viewer v-if="method.signature" language="rust" no-copy wrap>{{ method.signature }}</nldd-code-viewer>
      <template v-if="docsHtml">
        <nldd-spacer size="8" />
        <!-- Rendered from the doc comment and sanitised by DOMPurify in renderDocs. -->
        <nldd-rich-text v-html="docsHtml" />
      </template>
      <template v-if="selected">
        <template v-if="n(method.callers)">
          <nldd-spacer size="12" />
          <nldd-title :size="6" :text="`Called from (${n(method.callers)})`" :heading-level="4" />
          <CallList :g="g" :label="`Functions that call ${method.key}`" :functions="method.callers" />
        </template>
        <template v-if="n(method.callees)">
          <nldd-spacer size="12" />
          <nldd-title :size="6" :text="`Calls (${n(method.callees)})`" :heading-level="4" />
          <CallList :g="g" :label="`Functions ${method.key} calls`" :functions="method.callees" />
        </template>
      </template>
    </nldd-container>
  </nldd-box>
</template>
