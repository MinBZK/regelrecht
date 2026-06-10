<script setup>
/**
 * TrajectDocuments — markdown/plain-text documents that live in a traject's
 * corpus branch.
 *
 * Two NLDD overlays, mounted once per app and triggered from the
 * TrajectMenu ("Documenten…"):
 *   1. A browser **sheet** (nldd-sheet) — the file list + create + delete,
 *      mirroring the "Nieuw traject" sheet flow.
 *   2. An **edit window** (nldd-window, modeless + movable) — the active
 *      document's markdown editor with a live preview, so it can be dragged
 *      aside while the law text stays visible.
 *
 * Built entirely from NLDD design-system components (no bespoke markup or
 * CSS): list/list-item/button for the file list, multi-line-text-field for
 * the editor, rich-text for the preview, inline-dialog for every banner.
 */
import { computed, nextTick, ref, watch } from 'vue';
import { useTrajects } from '../composables/useTrajects.js';
import { useTrajectDocuments } from '../composables/useTrajectDocuments.js';
import { renderArticleHtml } from '../composables/useArticleMarkdown.js';
import { useDocumentsSheet } from '../composables/useDocumentsSheet.js';

// Self-source the active traject so the component can be dropped into any
// app (EditorApp / LibraryApp) with no props.
const { activeTrajectRef } = useTrajects();
const { isOpen: browserOpen, close: closeBrowser } = useDocumentsSheet();

const {
  documents,
  loading: listLoading,
  listError,
  currentPath,
  currentBody,
  docLoading,
  docError,
  saving,
  saveError,
  conflict,
  deletedRemotely,
  openDocument,
  saveCurrent,
  reloadCurrent,
  createDocument,
  deleteDocument,
  dropDraft,
} = useTrajectDocuments(activeTrajectRef);

// Browser sheet (file list) — imperative show()/hide() driven by the shared
// flag, the same pattern the other NLDD sheets/dialogs use.
const browserEl = ref(null);
watch(browserOpen, async (open) => {
  await nextTick();
  if (open) browserEl.value?.show();
  else browserEl.value?.hide();
});

// Edit window (active document) — local flag.
const windowEl = ref(null);
const windowOpen = ref(false);
watch(windowOpen, async (open) => {
  await nextTick();
  if (open) windowEl.value?.show();
  else windowEl.value?.hide();
});

// Switching traject clears the loaded document in the composable; close both
// overlays so neither shows stale content for the new traject.
watch(activeTrajectRef, () => {
  windowOpen.value = false;
  closeBrowser();
});

const previewHtml = computed(() => renderArticleHtml(currentBody.value));

function onBodyInput(e) {
  currentBody.value = e.detail?.value ?? e.target?.value ?? currentBody.value;
}

// Open a document: close the browser sheet and show it in the window
// (mirrors the "Nieuw traject" flow where the sheet closes on action).
async function openInWindow(path) {
  await openDocument(path);
  closeBrowser();
  windowOpen.value = true;
}

// --- Create ---
const newPath = ref('');
const createError = ref(null);
const submittingCreate = ref(false);

function onNewPathInput(e) {
  newPath.value = e.detail?.value ?? e.target?.value ?? '';
}

// Lightweight client-side validation mirroring the backend rules so the user
// gets immediate feedback instead of a 400.
function validateNewPath(value) {
  if (!value) return 'Geef een naam op.';
  if (value.startsWith('/')) return "Pad mag niet beginnen met '/'.";
  if (value.includes('\\')) return 'Pad mag geen backslashes bevatten.';
  // No blanket `includes('..')` check: the backend only rejects `.` / `..`
  // as whole segments, which the per-segment `startsWith('.')` guard below
  // already covers; a substring check would also reject legitimate names
  // like `a..b.md`, diverging from the backend's authoritative validation.
  const segments = value.split('/');
  for (const seg of segments) {
    if (!seg) return 'Pad bevat lege segmenten.';
    if (seg.startsWith('.')) return "Pad mag geen verborgen segmenten ('.') bevatten.";
    if (!/^[a-z0-9._-]+$/.test(seg)) {
      return "Gebruik alleen kleine letters, cijfers en '._-'.";
    }
  }
  if (!/\.(md|txt)$/.test(value)) return 'Naam moet eindigen op .md of .txt.';
  return null;
}

