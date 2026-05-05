<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useBwbSearch, MIN_QUERY_LENGTH } from '../composables/useBwbSearch.js';
import { useBwbHarvest } from '../composables/useBwbHarvest.js';
import { useAuth } from '../composables/useAuth.js';

const props = defineProps({
  laws: { type: Array, default: () => [] },
});

const emit = defineEmits(['select-law', 'harvest-available']);

const { results: bwbResults, loading: bwbLoading, search: searchBwb, clear: clearBwb } = useBwbSearch();
const {
  harvestStatus, harvestSlugs, hasActiveHarvests,
  requestHarvest, isAvailable, isPolling, isTerminal,
  statusText, statusIcon,
} = useBwbHarvest();
const { authenticated, oidcConfigured, login } = useAuth();

const needsLogin = computed(() => oidcConfigured.value && !authenticated.value);

const search = ref('');
const popoverRef = ref(null);
const useCenteredPosition = ref(true);
// md only: popover anchors below the trigger button — clicking outside closes
// it, so an explicit Sluit button is just clutter. On sm (full-height sheet)
// and lg (centered overlay) the user needs an explicit way to close.
const isAnchored = ref(false);

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
  popoverRef.value?.hide();
}

/**
 * Escape behaviour: first press clears the query, second press (when
 * already empty) closes the popover. Mirrors how Safari/Spotlight handle
 * Esc in search inputs. We always preventDefault so cross-browser
 * inconsistencies in native <input type="search"> Esc-clear don't leak
 * through (Safari clears, Chromium doesn't).
 */
function onSearchKeydown(e) {
  if (e.key !== 'Escape') return;
  e.preventDefault();
  if (search.value) {
    search.value = '';
    clearBwb();
  } else {
    close();
  }
}

function onPopoverClose() {
  search.value = '';
  clearBwb();
}

function selectLaw(lawId) {
  emit('select-law', lawId);
  close();
}

/**
 * Public API: open the popover anchored to the given trigger element.
 * Parent calls this from its trigger click/focus handlers and passes
 * `event.currentTarget` (or any DOM element) as the anchor. We set
 * `anchorElement` programmatically so the same popover can be triggered
 * from multiple places (e.g. desktop search-field and mobile icon-button)
 * without each one needing a stable ID.
 *
 * Optional `initialSearch` lets the trigger seed the search value — used
 * for the Spotlight-style "type-to-open" UX where pressing a key on the
 * bar's search-field intercepts the keystroke and forwards it as the
 * initial query in the popover.
 */
async function show(anchorEl, initialSearch = '') {
  if (!popoverRef.value) return;
  if (anchorEl) popoverRef.value.anchorElement = anchorEl;
  if (initialSearch) search.value = initialSearch;
  // On lg the popover is centered top (large dropdown overlay style).
  // On md it anchors below the trigger button (Floating UI placement),
  // matching the smaller toolbar's button-as-trigger feel.
  // On sm the popover renders as a full-height sheet (CSS-driven).
  useCenteredPosition.value = window.matchMedia('(min-width: 1008px)').matches;
  isAnchored.value = window.matchMedia('(min-width: 696px) and (max-width: 1007px)').matches;
  // Wait for Vue to propagate the new `centered` / `top` / `width` props
  // onto the popover element before opening — otherwise the first
  // reposition() inside the popover's toggle handler runs against the
  // previous breakpoint's props (e.g. centered=true held over from lg)
  // and the popover lands in the wrong position. The Lit-side update on
  // prop change DOES re-trigger reposition, but in some cases the
  // initial Floating-UI run wins for the visual frame.
  await nextTick();
  popoverRef.value.show();
}

/**
 * Auto-focus the internal search input on open. Listening to the popover's
 * `open` event (rather than awaiting nextTick after show()) is the only
 * reliable hook: popover's own _manageFocus() runs after computePosition +
 * updateComplete and would otherwise steal focus back to the host. The
 * `open` event fires AFTER _manageFocus, so our focus call wins last.
 *
 * The shadow-root vs light-DOM lookup is to be defensive — different
 * design-system versions may render the <input> differently.
 */
function onPopoverOpen() {
  const field = popoverRef.value?.querySelector('nldd-search-field');
  const native = field?.shadowRoot?.querySelector('input') ?? field?.querySelector('input');
  native?.focus();
}

defineExpose({ show });
</script>

<template>
  <!--
    Two positioning modes — toggled by `show()` based on viewport:
    - lg+: free-positioned (centered horizontally, top-aligned). Big
      Spotlight-style overlay regardless of which trigger fired show().
    - md: anchored below the trigger button via Floating UI (default).
    On sm the bottom-sheet @media rule wins; sm-full-height fills.
  -->
  <nldd-popover
    ref="popoverRef"
    accessible-label="Zoeken in wetten"
    :width="useCenteredPosition ? '720px' : '360px'"
    placement="bottom-end"
    :centered="useCenteredPosition || null"
    :top="useCenteredPosition ? '0' : null"
    sm-full-height
    @open="onPopoverOpen"
    @close="onPopoverClose"
  >
    <nldd-container padding="16">
      <div class="search-popover-search-row">
        <nldd-search-field
          size="md"
          placeholder="Zoeken"
          :value="search"
          @input="search = $event.target.value"
          @keydown="onSearchKeydown"
        ></nldd-search-field>
        <!-- Op md anchort de popover naast de trigger — naast de popover
             klikken sluit 'm. Op sm (full-height sheet) en lg (centered
             overlay) heeft de gebruiker een expliciete sluit-knop nodig. -->
        <nldd-button v-if="!isAnchored" size="md" text="Sluit" @click="close"></nldd-button>
      </div>

      <template v-if="hasSearch">
        <nldd-spacer size="16"></nldd-spacer>
        <div v-if="filteredLaws.length === 0 && needsLogin && search.length >= MIN_QUERY_LENGTH" class="search-popover-login-prompt">
          <div class="search-popover-empty-title">Log in om externe bronnen te doorzoeken</div>
          <div class="search-popover-empty-subtitle">Inloggen is vereist om wetten op te halen van wetten.overheid.nl</div>
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
                :supporting-text="statusText(result.bwb_id, `${result.type} — ${result.bwb_id}`)"
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
  </nldd-popover>
</template>

<style>
.search-popover-search-row {
  display: flex;
  align-items: center;
  gap: var(--primitives-space-8, 8px);
}

.search-popover-search-row nldd-search-field {
  flex: 1;
}

.search-popover-login-prompt {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--primitives-space-64, 64px) var(--primitives-space-16, 16px);
  text-align: center;
}

.search-popover-empty-title {
  font-size: var(--primitives-font-size-200, 1.125rem);
  font-weight: var(--primitives-font-weight-medium, 500);
  color: light-dark(
    var(--primitives-color-neutral-400, #94a3b8),
    var(--primitives-color-neutral-500, #64748b)
  );
  margin-bottom: var(--primitives-space-4, 4px);
}

.search-popover-empty-subtitle {
  font-size: var(--primitives-font-size-100, 0.875rem);
  color: light-dark(
    var(--primitives-color-neutral-350, #a1aab8),
    var(--primitives-color-neutral-550, #556275)
  );
}
</style>
