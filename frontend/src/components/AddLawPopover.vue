<script setup>
/**
 * AddLawPopover - "Wet toevoegen" vanuit een traject, in één flow:
 *
 * 1. Zoek in het centrale corpus (op naam en law-id) via dezelfde
 *    traject-scoped zoek-API die de bibliotheekzoeker voedt — het traject
 *    federeert de centrale corpus-seed, dus die zoekt door het volledige
 *    centrale corpus én markeert wat al in de eigen repo staat
 *    (source_priority 0). Géén tweede zoekpad.
 * 2. Gevonden? "Toevoegen aan traject" kopieert de volledige wet-map
 *    (versie-YAML's + scenario's) naar de traject-repo via de
 *    promote-endpoint. Staat de wet al in het traject, dan is de rij
 *    uitgeschakeld ("Al in dit traject").
 * 3. Niet gevonden? Met een BWB-id (direct getypt, of via de
 *    wetten.overheid.nl-zoeker) start "Ophalen naar traject" een
 *    traject-scoped harvest via het taken-mechanisme; de aanvraag is
 *    direct zichtbaar in het takenpaneel en het resultaat komt terug
 *    als review-taak.
 *
 * Positionering/plumbing (show/anchor/breakpoints, listbox-als-zoek-UI)
 * spiegelt SearchPopover.vue - dezelfde design-system-componenten en
 * hetzelfde combobox-patroon.
 */
import { ref, computed, watch, nextTick } from 'vue';
import { useBwbSearch, MIN_QUERY_LENGTH } from '../composables/useBwbSearch.js';
import { useTrajects } from '../composables/useTrajects.js';
import { lawsListUrl, lawPromoteUrl, trajectHarvestUrl } from '../composables/corpusUrls.js';
import { apiFetch, apiFetchJson, ApiError } from '../lib/apiFetch.js';
import { useLatest } from '../lib/useLatest.js';

const listTranslations = { 'components.list.search-placeholder-text': 'Zoek een wet of BWB-id…' };

const emit = defineEmits(['promoted', 'harvest-requested']);

const { activeTrajectRef } = useTrajects();
const { results: bwbResults, loading: bwbLoading, search: searchBwb, clear: clearBwb } = useBwbSearch();

const search = ref('');
const popoverRef = ref(null);
const useCenteredPosition = ref(true);
const isAnchored = ref(false);

// Zelfde breakpoints als SearchPopover (bron: design-system breakpoints.ts).
const BREAKPOINT_MD_MIN = 641;
const BREAKPOINT_LG_MIN = 1008;

// Corpus-treffers (traject-gefedereerd: eigen repo + centrale seed) voor de
// huidige zoekterm.
const centralLaws = ref([]);
// Law-ids die al in de traject-repo staan (source_priority 0): promoten is
// dan niet mogelijk. Een 409 van de backend vult deze set ook bij.
const inTrajectIds = ref(new Set());
const searching = ref(false);
const searchFailed = ref(false);

// Per law_id: 'busy' tijdens de promote-POST; 409 markeert de wet alsnog
// als al-in-traject. Per bwb_id: 'busy' | 'requested' | 'conflict' | 'error'.
const promoteState = ref({});
const promoteError = ref(null);
const harvestState = ref({});

