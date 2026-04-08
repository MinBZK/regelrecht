<script setup>
import { computed } from 'vue';

const props = defineProps({
  article: { type: Object, default: null },
});

const paragraphs = computed(() => {
  if (!props.article?.text) return [];
  return props.article.text.split('\n\n').map((block) => {
    const match = block.match(/^(\d+[a-z]?\.)\s*/);
    if (match) {
      return { prefix: match[1], body: block.slice(match[0].length) };
    }
    // Check for letter prefixes like "a. ", "b. "
    const letterMatch = block.match(/^([a-z]\.)\s*/);
    if (letterMatch) {
      return { prefix: letterMatch[1], body: block.slice(letterMatch[0].length) };
    }
    return { prefix: null, body: block };
  });
});
</script>

<template>
  <ndd-simple-section v-if="article">
    <ndd-rich-text>
      <p v-for="(para, i) in paragraphs" :key="i">
        <strong v-if="para.prefix">{{ para.prefix }}</strong>
        {{ para.prefix ? ' ' : '' }}{{ para.body }}
      </p>
    </ndd-rich-text>
  </ndd-simple-section>
  <ndd-simple-section v-else align="center">
    <ndd-inline-dialog text="Geen artikel geselecteerd"></ndd-inline-dialog>
  </ndd-simple-section>
</template>
