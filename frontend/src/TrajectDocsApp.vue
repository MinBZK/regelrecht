<script setup>
// Root view for the /trajects/:trajectId/docs/:sourceId?/:path* route.
//
// Layout mirrors LibraryApp: `nldd-app-view` → `nldd-navigation-split-view`
// with a navigation pane on the left (DocsSidebar) and the main pane on
// the right (DocsContent). Selection state lives in the URL — clicks push
// the route, and the route drives all data refetches via the composables.

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

const { tree, error: treeError } = useDocsTree(trajectId);
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
  <nldd-app-view>
    <nldd-navigation-split-view>
      <!-- Sidebar pane. LibraryApp's nested split-view passes confirm the
           slot is `sidebar` (not `navigation`) and the main content is the
           default-slot pane without an explicit `slot=` attribute. -->
      <nldd-split-view-pane slot="sidebar" has-content>
        <nldd-page sticky-header>
          <nldd-top-title-bar slot="header" text="Documentatie"></nldd-top-title-bar>
          <nldd-simple-section width="full">
            <DocsSidebar
              :tree="tree"
              :selected-source="sourceId"
              :selected-path="path"
              @select="onSelect"
            />
          </nldd-simple-section>
        </nldd-page>
      </nldd-split-view-pane>

      <!-- Main content pane. nldd-navigation-split-view only reveals the
           main pane when `has-content` is truthy (same gate LibraryApp uses
           on its article pane); without it the pane stays collapsed and a
           click on the sidebar shows nothing. We always have *something* to
           show (placeholder / error / loading / page), so has-content is
           effectively always true here. -->
      <nldd-split-view-pane slot="main" has-content>
        <nldd-page sticky-header>
          <nldd-simple-section width="full">
            <nldd-inline-dialog
              v-if="treeError"
              text="Kon docs-overzicht niet laden. Mogelijk ben je geen lid van dit traject."
            ></nldd-inline-dialog>
            <nldd-inline-dialog
              v-else-if="!hasSelection"
              text="Kies een pagina links om te lezen."
            ></nldd-inline-dialog>
            <nldd-inline-dialog
              v-else-if="pageError"
              text="Kon de pagina niet laden."
            ></nldd-inline-dialog>
            <nldd-inline-dialog v-else-if="pageLoading" text="Laden…"></nldd-inline-dialog>
            <DocsContent v-else :text="text" />
          </nldd-simple-section>
        </nldd-page>
      </nldd-split-view-pane>
    </nldd-navigation-split-view>
  </nldd-app-view>
</template>
