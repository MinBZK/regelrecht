<template>
  <div class="timeline-chart">
    <v-chart v-if="option" class="chart" :option="option" autoresize :theme="dark ? 'dark' : undefined" />
    <p v-else class="tc-empty">Geen tijdlijn beschikbaar.</p>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { LineChart, BarChart } from 'echarts/charts';
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
  MarkAreaComponent,
} from 'echarts/components';
import VChart from 'vue-echarts';
import { euro } from '../../lib/format.js';
import { categorieKleur, useDonker } from '../../lib/tokens.js';

use([
  CanvasRenderer,
  LineChart,
  BarChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  MarkAreaComponent,
]);

const props = defineProps({
  timeline: { type: Array, default: () => [] },
});

const dark = useDonker();

// Perioden waarin jokerjaren zijn ingezet, als gearceerde markzones.
const jokerMarks = computed(() => {
  const marks = [];
  for (const t of props.timeline) {
    if (t.jokermaanden > 0) {
      marks.push([{ xAxis: String(t.jaar) }, { xAxis: String(t.jaar) }]);
    }
  }
  return marks;
});

const option = computed(() => {
  if (!props.timeline.length) return null;
  dark.value; // kleuren opnieuw oplossen bij een themawissel
  const kleurRestschuld = categorieKleur('lintblauw', '#154273');
  const kleurMaandbedrag = categorieKleur('oranje', '#d9730d');
  const kleurNibud = categorieKleur('groen', '#3f8f2e');
  const kleurJoker = categorieKleur('paars', 'rgba(139, 92, 246, 0.15)', 'tinted');
  const jaren = props.timeline.map((t) => String(t.jaar));
  const restschuld = props.timeline.map((t) => Math.round(t.restschuld) / 100);
  const maandbedrag = props.timeline.map((t) => Math.round(t.maandbedrag) / 100);
  const afloscap = props.timeline.map((t) =>
    t.afloscapaciteit === null ? null : Math.round(t.afloscapaciteit) / 100,
  );

  return {
    tooltip: {
      trigger: 'axis',
      valueFormatter: (v) => (v === null || v === undefined ? '—' : euro(Math.round(v * 100))),
    },
    legend: { data: ['Restschuld', 'Maandbedrag', 'Nibud-marge'], bottom: 0 },
    grid: { left: 70, right: 70, top: 24, bottom: 48 },
    xAxis: { type: 'category', data: jaren, name: 'jaar', nameLocation: 'end' },
    yAxis: [
      {
        type: 'value',
        name: 'restschuld (€)',
        position: 'left',
        axisLabel: { formatter: (v) => (v >= 1000 ? `${Math.round(v / 1000)}k` : v) },
      },
      {
        type: 'value',
        name: 'per maand (€)',
        position: 'right',
        splitLine: { show: false },
      },
    ],
    series: [
      {
        name: 'Restschuld',
        type: 'line',
        yAxisIndex: 0,
        areaStyle: { opacity: 0.15 },
        smooth: true,
        showSymbol: false,
        lineStyle: { width: 2 },
        color: kleurRestschuld,
        data: restschuld,
        markArea: jokerMarks.value.length
          ? {
              itemStyle: { color: kleurJoker, opacity: 0.6 },
              label: { show: true, position: 'top', formatter: 'joker', fontSize: 10 },
              data: jokerMarks.value,
            }
          : undefined,
      },
      {
        name: 'Maandbedrag',
        type: 'line',
        yAxisIndex: 1,
        smooth: true,
        showSymbol: false,
        lineStyle: { width: 2 },
        color: kleurMaandbedrag,
        data: maandbedrag,
      },
      {
        name: 'Nibud-marge',
        type: 'line',
        yAxisIndex: 1,
        smooth: true,
        showSymbol: false,
        lineStyle: { width: 1, type: 'dashed' },
        color: kleurNibud,
        data: afloscap,
      },
    ],
  };
});
</script>

<style scoped>
.timeline-chart { width: 100%; }
.chart { height: 320px; width: 100%; }
.tc-empty { color: var(--semantics-content-secondary-color); }
</style>
