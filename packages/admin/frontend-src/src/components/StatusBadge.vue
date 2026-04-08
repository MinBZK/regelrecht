<script setup>
import { computed } from 'vue';
import { STATUS_BADGE_MAP } from '../constants.js';

const props = defineProps({
  status: { type: String, required: true },
});

const variant = computed(() => STATUS_BADGE_MAP[props.status] || 'grey');
</script>

<template>
  <span class="badge" :class="'badge--' + variant">{{ status }}</span>
</template>
