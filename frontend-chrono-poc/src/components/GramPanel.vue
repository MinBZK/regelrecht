<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import { fieldValue } from '../world/events.js';
import { formatMoment } from '../world/format.js';
import { allGrams, cells, gramKind } from '../world/snapshot.js';

// Alle grammen van alle cellen in één chronologisch overzicht: moment, cel,
// kroniek, type, naam (met de zaak eronder als het gram er een draagt), kanaal en
// grondslag, en per rij het ruwe gram als JSON.
//
// De kolommen per cel vertellen wat één cel weet; dit vertelt wat er in de hele
// wereld ligt, op volgorde van gebeuren. Dat is — net als het observatielog —
// een leesbeeld van de opstelling: geen cel kan het opvragen, en er is niets
// hier dat een kroniek verandert.
//
// De uitklap toont het gram zoals het beeld het geeft, zonder uittreksel: elk
// veld met zijn herkomst, en verder alles wat het gram draagt. Het **receipt**
// zit er bewust niet in — `packages/simulator/src/snapshot.rs` laat het uit het
// beeld omdat het wandkloktijd draagt en een beeld dat per run verschilt geen
// contract is. Wat een lezer van het receipt nodig heeft, de herkomst van elke
// waarde, staat er per veld wel. Draagt een beeld ooit méér, dan staat dat hier
// vanzelf: er wordt niets weggelaten.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /**
   * Het gram waar de pagina op uitkomt, of leeg.
   *
   * Een id zoals het beeld het geeft (`<cel>|<kroniek>|<plek>`): dat is de
   * sleutel waarmee het journaal en de uitleg bij een lexostatus naar een gram
   * wijzen, en het is precies de sleutel van een rij hieronder.
   */
  focusGram: { type: String, default: '' },
});

const emit = defineEmits(['clear-focus']);

const rows = computed(() => allGrams(props.snapshot));

/** De cellen om op te filteren: uit het beeld, ook als er nog niets ligt. */
const cellOptions = computed(() => cells(props.snapshot).map((cell) => cell.id));

/**
 * De soorten om op te filteren: alleen wat er ligt.
 *
 * Een soort die in geen enkele kroniek voorkomt als keuze aanbieden, belooft een
 * uitkomst die er niet is.
 */
const kindOptions = computed(() => {
  const seen = [];
  for (const row of rows.value) if (!seen.includes(row.kind)) seen.push(row.kind);
  return seen.map((kind) => ({ kind, label: gramKind(kind).label }));
});

/** Leeg is "alle": een filter dat niets uitsluit hoort geen keuze te heten. */
const cellFilter = ref('');
const kindFilter = ref('');

function matches(row) {
  if (cellFilter.value && row.cell !== cellFilter.value) return false;
  if (kindFilter.value && row.kind !== kindFilter.value) return false;
  return true;
}

/** Wat er na het filteren overblijft, voor de telling boven de tabel. */
const shown = computed(() => rows.value.filter(matches));

// De rijen die eruit gefilterd zijn blijven staan met `hidden`: zo is het de
// lijst zelf die "niets gevonden" zegt (haar `no-results`-slot) in plaats van
// deze code, en blijven de filters staan als weg terug.
const open = ref(new Set());

function isOpen(id) {
  return open.value.has(id);
}

/**
 * Een rij open- of dichtdoen.
 *
 * Een klik ín de uitklap telt niet mee. Die bubbelt over de rij heen, en de rij
 * is zelf de knop, dus zonder deze toets zou de kopieerknop van de viewer — of
 * het aanwijzen van een regel JSON om hem te selecteren — de rij onder je handen
 * dichtdoen. Het ontwerpsysteem maakt in `nldd-list-item` dezelfde afweging: dat
 * toetst zijn eigen activering ook aan het pad waarlangs de klik kwam.
 *
 * De kinderrijen staan in `slot="children"`, dus daar is het aan te zien. Een
 * klik in de schaduw-DOM van een component daarbinnen wijst na retargeting naar
 * dat component zelf, en dat staat in dezelfde boom; het toetsenbord van de boom
 * klikt de knop van de rij aan en wijst daarmee naar de rij.
 */
function toggle(id, event) {
  if (event?.target?.closest?.('nldd-list-item[slot="children"]')) return;
  const next = new Set(open.value);
  if (!next.delete(id)) next.add(id);
  open.value = next;
}

/** Het ruwe gram, zoals het beeld het geeft. */
function gramJson(row) {
  return JSON.stringify(row.gram, null, 2);
}

/**
 * Een aangewezen gram in beeld brengen: filters die het verbergen gaan eraf, de
 * rij gaat open, en de pagina scrolt ernaartoe.
 *
 * De filters gaan alleen weg waar ze in de weg staan. Wie op een cel gefilterd
 * heeft en een gram van diezelfde cel aanwijst, houdt zijn filter — het weghalen
 * zou een keuze ongedaan maken die niets in de weg zat.
 *
 * De aanwijzing wordt daarna teruggemeld als verwerkt: zonder dat zou hetzelfde
 * gram een tweede keer aanwijzen niets doen, want de waarde verandert dan niet.
 */
