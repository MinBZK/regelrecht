<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useTrajects } from '../composables/useTrajects.js';
import TrajectMembersDialog from './TrajectMembersDialog.vue';

const props = defineProps({
  // Suffix to keep ids unique when this component is mounted in multiple
  // responsive headers (md/lg/sm) at the same time.
  idSuffix: { type: String, default: '' },
});

const emit = defineEmits(['switched']);

const {
  trajects,
  activeTrajectRef,
  activeTraject,
  loading,
  createTraject,
} = useTrajects();
const route = useRoute();
const router = useRouter();

/**
 * Navigate to a traject — push the user into `/editor/{ref}/{lawId?}`
 * preserving whichever law they were viewing. Per-tab state: a switch
 * here only affects this tab, never other open editors.
 */
async function goToTraject(trajectRef) {
  const lawId = route.params.lawId || undefined;
  const articleNumber = route.params.articleNumber || undefined;
  await router.push({
    name: 'editor',
    params: { trajectRef, lawId, articleNumber },
  });
}

/**
 * Leave traject scope — go to the global library. Keeps the open law
 * in the URL so the user lands on the same law (read-only).
 */
async function goToLibrary() {
  const lawId = route.params.lawId || undefined;
  const articleNumber = route.params.articleNumber || undefined;
  await router.push({
    name: 'library',
    params: { lawId, articleNumber },
  });
}

const menuBtnId = computed(() => `traject-menu-btn-${props.idSuffix}`);
const menuId = computed(() => `traject-menu-${props.idSuffix}`);

const activeLabel = computed(() => {
  if (loading.value) return 'Traject…';
  if (!activeTraject.value) return 'Geen traject';
  return activeTraject.value.name;
});

// --- Create-sheet state ---
const sheetEl = ref(null);
const showCreate = ref(false);

function emptyForm() {
  return {
    name: '',
    description: '',
    scope: '',
  };
}

const form = ref(emptyForm());
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
  form.value = emptyForm();
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

function closeCreate() {
  if (createBusy.value) return;
  showCreate.value = false;
}

async function selectNoTraject() {
  await goToLibrary();
  emit('switched', null);
}

async function selectTraject(t) {
  if (t.ref === activeTrajectRef.value) return;
  await goToTraject(t.ref);
  emit('switched', t.ref);
}

async function submitCreate() {
  createError.value = null;
  if (!form.value.name.trim()) {
    createError.value = 'Naam is verplicht';
    return;
  }
  createBusy.value = true;
  try {
    const created = await createTraject({
      name: form.value.name.trim(),
      description: form.value.description,
      scope: form.value.scope,
    });
    showCreate.value = false;
    // Jump straight into the new traject — same per-tab navigation as
    // selecting from the dropdown.
    await goToTraject(created.ref);
    emit('switched', created.ref);
  } catch (e) {
    createError.value = e.message || 'Aanmaken mislukt';
  } finally {
    createBusy.value = false;
  }
}

// Input event handlers: NDD text-field/text-area dispatch on the bare
// <input>/<textarea> element, so target.value is set; some custom-element
// variants dispatch a custom event with detail.value. Read both to stay
// robust across NDD versions (matches the pattern in EditSheet.vue).
function bind(field) {
  return (event) =>
    (form.value[field] = event.target?.value ?? event.detail?.value ?? form.value[field]);
}
</script>

<template>
  <nldd-button
    :id="menuBtnId"
    size="md"
    expandable
    :text="activeLabel"
    :popovertarget="menuId"
  ></nldd-button>
  <nldd-menu :id="menuId" :anchor="menuBtnId">
    <nldd-menu-item
      type="radio"
      :selected="activeTrajectRef === null || undefined"
      text="Geen traject"
      @select="selectNoTraject"
    ></nldd-menu-item>
    <nldd-menu-divider v-if="trajects.length > 0"></nldd-menu-divider>
    <nldd-menu-item
      v-for="t in trajects"
      :key="t.id"
      type="radio"
      :selected="t.ref === activeTrajectRef || undefined"
      :text="`${t.name}${t.status === 'afgerond' ? ' (afgerond)' : ''}`"
      @select="selectTraject(t)"
    ></nldd-menu-item>
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item
      v-if="activeTraject"
      text="Beheer leden…"
      start-icon="users"
      @click="openMembersForActive"
    ></nldd-menu-item>
    <nldd-menu-item
      text="Nieuw traject…"
      start-icon="plus"
      @click="openCreate"
    ></nldd-menu-item>
  </nldd-menu>

  <TrajectMembersDialog
    v-model="showMembers"
    :traject-id="membersTrajectId"
    :traject-name="membersTrajectName"
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
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar
          slot="header"
          text="Nieuw traject"
          :dismiss-text="createBusy ? '' : 'Annuleer'"
          @dismiss="closeCreate"
        ></nldd-top-title-bar>

        <nldd-simple-section>
          <nldd-list variant="box" class="traject-form-list">
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Naam"
                supporting-text="Korte herkenbare titel, bijv. 'Tariefswijziging 2026'."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  required
                  :value="form.name"
                  @input="bind('name')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Beschrijving"
                supporting-text="Waarom dit traject? (optioneel)"
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.description"
                  @input="bind('description')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Scope"
                supporting-text="Welke wetten of onderwerpen vallen onder dit traject? (vrije tekst, optioneel)"
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.scope"
                  @input="bind('scope')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

          <div class="traject-source-hint">
            Edits in dit traject worden gepusht naar een aparte branch op
            <code>MinBZK/regelrecht-corpus</code> (basis: <code>development</code>).
            Per-gebruiker GitHub-auth komt later.
          </div>

          <div v-if="createError" class="traject-error">{{ createError }}</div>
        </nldd-simple-section>

        <nldd-container slot="footer" padding="16">
          <div v-if="createBusy" class="traject-busy" role="status">
            <span class="traject-spinner" aria-hidden="true"></span>
            <span>Traject wordt aangemaakt en branch wordt op de remote gezet — dit kan even duren.</span>
          </div>
          <nldd-button
            variant="primary"
            size="md"
            full-width
            :text="createBusy ? 'Bezig…' : 'Aanmaken'"
            :disabled="createBusy || undefined"
            @click="submitCreate"
          ></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
.traject-form-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.traject-form-list nldd-text-field {
  width: 100%;
}
.traject-source-hint {
  margin-top: 16px;
  padding: 10px 12px;
  font-size: 13px;
  line-height: 1.4;
  color: var(--semantics-content-secondary-color, #555);
  background: var(--semantics-surfaces-tinted-background-color, #f4f4f4);
  border-radius: 6px;
}
.traject-source-hint code {
  font-size: 12px;
  padding: 1px 4px;
  background: var(--semantics-surfaces-background-color, #fff);
  border-radius: 3px;
}
.traject-error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
  margin-top: 12px;
}
.traject-busy {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  margin-bottom: 12px;
}
.traject-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: traject-spin 0.8s linear infinite;
  flex-shrink: 0;
}
@keyframes traject-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
