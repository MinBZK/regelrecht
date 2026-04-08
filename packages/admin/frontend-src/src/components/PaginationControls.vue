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
    <ndd-button
      variant="neutral-tinted"
      size="md"
      :disabled="currentPage <= 1 ? '' : undefined"
      title="Previous page"
      @click="emit('prev')"
    >&#8249;</ndd-button>
    <span class="pagination-info">
      {{ currentPage }} / {{ totalPages }} ({{ totalCount }} {{ unit }})
    </span>
    <ndd-button
      variant="neutral-tinted"
      size="md"
      :disabled="currentPage >= totalPages ? '' : undefined"
      title="Next page"
      @click="emit('next')"
    >&#8250;</ndd-button>
  </div>
</template>

<style scoped>
.pagination-controls {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8);
}
</style>
