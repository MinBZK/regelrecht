<script setup>
import { computed } from 'vue';
import { serviceInfo } from '../data/loadCorpus.js';
import { useDemo } from '../store/demoStore.js';

// The mark of an organisation: its logo when the demo ships one, otherwise
// its abbreviation in a tag. Used on tiles and in the data lineage tree.
const props = defineProps({
  service: { type: String, required: true },
  size: { type: String, default: 'md' },
});
const { corpus } = useDemo();
const info = computed(() => (corpus.value ? serviceInfo(corpus.value, props.service) : { code: props.service, name: props.service, logo: null }));
</script>

<template>
  <nldd-tooltip :text="info.name">
    <span v-if="info.logo" :class="['org-logo', size === 'sm' ? 'org-logo--sm' : '']">
      <img :src="info.logo" :alt="info.name" />
    </span>
    <nldd-tag v-else :size="size === 'sm' ? 'sm' : 'md'" :text="info.code"></nldd-tag>
  </nldd-tooltip>
</template>