function displayName(law) {
  if (law.display_name) return law.display_name;
  if (law.name) return law.name;
  return law.law_id.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

const sortedLaws = computed(() =>
  [...centralLaws.value].sort(
    (a, b) =>
      (a.source_priority ?? Number.MAX_SAFE_INTEGER) -
      (b.source_priority ?? Number.MAX_SAFE_INTEGER),
  ),
);

// Direct getypt BWB-id: bied ophalen aan zonder dat de externe zoeker het
// hoeft te vinden (acceptatie: "een BWB-id mag ook").
const bwbIdQuery = computed(() => {
  const term = search.value.trim().toUpperCase();
  return /^BWB[A-Z0-9]{3,17}$/.test(term) ? term : null;
});

// De directe rij alleen tonen zolang de externe zoeker hetzelfde id niet al
// als resultaat (met titel) toont — anders staan er twee rijen voor dezelfde
// wet met dezelfde harvest-knop.
const showDirectBwbRow = computed(
  () =>
    bwbIdQuery.value &&
    sortedLaws.value.length === 0 &&
    !bwbResults.value.some((r) => r.bwb_id === bwbIdQuery.value),
);

const claimSearch = useLatest();
let debounceTimer = null;

watch(search, (q) => {
  clearBwb();
  searchFailed.value = false;
  promoteError.value = null;
  if (debounceTimer) clearTimeout(debounceTimer);
  const term = q.trim();
  if (term.length < MIN_QUERY_LENGTH) {
    claimSearch();
    centralLaws.value = [];
    searching.value = false;
    return;
  }
  searching.value = true;
  debounceTimer = setTimeout(() => runSearch(term), 200);
});

async function runSearch(term) {
  const isCurrent = claimSearch();
  try {
    // Traject-scoped zoekopdracht: dekt de volledige centrale seed (het
    // traject federeert die) én de eigen repo. source_priority 0 = de
    // eigen schrijfbare repo → al in het traject, niet promoteerbaar.
    const laws = await apiFetchJson(
      lawsListUrl(activeTrajectRef.value, `q=${encodeURIComponent(term)}&limit=60`),
    );
    if (!isCurrent()) return;
    centralLaws.value = laws;
    inTrajectIds.value = new Set(
      laws.filter((l) => l.source_priority === 0).map((l) => l.law_id),
    );
    searchFailed.value = false;
    // Niets in het centrale corpus: val terug op wetten.overheid.nl zodat
    // de gebruiker met een BWB-id een traject-harvest kan starten.
    if (laws.length === 0) searchBwb(term);
  } catch {
    if (isCurrent()) {
      centralLaws.value = [];
      searchFailed.value = true;
    }
  } finally {
    if (isCurrent()) searching.value = false;
  }
}

function isInTraject(law) {
  return inTrajectIds.value.has(law.law_id);
}

async function promote(law) {
  if (!activeTrajectRef.value) return;
  const id = law.law_id;
  if (promoteState.value[id] === 'busy' || isInTraject(law)) return;
  promoteState.value = { ...promoteState.value, [id]: 'busy' };
  promoteError.value = null;
  try {
    await apiFetchJson(lawPromoteUrl(activeTrajectRef.value, id), { method: 'POST' });
    promoteState.value = { ...promoteState.value, [id]: 'done' };
    emit('promoted', id);
    close();
  } catch (e) {
    promoteState.value = { ...promoteState.value, [id]: null };
    if (e instanceof ApiError && e.status === 409) {
      // Backend is de autoriteit: markeer als al-in-traject.
      inTrajectIds.value = new Set([...inTrajectIds.value, id]);
    } else {
      promoteError.value =
        'Toevoegen aan het traject is mislukt. Probeer het opnieuw of neem contact op.';
    }
  }
}

async function requestTrajectHarvest(bwbId, lawName) {
  if (!activeTrajectRef.value) return;
  const state = harvestState.value[bwbId];
  if (state === 'busy' || state === 'requested') return;
  harvestState.value = { ...harvestState.value, [bwbId]: 'busy' };
  try {
    await apiFetch(trajectHarvestUrl(activeTrajectRef.value), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: bwbId, law_name: lawName || undefined }),
    });
    harvestState.value = { ...harvestState.value, [bwbId]: 'requested' };
    emit('harvest-requested', bwbId);
  } catch (e) {
    harvestState.value = {
      ...harvestState.value,
      [bwbId]: e instanceof ApiError && e.status === 409 ? 'conflict' : 'error',
    };
  }
}

function harvestStatusText(bwbId, fallback) {
  switch (harvestState.value[bwbId]) {
    case 'busy':
      return 'Aanvragen…';
    case 'requested':
      return 'Aanvraag gestart — volg de voortgang bij Taken';
    case 'conflict':
      return 'Er loopt al een aanvraag voor deze wet';
    case 'error':
      return 'Aanvragen mislukt — probeer het opnieuw';
    default:
      return fallback || '';
  }
}

function close() {
  popoverRef.value?.hide();
}

function onListInput(e) {
  search.value = e.detail?.value ?? '';
}

function onListKeydown(e) {
  if (e.key === 'Escape' && !e.defaultPrevented) close();
}

function onPopoverClose() {
  search.value = '';
  clearBwb();
  promoteError.value = null;
}

/** Public API: open het popover, verankerd aan het gegeven trigger-element. */
async function show(anchorEl) {
  if (!popoverRef.value) return;
  if (anchorEl) popoverRef.value.anchorElement = anchorEl;
  useCenteredPosition.value = window.matchMedia(`(min-width: ${BREAKPOINT_LG_MIN}px)`).matches;
  isAnchored.value = window.matchMedia(
    `(min-width: ${BREAKPOINT_MD_MIN}px) and (max-width: ${BREAKPOINT_LG_MIN - 1}px)`,
  ).matches;
  await nextTick();
  popoverRef.value.show();
}

// Zelfde focus-hook als SearchPopover: de listbox bezit z'n eigen input.
function onPopoverOpen() {
  const list = popoverRef.value?.querySelector('nldd-list');
  const input = list?.shadowRoot?.querySelector('.list__search-field-input');
  if (input) {
    input.value = '';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.focus();
  }
}

