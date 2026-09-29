<script setup>
import { renderStep } from '../data/gherkinNl.js';
import { displayCell, emphasiseArguments, hasHeaderRow, isExpectation } from '../data/scenarioSteps.js';
import { useI18n } from '../i18n/index.js';

// The steps of a scenario (or of the background) as Gherkin: in order, keyword
// first, data tables as tables. It shows what the file says and nothing it
// would take the engine to know; the only marks from a run are pass/fail on
// the expectations and the error under the step that failed.
const props = defineProps({
  steps: { type: Array, required: true },
  // Run status per step, aligned with `steps`: `{ status, error }` or nothing.
  results: { type: Array, default: () => [] },
});

const { t } = useI18n();

const result = (index) => props.results[index] ?? null;

function mark(step, index) {
  const status = result(index)?.status;
  if (!isExpectation(step) || (status !== 'pass' && status !== 'fail')) return null;
  return status === 'pass' ? { icon: 'check-mark-circle', color: 'success' } : { icon: 'dismiss-circle', color: 'critical' };
}

function supporting(step, index) {
  const r = result(index);
  if (r?.error) return r.error;
  // The icon is decorative to a screen reader; a passed expectation says so in words.
  if (r?.status === 'pass' && isExpectation(step)) return t('scenario.passed');
  return renderStep(step).matched ? undefined : t('scenario.error.unknown_step');
}

// One track per column, sized to its content, plus a trailing filler track so
// the row dividers run to the edge of the box. The table scrolls sideways on
// its own when it is wider than the card.
function columns(table) {
  const width = Math.max(...table.map((row) => row.length));
  return `repeat(${width}, max-content) 1fr`;
}
</script>

<template>
  <nldd-container gap="8">
    <template v-for="(step, i) in steps" :key="i">
      <!-- Mark column, keyword column, step text. The keyword column has a
           fixed width so the step texts line up the way a feature file does. -->
      <nldd-container layout="row" gap="8" vertical-alignment="top">
        <nldd-icon-cell v-if="mark(step, i)" :icon="mark(step, i).icon" :color="mark(step, i).color" size="16" vertical-alignment="top"></nldd-icon-cell>
        <nldd-spacer-cell v-else size="16"></nldd-spacer-cell>
        <nldd-text-cell width="64px" vertical-alignment="top" color="accent" :text="`**${renderStep(step).keyword}**`"></nldd-text-cell>
        <nldd-text-cell vertical-alignment="top" :text="emphasiseArguments(renderStep(step).text)" :supporting-text="supporting(step, i)"></nldd-text-cell>
      </nldd-container>
      <!-- Indented to the step text: 16 mark + 8 gap + 64 keyword + 8 gap. -->
      <nldd-container v-if="step.dataTable?.length" padding-left="96">
        <nldd-table :columns="columns(step.dataTable)" :accessible-label="t('scenario.table_for', { step: renderStep(step).text })">
          <nldd-table-row v-if="hasHeaderRow(step)" slot="header">
            <nldd-text-cell v-for="(cell, ci) in step.dataTable[0]" :key="ci" size="sm" :text="cell.trim()"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="(row, ri) in step.dataTable.slice(hasHeaderRow(step) ? 1 : 0)" :key="ri">
            <nldd-text-cell
              v-for="(cell, ci) in row"
              :key="ci"
              size="sm"
              :color="displayCell(cell).quiet ? 'secondary' : undefined"
              :text="displayCell(cell).text"
            ></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
      </nldd-container>
    </template>
  </nldd-container>
</template>
