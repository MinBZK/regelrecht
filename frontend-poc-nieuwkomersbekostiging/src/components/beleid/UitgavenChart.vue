<template>
  <div class="uitgaven-chart">
    <div class="uc-head">
      <span class="uc-title">{{ title }}</span>
      <span class="uc-sub">{{ subtitle }}</span>
    </div>
    <v-chart v-if="option" class="chart" :option="option" autoresize />
    <p v-else class="uc-empty">Nog geen uitkomsten.</p>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { BarChart } from 'echarts/charts';
import { GridComponent, TooltipComponent, LegendComponent } from 'echarts/components';
import VChart from 'vue-echarts';
import { euroCompact } from '../../lib/format.js';
import { useThema } from '../../composables/useThema.js';

use([CanvasRenderer, BarChart, GridComponent, TooltipComponent, LegendComponent]);

function kortLabel(col) {
  if (!col.variantId) return 'huidig recht';
  const m = String(col.variantId).match(/^([a-z]+-\d+)/i);
  return m ? m[1] : String(col.variantId).slice(0, 8);
}

const props = defineProps({
  title: { type: String, required: true },
  subtitle: { type: String, default: '' },
  columns: { type: Array, required: true },
  metricsByColumn: { type: Object, required: true },
  jaren: { type: Array, required: true },
  /** [{ key, label, pick(jaarMetrics) -> eurocent }] in vaste volgorde */
  posten: { type: Array, required: true },
});

const { thema } = useThema();

// Kleuren komen uit de categorie-tokens van het design system (light-dark),
// opgelost via een proefelement; de hexwaarden zijn alleen terugval.
const PALETTE_LIGHT = ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4'];
const PALETTE_DARK = ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181'];
const KLEUR_TOKENS = ['lintblauw', 'oranje', 'groen', 'geel', 'roze'].map(
  (k) => `--semantics-categories-${k}-filled-background-color`,
);

function resolveToken(token, fallback) {
  if (typeof document === 'undefined') return fallback;
  const el = document.createElement('span');
  el.style.cssText = `position:absolute;visibility:hidden;color:var(${token})`;
  document.body.appendChild(el);
  const kleur = getComputedStyle(el).color;
  el.remove();
  return kleur && kleur !== 'rgba(0, 0, 0, 0)' ? kleur : fallback;
}

function isDark() {
  if (thema.value === 'donker') return true;
  if (thema.value === 'licht') return false;
  return typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches;
}

const option = computed(() => {
  const cols = props.columns.filter((c) => props.metricsByColumn[c.key]);
  if (!cols.length) return null;
  const dark = isDark();
  const terugval = dark ? PALETTE_DARK : PALETTE_LIGHT;
  const palette = KLEUR_TOKENS.map((t, i) => resolveToken(t, terugval[i]));
  const ink = resolveToken('--semantics-content-secondary-color', dark ? '#c3c2b7' : '#52514e');
  const grid = resolveToken('--semantics-dividers-color', dark ? '#33322f' : '#e6e5e1');
  const rand = resolveToken('--semantics-surfaces-base-background-color', dark ? '#1a1a19' : '#fcfcfb');

  // Jaren op de as; per kolom een eigen stapel (stack = kolomsleutel), zodat
  // ook vier kolommen leesbaar blijven. De kolomsleutel staat als label boven
  // de staaf; de volledige titel staat in de tabelkop en in de tooltip.
  const categories = props.jaren.map(String);
  const series = [];
  const seriesCol = [];
  for (const col of cols) {
    props.posten.forEach((post, i) => {
      const last = i === props.posten.length - 1;
      seriesCol.push(col);
      series.push({
        name: post.label,
        type: 'bar',
        stack: col.key,
        barMaxWidth: 28,
        barGap: '15%',
        itemStyle: { color: palette[i % palette.length], borderColor: rand, borderWidth: 1 },
        emphasis: { focus: 'series' },
        label: { show: false },
        data: props.jaren.map((jaar, j) => {
          const m = props.metricsByColumn[col.key];
          const v = m ? post.pick(m.perJaar[jaar]) : 0;
          const value = Math.round((v ?? 0) / 100); // euro's
          // Kolomsleutel alleen boven het eerste jaar, rechtop: de volgorde
          // van de stapels is in elk jaar dezelfde.
          if (last && cols.length > 1 && j === 0) {
            return { value, label: { show: true, position: 'top', rotate: 90, align: 'left', verticalAlign: 'middle', formatter: () => kortLabel(col), fontSize: 9, color: ink, distance: 4 } };
          }
          return value;
        }),
      });
    });
  }
  const tooltipFormatter = (params) => {
    const lines = [`<b>${params[0]?.axisValue ?? ''}</b>`];
    let vorige = null;
    for (const p of params) {
      const col = seriesCol[p.seriesIndex];
      if (col !== vorige) { lines.push(`<span style="opacity:.75">${kortLabel(col)}</span>`); vorige = col; }
      lines.push(`${p.marker} ${p.seriesName}: ${euroCompact(Number(p.value) * 100)}`);
    }
    return lines.join('<br/>');
  };

  return {
    color: palette,
    textStyle: { color: ink },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
      formatter: tooltipFormatter,
    },
    legend: { bottom: 0, textStyle: { color: ink }, icon: 'roundRect' },
    grid: { left: 64, right: 16, top: 24, bottom: 80 },
    xAxis: {
      type: 'category',
      data: categories,
      axisLine: { lineStyle: { color: grid } },
      axisTick: { show: false },
      axisLabel: { color: ink, interval: 0, fontSize: 11, lineHeight: 14 },
    },
    yAxis: {
      type: 'value',
      axisLabel: { color: ink, formatter: (v) => euroCompact(v * 100) },
      splitLine: { lineStyle: { color: grid } },
    },
    series,
  };
});
</script>

<style scoped>
.uitgaven-chart { width: 100%; display: flex; flex-direction: column; gap: var(--primitives-space-4, 4px); }
.uc-head { display: flex; align-items: baseline; gap: var(--primitives-space-8); flex-wrap: wrap; }
.uc-title { font-weight: 600; font-size: 0.95em; }
.uc-sub { font-size: 0.8em; color: var(--semantics-content-secondary-color); }
.chart { height: 280px; width: 100%; }
.uc-empty { color: var(--semantics-content-secondary-color); font-size: 0.9em; margin: 0; }
</style>
