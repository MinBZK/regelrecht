<script setup>
import { computed, ref, watch } from 'vue';
import TraceNode from './TraceNode.vue';
import { fetchGramReceipt } from '../api/worldApi.js';
import { formatMoment, formatValue, humanize } from '../world/format.js';
import {
  acceptedValues,
  loadedRegulations,
  receiptSections,
  receiptTimestamp,
  receiptTrace,
} from '../world/receipt.js';

// Het uitvoeringsreceipt van één decretogram, leesbaar.
//
// RFC-022 §1.2: een decretogram *is* het RFC-013 Execution Receipt van het
// besluit. Het beeld van de wereld draagt het met opzet niet — het bevat
// wandkloktijd, en een contract dat per run verschilt is geen contract — dus het
// wordt hier **apart** opgehaald, per gram en alleen als een lezer erom vraagt.
// Dat is de hele reden dat dit een eigen component is met een eigen verzoek: het
// beeld blijft receipt-loos.
//
// De secties komen van de server en worden hier niet uitgedund (zie
// `world/receipt.js`). Drie krijgen een eigen weergave, omdat ze in een opsomming
// zouden verdwijnen terwijl ze het bewijs dragen: de **geladen regelingen** met
// hun hash (zonder die is de uitvoering niet te reproduceren), de
// **geaccepteerde waarden** met hun bron-cel en het bevoegd gezag van die bron
// (invariant I5 — geaccepteerd hoort niet op berekend te lijken), en de
// **uitvoeringstrace**: de stappen waarlangs het besluit tot stand kwam, met per
// stap de regeling en het artikel. Die laatste is een boom, en uitgevouwen tot
// regels zou precies de vorm wegvallen die haar leesbaar maakt.

const props = defineProps({
  /** De cel in wiens kroniek het gram ligt. */
  cell: { type: String, required: true },
  /** De kroniekstroom. */
  chronicle: { type: String, required: true },
  /** De plek van het gram in die stroom, geteld vanaf nul. */
  index: { type: Number, required: true },
});

const receipt = ref(null);
const error = ref(null);
const loading = ref(false);

/**
 * De takken van de uitvoeringstrace die uitgeklapt staan, op hun pad.
 *
 * Alleen de wortel bij het binnenkomen: een trace van honderden stappen in één
 * keer is geen uitleg maar een muur. Wie een stap opendoet, kiest zelf hoe diep
 * hij kijkt.
 */
const ROOT = '0';
const openBranches = ref(new Set([ROOT]));

/**
 * Haal het receipt op.
 *
 * Bij elke wijziging van het gram opnieuw, en niet één keer bij het opbouwen: de
 * lijst van grammen hergebruikt haar rijen, dus een component dat blijft staan
 * kan een ander gram te zien krijgen. Een verouderd antwoord wordt weggegooid —
 * het laatste verzoek wint — zodat er nooit het receipt van een vorig gram onder
 * de naam van dit gram staat.
 */
let latest = 0;
async function load(cell, chronicle, index) {
  const ticket = (latest += 1);
  loading.value = true;
  error.value = null;
  try {
    const answer = await fetchGramReceipt(cell, chronicle, index);
    if (ticket !== latest) return;
    receipt.value = answer;
    // Een ander receipt is een andere trace: de paden van de ene zeggen niets
    // over de andere, en een tak die "nog open stond" zou hier een willekeurige
    // tak zijn.
    openBranches.value = new Set([ROOT]);
  } catch (e) {
    if (ticket !== latest) return;
    receipt.value = null;
    error.value = e?.message ?? 'Het receipt kon niet opgehaald worden.';
  } finally {
    if (ticket === latest) loading.value = false;
  }
}

watch(
  () => [props.cell, props.chronicle, props.index],
  ([cell, chronicle, index]) => load(cell, chronicle, index),
  { immediate: true },
);

const sections = computed(() => receiptSections(receipt.value));
const regulations = computed(() => loadedRegulations(receipt.value));
const accepted = computed(() => acceptedValues(receipt.value));
const timestamp = computed(() => receiptTimestamp(receipt.value));
const trace = computed(() => receiptTrace(receipt.value));

