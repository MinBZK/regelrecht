<script setup>
import { ref, computed, watch } from 'vue';
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

// Source of truth: @nldd/design-system's `assets/styles/breakpoints.ts`
// (smMax: 640px, mdMin: 641px, mdMax: 1007px, lgMin: 1008px). Keep in
// sync until the design system exports breakpoints publicly — currently
// the file isn't listed in the package's `exports` field, so a deep
// import isn't supported. The previous values used `696` for mdMin
// which didn't match the design-system at all (probably a typo); aligned
// with mdMin=641 so the popover's anchored-vs-centered transition fires
// at the same width as bar-split-view's md/lg switch.
const BREAKPOINT_MD_MIN = 641;
const BREAKPOINT_LG_MIN = 1008;

function displayName(law) {
  // Prefer the API's resolved `display_name`: laws can have a dynamic
  // `name: "#output_ref"` in YAML that the backend resolves via the
  // matching action output. Without this check we'd render the raw
  // `#output_ref` string for those laws.
  if (law.display_name) return law.display_name;
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
  if (!q || q.length < MIN_QUERY_LENGTH || filtered.length > 0) return;
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
/**
 * Open + focus MUST stay synchronous within the trusted tap gesture.
 *
 * iOS/iPadOS (and partly Android) only raise the on-screen keyboard when
 * `input.focus()` runs synchronously inside the user-activation of the
 * originating tap. An `await nextTick()` (or focusing later from the
 * popover's `open` event, after its async _manageFocus) breaks that
 * activation: focus still lands in the field but the keyboard never
 * appears. So no `await` may run before show()+focus().
 *
 * The nextTick previously existed only to let Vue propagate the
 * `centered`/`top`/`width` props before the first Floating-UI reposition
 * (md anchored mode). We instead set those props imperatively on the
 * element here, which is effective synchronously — keeping positioning
 * correct without breaking the gesture, on every viewport (phone + iPad).
 */
function show(anchorEl, initialSearch = '') {
  const pop = popoverRef.value;
  if (!pop) return;
  if (anchorEl) pop.anchorElement = anchorEl;
  if (initialSearch) search.value = initialSearch;

  // lg: centered top overlay · md: anchored below trigger · sm: CSS sheet
  const centered = window.matchMedia(`(min-width: ${BREAKPOINT_LG_MIN}px)`).matches;
  const anchored = window.matchMedia(`(min-width: ${BREAKPOINT_MD_MIN}px) and (max-width: ${BREAKPOINT_LG_MIN - 1}px)`).matches;
  useCenteredPosition.value = centered;
  isAnchored.value = anchored;

  // Set positioning props imperatively so they take effect this tick
  // (no await). Vue's reactive bindings re-apply the same values on the
  // next update — no conflict.
  pop.centered = centered || null;
  pop.top = centered ? '0' : null;
  pop.width = centered ? '720px' : '360px';

  pop.show();
  focusSearchInput();
}

/**
 * Focus the internal search input. Called synchronously from show()
 * (raises the mobile keyboard, in-gesture) and again from the popover's
 * `open` event as a desktop fallback — the popover's own _manageFocus()
 * runs after computePosition + updateComplete and would otherwise steal
 * focus back to the host; re-focusing the same input is idempotent and
 * does not dismiss an already-open keyboard.
 *
 * Shadow-root vs light-DOM lookup is defensive — different design-system
 * versions may render the <input> differently.
 */
function focusSearchInput() {
  const field = popoverRef.value?.querySelector('nldd-search-field');
  const native = field?.shadowRoot?.querySelector('input') ?? field?.querySelector('input');
  native?.focus();
}

function onPopoverOpen() {
  focusSearchInput();
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
