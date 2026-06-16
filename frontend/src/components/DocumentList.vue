<script setup>
// Presentational werkdocumenten list. Two consumers:
//  - the launcher sheet: pass `hrefFor` so each document row is a native link
//    that opens the standalone page in a new tab (open-new-page icon,
//    target=_blank) — middle-click and modifier-click work as expected;
//  - the standalone page's sidebar: no `hrefFor`, so rows are buttons that
//    select in place (chevron icon, emits `select`).
// "Nieuw document" is always a button (it creates, it has no stable URL).
const props = defineProps({
  documents: { type: Array, default: () => [] },
  // Highlight the open document (the page sidebar only).
  selectedPath: { type: String, default: null },
  // Maps a document path to a URL; when set, rows become new-tab links.
  hrefFor: { type: Function, default: null },
});
const emit = defineEmits(['select', 'new']);

function title(path) {
  return path ? path.replace(/\.md$/, '') : '';
}
// Link rows navigate natively; only in-place rows report a selection.
function onRowClick(path) {
  if (!props.hrefFor) emit('select', path);
}
</script>

<template>
  <nldd-list variant="simple">
    <nldd-list-item
      v-for="doc in documents"
      :key="doc.path"
      size="md"
      :button="hrefFor ? undefined : true"
      :href="hrefFor ? hrefFor(doc.path) : undefined"
      :target="hrefFor ? '_blank' : undefined"
      :rel="hrefFor ? 'noopener' : undefined"
      :selected="doc.path === selectedPath || undefined"
      @click="onRowClick(doc.path)"
    >
      <nldd-icon-cell size="20"><nldd-icon name="document"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell :text="title(doc.path)"></nldd-text-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-icon-cell size="20"><nldd-icon :name="hrefFor ? 'open-new-page' : 'chevron-right'"></nldd-icon></nldd-icon-cell>
    </nldd-list-item>
    <nldd-list-item size="md" button @click="$emit('new')">
      <nldd-icon-cell size="20"><nldd-icon name="plus"></nldd-icon></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell text="Nieuw document"></nldd-text-cell>
      <template v-if="hrefFor">
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-icon-cell size="20"><nldd-icon name="open-new-page"></nldd-icon></nldd-icon-cell>
      </template>
    </nldd-list-item>
  </nldd-list>
</template>
