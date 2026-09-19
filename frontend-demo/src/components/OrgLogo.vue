<script setup>
import { computed } from 'vue';
import { serviceInfo } from '../data/loadCorpus.js';
import { useDemo } from '../store/demoStore.js';

// The mark of an organisation: its logo in a padded tile when the demo ships
// one, otherwise a design-system avatar with its initials (a short code such as
// CBS or UWV as-is, else derived from the name). Used on tiles, in the law
// browser and in the data lineage.
const props = defineProps({
  service: { type: String, required: true },
  size: { type: String, default: 'md' },
});
const { corpus } = useDemo();
const info = computed(() => (corpus.value ? serviceInfo(corpus.value, props.service) : { code: props.service, name: props.service, logo: null }));
const initials = computed(() => (info.value.code.length <= 3 ? info.value.code : undefined));
const px = computed(() => (props.size === 'sm' ? '24' : '40'));
</script>

<template>
  <nldd-tooltip v-if="info.logo" :text="info.name">
    <span :class="['org-logo', size === 'sm' ? 'org-logo--sm' : '']">
      <img :src="info.logo" :alt="info.name" />
    </span>
  </nldd-tooltip>
  <nldd-avatar v-else type="organization" :size="px" :name="info.name" :initials="initials"></nldd-avatar>
</template>
