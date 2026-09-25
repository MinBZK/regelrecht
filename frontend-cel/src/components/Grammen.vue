<script setup>
// Grammen als tabel, met de ruwe YAML eronder. Alleen een gram van een event
// met een zaak heeft een zaakkenmerk. Elk gram heeft twee tijden: op_moment,
// wanneer het feit rechtens geldt (bij een aanvraag van het loket de dag van
// ontvangst), en vastgelegd_op, wanneer de cel het vastlegde.
const props = defineProps({
  // [{gram, yaml}], in de volgorde waarin ze getoond worden.
  items: { type: Array, required: true },
  // Het zojuist vastgelegde gram, om het in de tabel aan te wijzen.
  nieuw: { type: Object, default: null },
  leegTekst: { type: String, default: undefined },
});

// Een gram heeft geen eigen id; deze velden wijzen het aan.
const sleutel = (g) => [g.chronicle, g.name, g.op_moment, g.zaakkenmerk ?? ''].join('|');
const isNieuw = (g) => props.nieuw !== null && sleutel(g) === sleutel(props.nieuw);
</script>

<template>
  <nldd-table
    columns="190px 190px minmax(110px,1fr) minmax(150px,1fr) 130px minmax(180px,1fr) 110px"
    accessible-label="Vastgelegde grammen"
    empty-text="Nog niets vastgelegd"
    :empty-supporting-text="leegTekst"
  >
    <nldd-table-row slot="header">
      <nldd-text-cell text="Op moment"></nldd-text-cell>
      <nldd-text-cell text="Vastgelegd op"></nldd-text-cell>
      <nldd-text-cell text="Kroniek"></nldd-text-cell>
      <nldd-text-cell text="Event"></nldd-text-cell>
      <nldd-text-cell text="Type"></nldd-text-cell>
      <nldd-text-cell text="Zaakkenmerk"></nldd-text-cell>
      <nldd-text-cell text="Herkomst"></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="(i, n) in items" :key="n + sleutel(i.gram)" :selected="isNieuw(i.gram) || undefined">
      <nldd-text-cell :text="i.gram.op_moment"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.vastgelegd_op || i.gram.op_moment"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.chronicle"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.name"></nldd-text-cell>
      <nldd-text-cell :text="[i.gram.type, i.gram.soort, i.gram.stage].filter(Boolean).join(' / ')"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.zaakkenmerk ?? 'geen zaak'"></nldd-text-cell>
      <nldd-text-cell :text="i.gram.herkomst ?? 'vastgesteld'"></nldd-text-cell>
    </nldd-table-row>
  </nldd-table>
  <template v-for="(i, n) in items" :key="'yaml-' + n + sleutel(i.gram)">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="5"><h2>{{ i.gram.name }}{{ i.gram.zaakkenmerk ? `, zaak ${i.gram.zaakkenmerk}` : `, ${i.gram.op_moment}` }}</h2></nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-code-viewer language="yaml" wrap>{{ i.yaml }}</nldd-code-viewer>
  </template>
</template>
