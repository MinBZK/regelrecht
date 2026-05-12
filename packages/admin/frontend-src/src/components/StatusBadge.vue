<script setup>
import { computed } from 'vue';
import { STATUS_BADGE_MAP } from '../constants.js';

const props = defineProps({
  status: { type: String, required: true },
  size: { type: String, default: 'sm' },
  errorMessage: { type: String, default: null },
});

const variant = computed(() => STATUS_BADGE_MAP[props.status] || 'neutral');
</script>

<template>
  <nldd-tooltip v-if="errorMessage" :text="errorMessage" placement="top">
    <nldd-tag
      :variant="variant"
      :text="status"
      :size="size"
      icon="info"
    />
  </nldd-tooltip>
  <nldd-tag
    v-else
    :variant="variant"
    :text="status"
    :size="size"
  />
</template>