function toggleBranch(path) {
  const next = new Set(openBranches.value);
  if (!next.delete(path)) next.add(path);
  openBranches.value = next;
}

/**
 * De toetsaanslagen van de trace blijven bij de trace.
 *
 * Dit paneel hangt in de uitklap van een rij van het grammenoverzicht, en dat
 * overzicht is zélf een `nldd-list` van het type `tree`. Een boom voert zijn
 * toetsenbord uit op de `keydown` die bij hem langskomt, en hij houdt die
 * gebeurtenis niet tegen. Zonder deze regel handelt de trace een pijltje af én
 * borrelt hetzelfde pijltje door naar het overzicht eromheen, dat de aanwijzing
 * dan uit de trace wegtrekt naar een gram — de lezer raakt bij de eerste pijl
 * omlaag kwijt waar hij was.
 *
 * `stopPropagation` en niet `stopImmediatePropagation`: de boom hiernaast — de
 * lijst waarop deze regel staat — hoort zijn eigen toets gewoon af te handelen.
 * Alleen de weg naar buiten gaat dicht.
 */
function keepKeysInTheTrace(event) {
  event.stopPropagation();
}

/**
 * De kop van de tijdstempel-banner.
 *
 * Draagt het receipt geen wandkloktijd, dan staat dát er — een lege plek is hier
 * zelf iets om te zien, en "Uitgevoerd op" gevolgd door niets is een melding die
 * belooft wat ze niet waarmaakt.
 */
const executedAt = computed(() =>
  timestamp.value?.wallClock
    ? `Uitgevoerd op ${timestamp.value.wallClock}`
    : 'Geen wandkloktijd vastgelegd bij deze uitvoering',
);

/** De regel onder een geaccepteerde waarde: waar ze vandaan komt en wanneer. */
function acceptedDetail(value) {
  return [
    value.lexostatus ? `lexostatus: ${value.lexostatus}` : '',
    value.field && value.field !== value.output ? `uitkomst: ${value.field}` : '',
    value.opMoment ? `geldig op ${formatMoment(value.opMoment)}` : '',
    value.askedBy ? `gevraagd door ${value.askedBy}` : '',
  ]
    .filter(Boolean)
    .join(' · ');
}

