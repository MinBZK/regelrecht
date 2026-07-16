<script setup>
// Presentational werkdocumenten list for the Home werkdocumenten sidebar: each
// row is a button that selects the document in place (emits `select`). Creating
// a new document is a toolbar action above the list, not a row here.
import { docDisplayTitle } from '../lib/docTitle.js';

defineProps({
  documents: { type: Array, default: () => [] },
  // Highlight the open document.
  selectedPath: { type: String, default: null },
});
defineEmits(['select']);
</script>

<template>
  <nldd-list variant="simple" arrow-navigation empty-text="Geen werkdocumenten">
    <nldd-list-item
      v-for="doc in documents"
      :key="doc.path"
      size="md"
      button
      :selected="doc.path === selectedPath || undefined"
      @click="$emit('select', doc.path)"
    >
      <nldd-icon-cell slot="start" size="20"><nldd-icon name="text-document"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
      <nldd-text-cell :text="docDisplayTitle(doc)"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
    </nldd-list-item>
  </nldd-list>
</template>
