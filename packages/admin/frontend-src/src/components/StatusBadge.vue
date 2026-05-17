<script setup>
import { computed } from 'vue';
import { STATUS_BADGE_MAP } from '../constants.js';
import { formatStatus } from '../formatters.js';

const props = defineProps({
  status: { type: String, required: true },
  size: { type: String, default: 'sm' },
  errorMessage: { type: String, default: null },
});

// Colour stays keyed on the raw enum value; only the visible label is humanised.
const variant = computed(() => STATUS_BADGE_MAP[props.status] || 'neutral');
const label = computed(() => formatStatus(props.status));
</script>

<template>
  <nldd-tooltip v-if="errorMessage" :text="errorMessage" placement="top" timing="instant">
    <nldd-tag
      :color="variant"
      :text="label"
      :size="size"
      icon="info"
    />
  </nldd-tooltip>
  <nldd-tag
    v-else
    :color="variant"
    :text="label"
    :size="size"
  />
</template>
