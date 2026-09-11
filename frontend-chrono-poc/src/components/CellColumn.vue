<script setup>
import { computed } from 'vue';
import GramRow from './GramRow.vue';
import { chronicles, gramsInTimeOrder, isNewGram } from '../world/snapshot.js';

// Eén kolom per cel: wat deze cel laadt, wat ze publiceert, waarover ze kan
// besluiten, en per kroniek de grammen in tijdsvolgorde.
//
// Een cel zonder wetten is een bron-cel: ze legt vast en reduceert, en besluit
// niet. Dat staat er, omdat het te zien hoort te zijn — het is de toets dat een
// lexostatus geen engine nodig heeft.

const props = defineProps({
  /** De cel uit het beeld. */
  cell: { type: Object, required: true },
  /** Waar de klok staat. */
  clock: { type: String, default: null },
  /** Aantal grammen per kroniek vóór de laatste stap; `null` = niets nieuw. */
  previousCounts: { type: Map, default: null },
});

const streams = computed(() =>
  chronicles(props.cell).map((chronicle) => {
    const entries = gramsInTimeOrder(chronicle);
    return {
      stream: chronicle.stream,
      key: chronicle.key,
      entries,
      // Een kroniek met een besluit erin is een boom: die rijen klappen uit naar
      // de herkomst van elke waarde. Een kroniek van alleen vastleggingen klapt
      // nergens uit en is dus een lijst — een boom waarin niets opengaat zou een
      // belofte doen die geen rij nakomt.
      type: entries.some((entry) => entry.gram.kind === 'decretogram') ? 'tree' : 'list',
    };
  }),
);

const subtitle = computed(() => {
  const laws = props.cell.laws ?? [];
  return laws.length > 0
    ? `${laws.length} ${laws.length === 1 ? 'regeling' : 'regelingen'} geladen`
    : 'bron-cel: legt vast en reduceert, besluit niet';
});

/**
 * De plek van een gram in zijn kroniek, voor de lijn van de tijdlijn-cel.
 *
 * De namen zijn die van `nldd-timeline-track-cell`: `only` is het enige gram in
 * de kroniek en krijgt aan geen van beide kanten een lijn — een spoor van één
 * punt leidt nergens heen.
 */
function position(index, total) {
  if (total === 1) return 'only';
  if (index === 0) return 'first';
  return index === total - 1 ? 'last' : 'between';
}

function isNew(stream, index) {
  return isNewGram(props.previousCounts, props.cell.id, stream, index);
}
</script>

<template>
  <nldd-card :accessible-label="`Cel ${cell.id}`">
    <nldd-container slot="header" layout="stack" gap="8" padding="16" padding-bottom="8">
      <nldd-title size="5">
        <span slot="overline">Cel</span>
        <span>{{ cell.id }}</span>
        <span slot="subtitle">{{ subtitle }}</span>
      </nldd-title>
      <nldd-container layout="wrap" gap="4">
        <nldd-tag v-for="law in cell.laws ?? []" :key="law" size="sm" color="paars" icon="book" :text="law"></nldd-tag>
        <nldd-tag
          v-for="name in cell.lexostatussen ?? []"
          :key="`lexo-${name}`"
          size="sm"
          color="hemelblauw"
          icon="radar"
          :text="name"
        ></nldd-tag>
        <nldd-tag
          v-for="name in cell.besluiten ?? []"
          :key="`besluit-${name}`"
          size="sm"
          color="donkerblauw"
          icon="certificate"
          :text="name"
        ></nldd-tag>
      </nldd-container>
    </nldd-container>

    <nldd-container layout="stack" gap="16" padding="16" padding-top="8">
      <nldd-container v-for="stream in streams" :key="stream.stream" layout="stack" gap="4">
        <nldd-title size="6">
          <span>{{ stream.stream }}</span>
          <span slot="subtitle">sleutel: {{ stream.key }}</span>
        </nldd-title>
        <nldd-list
          :type="stream.type"
          variant="box-base"
          :accessible-label="`Kroniek ${stream.stream} van cel ${cell.id}`"
          empty-text="Nog niets vastgelegd"
          empty-supporting-text="Deze kroniek is leeg tot er iets gebeurt."
        >
          <GramRow
            v-for="(entry, order) in stream.entries"
            :key="`${stream.stream}-${entry.index}`"
            :gram="entry.gram"
            :clock="clock"
            :is-new="isNew(stream.stream, entry.index)"
            :position="position(order, stream.entries.length)"
          />
        </nldd-list>
      </nldd-container>

      <nldd-inline-dialog
        v-if="streams.length === 0"
        icon="database"
        text="Geen kronieken"
        supporting-text="Deze cel houdt geen eigen kroniek bij."
      ></nldd-inline-dialog>
    </nldd-container>
  </nldd-card>
</template>