async function submitCreate() {
  // Guard against a double-fire: the button @click and an Enter-submit can
  // both invoke this in the same turn. `submittingCreate` is set
  // synchronously before the first `await`, so the second caller already
  // sees `true` here and exits instead of issuing a duplicate create.
  if (submittingCreate.value) return;
  createError.value = null;
  const value = newPath.value.trim();
  const err = validateNewPath(value);
  if (err) {
    createError.value = err;
    return;
  }
  if (documents.value.some((d) => d.path === value)) {
    createError.value = 'Een document met deze naam bestaat al.';
    return;
  }
  submittingCreate.value = true;
  try {
    const result = await createDocument(value);
    if (!result.ok) {
      createError.value = saveError.value?.message || 'Aanmaken mislukt.';
      return;
    }
    newPath.value = '';
    // createDocument already set currentPath/currentBody and persisted, so
    // close the browser and reveal the new document in the window.
    closeBrowser();
    windowOpen.value = true;
  } finally {
    submittingCreate.value = false;
  }
}

async function handleSave() {
  await saveCurrent();
}

// Resolve a 412 conflict by force-overwriting with `If-Match: *` (the stale
// `currentEtag` would just trip the precondition again).
function overwriteServer() {
  return saveCurrent({ ifMatch: '*' });
}

// --- Delete confirmation (nldd-modal-dialog) ---
const deleteModalEl = ref(null);
const pendingDeletePath = ref(null);
// Browser-level feedback for a failed delete; kept separate from the
// window's save-conflict banner (a delete 412 must not offer the
// reload/overwrite actions, which act on the open document).
const deleteNotice = ref(null);

watch(pendingDeletePath, async (path) => {
  await nextTick();
  const el = deleteModalEl.value;
  if (!el) return;
  if (path) el.show?.();
  else el.hide?.();
});

function askDelete(path) {
  if (path) pendingDeletePath.value = path;
}

function cancelDelete() {
  if (pendingDeletePath.value === null) return; // idempotent: @close + button
  pendingDeletePath.value = null;
}

async function confirmDelete() {
  const path = pendingDeletePath.value;
  pendingDeletePath.value = null;
  if (!path) return;
  deleteNotice.value = null;
  const result = await deleteDocument(path);
  if (result?.ok) {
    if (path === currentPath.value) windowOpen.value = false;
  } else if (result?.conflict) {
    deleteNotice.value =
      `"${path}" is intussen door iemand anders gewijzigd; de lijst is ververst. ` +
      `Open het document om de huidige versie te zien voordat je het verwijdert.`;
  } else {
    deleteNotice.value =
      saveError.value?.message || `Verwijderen van "${path}" is mislukt.`;
  }
}

function closeWindow() {
  windowOpen.value = false;
}

// Ctrl/Cmd+S = save.
function handleKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    if (currentPath.value && !saving.value) {
      e.preventDefault();
      handleSave();
    }
  }
}
</script>

