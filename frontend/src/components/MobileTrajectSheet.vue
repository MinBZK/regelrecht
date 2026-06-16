<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useTrajects } from '../composables/useTrajects.js';
import { useAuth } from '../composables/useAuth.js';
import { useDocumentsSheet } from '../composables/useDocumentsSheet.js';
import { useAppChrome } from '../composables/useAppChrome.js';
import TrajectMembersDialog from './TrajectMembersDialog.vue';
import TrajectInfoDialog from './TrajectInfoDialog.vue';
import TrajectCreateForm from './TrajectCreateForm.vue';

// Mobiele (sm) samenvoeging van de traject-knop en de artikel-tabbladen-knop:
// één full-width knop die een bottom-sheet opent met een "Traject"-lijst (de
// menu-acties + traject-switcher + nieuw traject) en — in de editor — een
// "Artikelen"-lijst (de open tabbladen). md/lg houden TrajectMenu + de
// document-tab-bar. Hergebruikt dezelfde composables/handlers als TrajectMenu.
const { trajects, activeTrajectRef, activeTraject, loading, createTraject } = useTrajects();
const { authenticated, login } = useAuth();
const documentsSheet = useDocumentsSheet();
const { documentTabs, activeDocumentTab, tabActions } = useAppChrome();
const route = useRoute();
const router = useRouter();

const sheetEl = ref(null);
const sheetMode = ref('list'); // 'list' | 'create'

// Editor = de editor-view publiceert open tabbladen via useAppChrome.
const hasArticles = computed(() => documentTabs.value.length > 0 && !!tabActions.value);

// --- Knoptekst: in de editor het actieve artikel · wet, met het traject als
//     ondersteunende tekst; in de bibliotheek de trajectnaam; anders Trajecten.
const buttonText = computed(() => {
  const tab = activeDocumentTab.value;
  if (tab && tabActions.value) {
    return `Artikel ${tab.articleNumber} · ${tabActions.value.displayName(tab)}`;
  }
  if (activeTraject.value) return activeTraject.value.name;
  if (loading.value && authenticated.value) return 'Traject…';
  return 'Trajecten';
});
const buttonSupporting = computed(() =>
  activeDocumentTab.value && activeTraject.value ? activeTraject.value.name : undefined,
);

const sheetTitle = computed(() => {
  if (sheetMode.value === 'create') return 'Nieuw traject';
  if (authenticated.value && activeTraject.value) return activeTraject.value.name;
  return 'Trajecten';
});

function openSheet() {
  sheetMode.value = 'list';
  sheetEl.value?.show();
}
function closeSheet() {
  sheetEl.value?.hide();
}
// Reset naar de lijst zodat een volgende opening niet in create-modus blijft.
function onSheetClose() {
  sheetMode.value = 'list';
}

// --- Navigatie naar traject (bibliotheek vs editor, zelfde wet/artikel) ---
async function goToTraject(trajectRef) {
  const lawId = route.params.lawId || undefined;
  const articleNumber = route.params.articleNumber || undefined;
  const inLibrary = route.name === 'library' || route.name === 'library-traject';
  await router.push({
    name: inLibrary ? 'library-traject' : 'editor-traject',
    params: { trajectRef, lawId, articleNumber },
  });
}

async function selectTraject(t) {
  if (!t.ref) {
    console.warn('MobileTrajectSheet: traject has no ref', t);
    return;
  }
  closeSheet();
  if (t.ref === activeTrajectRef.value) return;
  await goToTraject(t.ref);
}

// --- Traject-acties (sluiten dan openen — geen sheet-over-sheet) ---
function openDocuments() {
  closeSheet();
  documentsSheet.open();
}

const showMembers = ref(false);
const membersTrajectId = ref(null);
const membersTrajectName = ref('');
function openMembers() {
  if (!activeTraject.value) return;
  membersTrajectId.value = activeTraject.value.id;
  membersTrajectName.value = activeTraject.value.name;
  closeSheet();
  showMembers.value = true;
}

const showInfo = ref(false);
const infoTrajectId = ref(null);
const infoTrajectName = ref('');
function openInfo() {
  if (!activeTraject.value) return;
  infoTrajectId.value = activeTraject.value.id;
  infoTrajectName.value = activeTraject.value.name;
  closeSheet();
  showInfo.value = true;
}
function onTrajectDeleted(deletedId) {
  const activeRef = activeTrajectRef.value;
  const tail = String(deletedId).replace(/-/g, '').slice(-8);
  if (!activeRef || !activeRef.endsWith(`-${tail}`)) return;
  const inLibrary = route.name === 'library' || route.name === 'library-traject';
  router.push(inLibrary ? { name: 'library' } : { name: 'editor' });
}

