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
 * Navigate to a traject — push the user into the traject-scoped
 * editor at the same law they were viewing. Per-tab state: a switch
 * here only affects this tab, never other open editors.
 */
async function goToTraject(trajectRef) {
  const lawId = route.params.lawId || undefined;
  const articleNumber = route.params.articleNumber || undefined;
  await router.push({
    name: 'editor-traject',
    params: { trajectRef, lawId, articleNumber },
  });
}

/**
 * Leave traject scope. Stays in the editor (read-only view) when the
 * user is currently editing; goes to the library when they were
 * browsing. Either way the open law is preserved.
 */
async function leaveTraject() {
  const lawId = route.params.lawId || undefined;
  const articleNumber = route.params.articleNumber || undefined;
  const inEditor =
    route.name === 'editor' || route.name === 'editor-traject';
  await router.push({
    name: inEditor ? 'editor' : 'library',
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
    // When `useCustomRepo` is false the create call omits the repo
    // fields and the backend falls back to the default MinBZK repo —
    // existing trajects keep their behaviour unchanged.
    useCustomRepo: false,
    repo_owner: '',
    repo_name: '',
    base_branch: 'main',
    // Sub-path within the repo where regulation YAML files live. Empty
    // means "everything under repo root" — the right default for user
    // repos dedicated to regulations. Set to e.g. `regulation/nl` when
    // the YAMLs live in a sub-directory.
    repo_path: '',
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
  await leaveTraject();
  emit('switched', null);
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
  if (!form.value.name.trim()) {
    createError.value = 'Naam is verplicht';
    return;
  }

  // Build the request body. Only attach the repo fields when the toggle
  // is on — leaving them out lets the backend pick the MinBZK default
  // so existing flows are unchanged for users who don't need a custom
  // repo.
  const payload = {
    name: form.value.name.trim(),
    description: form.value.description,
    scope: form.value.scope,
  };
  if (form.value.useCustomRepo) {
    const owner = form.value.repo_owner.trim();
    const repo = form.value.repo_name.trim();
    const branch = form.value.base_branch.trim();
    if (!owner || !repo || !branch) {
      createError.value =
        'Eigen repo: vul owner, repo en base-branch in (of zet de schakelaar uit).';
      return;
    }
    payload.repo_owner = owner;
    payload.repo_name = repo;
    payload.base_branch = branch;
    // `repo_path` is optional; only attach when the user filled it in
    // so the backend keeps using its empty-string default ("repo root")
    // for personal regulation repos.
    const subpath = form.value.repo_path.trim();
    if (subpath) {
      payload.repo_path = subpath;
    }
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

          <!-- The `<label>` wraps the switch so clicking the label text
               toggles the control and screen readers announce the two
               together — `for=`/`id=` pairing is harder to keep stable
               across NDD's slotted internals. -->
          <label class="traject-source-toggle">
            <nldd-switch
              :checked="form.useCustomRepo ? true : undefined"
              @change="form.useCustomRepo = Boolean($event.detail?.checked)"
            ></nldd-switch>
            <span>Eigen GitHub-repo gebruiken (i.p.v. de standaard MinBZK-repo)</span>
          </label>

          <nldd-list v-if="form.useCustomRepo" variant="box" class="traject-form-list">
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Repo owner"
                supporting-text="Organisatie of gebruiker op GitHub, bijv. 'MinBZK'."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.repo_owner"
                  @input="bind('repo_owner')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Repo"
                supporting-text="Naam van de repository."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.repo_name"
                  @input="bind('repo_name')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Base branch"
                supporting-text="Branch waarop het traject z'n PR opent (vaak 'main')."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.base_branch"
                  @input="bind('base_branch')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell
                text="Subpath (optioneel)"
                supporting-text="Submap met regulation YAML-bestanden. Laat leeg voor repo-root."
                max-width="180px"
              ></nldd-text-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field
                  size="md"
                  :value="form.repo_path"
                  @input="bind('repo_path')($event)"
                ></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

          <div class="traject-source-hint">
            <template v-if="form.useCustomRepo">
              Edits worden gepusht naar
              <code>{{ form.repo_owner || '…' }}/{{ form.repo_name || '…' }}</code>
              (basis: <code>{{ form.base_branch || 'main' }}</code>).
              Je beheerder moet voor deze repo een <code>CORPUS_AUTH_*_TOKEN</code>
              env-var hebben gezet — anders krijg je een foutmelding bij aanmaken.
              Commits verschijnen onder je eigen naam (uit je SSO-account), niet
              onder het service-account.
            </template>
            <template v-else>
              Edits in dit traject worden gepusht naar een aparte branch op
              <code>MinBZK/regelrecht-corpus</code> (basis:
              <code>development</code>). Commits verschijnen onder je eigen naam
              (uit je SSO-account).
            </template>
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
.traject-source-toggle {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 16px 0 8px;
  font-size: 14px;
  cursor: pointer;
  user-select: none;
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
