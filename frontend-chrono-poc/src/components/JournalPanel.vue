<script setup>
import { computed, ref, watch } from 'vue';
import GramRow from './GramRow.vue';
import { fieldValue } from '../world/events.js';
import { formatMoment, formatValue } from '../world/format.js';
import {
  askingCell,
  describeAccepted,
  describeAnswer,
  describeChange,
  journalCellOptions,
  journalRows,
} from '../world/journal.js';
import { gramByRef, gramKind } from '../world/snapshot.js';

// Het journaal: wie deed wat, en wat veranderde er daardoor aan de stand van de
// zaak. Eén regel per gebeurtenis, in de volgorde waarin ze ontstond.
//
// Dit is de hoofdweergave. De kolommen per cel laten zien wát er per cel ligt en
// het grammenpaneel wát er in de hele wereld ligt; dit laat zien hoe het zover
// kwam. Een cross-cel-vraag staat ingesprongen onder het besluit dat haar
// uitlokte: los gelezen is ze een vraag zonder aanleiding.
//
// Alles hier komt uit het beeld. Het verschil tussen "was" en "is" is door de
// wereld gemeten, op de kronieken zelf, vóór en ná de gebeurtenis; deze app
// rekent niets uit en zou dat ook niet kunnen — ze heeft geen kroniek.
//
// Een regel klapt uit naar haar grammen, wat het besluit accepteerde, en de
// verschillen per cel. De grammen zijn knoppen: ze openen het gram zoals het in
// zijn eigen kroniek staat, met de herkomst van elke waarde erbij. Dat is
// dezelfde rij als in de kolom van de cel (`GramRow`) en niet een tweede
// weergave ernaast. Bij een gram met een zaakkenmerk staat dat kenmerk vooraan:
// zo is te zien welke zaak deze gebeurtenis raakte, en waar het kenmerk vandaan
// komt dat een reductie straks als sleutel vraagt.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /** Het aantal journaalregels vóór de laatste stap; `null` = niets nieuw. */
  previousLength: { type: Number, default: null },
  /** De dag waarop de tijdlijn wijst; leeg is alle dagen. */
  focusMoment: { type: String, default: '' },
});

const emit = defineEmits(['clear-focus']);

/** Leeg is "alle": een filter dat niets uitsluit hoort geen keuze te heten. */
const actorFilter = ref('');
const cellFilter = ref('');

const rows = computed(() =>
  journalRows(props.snapshot, {
    previousLength: props.previousLength,
    actor: actorFilter.value,
    cell: cellFilter.value,
    moment: props.focusMoment,
  }),
);

/**
 * De actoren om op te filteren: wie er in dit journaal voorkomt.
 *
 * Uit de regels en niet uit de cellenlijst: de klok is ook een actor hier, en die
 * staat in geen enkele cellenlijst.
 */
const actorOptions = computed(() => {
  const seen = new Map();
  for (const row of rows.value) if (!seen.has(row.actor.id)) seen.set(row.actor.id, row.actor);
  return [...seen.values()];
});

/** De cellen om op te filteren: uit het beeld, ook als er nog niets gebeurde. */
const cellOptions = computed(() => journalCellOptions(props.snapshot));

const shown = computed(() => rows.value.filter((row) => row.matches));

const open = ref(new Set());

function isOpen(id) {
  return open.value.has(id);
}

/**
 * Een regel open- of dichtdoen.
 *
 * Een klik ín de uitklap telt niet mee: die bubbelt over de rij heen, en de rij
 * is zelf de knop. Dezelfde afweging als in het grammenpaneel, en dezelfde toets
 * als `nldd-list-item` zelf maakt — op het pad waarlangs de klik kwam.
 */
function toggle(id, event) {
  if (event?.target?.closest?.('nldd-list-item[slot="children"]')) return;
  const next = new Set(open.value);
  if (!next.delete(id)) next.add(id);
  open.value = next;
}

// Wijst de tijdlijn een dag aan, dan gaan de regels van die dag open: de klik
// bedoelde ze te zien, niet ze alleen over te houden.
watch(
  () => props.focusMoment,
  (moment) => {
    if (!moment) return;
    const next = new Set(open.value);
    for (const row of rows.value) if (row.entry.moment === moment) next.add(row.id);
    open.value = next;
  },
);

/** Het gram dat nu in de dialoog staat, met de kroniek waarin het ligt. */
const detail = ref(null);
const gramDialog = ref(null);

function showGram(row, gram) {
  // Het gram-id is `<cel>|<kroniek>|<plek>`; dezelfde weg terug als waarmee de
  // regel haar zaak leest, en dus niet een tweede manier om hetzelfde gram te
  // vinden.
  const found = gramByRef(props.snapshot, gram);
  detail.value = found ? { gram: found, ref: gram, moment: row.entry.moment } : null;
  if (found) gramDialog.value?.show?.();
}

