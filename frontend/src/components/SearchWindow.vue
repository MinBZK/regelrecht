<script setup>
import { ref, computed, watch, nextTick } from 'vue';

const props = defineProps({
  laws: { type: Array, default: () => [] },
  favorites: { type: Object, default: null },
  modelValue: { type: Boolean, default: false },
  anchorRect: { type: Object, default: null },
});

const emit = defineEmits(['update:modelValue', 'select-law']);

const search = ref('');
const searchFieldRef = ref(null);

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

function close() {
  search.value = '';
  emit('update:modelValue', false);
}

function selectLaw(lawId) {
  emit('select-law', lawId);
  close();
}

function onKeydown(e) {
  if (e.key === 'Escape') close();
}

watch(() => props.modelValue, async (open) => {
  if (open) {
    await nextTick();
    const input = searchFieldRef.value?.shadowRoot?.querySelector('input')
      ?? searchFieldRef.value?.querySelector('input');
    input?.focus();
  }
});
</script>

<template>
  <Teleport to="body">
    <div v-if="modelValue" class="search-window-backdrop" @click="close"></div>
    <div
      v-if="modelValue"
      class="search-window"
      :style="anchorRect ? {
        top: anchorRect.top + 'px',
        left: anchorRect.left + 'px',
        width: anchorRect.width + 'px',
      } : {}"
      @keydown="onKeydown"
    >
      <div class="search-window-header">
        <div class="search-window-search-row">
          <ndd-search-field
            ref="searchFieldRef"
            size="md"
            placeholder="Zoeken"
            :value="search"
            @input="search = $event.target.value"
          ></ndd-search-field>
          <ndd-button size="md" text="Sluit" @click="close"></ndd-button>
        </div>
        <div class="search-window-filters">
          <ndd-button size="sm" expandable disabled text="Alle ministeries"></ndd-button>
          <ndd-button size="sm" expandable disabled text="Alle regelgeving"></ndd-button>
          <ndd-button size="sm" expandable disabled text="Alle onderdelen"></ndd-button>
          <ndd-button size="sm" expandable disabled text="Alle periodes"></ndd-button>
        </div>
      </div>

      <div v-if="hasSearch" class="search-window-results">
        <div v-if="filteredLaws.length === 0" class="search-window-empty">
          <div class="search-window-empty-title">Geen resultaten gevonden</div>
          <div class="search-window-empty-subtitle">Pas je zoektermen of voorkeuren aan</div>
        </div>
        <ndd-list v-else variant="simple">
          <ndd-list-item
            v-for="law in filteredLaws"
            :key="law.law_id"
            size="md"
            type="button"
            @click="selectLaw(law.law_id)"
          >
            <ndd-text-cell :text="displayName(law)" :supporting-text="law.source_name">
            </ndd-text-cell>
          </ndd-list-item>
        </ndd-list>
      </div>
    </div>
  </Teleport>
</template>

<style>
.search-window-backdrop {
  position: fixed;
  inset: 0;
  z-index: 200;
}

.search-window {
  position: fixed;
  max-height: calc(100vh - 100px);
  display: flex;
  flex-direction: column;
  background: light-dark(
    var(--primitives-color-neutral-0, #fff),
    var(--primitives-color-neutral-950, #0c0e14)
  );
  border: 1px solid light-dark(
    var(--primitives-color-neutral-200, #e2e8f0),
    var(--primitives-color-neutral-800, #1e293b)
  );
  border-radius: var(--primitives-corner-radius-lg, 12px);
  box-shadow:
    0 4px 6px -1px rgba(0, 0, 0, 0.1),
    0 10px 15px -3px rgba(0, 0, 0, 0.2),
    0 20px 25px -5px rgba(0, 0, 0, 0.15);
  z-index: 201;
  overflow: hidden;
}

.search-window-header {
  display: flex;
  flex-direction: column;
  gap: var(--primitives-space-8, 8px);
  padding: var(--primitives-space-12, 12px);
}

.search-window-search-row {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8, 8px);
}

.search-window-search-row ndd-search-field {
  flex: 1;
}

.search-window-filters {
  display: flex;
  gap: var(--primitives-space-6, 6px);
  flex-wrap: wrap;
}

.search-window-results {
  flex: 1;
  overflow-y: auto;
  max-height: 340px;
  padding: 0 var(--primitives-space-12, 12px) var(--primitives-space-12, 12px);
}

.search-window-empty {
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