<template>
  <!-- Documenten browser sheet — opened from the TrajectMenu. -->
  <Teleport to="body">
    <nldd-sheet ref="browserEl" placement="right" width="520px" full-height @close="closeBrowser">
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar
          slot="header"
          text="Documenten"
          dismiss-text="Sluit"
          @dismiss="closeBrowser"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-inline-dialog
            v-if="deleteNotice"
            variant="warning"
            :text="deleteNotice"
          ></nldd-inline-dialog>

          <nldd-activity-indicator v-if="listLoading" text="Documenten laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog
            v-else-if="listError"
            variant="alert"
            text="Documenten niet geladen"
            :supporting-text="listError.message"
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="documents.length === 0"
            text="Nog geen documenten in dit traject."
          ></nldd-inline-dialog>
          <nldd-list v-else variant="box">
            <nldd-list-item v-for="doc in documents" :key="doc.path" size="md">
              <nldd-button
                variant="neutral-transparent"
                size="md"
                :text="doc.path"
                @click="openInWindow(doc.path)"
              ></nldd-button>
              <nldd-icon-button
                slot="end"
                icon="delete"
                size="md"
                accessible-label="Verwijderen"
                @click="askDelete(doc.path)"
              ></nldd-icon-button>
            </nldd-list-item>
          </nldd-list>
        </nldd-simple-section>

        <!-- Geen aanmaakformulier zolang de lijst zelf niet laadt (bijv.
             403 zonder corpus-token): aanmaken zou op dezelfde fout
             stranden. -->
        <nldd-container v-if="!listError" slot="footer" padding="16">
          <nldd-inline-dialog
            v-if="createError"
            variant="alert"
            :text="createError"
          ></nldd-inline-dialog>
          <nldd-text-field
            :value="newPath"
            placeholder="bv. notes.md of mvt/concept.md"
            accessible-label="Nieuw documentpad"
            @input="onNewPathInput"
          ></nldd-text-field>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button
            variant="primary"
            size="md"
            width="full"
            :text="submittingCreate ? 'Bezig…' : '+ Nieuw document'"
            :disabled="submittingCreate || undefined"
            @click="submitCreate"
          ></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

  <!-- Active-document editor — modeless, movable nldd-window
       (storybook components-layout-window). The title bar is the drag
       handle; the law text stays visible behind it.
       `top`/`right` are required: a modeless dialog is position:absolute and
       would otherwise center within the (tall) editor document, landing
       below the fold. Pinning it to the top-right corner opens it in view;
       being movable, the user can reposition from there. -->
  <Teleport to="body">
    <nldd-window
      ref="windowEl"
      modeless
      movable
      top="72px"
      right="24px"
      width="max(280px, 40vw)"
      accessible-label="Document bewerken"
      @close="closeWindow"
    >
      <nldd-page sticky-header sticky-footer @keydown="handleKeydown">
        <nldd-top-title-bar
          slot="header"
          window-drag-handle
          :text="currentPath || 'Document'"
          dismiss-text="Sluit"
          @dismiss="closeWindow"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-activity-indicator v-if="docLoading" text="Document laden" show-text></nldd-activity-indicator>
          <template v-else>
            <nldd-inline-dialog v-if="conflict" variant="warning" :text="conflict">
              <nldd-button slot="actions" size="md" text="Server-versie laden" @click="reloadCurrent"></nldd-button>
              <nldd-button slot="actions" size="md" text="Lokaal overschrijven" @click="overwriteServer"></nldd-button>
            </nldd-inline-dialog>

            <nldd-inline-dialog
              v-if="deletedRemotely"
              variant="warning"
              :text="deletedRemotely"
            ></nldd-inline-dialog>

            <nldd-inline-dialog
              v-if="docError && docError.kind === 'draft-present'"
              :text="docError.message"
            >
              <nldd-button slot="actions" size="md" text="Draft verwerpen" @click="dropDraft"></nldd-button>
            </nldd-inline-dialog>

            <nldd-inline-dialog
              v-if="docError && docError.kind !== 'draft-present'"
              variant="alert"
              :text="docError.message"
            ></nldd-inline-dialog>

            <nldd-inline-dialog
              v-if="saveError"
              variant="alert"
              text="Actie mislukt"
              :supporting-text="saveError.message"
            ></nldd-inline-dialog>

            <nldd-multi-line-text-field
              :value="currentBody"
              rows="14"
              resize="vertical"
              no-spellcheck
              accessible-label="Document-inhoud (markdown)"
              placeholder="# Titel"
              @input="onBodyInput"
            ></nldd-multi-line-text-field>
            <nldd-divider></nldd-divider>
            <!-- v-html is safe: renderArticleHtml runs DOMPurify over the
                 marked output, identical to the law-text path. nldd-rich-text
                 applies the design-system typography to the slotted HTML. -->
            <nldd-rich-text spacing="snug" v-html="previewHtml"></nldd-rich-text>
          </template>
        </nldd-simple-section>

        <nldd-container slot="footer" padding="16">
          <nldd-button
            variant="primary"
            size="md"
            width="full"
            :text="saving ? 'Opslaan…' : 'Opslaan'"
            :disabled="saving || !currentPath || undefined"
            @click="handleSave"
          ></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-window>
  </Teleport>

  <!-- Delete confirmation — NLDD modal, consistent with the editor's
       clear-drafts dialog. -->
  <Teleport to="body">
    <nldd-modal-dialog
      ref="deleteModalEl"
      variant="alert"
      text="Document verwijderen?"
      :supporting-text="pendingDeletePath
        ? `${pendingDeletePath} wordt definitief uit het traject verwijderd.`
        : ''"
      @close="cancelDelete"
    >
      <nldd-button slot="actions" text="Annuleer" @click="cancelDelete"></nldd-button>
      <nldd-button
        slot="actions"
        variant="destructive"
        text="Verwijder"
        @click="confirmDelete"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
