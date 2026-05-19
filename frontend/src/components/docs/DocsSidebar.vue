<script setup>
// Left-rail navigation for the docs view. One `<nldd-simple-section>` per
// source the user has access to (typically one per corpus repo attached
// to the traject), with files grouped by their top-level directory
// (analysis/, diagrams/, etc.) into separate `<nldd-list>` blocks. Mirrors
// the LibraryApp law/article sidebar pattern: nldd-list-item type="button"
// with :selected, click emits ['select', { sourceId, path }].

import { computed } from 'vue';

const props = defineProps({
  /** Shape: { sources: [{ source_id, name, tree: [{ path }] }] } */
  tree: { type: Object, default: () => ({ sources: [] }) },
  /** Currently selected page (highlights one row). */
  selectedSource: { type: String, default: '' },
  selectedPath: { type: String, default: '' },
});

defineEmits(['select']);

/** Group tree entries by their top-level folder so analysis/, diagrams/,
 * etc. each become a labelled sub-list inside the source's section. */
function group(tree) {
  const groups = new Map();
  for (const entry of tree || []) {
    const segments = entry.path.split('/');
    const top = segments.length > 1 ? segments[0] : '(root)';
    if (!groups.has(top)) groups.set(top, []);
    groups.get(top).push(entry);
  }
  return Array.from(groups.entries()).map(([name, items]) => ({ name, items }));
}

const groupedSources = computed(() =>
  (props.tree?.sources || []).map((s) => ({
    ...s,
    groups: group(s.tree),
  })),
);

function isSelected(sourceId, path) {
  return sourceId === props.selectedSource && path === props.selectedPath;
}

function prettify(path) {
  // Drop the .md extension and the folder prefix; capitalize-ish.
  const segments = path.replace(/\.md$/, '').split('/');
  const tail = segments[segments.length - 1];
  return tail.replace(/[-_]/g, ' ');
}
</script>

<template>
  <nldd-inline-dialog
    v-if="groupedSources.length === 0"
    text="Geen documentatie beschikbaar voor dit traject."
  ></nldd-inline-dialog>

  <nldd-simple-section
    v-for="src in groupedSources"
    :key="src.source_id"
    :heading="src.name"
  >
    <template v-for="grp in src.groups" :key="grp.name">
      <nldd-title :text="grp.name" size="sm" class="docs-group-title"></nldd-title>
      <nldd-list variant="simple">
        <nldd-list-item
          v-for="entry in grp.items"
          :key="entry.path"
          size="md"
          type="button"
          :selected="isSelected(src.source_id, entry.path) || undefined"
          @click="$emit('select', { sourceId: src.source_id, path: entry.path })"
        >
          <nldd-text-cell :text="prettify(entry.path)"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </nldd-simple-section>
</template>

<style scoped>
.docs-group-title {
  display: block;
  text-transform: capitalize;
  margin: 0.75rem 0 0.25rem;
}
</style>
