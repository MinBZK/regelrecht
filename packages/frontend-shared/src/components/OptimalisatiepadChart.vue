<template>
  <div class="opt-pad">
    <span class="op-title">{{ titel }}</span>
    <v-chart class="chart" :option="option" autoresize @click="opPunt" />
    <p class="op-hint">{{ hint }}</p>
  </div>
</template>

<!--
  Het pad dat de beleidsassistent aflegt naar een doel: één punt per meting die
  hij onderweg doet.

  Waarom dit gedeeld is en niet per casus: de vorm is dezelfde (meten, bijstellen,
  opnieuw meten), alleen de grootheid verschilt. Terugbetaalregimes meet een
  percentage debiteuren met betalingsproblemen; nieuwkomersbekostiging meet
  bedragen, en daar staan er drie naast elkaar (po, vo, uitvoeringslast) omdat
  een doel als "gelijktrekken zonder dat de uitgave stijgt" over de verhouding
  tussen die posten gaat. Vandaar reeksen met een eigen eenheid in plaats van
  een vaste procent-as.

  De punten zijn aanklikbaar: tijdens een lopende optimalisatie kun je een
  tussenstand pakken die je bevalt, zonder te wachten tot hij convergeert.
-->

<script setup>
import { computed } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { LineChart } from 'echarts/charts';
import { GridComponent, TooltipComponent, LegendComponent } from 'echarts/components';
import VChart from 'vue-echarts';

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent, LegendComponent]);

const props = defineProps({
  /**
   * De metingen, oplopend: [{ iteratie, waarden: { <reeks-sleutel>: number } }].
   * Een reeks die in een meting ontbreekt, krijgt een gat in plaats van een nul:
   * niet gemeten is iets anders dan nul.
   */
  pad: { type: Array, default: () => [] },
  /**
   * Wat er getekend wordt: [{ sleutel, label, kleur, as?, formatter? }].
   * `as` is 0 (links) of 1 (rechts), voor twee grootheden naast elkaar.
   */
  reeksen: { type: Array, required: true },
  /** Titels van de assen, per index. */
  asTitels: { type: Array, default: () => [] },
  /** Formatters per as: (waarde) => string. */
  asFormatters: { type: Array, default: () => [] },
  titel: { type: String, default: 'Optimalisatiepad' },
  hint: { type: String, default: 'Klik op een punt om die tussenstand te bekijken.' },
  /** Index van het punt dat als gekozen wordt getoond, of null. */
  gekozen: { type: Number, default: null },
});

const emit = defineEmits(['kies']);

/** Welke iteratie hoort bij de aangeklikte punt-index. */
function opPunt(e) {
  if (typeof e?.dataIndex !== 'number') return;
  emit('kies', e.dataIndex);
}

const heeftTweedeAs = computed(() => props.reeksen.some((r) => r.as === 1));

function formatteer(reeks, waarde) {
  if (waarde === null || waarde === undefined) return '—';
  return reeks.formatter ? reeks.formatter(waarde) : String(waarde);
}

const option = computed(() => ({
  tooltip: {
    trigger: 'axis',
    formatter: (params) => {
      const kop = `meting ${params[0]?.axisValue ?? ''}`;
      const regels = params.map((p) => {
        const reeks = props.reeksen[p.seriesIndex] ?? {};
        return `${p.marker} ${p.seriesName}: ${formatteer(reeks, p.value)}`;
      });
      return [kop, ...regels].join('<br>');
    },
  },
  legend: props.reeksen.length > 1
    ? { top: 0, data: props.reeksen.map((r) => r.label), textStyle: { fontSize: 11 } }
    : undefined,
  grid: {
    left: 52,
    right: heeftTweedeAs.value ? 52 : 16,
    top: props.reeksen.length > 1 ? 28 : 16,
    bottom: 32,
  },
  xAxis: {
    type: 'category',
    data: props.pad.map((p) => p.iteratie),
    name: 'meting',
    nameLocation: 'end',
  },
  yAxis: [0, 1].filter((i) => i === 0 || heeftTweedeAs.value).map((i) => ({
    type: 'value',
    name: props.asTitels[i] ?? '',
    axisLabel: {
      formatter: props.asFormatters[i] ? (v) => props.asFormatters[i](v) : undefined,
    },
    // Niet vanaf nul: een doel-run beweegt vaak in een smalle band, en dan
    // is een as vanaf nul een vlakke lijn waar niets aan te zien is.
    scale: true,
  })),
  series: props.reeksen.map((reeks) => ({
    name: reeks.label,
    type: 'line',
    smooth: true,
    showSymbol: true,
    symbolSize: 8,
    yAxisIndex: heeftTweedeAs.value ? (reeks.as ?? 0) : 0,
    lineStyle: { width: 2 },
    color: reeks.kleur,
    data: props.pad.map((p) => p.waarden?.[reeks.sleutel] ?? null),
    // Het gekozen punt groter, zodat zichtbaar is welke tussenstand je bekijkt.
    markPoint: props.gekozen === null ? undefined : {
      symbol: 'circle',
      symbolSize: 14,
      itemStyle: { color: reeks.kleur, opacity: 0.35 },
      label: { show: false },
      data: [{ coord: [props.gekozen, props.pad[props.gekozen]?.waarden?.[reeks.sleutel] ?? null] }],
    },
  })),
}));
</script>

<style scoped>
.opt-pad { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.op-title { font-size: 0.85em; font-weight: 600; color: var(--semantics-content-secondary-color); }
.op-hint { margin: 0; font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.chart { height: 220px; width: 100%; cursor: pointer; }
</style>
