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
      <nldd-split-view-pane slot="navigation">
        <nldd-container padding="12">
          <DocsSidebar
            :tree="tree"
            :selected-source="sourceId"
            :selected-path="path"
            @select="onSelect"
          />
        </nldd-container>
      </nldd-split-view-pane>

      <nldd-split-view-pane>
        <nldd-container padding="16">
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
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-navigation-split-view>
  </nldd-app-view>
</template>
