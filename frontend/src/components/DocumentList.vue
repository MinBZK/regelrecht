<script setup>
// Presentational werkdocumenten list for the Home werkdocumenten sidebar: each
// row is a button that selects the document in place (emits `select`). Creating
// a new document is a toolbar action above the list, not a row here.
// In-progress / failed conversion jobs (from an upload) render as rows too, until
// the resulting .md lands in `documents`.
import { computed } from 'vue';

const props = defineProps({
  documents: { type: Array, default: () => [] },
  // Running / failed document-conversion jobs, shown as pending rows.
  jobs: { type: Array, default: () => [] },
  // Highlight the open document.
  selectedPath: { type: String, default: null },
});
defineEmits(['select']);

function title(path) {
  return path ? path.replace(/\.md$/, '') : '';
}
function jobTitle(job) {
  return title(job.target_path || 'document');
}

// Documents and conversion jobs in ONE list, sorted together by display name
// (numeric-aware, so untitled sorts before untitled-2 before untitled-10).
// Sorting jobs in place — rather than pinning them on top — keeps a converting
// row from jumping to a new position the moment it finishes and turns into a
// real document.
const rows = computed(() => {
  const items = [
    ...props.jobs.map((job) => ({ type: 'job', key: `job-${job.id}`, job, sortKey: jobTitle(job) })),
    ...props.documents.map((doc) => ({ type: 'doc', key: doc.path, doc, sortKey: title(doc.path) })),
  ];
  return items.sort((a, b) => a.sortKey.localeCompare(b.sortKey, 'nl', { numeric: true }));
});
</script>

<template>
  <nldd-list variant="simple" arrow-navigation>
    <template v-for="row in rows" :key="row.key">
      <!-- Conversion job: running rows open a loading main pane; failed rows show the error. -->
      <nldd-list-item
        v-if="row.type === 'job'"
        size="md"
        button
        :selected="row.job.target_path === selectedPath || undefined"
        @click="$emit('select', row.job.target_path)"
      >
        <nldd-cell v-if="row.job.status !== 'failed'" slot="start">
          <nldd-activity-indicator size="20" timing="instant"></nldd-activity-indicator>
        </nldd-cell>
        <nldd-icon-cell v-else slot="start" size="20">
          <nldd-icon name="alert"></nldd-icon>
        </nldd-icon-cell>
        <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
        <nldd-text-cell
          :text="jobTitle(row.job)"
          :supporting-text="row.job.status === 'failed' ? 'Conversie mislukt' : undefined"
        ></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
      </nldd-list-item>

      <!-- Document -->
      <nldd-list-item
        v-else
        size="md"
        button
        :selected="row.doc.path === selectedPath || undefined"
        @click="$emit('select', row.doc.path)"
      >
        <nldd-icon-cell slot="start" size="20"><nldd-icon name="text-document"></nldd-icon></nldd-icon-cell>
        <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
        <nldd-text-cell :text="title(row.doc.path)"></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-icon-cell size="20"><nldd-icon name="chevron-right"></nldd-icon></nldd-icon-cell>
      </nldd-list-item>
    </template>
  </nldd-list>
</template>
