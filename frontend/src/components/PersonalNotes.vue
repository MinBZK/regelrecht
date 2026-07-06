<script setup>
// Persoonlijke notities: private per-user notes on the whole law, stored
// server-side (Postgres via /api/user/notes) — never in git, never shared
// with a traject. Bodies are markdown (W3C TextualBody, text/markdown) and
// render through the same sanitizing pipeline as law text and document
// previews. Editing follows the DocumentEditor pattern: raw-markdown
// textarea with an editor/preview toggle.
import { ref, toRef } from 'vue';
import { useUserNotes } from '../composables/useUserNotes.js';
import { renderArticleHtml } from '../composables/useArticleMarkdown.js';

const props = defineProps({
  lawId: { type: String, required: true },
});

const { notes, loading, error, available, addNote, updateNote, removeNote } = useUserNotes(
  toRef(props, 'lawId'),
);

// A create/update/delete failure is an action error: surface it but keep
// the list and drafts intact so the user can retry in place.
const actionError = ref(null);
const saving = ref(false);

// Composer for a new note.
const draft = ref('');
const composerMode = ref('editor');

// In-place editing of one existing note at a time.
const editingId = ref(null);
const editDraft = ref('');
const editMode = ref('editor');

// Delete confirmation.
const deleteModalEl = ref(null);
const pendingDelete = ref(null);

function inputValue(e) {
  return e.detail?.value ?? e.target?.value ?? '';
}

function modeValue(e, fallback) {
  return e.detail?.value ?? fallback;
}

const dateFormat = new Intl.DateTimeFormat('nl-NL', { dateStyle: 'long', timeStyle: 'short' });

function noteMeta(note) {
  if (!note.modified) return '';
  // A never-edited note has identical created/modified timestamps — label
  // it as created rather than edited.
  const label = note.created === note.modified ? 'Aangemaakt' : 'Bewerkt';
  return `${label} ${dateFormat.format(new Date(note.modified))}`;
}

async function saveNew() {
  if (!draft.value.trim() || saving.value) return;
  saving.value = true;
  actionError.value = null;
  try {
    await addNote(draft.value);
    draft.value = '';
    composerMode.value = 'editor';
  } catch (e) {
    actionError.value = `Notitie opslaan mislukt: ${e.message}`;
  } finally {
    saving.value = false;
  }
}

function startEdit(note) {
  editingId.value = note.id;
  editDraft.value = note.body?.value ?? '';
  editMode.value = 'editor';
  actionError.value = null;
}

function cancelEdit() {
  editingId.value = null;
  editDraft.value = '';
}

async function saveEdit() {
  if (!editDraft.value.trim() || saving.value) return;
  saving.value = true;
  actionError.value = null;
  try {
    await updateNote(editingId.value, editDraft.value);
    cancelEdit();
  } catch (e) {
    actionError.value = `Notitie bijwerken mislukt: ${e.message}`;
  } finally {
    saving.value = false;
  }
}

function askDelete(note) {
  pendingDelete.value = note;
  deleteModalEl.value?.show?.();
}

function cancelDelete() {
  pendingDelete.value = null;
  deleteModalEl.value?.hide?.();
}

async function confirmDelete() {
  const note = pendingDelete.value;
  cancelDelete();
  if (!note) return;
  actionError.value = null;
  try {
    await removeNote(note.id);
    if (editingId.value === note.id) cancelEdit();
  } catch (e) {
    actionError.value = `Notitie verwijderen mislukt: ${e.message}`;
  }
}
</script>

