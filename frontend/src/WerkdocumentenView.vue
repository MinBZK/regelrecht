<script setup>
/**
 * WerkdocumentenView — standalone full-page werkdocumenten editor, opened in a
 * new browser tab from the in-sheet editor's "Open in nieuw tabblad". Gives the
 * documents a navigation-split-view (list sidebar + editor main) with all the
 * room the right-side sheet can't, while sharing the exact same logic
 * (useDocumentsManager) and presentational pieces (DocumentList/DocumentEditor).
 *
 * Top-level route (sibling of AppShell, not nested) so it has no app chrome;
 * it carries its own compact top bar — title + traject subtitle on the left,
 * a focused account menu (theme + logout) on the right.
 */
import { computed, onMounted, watch, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useTrajects } from './composables/useTrajects.js';
import { useDocumentsManager } from './composables/useDocumentsManager.js';
import { useTrajectDocumentJobs } from './composables/useTrajectDocumentJobs.js';
import { useDocumentUpload } from './composables/useDocumentUpload.js';
import { useAuth } from './composables/useAuth.js';
import { useColorScheme } from './composables/useColorScheme.js';
import DocumentList from './components/DocumentList.vue';
import DocumentEditor from './components/DocumentEditor.vue';
import ConversionStatus from './components/ConversionStatus.vue';

const route = useRoute();
const router = useRouter();
const { activeTrajectRef, activeTraject } = useTrajects();
const { authenticated, oidcConfigured, person, loading: authLoading, login, logout } = useAuth();
const { colorScheme, setColorScheme } = useColorScheme();

const mgr = useDocumentsManager(activeTrajectRef);
const { documents, listLoading, listError, currentPath, displayTitle, open, startNew, close, uploadDocument } = mgr;

// Conversion-status poller for uploaded documents being converted to markdown.
const jobsPoller = useTrajectDocumentJobs(activeTrajectRef);
const { jobs: conversionJobs } = jobsPoller;

// Hidden native file input + upload trigger (shared with the launcher sheet).
const { fileInput, uploadError, onUpload, onFileChange } = useDocumentUpload(uploadDocument, () =>
  jobsPoller.startPolling(),
);

const colorSchemeOptions = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

const trajectName = computed(() => activeTraject.value?.name || '');
const hasOpenDoc = computed(() => !!currentPath.value);
const docTitle = computed(() => displayTitle(currentPath.value) || 'Document');

// HTML-titel: {werkdocument indien open} · {traject} · RegelRecht. Lege delen
// (traject nog niet geladen, geen document open) vallen weg.
watchEffect(() => {
  const parts = [];
  if (currentPath.value) parts.push(displayTitle(currentPath.value));
  if (trajectName.value) parts.push(trajectName.value);
  parts.push('RegelRecht');
  document.title = parts.join(' · ');
});

// Open the document addressed by the URL on first load (deep link / refresh),
// or start a fresh document when the launcher opened us with `?new=1`.
onMounted(() => {
  // Surface any conversions already running/failed for this traject, and keep
  // polling while the page is open (the composable stops on unmount).
  jobsPoller.startPolling();

  const initial = route.params.docPath;
  if (initial) {
    open(String(initial));
    return;
  }
  if (!route.query.new) return;
  // Wait for the traject's list to load so startNew picks the next free
  // untitled-N name instead of blind-writing over an existing untitled.md.
  const stop = watch(
    [listLoading, activeTraject],
    ([loading, traject]) => {
      if (loading || !traject) return;
      stop();
      startNew();
    },
    { immediate: true },
  );
});

// Mirror the open document into the URL so refresh, bookmark and browser back
// keep working. Guarded against the redundant replace the initial open would
// otherwise trigger (the URL already names the document).
watch(currentPath, (p) => {
  const target = {
    name: 'werkdocumenten',
    params: { trajectRef: activeTrajectRef.value, docPath: p || '' },
  };
  if (router.resolve(target).href !== route.fullPath) {
    router.replace(target).catch(() => {});
  }
});

function onSelect(path) {
  open(path);
}
function onNew() {
  startNew();
}
function backToList() {
  close();
}
</script>

<template>
  <nldd-app-view>
    <div class="werkdocumenten-view">
      <nldd-container padding="8" padding-left="16">
        <nldd-toolbar size="md">
          <nldd-toolbar-title slot="start" text="Werkdocumenten" :supporting-text="trajectName"></nldd-toolbar-title>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              id="wd-account-btn"
              size="md"
              icon="account"
              text="Account"
              tooltip-timing="never"
              expandable
              popovertarget="wd-account-menu"
            ></nldd-icon-button>
            <nldd-menu id="wd-account-menu" anchor="wd-account-btn">
              <nldd-menu-item v-if="!authLoading && authenticated" :text="person?.name || person?.email" disabled></nldd-menu-item>
              <nldd-menu-group text="Thema">
                <nldd-menu-item
                  v-for="[value, label] in colorSchemeOptions"
                  :key="`scheme-${value}`"
                  type="radio"
                  :selected="colorScheme === value || undefined"
                  :text="label"
                  @select="setColorScheme(value)"
                ></nldd-menu-item>
              </nldd-menu-group>
              <nldd-menu-divider></nldd-menu-divider>
              <nldd-menu-item v-if="!authLoading && authenticated" text="Uitloggen" icon="logout" @click="logout"></nldd-menu-item>
              <nldd-menu-item v-else-if="!authLoading && oidcConfigured" text="Inloggen" @click="login()"></nldd-menu-item>
            </nldd-menu>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>
      <nldd-divider></nldd-divider>

      <div class="werkdocumenten-view__body">
        <nldd-navigation-split-view sidebar-accessible-label="Werkdocumenten">
          <nldd-split-view-pane slot="sidebar" has-content>
            <nldd-page>
              <nldd-simple-section>
                <nldd-inline-dialog
                  v-if="uploadError"
                  variant="alert"
                  text="Uploaden mislukt"
                  :supporting-text="uploadError"
                ></nldd-inline-dialog>
                <ConversionStatus :jobs="conversionJobs"></ConversionStatus>
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
                  @upload="onUpload"
                ></DocumentList>
                <input
                  ref="fileInput"
                  type="file"
                  accept=".pdf,.doc,.docx"
                  hidden
                  @change="onFileChange"
                />
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>

          <nldd-split-view-pane slot="main" :has-content="hasOpenDoc || undefined">
            <nldd-page v-if="hasOpenDoc" sticky-header>
              <nldd-top-title-bar
                slot="header"
                :text="docTitle"
                back-text="Werkdocumenten"
                @back="backToList"
              ></nldd-top-title-bar>
              <DocumentEditor :manager="mgr"></DocumentEditor>
            </nldd-page>
            <nldd-page v-else>
              <nldd-simple-section>
                <nldd-inline-dialog text="Selecteer een document"></nldd-inline-dialog>
              </nldd-simple-section>
            </nldd-page>
          </nldd-split-view-pane>
        </nldd-navigation-split-view>
      </div>
    </div>
  </nldd-app-view>
</template>

<style scoped>
.werkdocumenten-view {
  display: flex;
  min-height: 0;
  flex-direction: column;
}
.werkdocumenten-view__body {
  display: flex;
  min-height: 0;
  flex-grow: 1;
}
</style>
