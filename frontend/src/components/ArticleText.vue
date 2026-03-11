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
  <rr-box v-if="article" on-tinted style="border-radius: 16px; padding: 16px;">
    <p v-for="(para, i) in paragraphs" :key="i">
      <strong v-if="para.prefix">{{ para.prefix }}</strong>
      {{ para.prefix ? ' ' : '' }}{{ para.body }}
    </p>
  </rr-box>
  <div v-else style="padding: 32px; color: #666; text-align: center;">
    Geen artikel geselecteerd
  </div>
</template>
