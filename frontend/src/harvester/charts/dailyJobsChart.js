/**
 * Option-builder for the daily harvest/enrich jobs charts (ECharts).
 *
 * Pure functions so the mapping is unit-testable without a canvas: the Vue
 * wrapper (DailyJobsChart.vue) resolves the design-token colors in the
 * browser and injects them here.
 */

export const SERIES_LABELS = {
  added: 'Toegevoegd',
  succeeded: 'Geslaagd',
  failed: 'Gefaald',
};

const AXIS_DATE_FORMAT = new Intl.DateTimeFormat('nl-NL', {
  day: 'numeric',
  month: 'short',
});

// 'YYYY-MM-DD' → short Dutch axis label, e.g. '6 jul'.
export function formatAxisDate(isoDate) {
  const date = new Date(`${isoDate}T00:00:00`);
  return Number.isNaN(date.getTime()) ? isoDate : AXIS_DATE_FORMAT.format(date);
}

/**
 * @param {Array<{date: string, added: number, succeeded: number, failed: number}>} entries
 *   One entry per day, ascending (as served by dashboard-stats `daily`).
 * @param {{succeeded: string, failed: string, added: string, text: string,
 *          textSecondary: string, grid: string, surface: string}} colors
 *   Resolved concrete colors (rgb/hex — canvas can't read CSS variables).
 */
export function buildDailyJobsOption(entries, colors) {
  // Succeeded under, failed on top: the stack totals "finished that day" and
  // the red cap is immediately visible. Added is a separate event (created vs
  // finished), so it rides on top as a line instead of stacking in.
  const bar = (key) => ({
    name: SERIES_LABELS[key],
    type: 'bar',
    stack: 'afgerond',
    data: entries.map((e) => e[key]),
    color: colors[key],
    barMaxWidth: 20,
    // 1px surface-colored border keeps stacked segments visually separated.
    itemStyle: { borderColor: colors.surface, borderWidth: 1 },
  });

  return {
    animation: false,
    grid: { left: 8, right: 8, top: 36, bottom: 8, containLabel: true },
    legend: {
      top: 0,
      left: 0,
      icon: 'roundRect',
      itemWidth: 10,
      itemHeight: 10,
      textStyle: { color: colors.text, fontSize: 12 },
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
    },
    xAxis: {
      type: 'category',
      data: entries.map((e) => formatAxisDate(e.date)),
      axisTick: { show: false },
      axisLine: { lineStyle: { color: colors.grid } },
      axisLabel: { color: colors.textSecondary, fontSize: 11 },
    },
    yAxis: {
      type: 'value',
      minInterval: 1,
      splitLine: { lineStyle: { color: colors.grid } },
      axisLabel: { color: colors.textSecondary, fontSize: 11 },
    },
    series: [
      bar('succeeded'),
      bar('failed'),
      {
        name: SERIES_LABELS.added,
        type: 'line',
        data: entries.map((e) => e.added),
        color: colors.added,
        lineStyle: { width: 2 },
        symbol: 'circle',
        symbolSize: 6,
      },
    ],
  };
}
