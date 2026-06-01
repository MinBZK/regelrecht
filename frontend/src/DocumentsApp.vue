<script setup>
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from './composables/useAuth.js';
import { useTrajectDocuments } from './composables/useTrajectDocuments.js';
import { renderArticleHtml } from './composables/useArticleMarkdown.js';

const route = useRoute();
const router = useRouter();
const { authenticated, loading: authLoading, login } = useAuth();

const trajectRef = computed(() => route.params.trajectRef || null);
const initialDocPath = computed(() => {
  const raw = route.params.docPath;
  if (!raw) return null;
  return Array.isArray(raw) ? raw.join('/') : raw;
});

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
  fetchList,
} = useTrajectDocuments(trajectRef);

// Local UI state for the create form.
const newPath = ref('');
const createError = ref(null);
const submittingCreate = ref(false);

// Markdown preview reuses the shared sanitised pipeline so XSS protection
// matches the law-text rendering elsewhere in the editor.
const previewHtml = computed(() => renderArticleHtml(currentBody.value));

// Sync between route param and currently-open document. The URL is the
// source of truth so a deep link / browser back/forward "just works".
watch(
  [trajectRef, initialDocPath],
  async ([, path]) => {
    if (!path) {
      currentPath.value = null;
      currentBody.value = '';
      return;
    }
    if (path !== currentPath.value) {
      await openDocument(path);
    }
  },
  { immediate: true },
);

function selectDocument(path) {
  if (path === currentPath.value) return;
  router.push({
    name: 'editor-documents',
    params: { trajectRef: trajectRef.value, docPath: path.split('/') },
  });
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
    // Navigate so the URL reflects the new document.
    router.push({
      name: 'editor-documents',
      params: { trajectRef: trajectRef.value, docPath: value.split('/') },
    });
  } finally {
    submittingCreate.value = false;
  }
}

async function handleSave() {
  const result = await saveCurrent();
  if (result?.created && currentPath.value) {
    // A fresh save on a brand-new path: align the URL.
    router.replace({
      name: 'editor-documents',
      params: {
        trajectRef: trajectRef.value,
        docPath: currentPath.value.split('/'),
      },
    });
  }
}

// Resolve a 412 conflict by force-overwriting the server version. We
// must bypass the now-stale `currentEtag` — re-sending it would just
// trip the precondition again and loop. `If-Match: '*'` tells the
// backend "match any existing version", so the write goes through and
// `saveCurrent` refreshes `currentEtag` + clears the conflict banner.
function overwriteServer() {
  return saveCurrent({ ifMatch: '*' });
}

async function handleDelete() {
  if (!currentPath.value) return;
  const path = currentPath.value;
  if (!confirm(`Weet je zeker dat je ${path} wilt verwijderen?`)) return;
  const result = await deleteDocument(path);
  if (result?.ok) {
    router.push({
      name: 'editor-documents',
      params: { trajectRef: trajectRef.value },
    });
  }
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
  <div class="documents-app" @keydown="handleKeydown">
    <header class="documents-app__bar">
      <RouterLink
        :to="{ name: 'editor-traject', params: { trajectRef } }"
        class="documents-app__back"
      >
        ← Terug naar editor
      </RouterLink>
      <h1 class="documents-app__title">Documenten · {{ trajectRef }}</h1>
    </header>

    <div v-if="authLoading" class="documents-app__placeholder">Bezig met laden…</div>
    <div v-else-if="!authenticated" class="documents-app__placeholder">
      <button @click="login()">Inloggen</button>
    </div>
    <div v-else class="documents-app__layout">
      <aside class="documents-app__sidebar" aria-label="Documenten">
        <h2 class="documents-app__sidebar-heading">Documenten</h2>
        <p v-if="listLoading" class="documents-app__hint">Bezig met laden…</p>
        <p v-else-if="listError" class="documents-app__error">
          {{ listError.message }}
        </p>
        <p v-else-if="documents.length === 0" class="documents-app__hint">
          Nog geen documenten in dit traject.
        </p>
        <ul v-else class="documents-app__list">
          <li v-for="doc in documents" :key="doc.path">
            <button
              type="button"
              :class="[
                'documents-app__item',
                { 'documents-app__item--active': doc.path === currentPath },
              ]"
              @click="selectDocument(doc.path)"
            >
              {{ doc.path }}
            </button>
          </li>
        </ul>

        <form class="documents-app__create" @submit.prevent="submitCreate">
          <label for="new-doc-path" class="documents-app__label">
            + Nieuw document
          </label>
          <input
            id="new-doc-path"
            v-model="newPath"
            type="text"
            placeholder="bv. notes.md of mvt/concept.md"
            autocomplete="off"
            spellcheck="false"
          />
          <button type="submit" :disabled="submittingCreate">Aanmaken</button>
          <p v-if="createError" class="documents-app__error">{{ createError }}</p>
        </form>
      </aside>

      <main class="documents-app__main">
        <div v-if="!currentPath" class="documents-app__placeholder">
          Selecteer een document of maak een nieuw document aan.
        </div>
        <div v-else-if="docLoading" class="documents-app__placeholder">
          Document laden…
        </div>
        <template v-else>
          <div class="documents-app__toolbar">
            <strong>{{ currentPath }}</strong>
            <span class="documents-app__spacer" />
            <button type="button" :disabled="saving" @click="handleSave">
              {{ saving ? 'Opslaan…' : 'Opslaan (⌘S)' }}
            </button>
            <button type="button" class="documents-app__danger" @click="handleDelete">
              Verwijderen
            </button>
          </div>

          <div v-if="conflict" class="documents-app__banner documents-app__banner--warn">
            {{ conflict }}
            <button type="button" @click="reloadCurrent">Server-versie laden</button>
            <button type="button" @click="overwriteServer">Lokaal overschrijven</button>
          </div>

          <div
            v-if="deletedRemotely"
            class="documents-app__banner documents-app__banner--warn"
          >
            {{ deletedRemotely }}
          </div>

          <div
            v-if="docError && docError.kind === 'draft-present'"
            class="documents-app__banner"
          >
            {{ docError.message }}
            <button type="button" @click="dropDraft">Draft verwerpen</button>
          </div>

          <div
            v-if="docError && docError.kind !== 'draft-present'"
            class="documents-app__banner documents-app__banner--error"
          >
            {{ docError.message }}
          </div>

          <div v-if="saveError" class="documents-app__banner documents-app__banner--error">
            Actie mislukt: {{ saveError.message }}
          </div>

          <div class="documents-app__panes">
            <textarea
              v-model="currentBody"
              class="documents-app__editor"
              spellcheck="false"
              :placeholder="`# ${currentPath}`"
            />
            <!-- v-html is safe here: renderArticleHtml runs DOMPurify
                 over the marked output, identical to the law-text path. -->
            <article class="documents-app__preview markdown-body" v-html="previewHtml" />
          </div>
        </template>
      </main>
    </div>
  </div>