// --- Nieuw traject: het gedeelde formulier in dezelfde sheet ---
const createFormEl = ref(null);
const createBusy = ref(false);
const createError = ref(null);
async function startCreate() {
  createError.value = null;
  sheetMode.value = 'create';
  await nextTick();
  createFormEl.value?.reset();
}
async function submitCreate() {
  createError.value = null;
  const { payload, error } = createFormEl.value.buildPayload();
  if (error) {
    createError.value = error;
    return;
  }
  createBusy.value = true;
  try {
    const created = await createTraject(payload);
    closeSheet();
    if (!created.ref) {
      console.warn('MobileTrajectSheet: created traject has no ref', created);
      return;
    }
    await goToTraject(created.ref);
  } catch (e) {
    createError.value = e.message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}

// --- Artikelen (open tabbladen), zelfde acties als de oude DocumentTabsSheet ---
function tabText(tab) {
  return `Artikel ${tab.articleNumber}`;
}
function tabSupporting(tab) {
  return tabActions.value?.displayName(tab);
}
function isActiveTab(tab) {
  const active = activeDocumentTab.value;
  return !!(active && tabActions.value && tabActions.value.key(active) === tabActions.value.key(tab));
}
function selectTab(tab) {
  closeSheet();
  tabActions.value?.select(tab);
}
function closeTab(tab) {
  tabActions.value?.close(tab);
}
// Reorder is tijdelijk uitgeschakeld op sm (drag-bugs). De editor exposeert
// `tabActions.reorder` nog, dus opnieuw inschakelen = `reorderable`
// @nldd-reorder + de drag-handle-cells terugzetten op de Artikelen-lijst.

// Sluit de sheet zodra het scherm groter dan sm wordt (md+ heeft eigen chrome).
let breakpointQuery = null;
function onBreakpointChange(e) {
  if (e.matches) closeSheet();
}
onMounted(() => {
  breakpointQuery = window.matchMedia?.('(min-width: 641px)') || null;
  breakpointQuery?.addEventListener?.('change', onBreakpointChange);
});
onBeforeUnmount(() => {
  breakpointQuery?.removeEventListener?.('change', onBreakpointChange);
});
</script>

<template>
  <nldd-button
    size="md"
    expandable
    start-icon="document"
    width="full"
    horizontal-alignment="left"
    single-line
    :text="buttonText"
    :supporting-text="buttonSupporting"
    @click="openSheet"
  ></nldd-button>

  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="bottom" @close="onSheetClose">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="sheetTitle"
          dismiss-text="Sluit"
          :back-text="sheetMode === 'create' ? 'Terug' : undefined"
          @dismiss="closeSheet"
          @back="sheetMode = 'list'"
        ></nldd-top-title-bar>

        <!-- Niet ingelogd -->
        <nldd-simple-section v-if="!authenticated">
          <nldd-inline-dialog
            icon="traject"
            text="Log in om een traject te kiezen of aan te maken"
            supporting-text="Zodra je bent ingelogd zie je hier je lopende trajecten en kun je gemakkelijk wisselen."
          >
            <nldd-button slot="actions" variant="primary" text="Inloggen" @click="login()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>

        <!-- Nieuw traject — zelfde formulier als de aanmaakpagina, in de sheet -->
        <nldd-simple-section v-else-if="sheetMode === 'create'">
          <TrajectCreateForm
            ref="createFormEl"
            :busy="createBusy"
            :error="createError"
            @submit="submitCreate"
          />
        </nldd-simple-section>

        <!-- Lijsten -->
        <nldd-simple-section v-else>
          <!-- Acties van het actieve traject — bovenaan, zonder titel (de
               sheet-titel dekt dit al). -->
          <nldd-list v-if="activeTraject" variant="box">
            <nldd-list-item size="md" button @click="openDocuments">
              <nldd-icon-cell size="20"><nldd-icon name="documents"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Werkdocumenten"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="md" button @click="openMembers">
              <nldd-icon-cell size="20"><nldd-icon name="person-2"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Leden"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="md" button @click="openInfo">
              <nldd-icon-cell size="20"><nldd-icon name="traject"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Traject details"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>

          <!-- Trajecten-switcher + nieuw traject — eigen lijst met titel. -->
          <nldd-spacer v-if="activeTraject" size="24"></nldd-spacer>
          <nldd-title size="5"><h2>Trajecten</h2></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list variant="box">
            <nldd-list-item
              v-for="t in trajects"
              :key="t.id"
              size="md"
              button
              :selected="t.ref === activeTrajectRef || undefined"
              @click="selectTraject(t)"
            >
              <nldd-text-cell :text="`${t.name}${t.status === 'afgerond' ? ' (afgerond)' : ''}`"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="md" button @click="startCreate">
              <nldd-icon-cell size="20"><nldd-icon name="plus"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Nieuw traject"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>

          <!-- Lijst 2: Artikelen (editor met open tabbladen). Reorder staat
               tijdelijk uit op sm — eerst nog wat drag-bugs oplossen. -->
          <template v-if="hasArticles">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5"><h2>Artikelen</h2></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="box">
              <nldd-list-item
                v-for="tab in documentTabs"
                :key="tabActions.key(tab)"
                size="md"
                button
                :selected="isActiveTab(tab) || undefined"
                @click="selectTab(tab)"
              >
                <nldd-text-cell :text="tabText(tab)" :supporting-text="tabSupporting(tab)"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <nldd-icon-button
                    size="sm"
                    icon="dismiss"
                    text="Sluit tabblad"
                    @click.stop="closeTab(tab)"
                  ></nldd-icon-button>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>

  <TrajectMembersDialog
    v-model="showMembers"
    :traject-id="membersTrajectId"
    :traject-name="membersTrajectName"
  />
  <TrajectInfoDialog
    v-model="showInfo"
    :traject-id="infoTrajectId"
    :traject-name="infoTrajectName"
    @deleted="onTrajectDeleted"
  />
</template>
