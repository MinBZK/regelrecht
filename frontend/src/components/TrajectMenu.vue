<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { useTrajects } from '../composables/useTrajects.js';

const props = defineProps({
  // Suffix to keep ids unique when this component is mounted in multiple
  // responsive headers (md/lg/sm) at the same time.
  idSuffix: { type: String, default: '' },
});

const emit = defineEmits(['switched']);

const {
  trajects,
  activeTrajectId,
  activeTraject,
  loading,
  switchTraject,
  createTraject,
} = useTrajects();

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
    gh_owner: '',
    gh_repo: '',
    gh_branch: '',
    gh_base_branch: '',
    auth_ref: '',
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

function closeCreate() {
  if (createBusy.value) return;
  showCreate.value = false;
}

async function selectNoTraject() {
  await switchTraject(null);
  emit('switched', null);
}

async function selectTraject(id) {
  if (id === activeTrajectId.value) return;
  await switchTraject(id);
  emit('switched', id);
}

async function submitCreate() {
  createError.value = null;
  if (!form.value.name.trim()) {
    createError.value = 'Naam is verplicht';
    return;
  }
  if (!form.value.gh_owner.trim() || !form.value.gh_repo.trim()) {
    createError.value = 'GitHub owner en repo zijn verplicht';
    return;
  }
  createBusy.value = true;
  try {
    const created = await createTraject({
      name: form.value.name.trim(),
      description: form.value.description,
      scope: form.value.scope,
      writable_source: {
        name: `${form.value.gh_owner}/${form.value.gh_repo}`,
        gh_owner: form.value.gh_owner.trim(),
        gh_repo: form.value.gh_repo.trim(),
        gh_branch: form.value.gh_branch.trim() || null,
        gh_base_branch: form.value.gh_base_branch.trim() || null,
        auth_ref: form.value.auth_ref.trim() || null,
      },
    });
    showCreate.value = false;
    emit('switched', created.id);
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
      :selected="activeTrajectId === null || undefined"
      text="Geen traject"
      @select="selectNoTraject"
    ></nldd-menu-item>
    <nldd-menu-divider v-if="trajects.length > 0"></nldd-menu-divider>
    <nldd-menu-item
      v-for="t in trajects"
      :key="t.id"
      type="radio"
      :selected="t.id === activeTrajectId || undefined"
      :text="`${t.name}${t.status === 'afgerond' ? ' (afgerond)' : ''}`"
      @select="selectTraject(t.id)"
    ></nldd-menu-item>
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item
      text="Nieuw traject…"
      start-icon="plus"
      @click="openCreate"
    ></nldd-menu-item>
  </nldd-menu>

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

          <nldd-spacer size="16"></nldd-spacer>
          <nldd-title size="6"><h6>Schrijfbare bron</h6></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>

          <nldd-list variant="box" class="traject-form-list">
            <nldd-list-item size="md">
              <nldd-text-cell
                text="GitHub owner"
                supporting-text="Bijv. 'MinBZK' voor de centrale repo, of de naam van jouw fork."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  required
                  :value="form.gh_owner"
                  @input="bind('gh_owner')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="GitHub repo"
                supporting-text="Bijv. 'regelrecht-corpus'."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  required
                  :value="form.gh_repo"
                  @input="bind('gh_repo')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Branch"
                supporting-text="Naam van de branch waarop dit traject pusht. Leeg laten om automatisch te genereren uit de naam."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.gh_branch"
                  @input="bind('gh_branch')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Basis-branch"
                supporting-text="Branch om vanaf te vertakken als de bovenstaande nog niet bestaat. Standaard 'main'."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.gh_base_branch"
                  @input="bind('gh_base_branch')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Auth-ref"
                supporting-text="Naam van de token-entry in corpus-auth.yaml — alleen nodig voor private repos."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.auth_ref"
                  @input="bind('auth_ref')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

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
