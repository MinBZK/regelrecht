<script setup>
// Presentational werkdocumenten list — used both as the sheet's master view
// and the standalone page's sidebar. Emits select/new; the host drives the
// view transition (sheet detail vs route).
defineProps({
  documents: { type: Array, default: () => [] },
  // Highlight the open document (the page sidebar / detail-open sheet).
  selectedPath: { type: String, default: null },
});
defineEmits(['select', 'new']);

function title(path) {
  return path ? path.replace(/\.md$/, '') : '';
}
</script>

<template>
  <nldd-list variant="simple">
    <nldd-list-item
      v-for="doc in documents"
      :key="doc.path"
      size="md"
      button
      :selected="doc.path === selectedPath || undefined"
      @click="$emit('select', doc.path)"
    >
      <nldd-icon-cell size="20"><nldd-icon name="document"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell :text="title(doc.path)"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
    </nldd-list-item>
    <nldd-list-item size="md" button @click="$emit('new')">
      <nldd-icon-cell size="20"><nldd-icon name="plus"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell text="Nieuw document"></nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
</template>
