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
  <nldd-pagination
    v-if="totalPages > 1"
    full-width
    :current="currentPage"
    :total="totalPages"
    @page-change="onPageChange"
  ></nldd-pagination>
</template>
