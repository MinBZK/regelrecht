<script setup>
defineProps({
  currentPage: { type: Number, required: true },
  totalPages: { type: Number, required: true },
  totalCount: { type: Number, required: true },
  unit: { type: String, default: 'results' },
});

const emit = defineEmits(['prev', 'next']);
</script>

<template>
  <div class="pagination-controls">
    <rr-button
      variant="neutral-tinted"
      size="md"
      :disabled="currentPage <= 1 ? '' : undefined"
      title="Previous page"
      @click="emit('prev')"
    >&#8249;</rr-button>
    <span class="pagination-info">
      {{ currentPage }} / {{ totalPages }} ({{ totalCount }} {{ unit }})
    </span>
    <rr-button
      variant="neutral-tinted"
      size="md"
      :disabled="currentPage >= totalPages ? '' : undefined"
      title="Next page"
      @click="emit('next')"
    >&#8250;</rr-button>
  </div>
</template>

<style scoped>
.pagination-controls {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8);
}
</style>
