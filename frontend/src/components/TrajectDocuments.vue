<script setup>
/**
 * TrajectDocuments — markdown/plain-text documents that live in a traject's
 * corpus branch.
 *
 * Two NLDD overlays, mounted once per app and triggered from the
 * TrajectMenu ("Documenten"):
 *   1. A browser **sheet** (nldd-sheet) — the file list with a "Nieuw
 *      document" row at the bottom; clicking it creates an untitled
 *      document and opens it straight in the edit window.
 *   2. An **edit window** (nldd-window, modeless + movable) — the active
 *      document's markdown editor with a live preview, so it can be dragged
 *      aside while the law text stays visible. The document name is edited
 *      here too, in a title field above the body; saving under a changed
 *      name writes the new path and deletes the old one.
 *
 * Naming: '.md' is an implementation detail and stays hidden everywhere
 * (list, window title, title field, delete confirm); a path is derived by
 * appending '.md' unless the user explicitly typed '.txt' (which stays
 * visible because it deviates from the default).
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
  currentEtag,
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

// --- Titels ---
// '.md' blijft verborgen voor de gebruiker; '.txt' wijkt af van de default
// en blijft daarom zichtbaar.
function displayTitle(path) {
  return path ? path.replace(/\.md$/, '') : '';
}

function pathFromTitle(title) {
  const t = title.trim();
  if (!t) return '';
  return /\.(md|txt)$/.test(t) ? t : `${t}.md`;
}

// Lightweight client-side validation mirroring the backend rules so the user
// gets immediate feedback instead of a 400.
function validatePath(value) {
  if (!value) return 'Geef een naam op.';
  if (value.startsWith('/')) return "Naam mag niet beginnen met '/'.";
  if (value.includes('\\')) return 'Naam mag geen backslashes bevatten.';
  // No blanket `includes('..')` check: the backend only rejects `.` / `..`
  // as whole segments, which the per-segment `startsWith('.')` guard below
  // already covers; a substring check would also reject legitimate names
  // like `a..b.md`, diverging from the backend's authoritative validation.
  const segments = value.split('/');
  for (const seg of segments) {
    if (!seg) return 'Naam bevat lege segmenten.';
    if (seg.startsWith('.')) return "Naam mag geen verborgen segmenten ('.') bevatten.";
    if (!/^[a-z0-9._-]+$/.test(seg)) {
      return "Gebruik alleen kleine letters, cijfers en '._-'.";
    }
  }
  return null;
}

// --- Nieuw document ---
// Eén klik maakt direct een 'untitled'-document aan (kleine letters: de
// backend staat alleen [a-z0-9._-] toe in paden) en opent het venster; de
// naam is daar vervolgens te bewerken.
const creating = ref(false);
const createError = ref(null);

function nextUntitledPath() {
  let path = 'untitled.md';
  for (let n = 2; documents.value.some((d) => d.path === path); n++) {
    path = `untitled-${n}.md`;
  }
  return path;
}

async function startNewDocument() {
  if (creating.value) return;
  createError.value = null;
  creating.value = true;
  try {
    const result = await createDocument(nextUntitledPath());
    if (!result.ok) {
      createError.value = saveError.value?.message || 'Aanmaken mislukt.';
      return;
    }
    closeBrowser();
    windowOpen.value = true;
  } finally {
    creating.value = false;
  }
}

// --- Titel bewerken in het venster ---
const titleDraft = ref('');
const titleError = ref(null);

watch(currentPath, (p) => {
  titleDraft.value = displayTitle(p);
  titleError.value = null;
});

function onTitleInput(e) {
  titleDraft.value = e.detail?.value ?? e.target?.value ?? titleDraft.value;
}

async function handleSave() {
  titleError.value = null;
  const finalPath = pathFromTitle(titleDraft.value);
  const err = validatePath(finalPath);
  if (err) {
    titleError.value = err;
    return;
  }
  if (finalPath === currentPath.value) {
    await saveCurrent();
    return;
  }
  // Hernoemen: er is geen rename-API, dus schrijf de inhoud eerst onder het
  // nieuwe pad (blind create) en verwijder daarna het oude pad. In die
  // volgorde kan een mislukking nooit inhoud kwijtraken — hooguit staat het
  // document tijdelijk onder beide namen.
  if (documents.value.some((d) => d.path === finalPath)) {
    titleError.value = 'Een document met deze naam bestaat al.';
    return;
  }
  const oldPath = currentPath.value;
  const oldEtag = currentEtag.value;
  currentPath.value = finalPath;
  currentEtag.value = null;
  const result = await saveCurrent({ ifMatch: null });
  if (!result?.ok) {
    // Terug naar het oude pad zodat een volgende save niet nogmaals tegen
    // het nieuwe (mislukte) pad aanloopt.
    currentPath.value = oldPath;
    currentEtag.value = oldEtag;
    return;
  }
  await deleteDocument(oldPath);
  // deleteDocument wist currentPath wanneer het het open document betrof —
  // hier verwijderden we bewust het OUDE pad terwijl het nieuwe open is,
  // dus die guard matcht niet en de state blijft op het nieuwe pad staan.
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
      `"${displayTitle(path)}" is intussen door iemand anders gewijzigd; de lijst is ververst. ` +
      `Open het document om de huidige versie te zien voordat je het verwijdert.`;
  } else {
    deleteNotice.value =
      saveError.value?.message || `Verwijderen van "${displayTitle(path)}" is mislukt.`;
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
      <nldd-page sticky-header>
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
          <nldd-inline-dialog
            v-if="createError"
            variant="alert"
            :text="createError"
          ></nldd-inline-dialog>

          <nldd-activity-indicator v-if="listLoading" text="Documenten laden" show-text></nldd-activity-indicator>
          <nldd-inline-dialog
            v-else-if="listError"
            variant="alert"
            text="Documenten niet geladen"
            :supporting-text="listError.message"
          ></nldd-inline-dialog>
          <nldd-list v-else variant="box">
            <nldd-list-item
              v-for="doc in documents"
              :key="doc.path"
              size="md"
              type="button"
              @click="openInWindow(doc.path)"
            >
              <nldd-icon-cell size="20">
                <nldd-icon name="document"></nldd-icon>
              </nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="displayTitle(doc.path)"></nldd-text-cell>
            </nldd-list-item>
            <nldd-inline-dialog
              v-if="documents.length === 0"
              text="Nog geen documenten in dit traject."
            ></nldd-inline-dialog>
            <nldd-list-item size="md"
              type="button"
              :disabled="creating || undefined"
              @click="startNewDocument"
            >
              <nldd-icon-cell size="20">
                <nldd-icon name="plus"></nldd-icon>
              </nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell :text="creating ? 'Bezig…' : 'Nieuw document'"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-simple-section>
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
          :text="titleDraft || 'Document'"
          dismiss-text="Sluit"
          @dismiss="closeWindow"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-activity-indicator v-if="docLoading" text="Document laden" show-text></nldd-activity-indicator>
          <template v-else>
            <!-- Ook hier: een delete kan nu vanuit dit venster starten, dus
                 de uitkomst moet hier zichtbaar zijn (de sheet is dicht). -->
            <nldd-inline-dialog
              v-if="deleteNotice"
              variant="warning"
              :text="deleteNotice"
            ></nldd-inline-dialog>
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

            <nldd-inline-dialog
              v-if="titleError"
              variant="alert"
              :text="titleError"
            ></nldd-inline-dialog>

            <nldd-text-field
              :value="titleDraft"
              accessible-label="Documentnaam"
              placeholder="documentnaam"
              @input="onTitleInput"
            ></nldd-text-field>
            <nldd-spacer size="8"></nldd-spacer>
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
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button
            variant="destructive"
            size="md"
            width="full"
            text="Verwijder"
            :disabled="saving || !currentPath || undefined"
            @click="askDelete(currentPath)"
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
        ? `${displayTitle(pendingDeletePath)} wordt definitief uit het traject verwijderd.`
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
