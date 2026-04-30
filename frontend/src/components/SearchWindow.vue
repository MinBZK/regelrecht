<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useBwbSearch, MIN_QUERY_LENGTH } from '../composables/useBwbSearch.js';
import { useBwbHarvest } from '../composables/useBwbHarvest.js';
import { useAuth } from '../composables/useAuth.js';

const props = defineProps({
  laws: { type: Array, default: () => [] },
  modelValue: { type: Boolean, default: false },
});

const emit = defineEmits(['update:modelValue', 'select-law', 'harvest-available']);

const { results: bwbResults, loading: bwbLoading, search: searchBwb, clear: clearBwb } = useBwbSearch();
const {
  harvestStatus, harvestSlugs, hasActiveHarvests,
  requestHarvest, isAvailable, isPolling, isTerminal,
  statusText, statusIcon,
} = useBwbHarvest();
const { authenticated, oidcConfigured, login } = useAuth();

const needsLogin = computed(() => oidcConfigured.value && !authenticated.value);

const search = ref('');
const searchFieldRef = ref(null);
const windowRef = ref(null);

function displayName(law) {
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}

const filteredLaws = computed(() => {
  const q = search.value.toLowerCase();
  if (!q) return props.laws;
  return props.laws.filter(law =>
    law.law_id.toLowerCase().includes(q) ||
    displayName(law).toLowerCase().includes(q)
  );
});

const hasSearch = computed(() => search.value.length > 0);

watch([search, filteredLaws], ([q, filtered]) => {
  clearBwb();
  if (!q || q.length < 3 || filtered.length > 0) return;
  if (needsLogin.value) return;
  searchBwb(q);
});

function bwbItemClick(result) {
  if (needsLogin.value) {
    login();
    return;
  }
  const status = harvestStatus.value[result.bwb_id];
  if (status === 'loading') return;
  const slug = harvestSlugs.value[result.bwb_id];
  if (isAvailable(status) && slug) {
    emit('harvest-available', slug);
    close();
  } else if (!status || isTerminal(status)) {
    requestHarvest(result.bwb_id);
  }
}

function close() {
  windowRef.value?.hide();
}

function onWindowClose() {
  search.value = '';
  clearBwb();
  emit('update:modelValue', false);
}

function selectLaw(lawId) {
  emit('select-law', lawId);
  close();
}

watch(() => props.modelValue, async (open) => {
  await nextTick();
  if (open) {
    windowRef.value?.show();
    await nextTick();
    const input = searchFieldRef.value?.shadowRoot?.querySelector('input')
      ?? searchFieldRef.value?.querySelector('input');
    input?.focus();
  } else {
    windowRef.value?.hide();
  }
});
</script>

<template>
  <Teleport to="body">
    <nldd-window
      ref="windowRef"
      accessible-label="Zoeken in wetten"
      top="0"
      left="calc(50vw - var(--components-window-default-width) / 2)"
      @close="onWindowClose"
    >
      <nldd-container padding="16">
        <div class="search-window-search-row">
          <nldd-search-field
            ref="searchFieldRef"
            size="md"
            placeholder="Zoeken"
            :value="search"
            @input="search = $event.target.value"
          ></nldd-search-field>
          <nldd-button size="md" text="Sluit" @click="close"></nldd-button>
        </div>

        <template v-if="hasSearch">
          <nldd-spacer size="16"></nldd-spacer>
          <div v-if="filteredLaws.length === 0 && needsLogin && search.length >= MIN_QUERY_LENGTH" class="search-window-login-prompt">
            <div class="search-window-empty-title">Log in om externe bronnen te doorzoeken</div>
            <div class="search-window-empty-subtitle">Inloggen is vereist om wetten op te halen van wetten.overheid.nl</div>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-button size="md" text="Inloggen" @click="login"></nldd-button>
          </div>
          <nldd-inline-dialog v-else-if="filteredLaws.length === 0 && bwbLoading" text="Zoeken op wetten.overheid.nl..."></nldd-inline-dialog>
          <template v-else-if="filteredLaws.length === 0 && bwbResults.length > 0">
            <nldd-title size="5"><h5>Resultaten van wetten.overheid.nl</h5></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="simple">
              <nldd-list-item
                v-for="result in bwbResults"
                :key="result.bwb_id"
                size="md"
                type="button"
                :disabled="harvestStatus[result.bwb_id] === 'loading'
                  || isPolling(harvestStatus[result.bwb_id])
                  || undefined"
                @click="bwbItemClick(result)"
              >
                <nldd-text-cell
                  :text="result.title"
                  :supporting-text="statusText(result.bwb_id, `${result.type} \u2014 ${result.bwb_id}`)"
                >
                </nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-icon-cell size="20">
                  <nldd-icon :name="statusIcon(result.bwb_id)"></nldd-icon>
                </nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
          <nldd-list
            v-else-if="filteredLaws.length > 0 || search.length >= MIN_QUERY_LENGTH"
            variant="simple"
            empty-text="Geen resultaten gevonden"
            empty-supporting-text="Pas je zoektermen of voorkeuren aan"
          >
            <nldd-list-item
              v-for="law in filteredLaws"
              :key="law.law_id"
              size="md"
              type="button"
              @click="selectLaw(law.law_id)"
            >
              <nldd-text-cell :text="displayName(law)" :supporting-text="law.source_name">
              </nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
      </nldd-container>
    </nldd-window>
  </Teleport>
</template>

<style>
.search-window-search-row {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8, 8px);
}

.search-window-search-row nldd-search-field {
  flex: 1;
}

.search-window-login-prompt {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--primitives-space-64, 64px) var(--primitives-space-16, 16px);
  text-align: center;
}

.search-window-empty-title {
  font-size: var(--primitives-font-size-200, 1.125rem);
  font-weight: var(--primitives-font-weight-medium, 500);
  color: light-dark(
    var(--primitives-color-neutral-400, #94a3b8),
    var(--primitives-color-neutral-500, #64748b)
  );
  margin-bottom: var(--primitives-space-4, 4px);
}

.search-window-empty-subtitle {
  font-size: var(--primitives-font-size-100, 0.875rem);
  color: light-dark(
    var(--primitives-color-neutral-350, #a1aab8),
    var(--primitives-color-neutral-550, #556275)
  );
}
</style>
