<script setup>
// Root view for the /trajects/:trajectId/docs/:sourceId?/:path* route.
//
// Two-pane layout: DocsSidebar (left) lists the traject's corpus sources
// and their docs trees; DocsContent (right) renders the currently-selected
// page as markdown (+ mermaid).
//
// Selection state lives in the URL; clicks update the router and let route
// reactivity drive the data refetch. That way refresh/back/forward all
// work without local-state tricks.

import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';

import DocsSidebar from './components/docs/DocsSidebar.vue';
import DocsContent from './components/docs/DocsContent.vue';
import { useDocsPage, useDocsTree } from './composables/useDocs.js';

const route = useRoute();
const router = useRouter();

const trajectId = computed(() => route.params.trajectId || '');
const sourceId = computed(() => route.params.sourceId || '');
const path = computed(() => {
  const match = route.params.pathMatch;
  if (Array.isArray(match)) return match.join('/');
  return match || '';
});

const { tree, loading: treeLoading, error: treeError } = useDocsTree(trajectId);
const {
  text,
  loading: pageLoading,
  error: pageError,
} = useDocsPage(trajectId, sourceId, path);

function onSelect({ sourceId: src, path: p }) {
  router.push({
    name: 'traject-docs',
    params: {
      trajectId: trajectId.value,
      sourceId: src,
      pathMatch: p.split('/'),
    },
  });
}

const hasSelection = computed(() => sourceId.value && path.value);
</script>

<template>
  <div class="traject-docs">
    <DocsSidebar
      :tree="tree"
      :selected-source="sourceId"
      :selected-path="path"
      @select="onSelect"
    />
    <main class="docs-main">
      <div v-if="treeError" class="error">
        Kon docs-overzicht niet laden. Mogelijk ben je geen lid van dit traject.
      </div>
      <div v-else-if="!hasSelection && !treeLoading" class="placeholder">
        <h1>Documentatie</h1>
        <p>Kies een pagina links om te lezen.</p>
      </div>
      <div v-else-if="pageError" class="error">
        Kon de pagina niet laden.
      </div>
      <div v-else-if="pageLoading" class="placeholder">Laden…</div>
      <DocsContent v-else :text="text" />
    </main>
  </div>
</template>

<style scoped>
.traject-docs {
  display: grid;
  grid-template-columns: minmax(220px, 280px) 1fr;
  min-height: calc(100vh - var(--site-header-height, 56px));
}

.docs-main {
  padding: 1.5rem 2rem;
  overflow-x: hidden;
}

.placeholder {
  color: var(--nldd-color-text-subtle, #666);
}

.error {
  padding: 1rem;
  background: var(--nldd-color-danger-subtle, #fdecea);
  border-left: 4px solid var(--nldd-color-danger, #c1432a);
  border-radius: 2px;
  color: var(--nldd-color-danger-strong, #842c1e);
}

@media (max-width: 768px) {
  .traject-docs {
    grid-template-columns: 1fr;
  }
}
</style>
