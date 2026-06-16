<script setup>
/**
 * TrajectDocuments — the werkdocumenten launcher sheet, opened from the traject
 * menu. It only lists the documents; opening or creating one always happens in
 * a separate browser tab on the standalone WerkdocumentenView page (there is no
 * in-sheet editing anymore). That keeps a single edit surface per document and
 * sidesteps the same-user self-conflict two live editors would invite.
 *
 * Mounted once per app. Uses useTrajectDocuments only for the list; the page
 * owns the actual editing state.
 */
import { computed, nextTick, ref, watch } from 'vue';
import { useTrajects } from '../composables/useTrajects.js';
import { useDocumentsSheet } from '../composables/useDocumentsSheet.js';
import { useTrajectDocuments } from '../composables/useTrajectDocuments.js';
import DocumentList from './DocumentList.vue';

const { activeTrajectRef, activeTraject } = useTrajects();
const { isOpen: browserOpen, close: closeBrowser } = useDocumentsSheet();
const { documents, loading: listLoading, listError, fetchList } = useTrajectDocuments(activeTrajectRef);

const browserEl = ref(null);

watch(browserOpen, async (o) => {
  await nextTick();
  if (o) {
    browserEl.value?.show();
    // Re-fetch on open so documents created in a page tab show up here.
    fetchList();
  } else {
    browserEl.value?.hide();
  }
});

function pageUrl(path) {
  const ref = activeTrajectRef.value;
  if (!ref) return null;
  return path ? `/werkdocumenten/${ref}/${path}` : `/werkdocumenten/${ref}`;
}

// Document rows are native new-tab links (DocumentList :href-for). "Nieuw
// document" has no stable URL — it is created on the page — so it stays a button
// that opens the page in a new tab. window.open sits directly in the click
// gesture here, so it isn't popup-blocked.
function openNew() {
  const url = pageUrl(null);
  if (url) window.open(url, '_blank', 'noopener');
}

const sheetTitle = computed(() =>
  activeTraject.value ? `Werkdocumenten · ${activeTraject.value.name}` : 'Werkdocumenten',
);
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="browserEl" placement="right" width="520px" full-height @close="closeBrowser">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="sheetTitle"
          dismiss-text="Sluit"
          @dismiss="closeBrowser"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-activity-indicator v-if="listLoading" text="Documenten laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog
            v-else-if="listError"
            variant="alert"
            text="Documenten niet geladen"
            :supporting-text="listError.message"
          ></nldd-inline-dialog>
          <DocumentList
            v-else
            variant="box"
            :documents="documents"
            :href-for="pageUrl"
            @new="openNew"
          ></DocumentList>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
