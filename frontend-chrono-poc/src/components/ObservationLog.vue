<script setup>
import { computed } from 'vue';
import { formatMoment, formatValue } from '../world/format.js';
import { crossings, readLexostatus } from '../world/snapshot.js';

// Het observatielog: elk contact dat over een celgrens ging.
//
// Dit is een **meetinstrument van de testopstelling**, en het staat er met dat
// label. Geen enkele cel heeft dit overzicht — de wereld houdt de unie bij van
// wat over de grenzen ging, en juist daarom hoort een lezer het niet voor een
// onderdeel van de opstelling te houden. In een echte deployment bestaat het niet.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
});

const rows = computed(() =>
  crossings(props.snapshot).map((crossing, index) => {
    const answer = readLexostatus(crossing.answer);
    return {
      id: index,
      askedBy: crossing.asked_by,
      signature: crossing.signature,
      params: Object.entries(crossing.params ?? {})
        .map(([name, value]) => `${name}: ${formatValue(value)}`)
        .join(' · '),
      cell: answer.cell,
      name: answer.name,
      opMoment: answer.opMoment,
      answer: answer.established
        ? answer.values.map((value) => `${value.name}: ${formatValue(value.value)}`).join(' · ')
        : 'niets vastgesteld',
      established: answer.established,
    };
  }),
);
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-banner
      variant="neutral"
      icon="binoculars"
      text="Meetinstrument van de testopstelling"
      supporting-text="Dit log staat naast de opstelling en niet erin: het bestaat in een echte deployment niet. Geen enkele cel kan dit overzicht opvragen."
    ></nldd-banner>

    <nldd-table
      columns="minmax(180px, 1fr) 120px minmax(200px, 1fr) minmax(200px, 1fr)"
      accessible-label="Contacten over een celgrens"
      empty-text="Nog geen contact over een celgrens"
      empty-supporting-text="Zolang geen cel iets bij een ander opvraagt, blijft dit log leeg."
    >
      <nldd-table-row slot="header">
        <nldd-text-cell size="sm" text="Bevraagde cel"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Moment"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Vraag"></nldd-text-cell>
        <nldd-text-cell size="sm" text="Antwoord"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="row in rows" :key="row.id">
        <nldd-text-cell size="sm" :text="row.cell" :supporting-text="`lexostatus: ${row.name}`"></nldd-text-cell>
        <nldd-text-cell size="sm" :text="formatMoment(row.opMoment)"></nldd-text-cell>
        <nldd-text-cell
          size="sm"
          :text="row.askedBy"
          :supporting-text="[row.params, row.signature].filter(Boolean).join(' · ')"
        ></nldd-text-cell>
        <nldd-text-cell size="sm" :color="row.established ? 'default' : 'secondary'" :text="row.answer"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </nldd-container>
</template>
