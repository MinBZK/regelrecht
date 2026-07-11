<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useTrajects } from '../composables/useTrajects.js';
import { useDocumentsSheet } from '../composables/useDocumentsSheet.js';
import { useAuth } from '../composables/useAuth.js';
import { useLoginToChooser } from '../composables/useLoginToChooser.js';
import { homeTarget, isHomeSection } from '../composables/useLastVisitedRoute.js';
import TrajectMembersDialog from './TrajectMembersDialog.vue';
import TrajectInfoDialog from './TrajectInfoDialog.vue';
import TrajectCreateForm from './TrajectCreateForm.vue';

const props = defineProps({
  // Suffix to keep ids unique when this component is mounted in multiple
  // responsive headers (md/lg/sm) at the same time.
  idSuffix: { type: String, default: '' },
  // Mobile presentation: stretch the trigger button to the full toolbar
  // width with left-aligned content.
  fullWidth: { type: Boolean, default: false },
});

const emit = defineEmits(['switched']);

const {
  trajects,
  activeTrajectRef,
  activeTraject,
  loading,
  createTraject,
} = useTrajects();
// Auth gates the menu: logged-in users get the traject switcher; everyone
// else gets a popover explaining that trajecten unlock after login.
const { authenticated } = useAuth();
const route = useRoute();
const router = useRouter();
const documentsSheet = useDocumentsSheet();

// Not logged in: log in, then land on the trajectchooser carrying the current
// section + law/article (shared composable, see also MobileTrajectSheet).
const loginToChooser = useLoginToChooser();

/**
 * Navigate to a traject — push the user into the traject-scoped view of
 * the section they are currently in (bibliotheek or editor), at the same
 * law they were viewing. Picking a traject from the bibliotheek keeps you
 * in the bibliotheek; from the editor it keeps you in the editor. Per-tab
 * state: a switch here only affects this tab, never other open tabs.
 */
async function goToTraject(trajectRef) {
  const lawId = route.params.lawId || undefined;
  const articleNumber = route.params.articleNumber || undefined;
  // Stay in the section you're in: Home keeps you on Home (bare traject or its
  // corpus, per homeTarget); the editor keeps you in the editor.
  const target = isHomeSection(route.name)
    ? homeTarget({ trajectRef, lawId, articleNumber })
    : { name: 'editor-traject', params: { trajectRef, lawId, articleNumber } };
  await router.push(target);
}

const menuBtnId = computed(() => `traject-menu-btn-${props.idSuffix}`);
const menuId = computed(() => `traject-menu-${props.idSuffix}`);

const activeLabel = computed(() => {
  if (activeTraject.value) return activeTraject.value.name;
  // No traject selected → the button invites you to pick one. While the
  // list is still loading for a logged-in user, show a placeholder.
  if (loading.value && authenticated.value) return 'Traject…';
  return 'Trajecten';
});

// --- Create-sheet state ---
// Veldenstate en validatie leven in het gedeelde TrajectCreateForm
// (zelfde formulier als de /editor/nieuw-traject-pagina); de sheet bezit
// alleen submit, busy-state en navigatie.
const sheetEl = ref(null);
const createFormEl = ref(null);
const showCreate = ref(false);

const createBusy = ref(false);
const createError = ref(null);

// nldd-sheet exposes imperative show()/hide(); mirror showCreate into those
// calls so the sheet animates instead of mounting/unmounting (matches the
// ActionSheet / EditSheet pattern in this app).
watch(showCreate, async (v) => {
  await nextTick();
  if (v) sheetEl.value?.show();
  else sheetEl.value?.hide();
});

function openCreate() {
  createError.value = null;
  createFormEl.value?.reset();
  showCreate.value = true;
}

// --- Members dialog state ---
const showMembers = ref(false);
const membersTrajectId = ref(null);
const membersTrajectName = ref('');

function openMembersForActive() {
  if (!activeTraject.value) return;
  membersTrajectId.value = activeTraject.value.id;
  membersTrajectName.value = activeTraject.value.name;
  showMembers.value = true;
}

// --- Info dialog state ---
const showInfo = ref(false);
const infoTrajectId = ref(null);
const infoTrajectName = ref('');

function openInfoForActive() {
  if (!activeTraject.value) return;
  infoTrajectId.value = activeTraject.value.id;
  infoTrajectName.value = activeTraject.value.name;
  showInfo.value = true;
}

// Na een verwijderd traject: stond je erin (de actieve ref eindigt op de
// laatste 8 hex-tekens van het uuid), navigeer dan naar de sectie-root —
// editor → trajectkeuze, bibliotheek → gewone bibliotheek. De trajectlijst
// zelf is al ververst door deleteTraject.
function onTrajectDeleted(deletedId) {
  const ref = activeTrajectRef.value;
  const tail = String(deletedId).replace(/-/g, '').slice(-8);
  if (!ref || !ref.endsWith(`-${tail}`)) return;
  const inLibrary = isHomeSection(route.name);
  router.push(inLibrary ? { name: 'home' } : { name: 'editor' });
}