<template>
  <!-- Anonymous session or no DB: the feature is off, render nothing. -->
  <template v-if="available">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-divider></nldd-divider>
    <nldd-spacer size="16"></nldd-spacer>
    <div data-testid="personal-notes">
      <nldd-rich-text spacing="snug">
        <h3>Persoonlijke notities</h3>
        <p>Alleen zichtbaar voor jou — niet gedeeld met het traject. Markdown wordt ondersteund.</p>
      </nldd-rich-text>
      <nldd-spacer size="16"></nldd-spacer>

      <nldd-inline-dialog
        v-if="error"
        variant="alert"
        text="Persoonlijke notities niet geladen"
        :supporting-text="error.message"
      ></nldd-inline-dialog>
      <nldd-activity-indicator
        v-else-if="loading"
        text="Persoonlijke notities laden"
        show-text
      ></nldd-activity-indicator>
      <template v-else>
        <nldd-inline-dialog
          v-if="actionError"
          variant="alert"
          data-testid="personal-notes-error"
          :text="actionError"
        ></nldd-inline-dialog>

        <template v-for="note in notes" :key="note.id">
          <!-- In-place editor for the note being edited. -->
          <template v-if="editingId === note.id">
            <nldd-segmented-control
              variant="icon"
              size="md"
              width="fit-content"
              :value="editMode"
              @change="editMode = modeValue($event, editMode)"
            >
              <nldd-segmented-control-item
                value="editor"
                icon="pencil-on-square"
                text="Bewerken"
              ></nldd-segmented-control-item>
              <nldd-segmented-control-item
                value="preview"
                icon="eye"
                text="Voorbeeld"
              ></nldd-segmented-control-item>
            </nldd-segmented-control>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-multi-line-text-field
              v-if="editMode === 'editor'"
              :value="editDraft"
              rows="6"
              resize="auto"
              no-spellcheck
              accessible-label="Notitie (markdown)"
              data-testid="personal-note-edit-field"
              @input="editDraft = inputValue($event)"
            ></nldd-multi-line-text-field>
            <!-- v-html is safe: renderArticleHtml runs DOMPurify over the
                 marked output, identical to the law-text path. -->
            <nldd-rich-text v-else spacing="snug" v-html="renderArticleHtml(editDraft)"></nldd-rich-text>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-button-group orientation="horizontal">
              <nldd-button
                variant="primary"
                size="md"
                :text="saving ? 'Opslaan…' : 'Opslaan'"
                :disabled="saving || !editDraft.trim() || undefined"
                data-testid="personal-note-save-edit"
                @click="saveEdit"
              ></nldd-button>
              <nldd-button size="md" text="Annuleren" @click="cancelEdit"></nldd-button>
            </nldd-button-group>
          </template>

          <!-- Rendered note. -->
          <template v-else>
            <nldd-rich-text
              spacing="snug"
              data-testid="personal-note-body"
              v-html="renderArticleHtml(note.body?.value)"
            ></nldd-rich-text>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-rich-text v-if="noteMeta(note)" spacing="snug">
              <p><small>{{ noteMeta(note) }}</small></p>
            </nldd-rich-text>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-button-group orientation="horizontal">
              <nldd-button
                size="md"
                text="Bewerken"
                data-testid="personal-note-edit"
                @click="startEdit(note)"
              ></nldd-button>
              <nldd-button
                size="md"
                variant="destructive"
                text="Verwijderen"
                data-testid="personal-note-delete"
                @click="askDelete(note)"
              ></nldd-button>
            </nldd-button-group>
          </template>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-divider></nldd-divider>
          <nldd-spacer size="16"></nldd-spacer>
        </template>

        <!-- Composer for a new note. -->
        <nldd-segmented-control
          variant="icon"
          size="md"
          width="fit-content"
          :value="composerMode"
          @change="composerMode = modeValue($event, composerMode)"
        >
          <nldd-segmented-control-item
            value="editor"
            icon="pencil-on-square"
            text="Bewerken"
          ></nldd-segmented-control-item>
          <nldd-segmented-control-item
            value="preview"
            icon="eye"
            text="Voorbeeld"
          ></nldd-segmented-control-item>
        </nldd-segmented-control>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-multi-line-text-field
          v-if="composerMode === 'editor'"
          :value="draft"
          rows="4"
          resize="auto"
          no-spellcheck
          accessible-label="Nieuwe persoonlijke notitie (markdown)"
          placeholder="Eigen context bij deze wet of regeling…"
          data-testid="personal-note-draft-field"
          @input="draft = inputValue($event)"
        ></nldd-multi-line-text-field>
        <nldd-rich-text v-else spacing="snug" v-html="renderArticleHtml(draft)"></nldd-rich-text>
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-button
          variant="primary"
          size="md"
          :text="saving ? 'Opslaan…' : 'Notitie toevoegen'"
          :disabled="saving || !draft.trim() || undefined"
          data-testid="personal-note-add"
          @click="saveNew"
        ></nldd-button>
      </template>
    </div>

    <Teleport to="body">
      <nldd-modal-dialog
        ref="deleteModalEl"
        variant="alert"
        text="Persoonlijke notitie verwijderen?"
        supporting-text="De notitie wordt definitief verwijderd. Dit kan niet ongedaan worden gemaakt."
        @close="cancelDelete"
      >
        <nldd-button slot="actions" variant="primary" text="Behoud notitie" @click="cancelDelete"></nldd-button>
        <nldd-button
          slot="actions"
          variant="destructive"
          text="Verwijder"
          data-testid="personal-note-confirm-delete"
          @click="confirmDelete"
        ></nldd-button>
      </nldd-modal-dialog>
    </Teleport>
  </template>
</template>
