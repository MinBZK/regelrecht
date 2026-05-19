<script setup>
// Left-rail navigation for the docs view: one section per source attached
// to the traject, each section listing the markdown files in that source's
// docs/ tree. Clicking a file emits ['select', { sourceId, path }] so the
// parent (TrajectDocsApp) can update its router state.

import { computed } from 'vue';

const props = defineProps({
  /** Shape: { sources: [{ source_id, name, tree: [{ path }] }] } */
  tree: { type: Object, default: () => ({ sources: [] }) },
  /** Currently selected page (highlight in sidebar). */
  selectedSource: { type: String, default: '' },
  selectedPath: { type: String, default: '' },
});

defineEmits(['select']);

/** Group tree entries by their top-level folder so analysis/, diagrams/,
 * issues/ etc. each become a collapsible section per source. */
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
  // Drop the .md extension and the folder prefix for the label.
  const segments = path.replace(/\.md$/, '').split('/');
  const tail = segments[segments.length - 1];
  return tail.replace(/[-_]/g, ' ');
}
</script>

<template>
  <nav class="docs-sidebar" aria-label="Docs-navigatie">
    <div v-if="groupedSources.length === 0" class="empty">
      Geen documentatie beschikbaar voor dit traject.
    </div>
    <section
      v-for="src in groupedSources"
      :key="src.source_id"
      class="source-section"
    >
      <h2 class="source-name">{{ src.name }}</h2>
      <div v-for="grp in src.groups" :key="grp.name" class="group">
        <h3 class="group-name">{{ grp.name }}</h3>
        <ul>
          <li v-for="entry in grp.items" :key="entry.path">
            <button
              type="button"
              :class="{ active: isSelected(src.source_id, entry.path) }"
              @click="$emit('select', { sourceId: src.source_id, path: entry.path })"
            >
              {{ prettify(entry.path) }}
            </button>
          </li>
        </ul>
      </div>
    </section>
  </nav>
</template>

<style scoped>
.docs-sidebar {
  padding: 1rem;
  font-size: 0.9rem;
  border-right: 1px solid var(--nldd-color-border, #e5e5e5);
  overflow-y: auto;
}

.docs-sidebar .empty {
  color: var(--nldd-color-text-subtle, #666);
  font-style: italic;
}

.source-section + .source-section {
  margin-top: 1.25rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--nldd-color-border, #e5e5e5);
}

.source-name {
  font-size: 0.85rem;
  font-weight: 600;
  margin: 0 0 0.4rem;
  color: var(--nldd-color-text-subtle, #555);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.group-name {
  font-size: 0.78rem;
  font-weight: 600;
  margin: 0.6rem 0 0.25rem;
  color: var(--nldd-color-text, #333);
}

.docs-sidebar ul {
  list-style: none;
  margin: 0;
  padding: 0;
}

.docs-sidebar li {
  margin: 0.1rem 0;
}

.docs-sidebar button {
  display: block;
  width: 100%;
  padding: 0.3rem 0.5rem;
  background: transparent;
  border: none;
  text-align: left;
  font-size: inherit;
  font-family: inherit;
  color: inherit;
  border-radius: 3px;
  cursor: pointer;
  text-transform: capitalize;
}

.docs-sidebar button:hover {
  background: var(--nldd-color-surface-subtle, #f0f0f0);
}

.docs-sidebar button.active {
  background: var(--nldd-color-primary-subtle, #e6effe);
  color: var(--nldd-color-primary, #0066cc);
  font-weight: 500;
}
</style>