watch(
  () => props.focusGram,
  (id) => {
    if (!id) return;
    const row = rows.value.find((candidate) => candidate.id === id);
    if (!row) {
      emit('clear-focus');
      return;
    }
    if (cellFilter.value && cellFilter.value !== row.cell) cellFilter.value = '';
    if (kindFilter.value && kindFilter.value !== row.kind) kindFilter.value = '';
    open.value = new Set(open.value).add(id);
    nextTick(() => {
      document.getElementById(`gram-${id}`)?.scrollIntoView?.({ block: 'center' });
      emit('clear-focus');
    });
  },
  { immediate: true },
);
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-title size="5">
      <span>Grammen</span>
      <span slot="subtitle">alles wat er in de wereld ligt, op volgorde van moment</span>
    </nldd-title>

    <nldd-list
      type="tree"
      variant="box-tinted"
      accessible-label="Alle grammen van alle cellen, chronologisch"
    >
      <nldd-container slot="toolbar" layout="wrap" gap="16" vertical-alignment="bottom">
        <nldd-form-field label="Cel">
          <nldd-dropdown
            size="sm"
            width="240px"
            accessible-label="Filter op cel"
            @change="cellFilter = fieldValue($event, cellFilter)"
          >
            <select>
              <option value="" :selected="cellFilter === ''">Alle cellen</option>
              <option v-for="id in cellOptions" :key="id" :value="id" :selected="id === cellFilter">{{ id }}</option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <nldd-form-field label="Type">
          <nldd-dropdown
            size="sm"
            width="240px"
            accessible-label="Filter op type gram"
            @change="kindFilter = fieldValue($event, kindFilter)"
          >
            <select>
              <option value="" :selected="kindFilter === ''">Alle typen</option>
              <option
                v-for="option in kindOptions"
                :key="option.kind"
                :value="option.kind"
                :selected="option.kind === kindFilter"
              >
                {{ option.label }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <nldd-tag
          size="sm"
          color="neutral"
          :text="`${shown.length} van ${rows.length} grammen`"
        ></nldd-tag>
      </nldd-container>

      <nldd-list-item
        v-for="row in rows"
        :id="`gram-${row.id}`"
        :key="row.id"
        size="sm"
        button
        :hidden="!matches(row) || undefined"
        :expanded="isOpen(row.id) || undefined"
        @click="toggle(row.id, $event)"
      >
        <nldd-icon-cell :icon="gramKind(row.kind).icon" size="16" color="secondary"></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell size="sm" width="104px" :text="formatMoment(row.opMoment)"></nldd-text-cell>
        <nldd-text-cell size="sm" width="150px" :text="row.cell"></nldd-text-cell>
        <!-- Wat er op een smal paneel als eerste weg mag: de kroniek staat ook in
             de uitklap, en het kanaal en de grondslag staan in de kolom van de
             cel zelf. Weglaten is wat het ontwerpsysteem hier aanbiedt; afkappen
             hoort niet bij de keuzes. -->
        <nldd-text-cell size="sm" width="150px" hide-below="md" :text="row.chronicle"></nldd-text-cell>
        <nldd-cell width="132px">
          <nldd-tag size="sm" :color="gramKind(row.kind).color" :text="gramKind(row.kind).label"></nldd-tag>
        </nldd-cell>
        <!-- Bij een besluit staat de zaak onder de naam: dat kenmerk is waaronder
             de zaak terug te vinden is, en het zegt waar dit gram bij hoort. Een
             gram zonder zaak draagt hier niets, en dan blijft de rij één regel. -->
        <nldd-text-cell size="sm" min-width="160px" :text="row.name" :supporting-text="row.zaak"></nldd-text-cell>
        <nldd-text-cell
          size="sm"
          width="120px"
          color="secondary"
          hide-below="lg"
          :text="row.intake"
        ></nldd-text-cell>
        <nldd-text-cell
          size="sm"
          min-width="140px"
          color="secondary"
          hide-below="lg"
          :text="row.grondslag"
        ></nldd-text-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>

        <!-- Alleen de open rij draagt haar viewer: honderd gesloten grammen
             zouden anders honderd editors opbouwen voor iets wat niemand ziet. -->
        <nldd-list-item v-if="isOpen(row.id)" slot="children" size="sm">
          <nldd-spacer-cell size="20"></nldd-spacer-cell>
          <nldd-cell width="full" vertical-alignment="top">
            <!-- De container eromheen is wat de viewer de volle breedte geeft:
                 een cel zet haar inhoud linksboven en laat haar krimpen, en dan
                 zou de kopieerknop midden in de rij komen te staan. -->
            <nldd-container layout="stack">
              <nldd-code-viewer language="json" variant="simple" wrap>{{ gramJson(row) }}</nldd-code-viewer>
            </nldd-container>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list-item>

      <nldd-inline-dialog
        slot="empty"
        icon="database"
        text="Nog geen grammen"
        supporting-text="Zolang er in geen enkele cel iets is vastgelegd, is er niets te tonen."
      ></nldd-inline-dialog>
      <nldd-inline-dialog
        slot="no-results"
        icon="filter"
        text="Geen gram voldoet aan het filter"
        supporting-text="Kies een andere cel of een ander type om ze weer te zien."
      ></nldd-inline-dialog>
    </nldd-list>
  </nldd-container>
</template>
