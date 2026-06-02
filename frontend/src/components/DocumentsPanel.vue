<script setup>
import { computed, nextTick, ref, toRef, watch } from 'vue';
import { useTrajectDocuments } from '../composables/useTrajectDocuments.js';
import { renderArticleHtml } from '../composables/useArticleMarkdown.js';

const props = defineProps({
  // Active traject ref. Documents live under `documents/<trajectRef>/`
  // in the traject's corpus branch — there is no global counterpart, so
  // this panel is only mounted when a traject is active (EditorApp gates
  // the `documents` view on it).
  trajectRef: { type: String, default: null },
});

const trajectRef = toRef(props, 'trajectRef');

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
} = useTrajectDocuments(trajectRef);

// --- Edit sheet (overlay) state. Mirrors the imperative show()/hide()
// pattern used by EditSheet / ScenarioBuilder / TrajectMenu: a flag
// drives the sheet's animate-in/out so it isn't mounted/unmounted. ---
const sheetEl = ref(null);
const sheetOpen = ref(false);

watch(sheetOpen, async (open) => {
  await nextTick();
  if (open) sheetEl.value?.show();
  else sheetEl.value?.hide();
});

// --- Create form ---
const newPath = ref('');
const createError = ref(null);
const submittingCreate = ref(false);

// Markdown preview reuses the shared sanitised pipeline so XSS protection
// matches the law-text rendering elsewhere in the editor.
const previewHtml = computed(() => renderArticleHtml(currentBody.value));

// Open a document in the edit sheet.
async function openInSheet(path) {
  await openDocument(path);
  sheetOpen.value = true;
}

function selectDocument(path) {
  if (path === currentPath.value && sheetOpen.value) return;
  openInSheet(path);
}

// Lightweight client-side validation mirroring the backend rules so the
// user gets immediate feedback on the create form instead of a 400.
function validateNewPath(value) {
  if (!value) return 'Geef een naam op.';
  if (value.startsWith('/')) return "Pad mag niet beginnen met '/'.";
  if (value.includes('\\')) return "Pad mag geen backslashes bevatten.";
  if (value.includes('..')) return "Pad mag geen '..' bevatten.";
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
  // Guard against a double-fire: the form's @submit.prevent and the
  // nldd-button @click can both invoke this in the same turn (the
  // button may render a type=submit internally). `submittingCreate`
  // is only set after the first `await` below, so without this early
  // return both calls clear the check and double-PUT.
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
    // createDocument already set currentPath/currentBody and persisted,
    // so just reveal the sheet on the freshly-created document.
    sheetOpen.value = true;
  } finally {
    submittingCreate.value = false;
  }
}

async function handleSave() {
  await saveCurrent();
}

// Resolve a 412 conflict by force-overwriting the server version. We
// must bypass the now-stale `currentEtag` — re-sending it would just
// trip the precondition again and loop. `If-Match: '*'` tells the
// backend "match any existing version".
function overwriteServer() {
  return saveCurrent({ ifMatch: '*' });
}

// Delete confirmation via nldd-modal-dialog (consistent with the
// editor's clear-drafts confirm). A pending-path ref drives show()/hide()
// the same way — null means closed. nldd-modal-dialog exposes show()/hide()
// (no close()); @close just clears the flag, avoiding hide()→@close→hide()
// recursion.
const deleteModalEl = ref(null);
const pendingDeletePath = ref(null);

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
  const result = await deleteDocument(path);
  if (result?.ok && path === currentPath.value) {
    sheetOpen.value = false;
  }
}

function closeSheet() {
  sheetOpen.value = false;
}