/** Het ruwe gram, zoals het beeld het geeft. */
const detailJson = computed(() => (detail.value ? JSON.stringify(detail.value.gram, null, 2) : ''));

/** De verschillen van één regel op één lijn, voor onder de omschrijving. */
function changeSummary(entry) {
  return (entry.changes ?? []).map((change) => `${change.label}: ${describeChange(change)}`).join(' · ');
}

/** De parameters waarmee een cross-cel-vraag gesteld is. */
function questionParams(question) {
  return Object.entries(question?.params ?? {})
    .map(([name, value]) => `${name}: ${formatValue(value)}`)
    .join(' · ');
}
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-title size="5">
      <span>Journaal</span>
      <span slot="subtitle">wie deed wat, en wat veranderde daardoor aan de stand van de zaak</span>
    </nldd-title>

    <nldd-list type="tree" variant="box-tinted" accessible-label="Journaal van de wereld">
      <nldd-container slot="toolbar" layout="wrap" gap="16" vertical-alignment="bottom">
        <nldd-form-field label="Actor">
          <nldd-dropdown
            size="sm"
            width="220px"
            accessible-label="Filter op actor"
            @change="actorFilter = fieldValue($event, actorFilter)"
          >
            <select>
              <option value="" :selected="actorFilter === ''">Alle actoren</option>
              <option
                v-for="option in actorOptions"
                :key="option.id"
                :value="option.id"
                :selected="option.id === actorFilter"
              >
                {{ option.label }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <nldd-form-field label="Cel">
          <nldd-dropdown
            size="sm"
            width="220px"
            accessible-label="Filter op cel"
            @change="cellFilter = fieldValue($event, cellFilter)"
          >
            <select>
              <option value="" :selected="cellFilter === ''">Alle cellen</option>
              <option v-for="id in cellOptions" :key="id" :value="id" :selected="id === cellFilter">{{ id }}</option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>

        <!-- De tijdlijn onderaan wijst een dag aan; hier staat welke, met de weg
             terug naar het hele verhaal. -->
        <nldd-button
          v-if="focusMoment"
          size="sm"
          variant="neutral-tinted"
          start-icon="dismiss"
          :text="`Alleen ${formatMoment(focusMoment)} — toon alles`"
          @click="emit('clear-focus')"
        ></nldd-button>

        <nldd-tag size="sm" color="neutral" :text="`${shown.length} van ${rows.length} gebeurtenissen`"></nldd-tag>
      </nldd-container>

      <nldd-list-item
        v-for="row in rows"
        :key="row.id"
        size="sm"
        button
        :hidden="!row.matches || undefined"
        :expanded="isOpen(row.id) || undefined"
        @click="toggle(row.id, $event)"
      >
        <!-- Een vraag hangt onder het besluit dat haar uitlokte; de inspringing
             zegt dat, ook als er gefilterd is. -->
        <nldd-spacer-cell v-if="row.indented" size="20"></nldd-spacer-cell>
        <nldd-icon-cell :icon="row.actor.icon" size="16" color="secondary"></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell size="sm" width="104px" :text="formatMoment(row.entry.moment)"></nldd-text-cell>
        <nldd-text-cell size="sm" width="140px" :text="row.actor.label"></nldd-text-cell>
        <nldd-cell width="160px" hide-below="md">
          <nldd-tag size="sm" :color="row.kind.color" :icon="row.kind.icon" :text="row.kind.label"></nldd-tag>
        </nldd-cell>
        <nldd-text-cell
          size="sm"
          min-width="200px"
          :text="row.entry.description"
          :supporting-text="changeSummary(row.entry)"
        ></nldd-text-cell>
        <nldd-cell v-if="row.isNew" width="fit-content">
          <nldd-tag size="sm" color="accent" icon="new" text="nieuw"></nldd-tag>
        </nldd-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>

        <!-- De grammen die door deze gebeurtenis ontstonden. Knoppen: ze openen
             het gram zoals het in zijn eigen kroniek staat. -->
        <nldd-list-item
          v-for="gram in isOpen(row.id) ? row.grams : []"
          :key="gram.id"
          slot="children"
          size="sm"
          button
          @click="showGram(row, gram)"
        >
          <nldd-spacer-cell size="20"></nldd-spacer-cell>
          <nldd-icon-cell :icon="gramKind(gram.kind).icon" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell width="132px">
            <nldd-tag size="sm" :color="gramKind(gram.kind).color" :text="gramKind(gram.kind).label"></nldd-tag>
          </nldd-cell>
          <!-- De zaak voorop: dat kenmerk zegt waar dit gram bij hoort, en het
               is wat een besluit, een betaling en een latere vaststelling aan
               elkaar knoopt. Draagt het gram er geen, dan begint de regel
               gewoon bij de cel. -->
          <nldd-text-cell
            size="sm"
            min-width="160px"
            :text="gram.name"
            :supporting-text="[gram.zaak, gram.cell, gram.chronicle].filter(Boolean).join(' · ')"
          ></nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
        </nldd-list-item>

        <!-- Wat dit besluit van een ander accepteerde in plaats van na te rekenen.
             Invariant I5 in het verhaal, en niet alleen in de herkomst van een
             veld. -->
        <nldd-list-item
          v-for="(accepted, index) in isOpen(row.id) ? row.entry.accepted : []"
          :key="`geaccepteerd-${index}`"
          slot="children"
          size="sm"
        >
          <nldd-spacer-cell size="20"></nldd-spacer-cell>
          <nldd-icon-cell icon="handshake" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="sm"
            :text="describeAccepted(accepted)"
            supporting-text="geaccepteerd van de cel die het vaststelde, hier niet nagerekend"
          ></nldd-text-cell>
        </nldd-list-item>

        <!-- Wat er aan de stand van de zaak veranderde, per cel: was → is. -->
        <nldd-list-item
          v-for="(change, index) in isOpen(row.id) ? row.entry.changes : []"
          :key="`verschil-${index}`"
          slot="children"
          size="sm"
        >
          <nldd-spacer-cell size="20"></nldd-spacer-cell>
          <nldd-icon-cell icon="radar" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="sm"
            min-width="160px"
            :text="change.label"
            :supporting-text="`${change.cell} · ${change.lexostatus}`"
          ></nldd-text-cell>
          <nldd-text-cell
            size="sm"
            width="fit-content"
            max-width="55%"
            horizontal-alignment="right"
            :text="describeChange(change)"
          ></nldd-text-cell>
        </nldd-list-item>

        <!-- De vraag zelf: wie vroeg wat aan wie, en wat kwam eruit. -->
        <nldd-list-item v-if="isOpen(row.id) && row.entry.question" slot="children" size="sm">
          <nldd-spacer-cell size="20"></nldd-spacer-cell>
          <nldd-icon-cell icon="question" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="sm"
            min-width="160px"
            :text="`${askingCell(row.entry.question)} vroeg ${row.entry.question.answer.name}`"
            :supporting-text="[questionParams(row.entry.question), row.entry.question.signature].filter(Boolean).join(' · ')"
          ></nldd-text-cell>
          <nldd-text-cell
            size="sm"
            width="fit-content"
            max-width="55%"
            horizontal-alignment="right"
            :text="describeAnswer(row.entry.question)"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list-item>

      <nldd-inline-dialog
        slot="empty"
        icon="clock"
        text="Er is nog niets gebeurd"
        supporting-text="Doe een actie of spoel de klok vooruit; wat er dan gebeurt, komt hier te staan. Wat er bij het optuigen al lag, staat in de kolommen van de cellen."
      ></nldd-inline-dialog>
      <nldd-inline-dialog
        slot="no-results"
        icon="filter"
        text="Geen gebeurtenis voldoet aan het filter"
        supporting-text="Kies een andere actor, een andere cel of een andere dag om ze weer te zien."
      ></nldd-inline-dialog>
    </nldd-list>

    <!-- Het gram achter een regel: de rij zoals de cel hem in haar eigen kroniek
         toont, met de herkomst van elke waarde, plus het ruwe gram uit het beeld.
         Dezelfde rij als in de kolom van de cel, en geen tweede weergave. -->
    <nldd-modal-dialog
      ref="gramDialog"
      :text="detail ? detail.ref.name : 'Gram'"
      :supporting-text="detail ? `${detail.ref.cell} · kroniek '${detail.ref.chronicle}'` : ''"
      accessible-label="Gram uit het journaal"
    >
      <nldd-container v-if="detail" layout="stack" gap="16">
        <nldd-list type="tree" variant="box-base" accessible-label="Het gram zelf">
          <GramRow :gram="detail.gram" :clock="snapshot?.clock ?? null" position="only" />
        </nldd-list>
        <nldd-code-viewer language="json" variant="simple" wrap>{{ detailJson }}</nldd-code-viewer>
      </nldd-container>
      <nldd-button
        slot="actions"
        variant="neutral-tinted"
        text="Sluiten"
        @click="gramDialog?.hide?.()"
      ></nldd-button>
    </nldd-modal-dialog>
  </nldd-container>
</template>
