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
      text="&#8249;"
      :disabled="currentPage <= 1 ? '' : undefined"
      title="Previous page"
      @click="emit('prev')"
    />
    <span class="pagination-info">
      {{ currentPage }} / {{ totalPages }} ({{ totalCount }} {{ unit }})
    </span>
    <ndd-button
      variant="neutral-tinted"
      size="md"
      text="&#8250;"
      :disabled="currentPage >= totalPages ? '' : undefined"
      title="Next page"
      @click="emit('next')"
    />
  </div>
</template>

<style scoped>
.pagination-controls {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8);
}
</style>
