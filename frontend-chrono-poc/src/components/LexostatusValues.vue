<script setup>
import { computed } from 'vue';
import { formatValue, humanize } from '../world/format.js';

// Wat een cel vaststelde, als weergave: de losse uitkomsten als lijst, en een
// uitkomst die uit regels bestaat als tabel.
//
// Eén weergave voor twee plekken: het tabblad Lexostatus van "Achter de
// schermen" en de kaarten van inzicht in je aanvraag. Het antwoord is in beide
// gevallen dat van één cel, en hoort er op beide plekken hetzelfde uit te zien.
// "Niets vastgesteld" staat hier niet in: dat is een antwoord met een eigen
// toon, en die kiest de plek die het toont.

const props = defineProps({
  /** Het antwoord zoals `readLexostatus` het uitsplitst, met `established`. */
  answer: { type: Object, required: true },
});

/**
 * Is deze uitkomst een lijst met regels?
 *
 * Een uitkomst mag een lijst van objecten zijn — de openstaandvorm levert er een
 * met de termijnen waar haar bedragen uit bestaan — en zo'n lijst als één regel
 * tekst tonen zou de uitkomst onleesbaar maken. Wat een lijst van iets ánders is
 * (getallen, teksten) blijft een gewone waarde: daar valt geen tabel van te
 * maken, want er zijn geen kolommen.
 *
 * Een lege lijst telt als lijst met regels. Aan een lege lijst is niet te zien
 * wat ze had kunnen dragen, en "er zijn geen regels" is voor elke lege lijst
 * waar — een lege termijnenlijst is juist het antwoord dat er niets vervallen
 * is, en hoort niet als "0 items" tussen de bedragen te verdwijnen.
 */
function isRowList(value) {
  return (
    Array.isArray(value) &&
    value.every((row) => row !== null && typeof row === 'object' && !Array.isArray(row))
  );
}

/** De losse uitkomsten van het antwoord: alles wat geen lijst met regels is. */
const plainValues = computed(() => (props.answer?.values ?? []).filter((value) => !isRowList(value.value)));

/**
 * De uitkomsten die een tabel zijn, met hun kolommen.
 *
 * De kolommen komen uit de regels zelf en niet uit deze app: wat een regel
 * draagt, zegt de cel, en een vaste kolomlijst hier zou een veld verbergen zodra
 * er een bij komt. De volgorde is die waarin de velden binnenkomen — op naam,
 * want het beeld komt uit een geordende map. Een "mooiere" volgorde hier zou een
 * lijstje veldnamen in deze app zetten, en dan weet de app iets van de casus.
 */
const tableValues = computed(() =>
  (props.answer?.values ?? [])
    .filter((value) => isRowList(value.value))
    .map((value) => {
      const columns = [];
      for (const row of value.value) for (const key of Object.keys(row)) if (!columns.includes(key)) columns.push(key);
      return { name: value.name, columns, rows: value.value };
    }),
);
</script>

<template>
  <nldd-container layout="stack" gap="16">
    <nldd-list
      v-if="plainValues.length > 0"
      variant="box-tinted"
      :accessible-label="`Antwoord van cel ${answer.cell}`"
    >
      <nldd-list-item v-for="value in plainValues" :key="value.name" size="sm">
        <nldd-text-cell size="sm" :text="humanize(value.name)"></nldd-text-cell>
        <nldd-text-cell
          size="sm"
          width="fit-content"
          horizontal-alignment="right"
          :text="formatValue(value.value)"
        ></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>

    <!-- Een uitkomst die uit regels bestaat, krijgt een tabel. De openstaandvorm
         levert zo haar termijnen: de bedragen erboven zijn er de optelling van,
         en zonder deze regels is die optelling niet na te lopen. De kolommen
         komen uit de regels zelf — deze app weet niet welke velden een cel
         erin zet. -->
    <template v-for="table in tableValues" :key="table.name">
      <nldd-title size="6">
        <span>{{ humanize(table.name) }}</span>
        <span slot="subtitle">waar de bedragen hierboven uit bestaan</span>
      </nldd-title>
      <nldd-table
        v-if="table.columns.length > 0"
        :columns="`repeat(${table.columns.length}, minmax(96px, 1fr))`"
        :accessible-label="`${humanize(table.name)} van cel ${answer.cell}`"
      >
        <nldd-table-row slot="header">
          <nldd-text-cell
            v-for="column in table.columns"
            :key="column"
            size="sm"
            :text="humanize(column)"
          ></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="(row, position) in table.rows" :key="position">
          <nldd-text-cell
            v-for="column in table.columns"
            :key="column"
            size="sm"
            :text="formatValue(row[column])"
          ></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <!-- Een lege lijst is een antwoord: er was op dit moment niets om te
           tonen. Een tabel zonder kolommen zou beweren dat er velden zijn. -->
      <nldd-inline-dialog
        v-else
        icon="info"
        text="Geen regels"
        :supporting-text="`Op dit moment draagt '${table.name}' geen enkele regel.`"
      ></nldd-inline-dialog>
    </template>
  </nldd-container>
</template>
