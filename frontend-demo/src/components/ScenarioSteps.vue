<script setup>
import { computed } from 'vue';
import { describeSteps, groupItems } from '../data/scenarioSteps.js';
import { fieldSpec, humanize } from '../data/format.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { useDemo } from '../store/demoStore.js';
import { useI18n } from '../i18n/index.js';
import OrgLogo from './OrgLogo.vue';

// The steps of a scenario (or of the background) as a reader sees them: the
// starting points in one list, a block per data source with a record per row,
// and each evaluation with what the scenario expects of it. The Gherkin itself
// is one toggle away ("Bestand"); this is the view for the audience.
const props = defineProps({
  steps: { type: Array, required: true },
  // Run status per step, aligned with `steps`: `{ status, error }` or nothing.
  results: { type: Array, default: () => [] },
  // The law the scenarios are about, for the specs of expected outputs.
  law: { type: String, default: null },
});

const { t } = useI18n();
const { corpus } = useDemo();

const lawName = (id) => corpus.value?.lawById(id)?.name ?? humanize(id);
const serviceName = (code) => (corpus.value ? serviceInfo(corpus.value, code).name : code);
const specFor = (lawId, field) => (lawId ? fieldSpec(corpus.value?.lawById(lawId)?.doc, field) : null);

// Rebuilt on a language switch too: the labels come from t() and humanize().
const blocks = computed(() => groupItems(describeSteps(props.steps, { lawName, serviceName, specFor, law: props.law })));

const result = (index) => props.results[index] ?? null;

function expectIcon(index) {
  const s = result(index)?.status;
  return s === 'pass' ? 'check-mark-circle' : s === 'fail' ? 'dismiss-circle' : 'circle-dashed';
}

function expectColor(index) {
  const s = result(index)?.status;
  return s === 'pass' ? 'success' : s === 'fail' ? 'critical' : 'secondary';
}

function recordLabel(source, record) {
  return record.title ? `${source.title}, ${record.title.label} ${record.title.text}` : source.title;
}

// The quiet fields of a record, one row per kind: a register row of 25 columns
// with two amounts in it reads as those two amounts plus one line of "0".
function quietRows(record) {
  return [
    { key: 'zero', labels: record.zero, text: '0' },
    { key: 'none', labels: record.none, text: t('format.none') },
    { key: 'not_stated', labels: record.notStated, text: t('scenario.value.not_stated') },
  ].filter((row) => row.labels.length);
}
</script>

<template>
  <nldd-container gap="16">
    <template v-for="(block, bi) in blocks" :key="bi">
      <nldd-list v-if="block.kind === 'settings'" variant="box-tinted" :accessible-label="t('scenario.given.settings')">
        <nldd-list-item v-for="(row, ri) in block.rows" :key="ri" size="sm">
          <nldd-icon-cell :icon="row.icon" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <!-- A parameters table is one step spread over several rows: its
               error goes under the first of them, not under every one. -->
          <nldd-text-cell :text="row.label" :supporting-text="(ri === block.rows.findIndex((r) => r.index === row.index) && result(row.index)?.error) || undefined"></nldd-text-cell>
          <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="row.text"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>

      <!-- Lanes: a narrow card keeps one column, a wide one sets the
           sources next to each other instead of in one long strip. -->
      <nldd-container v-else-if="block.kind === 'data'" layout="lanes" gap="16">
        <nldd-container v-for="source in block.sources" :key="source.index" gap="8">
          <nldd-container layout="row" gap="8" vertical-alignment="center">
            <OrgLogo v-if="source.org" :service="source.org" size="sm" />
            <nldd-icon-cell v-else icon="rectangle-stack" size="16" color="secondary"></nldd-icon-cell>
            <nldd-title-cell size="6" :text="source.title" :supporting-text="source.subtitle"></nldd-title-cell>
          </nldd-container>
          <nldd-banner v-if="result(source.index)?.error" variant="critical" :text="t('scenario.step_failed')" :supporting-text="result(source.index).error"></nldd-banner>
          <nldd-list v-for="(record, ri) in source.records" :key="ri" variant="box-tinted" :accessible-label="recordLabel(source, record)">
            <nldd-list-item v-if="record.title" size="sm">
              <nldd-text-cell :text="`**${record.title.label}**`"></nldd-text-cell>
              <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="`**${record.title.text}**`"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="field in record.fields" :key="field.name" size="sm">
              <nldd-text-cell :text="field.label"></nldd-text-cell>
              <nldd-text-cell width="fit-content" max-width="60%" horizontal-alignment="right" :text="field.text"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item v-for="row in quietRows(record)" :key="row.key" size="sm">
              <nldd-text-cell color="secondary" :text="row.labels.join(', ')"></nldd-text-cell>
              <nldd-text-cell width="fit-content" horizontal-alignment="right" color="secondary" :text="row.text"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-container>

      <nldd-list v-else-if="block.kind === 'check'" variant="box-base" :accessible-label="t('scenario.then.label')">
        <nldd-list-item v-if="block.evaluate" size="sm">
          <nldd-icon-cell icon="play" size="16" color="accent"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            :overline="t('scenario.when.overline')"
            :text="`**${block.evaluate.title}**`"
            :supporting-text="result(block.evaluate.index)?.error || (block.evaluate.outputs.length ? t('scenario.when.outputs', { outputs: block.evaluate.outputs.join(', ') }) : undefined)"
          ></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item v-for="item in block.expects" :key="item.index" size="sm">
          <nldd-icon-cell :icon="expectIcon(item.index)" size="16" :color="expectColor(item.index)"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <!-- The icon is decorative to a screen reader, so the outcome is also
               in words: the error on a failure, "Geslaagd" on a pass. -->
          <nldd-text-cell :text="item.label" :supporting-text="result(item.index)?.error || (result(item.index)?.status === 'pass' ? t('scenario.passed') : undefined)"></nldd-text-cell>
          <nldd-text-cell v-if="item.text" width="fit-content" horizontal-alignment="right" :text="`**${item.text}**`"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>

      <!-- A step this view has no shape for keeps its (translated) sentence. -->
      <nldd-list v-else variant="box-tinted" :accessible-label="t('scenario.steps.label')">
        <nldd-list-item v-for="item in block.items" :key="item.index" size="sm">
          <nldd-text-cell :overline="item.keyword" :text="item.text" :supporting-text="result(item.index)?.error || (item.matched ? undefined : t('scenario.error.unknown_step'))"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </nldd-container>
</template>