defineExpose({ show });
</script>

<template>
  <nldd-popover
    ref="popoverRef"
    accessible-label="Wet toevoegen aan traject"
    :width="useCenteredPosition ? '720px' : '360px'"
    placement="bottom-end"
    :centered="useCenteredPosition || null"
    :top="useCenteredPosition ? '0' : null"
    sm-full-height
    @open="onPopoverOpen"
    @close="onPopoverClose"
  >
    <nldd-container padding="16">
      <nldd-list
        type="listbox"
        variant="simple"
        height="min(70vh, 560px)"
        accessible-label="Wet toevoegen aan traject"
        :translations="listTranslations"
        empty-text="Geen resultaten gevonden"
        empty-supporting-text="Zoek op naam, law-id of BWB-id"
        @input="onListInput"
        @keydown="onListKeydown"
      >
        <nldd-button
          v-if="!isAnchored"
          slot="search-bar-end"
          size="md"
          text="Sluit"
          @click="close"
        ></nldd-button>

        <!-- Fout bij promoten: één banner boven de resultaten. -->
        <nldd-list-item v-if="promoteError" size="md">
          <nldd-icon-cell size="20" icon="alert" color="critical"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell :text="promoteError" color="critical"></nldd-text-cell>
        </nldd-list-item>

        <!-- Centrale-corpus-treffers: promoten naar het traject. -->
        <nldd-list-item
          v-for="law in sortedLaws"
          :key="law.law_id"
          size="md"
          :data-law-id="law.law_id"
        >
          <nldd-text-cell
            :text="displayName(law)"
            :supporting-text="isInTraject(law) ? 'Al in dit traject' : law.source_name || undefined"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell>
            <nldd-button
              size="sm"
              variant="primary"
              text="Toevoegen aan traject"
              :disabled="isInTraject(law) || undefined"
              :loading="promoteState[law.law_id] === 'busy' || undefined"
              @click="promote(law)"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>

        <!-- Direct getypt BWB-id: traject-harvest zonder externe zoeker.
             De corpus-zoeker matcht niet op BWB-id, dus "0 treffers" bewijst
             hier niet dat de wet buiten het corpus valt — geen "niet in het
             centrale corpus"-claim in de begeleidende tekst. -->
        <nldd-list-item v-if="showDirectBwbRow" size="md" :data-bwb-id="bwbIdQuery">
          <nldd-icon-cell size="20"><nldd-icon name="harvest"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            :text="bwbIdQuery"
            :supporting-text="harvestStatusText(bwbIdQuery, 'Haal dit BWB-id op uit wetten.overheid.nl')"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell>
            <nldd-button
              size="sm"
              variant="primary"
              text="Ophalen naar traject"
              :disabled="['busy', 'requested'].includes(harvestState[bwbIdQuery]) || undefined"
              @click="requestTrajectHarvest(bwbIdQuery)"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>

        <!-- Externe wetten.overheid.nl-treffers (alleen wanneer het centrale
             corpus niets vond): traject-scoped harvest per resultaat. -->
        <nldd-list-item
          v-for="result in bwbResults"
          :key="result.bwb_id"
          size="md"
          :data-bwb-id="result.bwb_id"
        >
          <nldd-icon-cell size="20"><nldd-icon name="harvest"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            :text="result.title"
            :supporting-text="harvestStatusText(result.bwb_id, `${result.type} - ${result.bwb_id}`)"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell>
            <nldd-button
              size="sm"
              variant="primary"
              text="Ophalen naar traject"
              :disabled="['busy', 'requested'].includes(harvestState[result.bwb_id]) || undefined"
              @click="requestTrajectHarvest(result.bwb_id, result.title)"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>

        <!-- States zonder zichtbare opties. -->
        <div slot="empty">
          <nldd-inline-dialog v-if="searching" text="Zoeken in het centrale corpus…"></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="searchFailed"
            variant="alert"
            text="Zoeken is mislukt"
            supporting-text="Het centrale corpus kon niet worden doorzocht. Probeer het opnieuw."
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="bwbLoading"
            text="Zoeken op wetten.overheid.nl..."
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else-if="search.length > 0 && search.length < MIN_QUERY_LENGTH"
            text="Typ minimaal drie tekens om te zoeken"
          ></nldd-inline-dialog>
          <nldd-inline-dialog
            v-else
            text="Geen resultaten gevonden"
            supporting-text="Zoek op naam, law-id of BWB-id (bijv. BWBR0002399)"
          ></nldd-inline-dialog>
        </div>
      </nldd-list>
    </nldd-container>
  </nldd-popover>
</template>
