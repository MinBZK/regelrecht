<script setup>
import { computed } from 'vue';
import { renderArticleHtml } from '../composables/useArticleMarkdown.js';

const props = defineProps({
  article: { type: Object, default: null },
  raw: { type: Boolean, default: false },
  // Center the rich-text column. Opt-in so the library reading view can
  // center while the editor's read-only pane stays left-aligned.
  centered: { type: Boolean, default: false },
});

// Shared marked + DOMPurify pipeline so the notes-on view (AnnotatedText)
// renders byte-identically to this notes-off view (#646).
const html = computed(() => renderArticleHtml(props.article?.text || ''));

const paragraphs = computed(() => {
  if (!props.article?.text) return [];
  return props.article.text.split(/\n\n+/);
});
</script>

<template>
  <template v-if="article">
    <nldd-rich-text v-if="raw" :centered="centered || undefined">
      <p v-for="(p, i) in paragraphs" :key="i">{{ p }}</p>
    </nldd-rich-text>
    <nldd-rich-text v-else :centered="centered || undefined" v-html="html"></nldd-rich-text>
  </template>
  <nldd-inline-dialog v-else text="Geen artikel geselecteerd"></nldd-inline-dialog>
</template>
