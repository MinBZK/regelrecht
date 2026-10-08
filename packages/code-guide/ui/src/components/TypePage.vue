<script setup>
import { computed, nextTick, onMounted, watch } from 'vue';
import { visibleItems } from '../lib/guide.js';
import { renderDocs, selfResolver } from '../lib/markdown.js';
import MethodEntry from './MethodEntry.vue';

// One type: an index of its methods, its doc comment, and every method with its
// signature, docs, and the calls into and out of it.
const props = defineProps({ g: { type: Object, required: true } });

const t = computed(() => props.g.typeView);
// The method the address names is always shown, public or not: a link from a
// list of callers to a private method or a trait implementation must land on it.
const methods = computed(() => {
  const shown = new Set(visibleItems(t.value.methods, props.g.includePrivate));
  return t.value.methods.filter((m) => shown.has(m) || m.key === props.g.route.method);
});
const hidden = computed(() => t.value.methods.length - methods.value.length);
const documented = computed(() => methods.value.filter((m) => m.docs).length);
const resolve = computed(() =>
  selfResolver(
    t.value.methods.map((m) => m.key),
    (key) => props.g.hrefFor({ module: t.value.module, item: t.value.name, method: key }),
  ),
);
const docsHtml = computed(() => renderDocs(t.value.docs, { resolve: resolve.value }));
const indexColumns = computed(() => {
  const half = Math.ceil(methods.value.length / 2);
  return [methods.value.slice(0, half), methods.value.slice(half)];
});
const methodHref = (m) => props.g.hrefFor({ module: t.value.module, item: t.value.name, method: m.key });

// A selected method is scrolled into view, so a link from elsewhere lands on it.
function reveal() {
  const key = props.g.route.method;
  if (!key) return;
  nextTick(() => document.getElementById(`method-${key}`)?.scrollIntoView({ block: 'start' }));
}
const onIndexClick = (m) => {
  if (props.g.route.method === m.key) reveal();
};
onMounted(reveal);
watch(() => props.g.route.method, reveal);
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" :size="4" :text="`${t.kind} ${t.name}`" :heading-level="2" />
    <nldd-tag :text="t.vis" />
    <nldd-tag
      :text="`${methods.length} ${g.includePrivate ? '' : 'public '}${methods.length === 1 ? 'method' : 'methods'}, ${documented} documented`"
    />
  </nldd-simple-section>

  <nldd-one-half-one-half-section v-if="methods.length">
    <nldd-title slot="header" :size="5" text="Index" :heading-level="2" />
    <nldd-list
      v-for="(column, i) in indexColumns"
      :key="i"
      :slot="i === 0 ? 'left' : 'right'"
      type="list"
      :accessible-label="`Index of the methods of ${t.name}, column ${i + 1} of 2`"
    >
      <nldd-list-item
        v-for="m in column"
        :key="m.key"
        size="sm"
        :href="methodHref(m)"
        :selected="g.route.method === m.key"
        @click="onIndexClick(m)"
      >
        <nldd-text-cell size="sm" :text="m.key" :supporting-text="m.doc || 'No documentation'" />
      </nldd-list-item>
    </nldd-list>
  </nldd-one-half-one-half-section>

  <nldd-simple-section>
    <nldd-title slot="header" :size="5" text="Description" :heading-level="2" />
    <nldd-code-viewer language="rust" no-copy wrap>{{ t.signature }}</nldd-code-viewer>
    <nldd-spacer size="12" />
    <!-- Rendered from the doc comment and sanitised by DOMPurify in renderDocs. -->
    <nldd-rich-text v-if="docsHtml" v-html="docsHtml" />
    <nldd-banner v-else variant="warning" :text="`${t.name} has no /// doc comment`" supporting-text="Add one above it in the source and it appears here." />
  </nldd-simple-section>

  <nldd-simple-section v-if="t.methods.length">
    <nldd-title slot="header" :size="4" text="Methods" :heading-level="2" />
    <nldd-text v-if="hidden">
      {{ hidden }} non-public {{ hidden === 1 ? 'method is' : 'methods are' }} hidden. Switch on "Include non-public items" to see
      {{ hidden === 1 ? 'it' : 'them' }}.
    </nldd-text>
    <nldd-spacer v-if="hidden" size="12" />
    <template v-for="m in methods" :key="m.key">
      <MethodEntry :g="g" :owner="t" :method="m" :resolve="resolve" />
      <nldd-spacer size="12" />
    </template>
  </nldd-simple-section>
</template>
