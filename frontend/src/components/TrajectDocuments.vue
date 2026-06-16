<script setup>
/**
 * TrajectDocuments — the werkdocumenten browser sheet, opened from the traject
 * menu. Master-detail inside one right-side sheet: the file list, and — drilled
 * into via the chevron — the document editor (DocumentEditor). The old
 * draggable nldd-window is gone; for more room, the detail offers "open in
 * nieuw tabblad" to the standalone WerkdocumentenView page.
 *
 * Mounted once per app; the document state lives in useDocumentsManager (shared
 * by the list and the editor so a save/delete refreshes the list).
 */
import { computed, nextTick, ref, watch } from 'vue';
import { useTrajects } from '../composables/useTrajects.js';
import { useDocumentsSheet } from '../composables/useDocumentsSheet.js';
import { useDocumentsManager } from '../composables/useDocumentsManager.js';
import DocumentList from './DocumentList.vue';
import DocumentEditor from './DocumentEditor.vue';

const { activeTrajectRef, activeTraject } = useTrajects();
const { isOpen: browserOpen, close: closeBrowser } = useDocumentsSheet();

const mgr = useDocumentsManager(activeTrajectRef);
const { documents, listLoading, listError, currentPath, displayTitle, open, startNew } = mgr;

const browserEl = ref(null);
const mode = ref('list'); // 'list' | 'detail'

watch(browserOpen, async (o) => {
  await nextTick();
  if (o) browserEl.value?.show();
  else browserEl.value?.hide();
});

// Switching traject clears the loaded document; reset to the list and close.
watch(activeTrajectRef, () => {
  mode.value = 'list';
  closeBrowser();
});

function onBrowserClose() {
  closeBrowser();
  mode.value = 'list';
}

async function onSelect(path) {
  await open(path);
  mode.value = 'detail';
}
async function onNew() {
  const path = await startNew();
  if (path) mode.value = 'detail';
}
function backToList() {
  mode.value = 'list';
}

// Deep link to the standalone page for the open document.
const tabUrl = computed(() =>
  activeTrajectRef.value && currentPath.value
    ? `/werkdocumenten/${activeTrajectRef.value}/${currentPath.value}`
    : null,
);

const sheetTitle = computed(() => {
  if (mode.value === 'detail') return displayTitle(currentPath.value) || 'Document';
  return activeTraject.value ? `Werkdocumenten · ${activeTraject.value.name}` : 'Werkdocumenten';
});
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="browserEl" placement="right" width="520px" full-height @close="onBrowserClose">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="sheetTitle"
          :back-text="mode === 'detail' ? 'Werkdocumenten' : undefined"
          dismiss-text="Sluit"
          @dismiss="closeBrowser"
          @back="backToList"
        ></nldd-top-title-bar>

        <!-- Detail: de document-editor -->
        <DocumentEditor
          v-if="mode === 'detail'"
          :manager="mgr"
          :tab-url="tabUrl"
          @deleted="backToList"
        />

        <!-- Master: de documentenlijst -->
        <nldd-simple-section v-else>
          <nldd-activity-indicator v-if="listLoading" text="Documenten laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog
            v-else-if="listError"
            variant="alert"
            text="Documenten niet geladen"
            :supporting-text="listError.message"
          ></nldd-inline-dialog>
          <DocumentList
            v-else
            :documents="documents"
            :selected-path="currentPath"
            @select="onSelect"
            @new="onNew"
          ></DocumentList>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
