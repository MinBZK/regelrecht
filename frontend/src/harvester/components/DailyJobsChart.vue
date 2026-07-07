<script setup>
/**
 * One daily-jobs chart (stacked succeeded/failed bars + added line) for a
 * single job type. Data comes in as plain entries; colors are resolved from
 * the NLDD tokens at mount and re-resolved when the color scheme flips
 * (data-scheme attribute for the explicit picker, the media query for
 * 'auto') — the tokens are light-dark() pairs the canvas can't track itself.
 */
import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { BarChart, LineChart } from 'echarts/charts';
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
} from 'echarts/components';
import VChart from 'vue-echarts';
import { buildDailyJobsOption } from '../charts/dailyJobsChart.js';
import { resolveChartColors } from '../charts/chartColors.js';

use([
  CanvasRenderer,
  BarChart,
  LineChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
]);

const props = defineProps({
  title: { type: String, required: true },
  entries: { type: Array, required: true },
});

const chartRoot = ref(null);
const colors = shallowRef(null);

function refreshColors() {
  colors.value = resolveChartColors(chartRoot.value ?? document.body);
}

let schemeObserver = null;
let darkMedia = null;
onMounted(() => {
  refreshColors();
  schemeObserver = new MutationObserver(refreshColors);
  schemeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-scheme'],
  });
  darkMedia = window.matchMedia?.('(prefers-color-scheme: dark)');
  darkMedia?.addEventListener?.('change', refreshColors);
});
onUnmounted(() => {
  schemeObserver?.disconnect();
  darkMedia?.removeEventListener?.('change', refreshColors);
});

const option = computed(() =>
  colors.value ? buildDailyJobsOption(props.entries, colors.value) : null,
);
</script>

<template>
  <nldd-card>
    <nldd-container padding="16">
      <nldd-title size="6">
        <h3>{{ title }}</h3>
      </nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <div ref="chartRoot" class="daily-jobs-chart">
        <v-chart v-if="option" :option="option" autoresize />
      </div>
    </nldd-container>
  </nldd-card>
</template>
