<template>
  <div class="opt-pad">
    <span class="op-title">Optimalisatiepad</span>
    <v-chart class="chart" :option="option" autoresize />
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { LineChart } from 'echarts/charts';
import { GridComponent, TooltipComponent } from 'echarts/components';
import VChart from 'vue-echarts';
import { resolveToken } from '../../lib/tokens.js';

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent]);

const props = defineProps({
  // [{iteratie, pct}] — pct als ratio (0..1)
  pad: { type: Array, default: () => [] },
});

const option = computed(() => ({
  tooltip: { trigger: 'axis', valueFormatter: (v) => `${v}%` },
  grid: { left: 40, right: 16, top: 16, bottom: 32 },
  xAxis: {
    type: 'category',
    data: props.pad.map((p) => p.iteratie),
    name: 'iteratie',
    nameLocation: 'end',
  },
  yAxis: { type: 'value', name: '%', axisLabel: { formatter: '{value}%' }, min: 0 },
  series: [
    {
      type: 'line',
      smooth: true,
      showSymbol: true,
      lineStyle: { width: 2 },
      color: resolveToken('--semantics-content-accent-color', '#154273'),
      data: props.pad.map((p) => Math.round(p.pct * 1000) / 10),
    },
  ],
}));
</script>

<style scoped>
.opt-pad { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.op-title { font-size: 0.85em; font-weight: 600; color: var(--semantics-content-secondary-color); }
.chart { height: 200px; width: 100%; }
</style>
