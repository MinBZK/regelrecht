<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useBwbSearch, MIN_QUERY_LENGTH } from '../composables/useBwbSearch.js';
import { useBwbHarvest } from '../composables/useBwbHarvest.js';
import { useAuth } from '../composables/useAuth.js';
import { useTrajects } from '../composables/useTrajects.js';
import { lawsListUrl } from '../composables/corpusUrls.js';
import { apiFetchJson } from '../lib/apiFetch.js';
import { useLatest } from '../lib/useLatest.js';

const emit = defineEmits(['select-law', 'harvest-available']);

const { activeTrajectRef } = useTrajects();

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

// Results of the server-side corpus search (`?q=`). The corpus index can
// hold thousands of laws, so the popover queries the backend rather than
// filtering a preloaded client-side list — that's the only way the search
// can reach every law, not just the first page. Ordered private-repo-first
// by the backend; `groupedLaws` sections them by source.
const serverLaws = ref([]);
// True while a corpus query is in flight, so the UI shows a spinner instead
// of briefly flashing "no results" (which would wrongly trigger the external
// fallback) before the response lands.
const searching = ref(false);
// True when the last corpus query failed (HTTP error or network error). Kept
// distinct from "0 results" so a backend failure surfaces an error instead of
// silently masquerading as "no match" and cascading to the external fallback.
const searchFailed = ref(false);

/**
 * Group the matched laws by their providing source, ordered by source
 * priority (lower = higher priority, so the traject's own writable repo at
 * priority 0 sorts above the seeded central corpus) and then alphabetically.
 * The popover renders one labelled section per group so a search surfaces
 * matches from the private repo and the central corpus under their own
 * headers — the external wetten.overheid.nl fallback only kicks in when the
 * corpus has no match (see `serverLaws` handling below).
 */
const groupedLaws = computed(() => {
  const groups = new Map();
  for (const law of serverLaws.value) {
    let group = groups.get(law.source_id);
    if (!group) {
      group = {
        source_id: law.source_id,
        source_name: law.source_name || '',
        priority: law.source_priority ?? Number.MAX_SAFE_INTEGER,
        laws: [],
      };
      groups.set(law.source_id, group);
    }
    group.laws.push(law);
  }
  return [...groups.values()].sort(
    (a, b) => a.priority - b.priority || a.source_name.localeCompare(b.source_name),
  );
});

const hasSearch = computed(() => search.value.length > 0);

// Debounce the corpus query so we don't fire a request per keystroke, and
// guard against out-of-order responses with a latest-claim.
const claimSearch = useLatest();
let debounceTimer = null;

watch(search, (q) => {
  clearBwb();
  searchFailed.value = false;
  if (debounceTimer) clearTimeout(debounceTimer);
  const term = q.trim();
  if (term.length < MIN_QUERY_LENGTH) {
    // Claim (and discard) a generation so any fetch already in flight
    // (debounce fired before this clear) is discarded when it resolves —
    // otherwise it would repopulate serverLaws for the cleared term and
    // fire a spurious BWB search.
    claimSearch();
    serverLaws.value = [];
    searching.value = false;
    return;
  }
  searching.value = true;
  debounceTimer = setTimeout(() => runCorpusSearch(term), 200);
});

async function runCorpusSearch(term) {
  const isCurrent = claimSearch();
  try {
    const url = lawsListUrl(activeTrajectRef.value, `q=${encodeURIComponent(term)}&limit=60`);
    const laws = await apiFetchJson(url);
    if (!isCurrent()) return; // a newer query superseded this one
    serverLaws.value = laws;
    searchFailed.value = false;
    // No match anywhere in the corpus → offer the external wetten.overheid.nl
    // search (unless the user must log in first to reach it).
    if (laws.length === 0 && !needsLogin.value) searchBwb(term);
  } catch {
    // Backend error (500/503/…) or network failure. Surface it rather than
    // letting the empty result cascade to the external wetten.overheid.nl
    // fallback, which would mask the failure with unrelated results.
    if (isCurrent()) {
      serverLaws.value = [];
      searchFailed.value = true;
    }
  } finally {
    if (isCurrent()) searching.value = false;
  }
}

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
  useCenteredPosition.value = window.matchMedia(`(min-width: ${BREAKPOINT_LG_MIN}px)`).matches;
  isAnchored.value = window.matchMedia(`(min-width: ${BREAKPOINT_MD_MIN}px) and (max-width: ${BREAKPOINT_LG_MIN - 1}px)`).matches;
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
        <!-- Corpus query in flight: show a spinner instead of briefly
             flashing "no results" and wrongly tripping the external fallback. -->
        <nldd-inline-dialog v-if="searching" text="Zoeken in de wetten…"></nldd-inline-dialog>
        <!-- Corpus query failed: surface the error instead of masking it as
             "no results" (which would cascade to the external fallback). -->
        <nldd-inline-dialog
          v-else-if="searchFailed"
          variant="alert"
          text="Zoeken is mislukt"
          supporting-text="De wetten konden niet worden doorzocht. Probeer het opnieuw."
        ></nldd-inline-dialog>
        <!-- Internal corpus matches: one labelled section per source, private
             repo first (priority 0), then the central corpus. The source name
             doubles as the group header, so the per-item supporting-text is
             dropped to avoid repeating it on every row. -->
        <template v-else-if="groupedLaws.length > 0">
          <template v-for="(group, groupIndex) in groupedLaws" :key="group.source_id">
            <nldd-spacer v-if="groupIndex > 0" size="16"></nldd-spacer>
            <nldd-title size="5"><h5>{{ group.source_name }}</h5></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list variant="simple">
              <nldd-list-item
                v-for="law in group.laws"
                :key="law.law_id"
                size="md"
                button
                @click="selectLaw(law.law_id)"
              >
                <nldd-text-cell :text="displayName(law)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </template>
        <!-- No corpus match → log in / search wetten.overheid.nl / empty. -->
        <div v-else-if="needsLogin && search.length >= MIN_QUERY_LENGTH" class="search-popover-login-prompt">
          <div class="search-popover-empty-title">Log in om externe bronnen te doorzoeken</div>
          <div class="search-popover-empty-subtitle">Inloggen is vereist om wetten op te halen van wetten.overheid.nl</div>
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-button size="md" text="Inloggen" @click="login()"></nldd-button>
        </div>
        <nldd-inline-dialog v-else-if="bwbLoading" text="Zoeken op wetten.overheid.nl..."></nldd-inline-dialog>
        <template v-else-if="bwbResults.length > 0">
          <nldd-title size="5"><h5>Resultaten van wetten.overheid.nl</h5></nldd-title>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-list variant="simple">
            <nldd-list-item
              v-for="result in bwbResults"
              :key="result.bwb_id"
              size="md"
              button
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
          v-else-if="search.length >= MIN_QUERY_LENGTH"
          variant="simple"
          empty-text="Geen resultaten gevonden"
          empty-supporting-text="Pas je zoektermen of voorkeuren aan"
        ></nldd-list>
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