function closeCreate() {
  if (createBusy.value) return;
  showCreate.value = false;
}

async function selectTraject(t) {
  // `t.ref` is a server-supplied `Option<String>` and serialises to
  // `null` when a `TrajectSummary` is built without calling
  // `fill_ref()`. Refuse to navigate — silently routing to
  // `/editor/null/...` would just bounce off the trajectRef regex
  // and confuse the user. Treat as a programming error on the
  // backend side, log and bail.
  if (!t.ref) {
    console.warn('TrajectMenu: traject has no ref', t);
    return;
  }
  if (t.ref === activeTrajectRef.value) return;
  await goToTraject(t.ref);
  emit('switched', t.ref);
}

async function submitCreate() {
  createError.value = null;
  // Validatie en request-body komen uit het gedeelde formulier.
  const { payload, error } = createFormEl.value.buildPayload();
  if (error) {
    createError.value = error;
    return;
  }

  createBusy.value = true;
  try {
    const created = await createTraject(payload);
    showCreate.value = false;
    // Jump straight into the new traject — same per-tab navigation as
    // selecting from the dropdown. Mirror the `selectTraject` guard:
    // the backend's `create` handler always calls `fill_ref()` so this
    // shouldn't fire today, but a future refactor that returns a
    // half-filled `TrajectSummary` would otherwise navigate to
    // `/editor/undefined/...` and silently no-op.
    if (!created.ref) {
      console.warn('TrajectMenu: created traject has no ref', created);
      return;
    }
    await goToTraject(created.ref);
    emit('switched', created.ref);
  } catch (e) {
    createError.value = e.message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}

</script>

<template>
  <nldd-button
    :id="menuBtnId"
    size="md"
    expandable
    :start-icon="fullWidth ? 'traject' : undefined"
    :text="activeLabel"
    :popovertarget="menuId"
    :width="fullWidth ? 'full' : undefined"
    :horizontal-alignment="fullWidth ? 'left' : undefined"
  ></nldd-button>
  <!-- Logged in: the active traject's actions first, then the traject switcher
       + create below a divider. "Geen traject" is not an option — you leave
       traject scope by navigating, not from this menu. -->
  <nldd-menu v-if="authenticated" :id="menuId" :anchor="menuBtnId">
    <nldd-menu-item
      v-if="activeTraject"
      text="Werkdocumenten"
      icon="documents"
      @click="documentsSheet.open()"
    ></nldd-menu-item>
    <nldd-menu-item
      v-if="activeTraject"
      text="Leden"
      icon="person-2"
      @click="openMembersForActive"
    ></nldd-menu-item>
    <nldd-menu-item
      v-if="activeTraject"
      text="Traject details"
      icon="traject"
      @click="openInfoForActive"
    ></nldd-menu-item>
    <!-- The group draws its own divider above (auto-suppressed when it's the
         first child, i.e. no active-traject actions precede it), so no manual
         nldd-menu-divider here. -->
    <nldd-menu-group text="Trajecten">
      <nldd-menu-item
        v-for="t in trajects"
        :key="t.id"
        type="radio"
        :selected="t.ref === activeTrajectRef || undefined"
        :text="`${t.name}${t.status === 'afgerond' ? ' (afgerond)' : ''}`"
        @select="selectTraject(t)"
      ></nldd-menu-item>
      <nldd-menu-item
        text="Nieuw traject…"
        icon="plus"
        @click="openCreate"
      ></nldd-menu-item>
    </nldd-menu-group>
  </nldd-menu>

  <!-- Not logged in: no menu — a popover explaining that trajecten unlock
       once you sign in. -->
  <nldd-popover
    v-else
    :id="menuId"
    :anchor="menuBtnId"
    accessible-label="Trajecten"
    width="320px"
  >
    <nldd-container padding="16">
      <nldd-inline-dialog
        icon="login"
        text="Log in om een traject te kiezen of aan te maken"
        supporting-text="Zodra je bent ingelogd zie je hier je lopende trajecten en kun je gemakkelijk wisselen."
      >
        <nldd-button
          slot="actions"
          variant="primary"
          text="Inloggen"
          @click="loginToChooser"
        ></nldd-button>
      </nldd-inline-dialog>
    </nldd-container>
  </nldd-popover>

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

  <!-- Teleport the sheet out of the toolbar so it doesn't inherit the
       toolbar's positioning / clipping. Matches the ScenarioBuilder
       pattern. Each TrajectMenu instance (md/lg/sm) teleports its own
       sheet to body but only one is active per breakpoint, so they
       don't visually collide. -->
  <Teleport to="body">
    <nldd-sheet
      ref="sheetEl"
      placement="right"
      width="520px"
      full-height
      @close="closeCreate"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Nieuw traject"
          :dismiss-text="createBusy ? '' : 'Annuleer'"
          @dismiss="closeCreate"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <TrajectCreateForm ref="createFormEl"
            :busy="createBusy"
            :error="createError"
            @submit="submitCreate"
          />
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
