<script setup>
import { computed } from 'vue';
import { marked } from 'marked';

const props = defineProps({
  article: { type: Object, default: null },
  raw: { type: Boolean, default: false },
});

const html = computed(() => {
  if (!props.article?.text) return '';
  return marked.parse(props.article.text);
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