</template>

<style scoped>
.documents-app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  font-family: var(--font-sans, system-ui, sans-serif);
  color: var(--color-text, #222);
  background: var(--color-bg, #fff);
}

.documents-app__bar {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.5rem 1rem;
  border-bottom: 1px solid var(--color-border, #e2e2e2);
  background: var(--color-bg-secondary, #f7f7f7);
}

.documents-app__title {
  font-size: 1rem;
  font-weight: 500;
  margin: 0;
}

.documents-app__back {
  color: inherit;
  text-decoration: none;
}

.documents-app__back:hover {
  text-decoration: underline;
}

.documents-app__layout {
  display: flex;
  flex: 1 1 auto;
  min-height: 0;
}

.documents-app__sidebar {
  flex: 0 0 18rem;
  border-right: 1px solid var(--color-border, #e2e2e2);
  padding: 1rem;
  overflow-y: auto;
}

.documents-app__sidebar-heading {
  font-size: 0.85rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-muted, #666);
  margin: 0 0 0.5rem;
}

.documents-app__hint {
  color: var(--color-muted, #888);
  font-size: 0.875rem;
}

.documents-app__list {
  list-style: none;
  padding: 0;
  margin: 0 0 1rem;
}

.documents-app__item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 0.4rem 0.5rem;
  background: transparent;
  border: 0;
  border-radius: 4px;
  font: inherit;
  color: inherit;
  cursor: pointer;
}

.documents-app__item:hover {
  background: var(--color-hover, rgba(0, 0, 0, 0.05));
}

.documents-app__item--active {
  background: var(--color-accent-soft, rgba(0, 96, 187, 0.12));
  font-weight: 600;
}

.documents-app__create {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin-top: 1rem;
}

.documents-app__label {
  font-size: 0.85rem;
  font-weight: 600;
}

.documents-app__create input {
  padding: 0.4rem 0.5rem;
  border: 1px solid var(--color-border, #ccc);
  border-radius: 4px;
  font: inherit;
}

.documents-app__main {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.documents-app__placeholder {
  padding: 2rem;
  color: var(--color-muted, #666);
}

.documents-app__toolbar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  border-bottom: 1px solid var(--color-border, #e2e2e2);
}

.documents-app__spacer {
  flex: 1 1 auto;
}

.documents-app__danger {
  color: #b00020;
}

.documents-app__banner {
  padding: 0.6rem 1rem;
  background: var(--color-info-soft, #eef6ff);
  border-bottom: 1px solid var(--color-border, #e2e2e2);
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}

.documents-app__banner--warn {
  background: var(--color-warn-soft, #fff7e0);
}

.documents-app__banner--error {
  background: var(--color-error-soft, #fdecea);
}

.documents-app__panes {
  flex: 1 1 auto;
  display: grid;
  grid-template-columns: 1fr 1fr;
  min-height: 0;
}

.documents-app__editor {
  font-family: var(--font-mono, ui-monospace, SFMono-Regular, monospace);
  font-size: 0.95rem;
  border: 0;
  border-right: 1px solid var(--color-border, #e2e2e2);
  padding: 1rem;
  resize: none;
  background: var(--color-bg, #fff);
  color: inherit;
}

.documents-app__editor:focus {
  outline: 2px solid var(--color-accent, #0060bb);
  outline-offset: -2px;
}

.documents-app__preview {
  padding: 1rem 1.5rem;
  overflow-y: auto;
}

.documents-app__error {
  color: #b00020;
  font-size: 0.875rem;
  margin: 0.3rem 0 0;
}
</style>
