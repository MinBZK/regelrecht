<script setup>
defineProps({
  currentPage: { type: Number, required: true },
  totalPages: { type: Number, required: true },
  totalCount: { type: Number, required: true },
  unit: { type: String, default: 'results' },
});

const emit = defineEmits(['page-change']);

function onPageChange(event) {
  emit('page-change', event.detail.page);
}
</script>

<template>
  <div class="pagination-controls">
    <ndd-pagination
      :current="currentPage"
      :total="totalPages"
      @page-change="onPageChange"
    ></ndd-pagination>
    <span class="pagination-info">{{ totalCount }} {{ unit }}</span>
  </div>
</template>

<style scoped>
.pagination-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--primitives-space-8);
  padding: var(--primitives-space-16) 0;
}
</style>
