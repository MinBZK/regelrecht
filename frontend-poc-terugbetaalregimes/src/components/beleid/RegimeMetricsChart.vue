<template>
  <div class="regime-metrics-chart">
    <v-chart v-if="option" class="chart" :option="option" autoresize />
    <p v-if="option" class="rmc-legenda">
      De staven zijn <strong>terugbetaalregimes</strong>: groepen debiteuren, bepaald door hun cohort.
      <template v-if="series.length > 1">
        Elke kleur is een <strong>kolom</strong>: huidig recht naast de varianten die je links hebt aangevinkt, dezelfde
        kolommen als in de tabel hierboven. Een variant is dus geen staaf, maar verandert de hoogte van de staven. Twee
        kolommen kunnen even hoog uitkomen: een variant die alleen de aflosfase of de kwijtschelding raakt, verandert
        niet wie boven zijn afloscapaciteit uitkomt. Kijk dan naar de tabel.
      </template>
      <template v-else>
        Vink links varianten aan om ze als extra kleur naast huidig recht te zetten.
      </template>
    </p>
    <p v-else class="rmc-empty">Nog geen populatie doorgerekend.</p>
  </div>
</template>

<!--
  Aandeel met betalingsproblemen per terugbetaalregime, één serie per kolom.

  Dezelfde kolommen als de postentabel erboven: huidig recht plus de
  aangevinkte varianten. De regimes staan op de x-as, dus de kleur onderscheidt
  hier de KOLOM en niet het regime; dat is de dimensie die je anders niet uit de
  grafiek kunt aflezen. Huidig recht houdt bewust de neutrale grijstint (het is
  de vergelijkingsbasis), de varianten krijgen de rijkskleuren in kolomvolgorde,
  dezelfde volgorde als in de tabel.
-->

<script setup>
import { computed } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { BarChart } from 'echarts/charts';
import { GridComponent, TooltipComponent, LegendComponent } from 'echarts/components';
import VChart from 'vue-echarts';
import { regimeLabel } from '../../lib/format.js';
import { resolveToken, categorieKleur, useDonker } from '../../lib/tokens.js';
import { KOLOM_BASIS } from '../../composables/usePopulation.js';

use([CanvasRenderer, BarChart, GridComponent, TooltipComponent, LegendComponent]);

const props = defineProps({
  columns: { type: Array, default: () => [] },
  metricsByColumn: { type: Object, default: () => ({}) },
});

const donker = useDonker();

const REGIME_ORDER = ['SF15_OUD', 'SF15_NIEUW', 'SF15_LLLK', 'SF35'];

/** Kleuren voor de variantkolommen, in kolomvolgorde; maximaal drie varianten. */
const KOLOM_KLEUREN = [
  ['lintblauw', '#154273'],
  ['oranje', '#d9730d'],
  ['paars', '#8b5cf6'],
];

/** De kolommen die al een doorgerekende uitkomst hebben. */
const gevuldeKolommen = computed(() => props.columns.filter((c) => props.metricsByColumn[c.key]));

/** Eén serie per kolom: de vier regimes als datapunten. */
const series = computed(() => gevuldeKolommen.value.map((col) => ({
  key: col.key,
  naam: kolomNaam(col),
  data: REGIME_ORDER.map((r) => {
    const m = props.metricsByColumn[col.key].regimes?.[r];
    return m ? Math.round(m.pctBetalingsprobleem * 1000) / 10 : 0;
  }),
})));

/** In de legenda: de kolomtitel, met de werkversie herkenbaar gemarkeerd. */
function kolomNaam(col) {
  return col.isWerkversie && col.key !== KOLOM_BASIS ? `${col.titel} (werkversie)` : col.titel;
}

const option = computed(() => {
  if (!series.value.length) return null;
  donker.value; // kleuren opnieuw oplossen bij een themawissel
  const neutraal = resolveToken('--semantics-content-secondary-color', '#9aa5b1');
  const labels = REGIME_ORDER.map(regimeLabel);

  let variantIndex = 0;
  const echartsSeries = series.value.map((s) => {
    // Huidig recht is de basis en blijft neutraal grijs; elke variant krijgt de
    // volgende rijkskleur, zodat de kleuren niet verspringen als je er een
    // bijkiest.
    let kleur = neutraal;
    if (s.key !== KOLOM_BASIS) {
      const [naam, fallback] = KOLOM_KLEUREN[variantIndex % KOLOM_KLEUREN.length];
      kleur = categorieKleur(naam, fallback);
      variantIndex += 1;
    }
    return { name: s.naam, type: 'bar', data: s.data, itemStyle: { color: kleur } };
  });

  const meerdere = echartsSeries.length > 1;
  return {
    tooltip: { trigger: 'axis', valueFormatter: (v) => `${v}%` },
    // Vier kolomnamen passen niet altijd op één regel; scroll houdt de legenda
    // op één regel in plaats van in de grafiek te lopen.
    legend: meerdere ? { bottom: 0, type: 'scroll' } : undefined,
    grid: { left: 40, right: 16, top: 16, bottom: meerdere ? 44 : 24 },
    xAxis: {
      type: 'category',
      data: labels,
      name: 'terugbetaalregime',
      nameLocation: 'middle',
      nameGap: 28,
      nameTextStyle: { color: neutraal },
    },
    yAxis: { type: 'value', name: '%', axisLabel: { formatter: '{value}%' } },
    series: echartsSeries,
  };
});
</script>

<style scoped>
.regime-metrics-chart { width: 100%; }
.chart { height: 260px; width: 100%; }
.rmc-empty { color: var(--semantics-content-secondary-color); font-size: 0.9em; }
.rmc-legenda { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
</style>
