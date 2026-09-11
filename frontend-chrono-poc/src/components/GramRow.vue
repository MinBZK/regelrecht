<script setup>
import { computed, ref } from 'vue';
import { formatMissing, formatMoment, formatValue, humanize } from '../world/format.js';
import { describeOrigin, gramFields, gramKind, obligationsOf, regulationOf } from '../world/snapshot.js';

// Eén gram in een kroniek: wat er gebeurde, wanneer, langs welk kanaal, en — als
// het een besluit is — waar elke waarde waarop besloten is vandaan komt.
//
// Het gram is een rij in de boom van zijn kroniek; de uitklap gebruikt de
// disclosure van het ontwerpsysteem (`button` + `expanded` op de rij, `disclosure`
// op het chevron) en de kinderrijen staan in de `children`-slot die de rij zelf
// als groep rendert. Geen eigen accordeon.
//
// Een decretogram klapt uit, een executogram niet: dat laatste is wat de cel
// overkwam, en dan is de herkomst van elk veld de vastlegging zelf — dat staat al
// in de rij. Een decretogram is besloten, en dan valt de herkomst per waarde
// uiteen. Dat verschil te kunnen zien is het punt.

const props = defineProps({
  /** Het gram uit het beeld. */
  gram: { type: Object, required: true },
  /** Waar de klok staat, om verleden van toekomst te scheiden. */
  clock: { type: String, default: null },
  /** Is dit gram erbij gekomen sinds de laatste stap? */
  isNew: { type: Boolean, default: false },
  /** Plek in de kroniek, voor de lijn van de tijdlijn-cel. */
  position: { type: String, default: 'between' },
});

const open = ref(false);
const kind = computed(() => gramKind(props.gram.kind));
const expandable = computed(() => props.gram.kind === 'decretogram');
// De namen van de tijdlijn-cel zelf: `status` is 'past' | 'current' | 'future' |
// 'none'. Wat op of vóór de klok ligt is gebeurd, wat erna komt staat er nog aan
// te komen.
const status = computed(() => (props.clock && props.gram.op_moment > props.clock ? 'future' : 'past'));
const regulation = computed(() => regulationOf(props.gram));
const obligations = computed(() => obligationsOf(props.gram));

/** De velden van het gram, elk met zijn herkomst al uitgeschreven. */
const fields = computed(() =>
  gramFields(props.gram).map((field) => {
    const origin = describeOrigin(field.origin);
    // Bij een onbekende waarde staat erbij wát er ontbreekt: een feit dat bestaat
    // en dat niemand aanleverde is iets anders dan een waarde die er niet is.
    const detail = [origin.details.map((pair) => `${pair.term}: ${pair.value}`).join(' · '), formatMissing(field.value)]
      .filter(Boolean)
      .join(' · ');
    return { ...field, origin, detail };
  }),
);

/** De regel onder de naam: wanneer, langs welk kanaal, op welke grondslag. */
const supporting = computed(() => {
  const parts = [formatMoment(props.gram.op_moment), props.gram.intake].filter(Boolean);
  if (props.gram.grondslag) parts.push(props.gram.grondslag);
  return parts.join(' · ');
});

/** Eén verplichting als regel: elke kolom die ze draagt, in haar eigen woorden. */
function obligationText(row) {
  return obligations.value.columns
    .filter((column) => row?.[column] !== undefined)
    .map((column) => `${column}: ${formatValue(row[column])}`)
    .join(' · ');
}
</script>

<template>
  <nldd-list-item
    size="sm"
    :button="expandable || undefined"
    :expanded="expandable ? open : undefined"
    @click="expandable && (open = !open)"
  >
    <nldd-timeline-track-cell :status="status" :position="position"></nldd-timeline-track-cell>
    <nldd-icon-cell :icon="kind.icon" size="16" color="secondary"></nldd-icon-cell>
    <nldd-spacer-cell size="8"></nldd-spacer-cell>
    <nldd-text-cell size="sm" min-width="120px" :text="gram.name" :supporting-text="supporting"></nldd-text-cell>
    <nldd-cell>
      <nldd-tag size="sm" :color="kind.color" :text="kind.label"></nldd-tag>
    </nldd-cell>
    <nldd-cell v-if="isNew">
      <nldd-tag size="sm" color="accent" icon="new" text="nieuw"></nldd-tag>
    </nldd-cell>
    <nldd-spacer-cell v-if="expandable" size="8"></nldd-spacer-cell>
    <nldd-icon-cell v-if="expandable" disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>

    <!-- De wetsversie waaronder besloten is: welke regeling, en vanaf wanneer de
         versie die op dit moment gold in werking was. -->
    <nldd-list-item v-if="expandable && regulation" slot="children" size="sm">
      <nldd-spacer-cell size="20"></nldd-spacer-cell>
      <nldd-icon-cell icon="book" size="16" color="secondary"></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell
        size="sm"
        text="Wetsversie"
        :supporting-text="regulation.validFrom ? `in werking vanaf ${formatMoment(regulation.validFrom)}` : 'geen inwerkingtreding vastgelegd'"
      ></nldd-text-cell>
      <nldd-text-cell
        size="sm"
        width="fit-content"
        max-width="45%"
        horizontal-alignment="right"
        :text="regulation.regulation"
      ></nldd-text-cell>
    </nldd-list-item>

    <!-- Elke waarde in het besluit met haar herkomst: geaccepteerd van een
         andere cel (met lexostatus, moment en ondertekening), uit de eigen
         kroniek, opgave bij de actie, of berekend door de regeling. -->
    <nldd-list-item v-for="field in expandable ? fields : []" :key="field.name" slot="children" size="sm">
      <nldd-spacer-cell size="20"></nldd-spacer-cell>
      <nldd-text-cell size="sm" min-width="120px" :text="humanize(field.name)" :supporting-text="field.detail">
        <nldd-tag slot="overline" size="sm" :color="field.origin.color" :text="field.origin.label"></nldd-tag>
      </nldd-text-cell>
      <nldd-text-cell
        size="sm"
        width="fit-content"
        max-width="45%"
        horizontal-alignment="right"
        :text="formatValue(field.value)"
      ></nldd-text-cell>
    </nldd-list-item>

    <!-- De verplichtingen die uit het besluit volgen. Ze staan in het gram, dus
         ze staan hier: een verplichting is een uitkomst van het besluit en geen
         aparte administratie. -->
    <nldd-list-item
      v-for="(row, index) in expandable ? obligations.rows : []"
      :key="`verplichting-${index}`"
      slot="children"
      size="sm"
    >
      <nldd-spacer-cell size="20"></nldd-spacer-cell>
      <nldd-icon-cell icon="handshake" size="16" color="secondary"></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell size="sm" :text="`Verplichting ${index + 1}`" :supporting-text="obligationText(row)"></nldd-text-cell>
    </nldd-list-item>
  </nldd-list-item>
</template>
