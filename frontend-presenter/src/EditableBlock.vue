<script setup>
import { computed, nextTick, ref } from 'vue';
import { renderBlock, renderInline } from './lib/renderMarkdown.js';
import WetBlock from './blocks/WetBlock.vue';
import MermaidBlock from './blocks/MermaidBlock.vue';

// One top-level markdown block of a slide. In edit mode a click turns it into
// a textarea with the block's own markdown source; blur or ⌘/Ctrl+Enter saves
// that text back over the block's range in the file, Esc cancels.
//
// `inline` renders only the block's inline content (no <h1>/<p> of its own),
// for a title or subtitle that the slide wraps in its own element.

const props = defineProps({
  block: { type: Object, required: true },
  editing: { type: Boolean, default: false },
  inline: { type: Boolean, default: false },
});
const emit = defineEmits(['commit']);

const open = ref(false);
const draft = ref('');
const area = ref(null);

const editable = computed(() => props.editing && props.block.start >= 0);
const piece = computed(() =>
  props.inline && 'text' in props.block.token && props.block.token.type !== 'code'
    ? { kind: 'html', html: renderInline(props.block.token.text) }
    : renderBlock(props.block),
);

async function start() {
  if (!editable.value || open.value) return;
  draft.value = props.block.raw.replace(/\n+$/, '');
  open.value = true;
  await nextTick();
  area.value?.focus();
  resize();
}
function resize() {
  const el = area.value;
  if (!el) return;
  el.style.height = 'auto';
  el.style.height = `${el.scrollHeight + 2}px`;
}
function commit() {
  if (!open.value) return;
  open.value = false;
  if (draft.value !== props.block.raw.replace(/\n+$/, '')) emit('commit', draft.value);
}
function cancel() {
  open.value = false;
}
function onKey(e) {
  if (e.key === 'Escape') {
    e.preventDefault();
    cancel();
  } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    commit();
  }
}
</script>

<template>
  <textarea
    v-if="open"
    ref="area"
    v-model="draft"
    class="block-editor"
    spellcheck="true"
    aria-label="Markdown van dit blok"
    @input="resize"
    @keydown="onKey"
    @blur="commit"
  ></textarea>
  <component
    :is="inline ? 'span' : 'div'"
    v-else
    class="block"
    :class="{ 'block-editable': editable, 'block-inline': inline }"
    :tabindex="editable ? 0 : undefined"
    :title="editable ? 'Klik om te bewerken' : undefined"
    @click="start"
    @keydown.enter.self.prevent="start"
  >
    <WetBlock v-if="piece.kind === 'wet' && !piece.error" :spec="piece.spec" />
    <p v-else-if="piece.kind === 'wet'" class="block-error">{{ piece.error }}</p>
    <MermaidBlock v-else-if="piece.kind === 'mermaid'" :source="piece.source" />
    <component :is="inline ? 'span' : 'div'" v-else class="block-html" v-html="piece.html"></component>
  </component>
</template>
