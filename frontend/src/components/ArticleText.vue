<script setup>
import { computed } from 'vue';
import { marked } from 'marked';
import DOMPurify from 'dompurify';

const props = defineProps({
  article: { type: Object, default: null },
  raw: { type: Boolean, default: false },
});

// marked v18 no longer sanitizes HTML in Markdown by default; run its output
// through DOMPurify before binding with v-html so law text (today author-
// controlled, but harvested laws could introduce arbitrary HTML) cannot inject
// <script>, event handlers or javascript: links.
const html = computed(() => {
  if (!props.article?.text) return '';
  return DOMPurify.sanitize(marked.parse(props.article.text));
});

const paragraphs = computed(() => {
  if (!props.article?.text) return [];
  return props.article.text.split(/\n\n+/);
});
</script>

<template>
  <template v-if="article">
    <nldd-rich-text v-if="raw">
      <p v-for="(p, i) in paragraphs" :key="i">{{ p }}</p>
    </nldd-rich-text>
    <nldd-rich-text v-else v-html="html"></nldd-rich-text>
  </template>
  <nldd-inline-dialog v-else text="Geen artikel geselecteerd"></nldd-inline-dialog>
</template>
