<script setup>
import { computed, ref } from 'vue';
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
const dialogId = computed(() => `traject-create-dialog-${props.idSuffix}`);

const activeLabel = computed(() => {
  if (loading.value) return 'Traject…';
  if (!activeTraject.value) return 'Geen traject';
  return activeTraject.value.name;
});

// --- Create-dialog state ---
const showCreate = ref(false);
const form = ref({
  name: '',
  description: '',
  scope: '',
  gh_owner: '',
  gh_repo: '',
  gh_branch: '',
  auth_ref: '',
});
const createBusy = ref(false);
const createError = ref(null);

function openCreate() {
  createError.value = null;
  form.value = {
    name: '',
    description: '',
    scope: '',
    gh_owner: '',
    gh_repo: '',
    gh_branch: '',
    auth_ref: '',
  };
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
    createError.value = 'Owner en repo van de schrijfbare source zijn verplicht';
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

  <nldd-dialog v-if="showCreate" :id="dialogId" open @close="closeCreate">
    <div slot="title">Nieuw traject</div>
    <div class="traject-form">
      <nldd-text-field
        label="Naam"
        required
        :value="form.name"
        @input="form.name = $event.target.value"
      ></nldd-text-field>
      <nldd-text-area
        label="Beschrijving"
        :value="form.description"
        @input="form.description = $event.target.value"
      ></nldd-text-area>
      <nldd-text-area
        label="Scope"
        supporting-text="Vrije tekst — welke wetten of onderwerpen vallen onder dit traject?"
        :value="form.scope"
        @input="form.scope = $event.target.value"
      ></nldd-text-area>
      <nldd-text-field
        label="GitHub owner"
        supporting-text="Bijv. MinBZK, of jouw eigen org/user voor een fork."
        required
        :value="form.gh_owner"
        @input="form.gh_owner = $event.target.value"
      ></nldd-text-field>
      <nldd-text-field
        label="GitHub repo"
        required
        :value="form.gh_repo"
        @input="form.gh_repo = $event.target.value"
      ></nldd-text-field>
      <nldd-text-field
        label="Branch (optioneel)"
        supporting-text="Leeg laten om automatisch te genereren op basis van de naam."
        :value="form.gh_branch"
        @input="form.gh_branch = $event.target.value"
      ></nldd-text-field>
      <nldd-text-field
        label="Auth-ref (optioneel)"
        supporting-text="Naam van de token-entry in corpus-auth.yaml als je er een nodig hebt."
        :value="form.auth_ref"
        @input="form.auth_ref = $event.target.value"
      ></nldd-text-field>
      <div v-if="createError" class="traject-error">{{ createError }}</div>
    </div>
    <nldd-button
      slot="actions"
      variant="primary"
      :text="createBusy ? 'Bezig…' : 'Aanmaken'"
      :disabled="createBusy || undefined"
      @click="submitCreate"
    ></nldd-button>
    <nldd-button
      slot="actions"
      variant="secondary"
      text="Annuleren"
      :disabled="createBusy || undefined"
      @click="closeCreate"
    ></nldd-button>
  </nldd-dialog>
</template>

<style scoped>
.traject-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 320px;
}
.traject-error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
}
</style>
