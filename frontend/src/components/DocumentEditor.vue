<script setup>
/**
 * DocumentEditor — the active-document editor + preview + bottom toolbar,
 * driven by a useDocumentsManager instance passed in as `manager`. The host
 * (sheet master-detail or standalone page) owns the surrounding chrome
 * (title bar / page toolbar) and the list; this is purely the document body.
 *
 * Replaces the old draggable nldd-window: no window, no movable dialog — it
 * renders inline in whatever container the host gives it.
 */
import { nextTick, ref, watch } from 'vue';

const props = defineProps({
  manager: { type: Object, required: true },
  // When set, the more-menu offers "Open in nieuw tabblad" pointing here
  // (only meaningful in the sheet; the standalone page IS the tab).
  tabUrl: { type: String, default: null },
});
const emit = defineEmits(['deleted']);

// The manager is a stable object (created once by the host); destructure its
// refs so the template auto-unwraps them.
const m = props.manager;
const {
  currentPath,
  currentBody,
  docLoading,
  docError,
  saving,
  saveError,
  conflict,
  deletedRemotely,
  viewMode,
  previewHtml,
  creating,
  titleDraft,
  titleError,
  pendingDeletePath,
  deleteNotice,
  displayTitle,
  onBodyInput,
  onTitleInput,
  onViewModeChange,
  handleSave,
  undoChanges,
  overwriteServer,
  reloadCurrent,
  dropDraft,
  askDelete,
  cancelDelete,
  confirmDelete,
} = m;

// Ctrl/Cmd+S = opslaan.
function handleKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    if (currentPath.value && !saving.value) {
      e.preventDefault();
      handleSave();
    }
  }
}

async function onConfirmDelete() {
  const wasOpenDocument = await confirmDelete();
  // The host returns to the list / navigates when the open document is gone.
  if (wasOpenDocument) emit('deleted');
}

// Delete confirmation modal — imperative show()/hide() driven by the manager's
// pendingDeletePath, matching the editor's other NLDD dialogs.
const deleteModalEl = ref(null);
watch(pendingDeletePath, async (path) => {
  await nextTick();
  const el = deleteModalEl.value;
  if (!el) return;
  if (path) el.show?.();
  else el.hide?.();
});
</script>

<template>
  <div class="document-editor" @keydown="handleKeydown">
    <nldd-container padding-inline="16" padding-top="4" padding-bottom="8">
      <nldd-toolbar size="md">
        <nldd-toolbar-item slot="start">
          <nldd-segmented-control variant="icon" size="md" width="fit-content" :value="viewMode" @change="onViewModeChange">
            <nldd-segmented-control-item value="editor" icon="pencil-on-square" text="Bewerken"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="preview" icon="eye" text="Voorbeeld"></nldd-segmented-control-item>
          </nldd-segmented-control>
        </nldd-toolbar-item>
        <nldd-toolbar-item slot="end">
          <nldd-button
            variant="primary"
            size="md"
            :text="saving ? 'Opslaan…' : 'Opslaan'"
            :disabled="saving || undefined"
            @click="handleSave"
          ></nldd-button>
        </nldd-toolbar-item>
        <nldd-toolbar-item slot="end">
          <nldd-icon-button id="document-more-btn" size="md" icon="more" text="Meer" tooltip-timing="never" popovertarget="document-more-menu"></nldd-icon-button>
          <nldd-menu id="document-more-menu" anchor="document-more-btn">
            <nldd-menu-item text="Maak wijzigingen ongedaan" icon="undo" @click="undoChanges"></nldd-menu-item>
            <nldd-menu-item
              v-if="tabUrl"
              text="Open in nieuw tabblad"
              icon="external-link"
              :href="tabUrl"
              target="_blank"
            ></nldd-menu-item>
            <nldd-menu-divider></nldd-menu-divider>
            <nldd-menu-item text="Verwijder document" icon="delete" destructive @click="askDelete(currentPath)"></nldd-menu-item>
          </nldd-menu>
        </nldd-toolbar-item>
      </nldd-toolbar>
    </nldd-container>

    <nldd-container padding="16" padding-top="8">
      <nldd-activity-indicator v-if="docLoading || creating" text="Document laden" show-text></nldd-activity-indicator>
      <template v-else>
        <nldd-inline-dialog v-if="deleteNotice" variant="warning" :text="deleteNotice"></nldd-inline-dialog>
        <nldd-inline-dialog v-if="conflict" variant="warning" :text="conflict">
          <nldd-button slot="actions" size="md" text="Server-versie laden" @click="reloadCurrent"></nldd-button>
          <nldd-button slot="actions" size="md" text="Lokaal overschrijven" @click="overwriteServer"></nldd-button>
        </nldd-inline-dialog>
        <nldd-inline-dialog v-if="deletedRemotely" variant="warning" :text="deletedRemotely"></nldd-inline-dialog>
        <nldd-inline-dialog v-if="docError && docError.kind === 'draft-present'" :text="docError.message">
          <nldd-button slot="actions" size="md" text="Draft verwerpen" @click="dropDraft"></nldd-button>
        </nldd-inline-dialog>
        <nldd-inline-dialog v-if="docError && docError.kind !== 'draft-present'" variant="alert" :text="docError.message"></nldd-inline-dialog>
        <nldd-inline-dialog v-if="saveError" variant="alert" text="Actie mislukt" :supporting-text="saveError.message"></nldd-inline-dialog>
        <nldd-inline-dialog v-if="titleError" variant="alert" :text="titleError"></nldd-inline-dialog>

        <template v-if="viewMode === 'editor'">
          <nldd-text-field
            :value="titleDraft"
            accessible-label="Documentnaam"
            placeholder="documentnaam"
            @input="onTitleInput"
          ></nldd-text-field>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-multi-line-text-field
            :value="currentBody"
            rows="12"
            resize="auto"
            no-spellcheck
            accessible-label="Document-inhoud (markdown)"
            placeholder="# Titel"
            @input="onBodyInput"
          ></nldd-multi-line-text-field>
        </template>
        <!-- v-html is safe: renderArticleHtml runs DOMPurify over the marked
             output, identical to the law-text path. -->
        <nldd-rich-text v-else spacing="snug" v-html="previewHtml"></nldd-rich-text>
      </template>
    </nldd-container>

    <Teleport to="body">
      <nldd-modal-dialog
        ref="deleteModalEl"
        variant="alert"
        :text="pendingDeletePath ? `${displayTitle(pendingDeletePath)} verwijderen?` : 'Document verwijderen?'"
        supporting-text="Het document wordt definitief uit het traject verwijderd. Dit kan niet ongedaan worden gemaakt."
        @close="cancelDelete"
      >
        <nldd-button slot="actions" variant="primary" text="Behoud document" @click="cancelDelete"></nldd-button>
        <nldd-button slot="actions" variant="destructive" text="Verwijder" @click="onConfirmDelete"></nldd-button>
      </nldd-modal-dialog>
    </Teleport>
  </div>
</template>

<style scoped>
.document-editor {
  display: flex;
  min-height: 0;
  flex-direction: column;
}
</style>