/** De geldigheid van een geladen regeling, in woorden. */
function validity(regulation) {
  if (!regulation.validFrom && !regulation.validTo) return 'geen versie vastgelegd';
  const from = regulation.validFrom ? `vanaf ${formatMoment(regulation.validFrom)}` : '';
  const to = regulation.validTo ? `tot en met ${formatMoment(regulation.validTo)}` : '';
  return [from, to].filter(Boolean).join(' · ');
}
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-activity-indicator
      v-if="loading"
      show-text
      text="Receipt ophalen…"
      timing="instant"
      size="24"
    ></nldd-activity-indicator>

    <nldd-inline-dialog
      v-else-if="error"
      icon="warning"
      text="Geen receipt"
      :supporting-text="error"
    ></nldd-inline-dialog>

    <template v-else-if="receipt">
      <!-- Het label dat het hele verschil draagt: dit is wandkloktijd, en dat is
           waarom het beeld van de wereld dit receipt niet kan dragen. -->
      <nldd-banner
        v-if="timestamp"
        variant="neutral"
        icon="time"
        :text="executedAt"
        :supporting-text="timestamp.note"
      ></nldd-banner>

      <!-- Ook als de lijst leeg is: dat een besluit niets van een ander overnam is
           zelf iets om te zien (invariant I5), en de lege stand van de tabel zegt
           dat met zoveel woorden in plaats van stil te verdwijnen. -->
      <nldd-title size="6">
        <span>Geaccepteerde waarden</span>
        <span slot="subtitle">van een ander overgenomen in plaats van nagerekend</span>
      </nldd-title>
      <nldd-table
        columns="minmax(160px, 1fr) 120px minmax(200px, 1fr) minmax(160px, 1fr)"
        accessible-label="Waarden die dit besluit van een andere cel accepteerde"
        empty-text="Geen geaccepteerde waarden"
        empty-supporting-text="Dit besluit rekende alles zelf uit."
      >
        <nldd-table-row slot="header">
          <nldd-text-cell size="sm" text="Waarde"></nldd-text-cell>
          <nldd-text-cell size="sm" text="Uitkomst"></nldd-text-cell>
          <nldd-text-cell size="sm" text="Bron"></nldd-text-cell>
          <nldd-text-cell size="sm" text="Bevoegd gezag"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="value in accepted" :key="value.output">
          <nldd-text-cell size="sm" :text="humanize(value.output)"></nldd-text-cell>
          <nldd-text-cell size="sm" :text="formatValue(value.value)"></nldd-text-cell>
          <nldd-text-cell
            size="sm"
            :text="`cel '${value.cell}'`"
            :supporting-text="acceptedDetail(value)"
          ></nldd-text-cell>
          <!-- Zwijgt de bron over haar gezag, dan staat dát er, en niet het
               cel-id nog een keer onder een andere naam: een adres is geen
               gezag. -->
          <nldd-cell v-if="value.authority">
            <nldd-tag size="sm" color="oranje" :text="value.authority"></nldd-tag>
          </nldd-cell>
          <nldd-text-cell
            v-else
            size="sm"
            color="secondary"
            text="niet genoemd door de bron"
          ></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>

      <nldd-title size="6">
        <span>Geladen regelingen</span>
        <span slot="subtitle">met de hash waarmee deze uitvoering te reproduceren is</span>
      </nldd-title>
      <nldd-table
        columns="minmax(260px, 1fr) 160px minmax(240px, 2fr)"
        accessible-label="De regelingen die tijdens de uitvoering geladen waren"
        empty-text="Geen geladen regelingen"
        empty-supporting-text="Dit receipt noemt geen regelingen."
      >
        <nldd-table-row slot="header">
          <nldd-text-cell size="sm" text="Regeling"></nldd-text-cell>
          <nldd-text-cell size="sm" text="Versie"></nldd-text-cell>
          <nldd-text-cell size="sm" text="Hash"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="(regulation, position) in regulations" :key="position">
          <nldd-text-cell size="sm" :text="regulation.id"></nldd-text-cell>
          <nldd-text-cell size="sm" :text="validity(regulation)"></nldd-text-cell>
          <nldd-text-cell
            size="sm"
            color="secondary"
            :text="regulation.hash ?? 'geen hash vastgelegd'"
          ></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>

      <!-- De trace: langs welke artikelen dit besluit tot stand kwam. Zonder
           haar staat er wel wát eruit kwam, maar niet waaróp — en dan is het
           besluit na te rekenen maar niet na te lopen (RFC-013). -->
      <nldd-title size="6">
        <span>Uitvoeringstrace</span>
        <span slot="subtitle">de stappen van de uitvoering, met per stap de regeling en het artikel</span>
      </nldd-title>
      <nldd-list
        v-if="trace"
        type="tree"
        variant="box-tinted"
        accessible-label="De stappen waarlangs dit besluit tot stand kwam"
        @keydown="keepKeysInTheTrace"
      >
        <TraceNode :node="trace" :open="openBranches" @toggle="toggleBranch" />
      </nldd-list>
      <nldd-inline-dialog
        v-else
        icon="info"
        text="Geen uitvoeringstrace"
        supporting-text="Dit receipt draagt er geen; het besluit is dan wel na te rekenen, maar niet stap voor stap na te lopen."
      ></nldd-inline-dialog>

      <template v-for="section in sections" :key="section.key">
        <nldd-title size="6">
          <span>{{ section.label }}</span>
          <span v-if="section.note" slot="subtitle">{{ section.note }}</span>
        </nldd-title>
        <nldd-list variant="box-tinted" :accessible-label="section.label">
          <nldd-list-item v-for="row in section.rows" :key="row.name" size="sm">
            <nldd-text-cell size="sm" min-width="160px" :text="humanize(row.name)"></nldd-text-cell>
            <nldd-text-cell
              size="sm"
              width="fit-content"
              max-width="55%"
              horizontal-alignment="right"
              :text="formatValue(row.value)"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </template>
    </template>
  </nldd-container>
</template>
