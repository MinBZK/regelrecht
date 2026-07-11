<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useTrajects } from '../composables/useTrajects.js';
import { useAuth } from '../composables/useAuth.js';
import { useLoginToChooser } from '../composables/useLoginToChooser.js';
import { homeTarget, isHomeSection } from '../composables/useLastVisitedRoute.js';
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
const { authenticated } = useAuth();
const { documentTabs, activeDocumentTab, tabActions } = useAppChrome();
const route = useRoute();
const router = useRouter();

// Not logged in: log in, then land on the trajectchooser carrying the current
// section + law/article (shared composable, see also TrajectMenu).
const loginToChooser = useLoginToChooser();

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
  if (menuLoading.value) return 'Trajecten';
  // Logged in without a traject = the global corpus scope.
  if (authenticated.value) return 'Corpus juris';
  return 'Trajecten';
});
// A traject in the URL whose name hasn't resolved yet: the trigger shows a
// spinner (see :loading) with the neutral 'Trajecten' label, not a '…'
// placeholder. Corpus juris (no ref) is known immediately, so no spinner there.
const menuLoading = computed(
  () => !!activeTrajectRef.value && loading.value && !activeTraject.value,
);
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
  const target = isHomeSection(route.name)
    ? homeTarget({ trajectRef, lawId, articleNumber })
    : { name: 'editor-traject', params: { trajectRef, lawId, articleNumber } };
  await router.push(target);
}

// Switch to the traject-less global corpus ("Corpus juris") — a peer of the
// trajecten in the switcher. Closes the sheet and carries the open law.
function goToCorpusJuris() {
  closeSheet();
  if (!activeTrajectRef.value) return; // already on Corpus juris
  router.push(homeTarget({
    lawId: route.params.lawId || undefined,
    articleNumber: route.params.articleNumber || undefined,
  }));
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
  if (activeTrajectRef.value) {
    router.push({ name: 'werkdocumenten-traject', params: { trajectRef: activeTrajectRef.value } });
  }
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
  const inLibrary = isHomeSection(route.name);
  router.push(inLibrary ? { name: 'home' } : { name: 'editor' });
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
    if (!created.ref) {
      // A half-filled summary would navigate to /editor/undefined; surface it
      // and keep the sheet open instead of silently closing after a real
      // resource was created (re-submitting would make a duplicate).
      console.warn('MobileTrajectSheet: created traject has no ref', created);
      createError.value =
        'Traject aangemaakt maar kon niet worden geopend. Ga terug en selecteer het.';
      return;
    }
    closeSheet();
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
    :loading="menuLoading || undefined"
    start-icon="traject"
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
            icon="login"
            text="Log in om een traject te kiezen of aan te maken"
            supporting-text="Zodra je bent ingelogd zie je hier je lopende trajecten en kun je gemakkelijk wisselen."
          >
            <nldd-button slot="actions" variant="primary" text="Inloggen" @click="loginToChooser"></nldd-button>
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
          <nldd-list v-if="activeTraject" variant="box" arrow-navigation>
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

          <!-- Trajecten-switcher + nieuw traject. The "Trajecten" section title
               only shows when the traject actions precede it (activeTraject); on
               Corpus juris nothing is above it, so it would just duplicate the
               sheet header. -->
          <template v-if="activeTraject">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5"><h2>Trajecten</h2></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
          </template>
          <nldd-list variant="box" arrow-navigation>
            <!-- "Corpus juris" = the traject-less global scope, the default
                 option (like `main` among the branches). -->
            <nldd-list-item
              size="md"
              button
              :selected="!activeTrajectRef || undefined"
              @click="goToCorpusJuris"
            >
              <nldd-spacer-cell slot="start" size="12"></nldd-spacer-cell>
              <nldd-icon-cell v-if="!activeTrajectRef" slot="start" size="20"><nldd-icon name="check-mark"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell v-else slot="start" size="20"></nldd-spacer-cell>
              <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
              <nldd-text-cell text="Corpus juris"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item
              v-for="t in trajects"
              :key="t.id"
              size="md"
              button
              :selected="t.ref === activeTrajectRef || undefined"
              @click="selectTraject(t)"
            >
              <nldd-spacer-cell slot="start" size="12"></nldd-spacer-cell>
              <nldd-icon-cell v-if="t.ref === activeTrajectRef" slot="start" size="20"><nldd-icon name="check-mark"></nldd-icon></nldd-icon-cell>
              <nldd-spacer-cell v-else slot="start" size="20"></nldd-spacer-cell>
              <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
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
                <nldd-spacer-cell slot="start" size="12"></nldd-spacer-cell>
                <nldd-icon-cell v-if="isActiveTab(tab)" slot="start" size="20"><nldd-icon name="check-mark"></nldd-icon></nldd-icon-cell>
                <nldd-spacer-cell v-else slot="start" size="20"></nldd-spacer-cell>
                <nldd-spacer-cell slot="start" size="8"></nldd-spacer-cell>
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
    @left="onTrajectDeleted"
  />
</template>
