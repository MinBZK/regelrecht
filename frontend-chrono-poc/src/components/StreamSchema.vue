<script setup>
import { computed, ref } from 'vue';
import { humanize } from '../world/format.js';

// Wat een kroniekstroom draagt, vóórdat er één gram in ligt: per gebeurtenis de
// velden met hun type, het kanaal waarlangs ze binnenkomt en de grondslag
// waarop ze vastgelegd wordt.
//
// Dit staat hier en niet als kolomkoppen boven de grammen, om de reden dat het
// schema er ook is als de kroniek leeg is — en dat is bij een betalingsstroom
// het normale beginbeeld. Wie de kolommen uit het eerste gram zou aflezen, ziet
// van een lege stroom niets en van een stroom met één gram alleen wat dát gram
// toevallig droeg.
//
// Een stroom zonder schema toont hier niets. Dat is geen gat in de weergave maar
// de stand van zaken: die stroom belooft niets over wat erin komt.

const props = defineProps({
  /** De kroniek uit het beeld. */
  chronicle: { type: Object, required: true },
  /** Het cel-id, voor de toegankelijke naam van de lijst. */
  cell: { type: String, required: true },
});

/** Welke gebeurtenis open staat; leeg is alles dicht. */
const open = ref('');

const rows = computed(() =>
  (props.chronicle.gebeurtenissen ?? []).map((gebeurtenis) => ({
    name: gebeurtenis.name,
    fields: gebeurtenis.fields ?? [],
    // Het kanaal en de grondslag horen bij de gebeurtenis en niet bij één gram:
    // dát een betaling op dit artikel berust, geldt voor elke betaling.
    summary: [
      gebeurtenis.intake,
      `${(gebeurtenis.fields ?? []).length} ${(gebeurtenis.fields ?? []).length === 1 ? 'veld' : 'velden'}`,
      gebeurtenis.grondslag,
    ]
      .filter(Boolean)
      .join(' · '),
  })),
);

function toggle(name) {
  open.value = open.value === name ? '' : name;
}
</script>

<template>
  <nldd-list
    v-if="rows.length > 0"
    type="tree"
    variant="box-base"
    :accessible-label="`Gebeurtenisschema van kroniek ${chronicle.stream} van cel ${cell}`"
  >
    <nldd-list-item
      v-for="row in rows"
      :key="`gebeurtenis-${row.name}`"
      size="sm"
      button
      :expanded="open === row.name"
      @click="toggle(row.name)"
    >
      <nldd-icon-cell icon="list" size="16" color="secondary"></nldd-icon-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-text-cell size="sm" min-width="120px" :text="row.name" :supporting-text="row.summary"></nldd-text-cell>
      <nldd-cell>
        <nldd-tag size="sm" color="grijs" icon="list" text="schema"></nldd-tag>
      </nldd-cell>
      <nldd-spacer-cell size="8"></nldd-spacer-cell>
      <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>

      <nldd-list-item v-if="open === row.name" slot="children" size="sm">
        <nldd-spacer-cell size="20"></nldd-spacer-cell>
        <nldd-cell width="full" vertical-alignment="top">
          <nldd-table
            columns="minmax(160px, 1fr) fit-content(140px)"
            background="base"
            :accessible-label="`Velden van gebeurtenis ${row.name}`"
          >
            <nldd-inline-dialog slot="empty" text="Deze gebeurtenis declareert geen velden"></nldd-inline-dialog>
            <nldd-table-row slot="header">
              <nldd-text-cell size="sm" text="Veld"></nldd-text-cell>
              <nldd-text-cell size="sm" text="Type"></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="field in row.fields" :key="`${row.name}-${field.name}`">
              <nldd-text-cell size="sm" vertical-alignment="top" :text="humanize(field.name)"></nldd-text-cell>
              <nldd-text-cell size="sm" vertical-alignment="top" :text="field.type"></nldd-text-cell>
            </nldd-table-row>
          </nldd-table>
        </nldd-cell>
      </nldd-list-item>
    </nldd-list-item>
  </nldd-list>
</template>