// Ctrl/Cmd+S = save without forcing the user to mouse to the button.
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
  <div class="documents-panel">
    <p v-if="listLoading" class="documents-panel__hint">Bezig met laden…</p>
    <p v-else-if="listError" class="documents-panel__error">
      {{ listError.message }}
    </p>
    <p v-else-if="documents.length === 0" class="documents-panel__hint">
      Nog geen documenten in dit traject.
    </p>
    <ul v-else class="documents-panel__list">
      <li
        v-for="doc in documents"
        :key="doc.path"
        class="documents-panel__row"
        :class="{ 'documents-panel__row--active': doc.path === currentPath && sheetOpen }"
      >
        <button
          type="button"
          class="documents-panel__item"
          @click="selectDocument(doc.path)"
        >
          {{ doc.path }}
        </button>
        <nldd-icon-button
          icon="delete"
          size="md"
          accessible-label="Verwijderen"
          @click="askDelete(doc.path)"
        ></nldd-icon-button>
      </li>
    </ul>

    <form class="documents-panel__create" @submit.prevent="submitCreate">
      <label for="new-doc-path" class="documents-panel__label">
        + Nieuw document
      </label>
      <div class="documents-panel__create-row">
        <input
          id="new-doc-path"
          v-model="newPath"
          type="text"
          placeholder="bv. notes.md of mvt/concept.md"
          autocomplete="off"
          spellcheck="false"
        />
        <nldd-button
          size="md"
          :text="submittingCreate ? 'Bezig…' : 'Aanmaken'"
          :disabled="submittingCreate || undefined"
          @click="submitCreate"
        ></nldd-button>
      </div>
      <p v-if="createError" class="documents-panel__error">{{ createError }}</p>
    </form>

    <!-- Edit sheet: same right-placement overlay pattern as the scenario
         editor. Teleported to body so it isn't clipped by the pane. -->
    <Teleport to="body">
      <nldd-sheet
        ref="sheetEl"
        placement="right"
        width="880px"
        full-height
        @close="closeSheet"
      >
        <nldd-page sticky-header @keydown="handleKeydown">
          <nldd-top-title-bar
            slot="header"
            :text="currentPath || 'Document'"
            dismiss-text="Sluiten"
            @dismiss="closeSheet"
          ></nldd-top-title-bar>

          <nldd-simple-section>
            <div v-if="docLoading" class="documents-panel__hint">Document laden…</div>
            <template v-else>
              <div
                v-if="conflict"
                class="documents-panel__banner documents-panel__banner--warn"
              >
                {{ conflict }}
                <nldd-button size="md" text="Server-versie laden" @click="reloadCurrent"></nldd-button>
                <nldd-button size="md" text="Lokaal overschrijven" @click="overwriteServer"></nldd-button>
              </div>

              <div
                v-if="deletedRemotely"
                class="documents-panel__banner documents-panel__banner--warn"
              >
                {{ deletedRemotely }}
              </div>

              <div
                v-if="docError && docError.kind === 'draft-present'"
                class="documents-panel__banner"
              >
                {{ docError.message }}
                <nldd-button size="md" text="Draft verwerpen" @click="dropDraft"></nldd-button>
              </div>

              <div
                v-if="docError && docError.kind !== 'draft-present'"
                class="documents-panel__banner documents-panel__banner--error"
              >
                {{ docError.message }}
              </div>

              <div
                v-if="saveError"
                class="documents-panel__banner documents-panel__banner--error"
              >
                Actie mislukt: {{ saveError.message }}
              </div>

              <div class="documents-panel__panes">
                <textarea
                  v-model="currentBody"
                  class="documents-panel__editor"
                  spellcheck="false"
                  :placeholder="`# ${currentPath || ''}`"
                />
                <!-- v-html is safe here: renderArticleHtml runs DOMPurify
                     over the marked output, identical to the law-text path. -->
                <article
                  class="documents-panel__preview markdown-body"
                  v-html="previewHtml"
                />
              </div>
            </template>
          </nldd-simple-section>

          <nldd-container slot="footer" padding="16">
            <nldd-button
              variant="primary"
              size="md"
              full-width
              :text="saving ? 'Opslaan…' : 'Opslaan (⌘S)'"
              :disabled="saving || !currentPath || undefined"
              @click="handleSave"
            ></nldd-button>
          </nldd-container>
        </nldd-page>
      </nldd-sheet>
    </Teleport>

    <!-- Delete confirmation — NLDD modal instead of the native confirm()
         so it matches the rest of the editor (e.g. the clear-drafts
         dialog). Teleported to body so it isn't clipped by the pane. -->
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
  </div>
</template>

<style scoped>
.documents-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
}
.documents-panel__hint {
  color: var(--semantics-content-secondary-color, #555);
  font-size: 14px;
}
.documents-panel__error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
}
.documents-panel__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.documents-panel__row {
  display: flex;
  align-items: center;
  gap: 4px;
}
.documents-panel__item {
  flex: 1;
  min-width: 0;
  text-align: left;
  background: none;
  border: none;
  padding: 8px 10px;
  border-radius: 6px;
  font: inherit;
  cursor: pointer;
  color: inherit;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.documents-panel__item:hover {
  background: var(--semantics-surfaces-tinted-background-color, #f4f4f4);
}
.documents-panel__row--active .documents-panel__item {
  background: var(--semantics-surfaces-tinted-background-color, #ececec);
  font-weight: 600;
}
.documents-panel__create {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border-top: 1px solid var(--semantics-borders-subtle-color, #e0e0e0);
  padding-top: 12px;
}
.documents-panel__label {
  font-size: 13px;
  font-weight: 600;
}
.documents-panel__create-row {
  display: flex;
  gap: 8px;
}
.documents-panel__create-row input {
  flex: 1;
  min-width: 0;
  padding: 6px 8px;
  border: 1px solid var(--semantics-borders-default-color, #ccc);
  border-radius: 6px;
  font: inherit;
}
.documents-panel__panes {
  display: flex;
  gap: 12px;
  align-items: stretch;
  min-height: 60vh;
}
.documents-panel__editor {
  flex: 1;
  min-width: 0;
  resize: none;
  border: 1px solid var(--semantics-borders-default-color, #ccc);
  border-radius: 6px;
  padding: 10px;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 13px;
  line-height: 1.5;
}
.documents-panel__preview {
  flex: 1;
  min-width: 0;
  overflow: auto;
  border: 1px solid var(--semantics-borders-subtle-color, #e0e0e0);
  border-radius: 6px;
  padding: 10px 14px;
  background: var(--semantics-surfaces-background-color, #fff);
}
.documents-panel__banner {
  padding: 10px 12px;
  border-radius: 6px;
  font-size: 13px;
  margin-bottom: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  background: var(--semantics-surfaces-tinted-background-color, #f4f4f4);
}
.documents-panel__banner--warn {
  background: var(--nldd-color-bg-warning, #fff4e5);
}
.documents-panel__banner--error {
  background: var(--nldd-color-bg-error, #fdecea);
  color: var(--nldd-color-text-error, #c62828);
}
@media (max-width: 720px) {
  .documents-panel__panes {
    flex-direction: column;
  }
}
</style>
