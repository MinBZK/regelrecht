<script setup>
defineProps({
  currentPage: { type: Number, required: true },
  totalPages: { type: Number, required: true },
});

const emit = defineEmits(['page-change']);

function onPageChange(event) {
  emit('page-change', event.detail.page);
}
</script>

<template>
  <ndd-pagination
    full-width
    :current="currentPage"
    :total="totalPages"
    @page-change="onPageChange"
  ></ndd-pagination>
</template>
