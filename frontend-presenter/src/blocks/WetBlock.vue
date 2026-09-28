<script setup>
import { computed, ref, watch } from 'vue';
import { fetchArticle } from '../lib/api.js';
import { renderHtml } from '../lib/renderMarkdown.js';
import { OPERATION_LABELS, buildOperationTree } from '@editor-utils/operationTree.js';
import { humanize, formatValue } from '@editor-utils/outputFormat.js';

// A ```wet block: one article from the corpus, in words instead of YAML.
//   law: wet_op_de_zorgtoeslag   ($id, the folder name in the corpus)
//   article: '2'
//   date: 2025-01-01             (optional; newest version on or before it)
//   show: [tekst, definities, invoer, uitvoer, regels]   (default: tekst)
//   leden: [1, 2]                (optional; only these leden of the text)

const props = defineProps({ spec: { type: Object, required: true } });

const data = ref(null);
const error = ref(null);

watch(
  () => JSON.stringify(props.spec),
  async () => {
    error.value = null;
    try {
      const s = props.spec;
      data.value = await fetchArticle({ law: s.law, article: s.article != null ? String(s.article) : '', date: s.date ? String(s.date).slice(0, 10) : '' });
    } catch (e) {
      data.value = null;
      error.value = e.message;
    }
  },
  { immediate: true },
);

const show = computed(() => new Set([].concat(props.spec.show ?? ['tekst'])));
const mr = computed(() => data.value?.article?.machine_readable ?? {});
const exec = computed(() => mr.value.execution ?? {});

const lawName = computed(() => {
  const law = data.value?.law;
  if (!law) return props.spec.law;
  const name = law.name ?? humanize(law.id);
  return name.charAt(0).toUpperCase() + name.slice(1);
});

/** The article text, optionally only the leden asked for ("1. …" paragraphs). */
const textHtml = computed(() => {
  const text = data.value?.article?.text ?? '';
  const leden = props.spec.leden ? [].concat(props.spec.leden).map(String) : null;
  if (!leden) return renderHtml(text);
  const parts = text.split(/\n\s*\n/).filter((p) => leden.includes(p.match(/^\s*(\d+[a-z]?)\./)?.[1]));
  return renderHtml(parts.join('\n\n'));
});

const EURO = new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' });
const PCT = new Intl.NumberFormat('nl-NL', { style: 'percent', maximumFractionDigits: 3 });
/** A literal from the YAML in the unit it declares; eurocent is shown as euros. */
function literal(value, unit) {
  if (typeof value === 'number') {
    if (unit === 'eurocent') return EURO.format(value / 100);
    if (unit === 'euro') return EURO.format(value);
    if (unit === 'ratio') return PCT.format(value);
  }
  return formatValue(value);
}
const unitOf = (field) => field?.type_spec?.unit ?? null;

const definitions = computed(() =>
  Object.entries(mr.value.definitions ?? {}).map(([name, d]) => ({
    name: humanize(name),
    value: d && typeof d === 'object' && 'value' in d ? literal(d.value, unitOf(d)) : literal(d),
  })),
);

const inputs = computed(() => [
  ...(exec.value.parameters ?? []).map((p) => ({ name: humanize(p.name), type: p.type, source: 'parameter' })),
  ...(exec.value.input ?? []).map((i) => ({
    name: humanize(i.name),
    type: i.type,
    source: i.source?.regulation
      ? `${humanize(i.source.regulation)} → ${humanize(i.source.output ?? '')}`
      : i.source?.output
        ? `dit artikel → ${humanize(i.source.output)}`
        : 'gegeven',
  })),
]);

const outputs = computed(() =>
  (exec.value.output ?? []).map((o) => ({ name: humanize(o.name), type: o.type, unit: unitOf(o) ?? '' })),
);

const outputUnits = computed(() => Object.fromEntries((exec.value.output ?? []).map((o) => [o.output ?? o.name, unitOf(o)])));

/** Each action as rows: a plain value is one line, an operation its tree. */
const rules = computed(() =>
  (exec.value.actions ?? []).map((action) => {
    const tree = buildOperationTree(action);
    if (tree.length) {
      return {
        output: humanize(action.output),
        rows: tree.map((n) => ({
          depth: n.number.split('.').length - 1,
          label: OPERATION_LABELS[n.operation] ?? n.operation?.toLowerCase(),
          text: n.number === '1' ? n.subtitle : n.title,
        })),
      };
    }
    const v = action.value;
    const shown = typeof v === 'string' && v.startsWith('$') ? humanize(v.slice(1)) : literal(v, outputUnits.value[action.output]);
    return { output: humanize(action.output), value: shown, rows: [] };
  }),
);
</script>

<template>
  <figure class="card wet">
    <figcaption class="wet-head">
      <span class="wet-law">{{ lawName }}</span>
      <span v-if="spec.article != null" class="wet-art">artikel {{ spec.article }}</span>
      <span v-if="data" class="wet-date">geldig vanaf {{ data.law.valid_from }}</span>
    </figcaption>

    <p v-if="error" class="wet-error">{{ error }}</p>
    <template v-else-if="data">
      <div v-if="show.has('tekst') && data.article?.text" class="wet-text" lang="nl" v-html="textHtml"></div>

      <section v-if="show.has('definities') && definitions.length">
        <h4>Vaste waarden</h4>
        <table>
          <tr v-for="d in definitions" :key="d.name"><th>{{ d.name }}</th><td class="num">{{ d.value }}</td></tr>
        </table>
      </section>

      <section v-if="show.has('invoer') && inputs.length">
        <h4>Invoer</h4>
        <table>
          <tr v-for="i in inputs" :key="i.name"><th>{{ i.name }}</th><td class="type">{{ i.type }}</td><td class="src">{{ i.source }}</td></tr>
        </table>
      </section>

      <section v-if="show.has('uitvoer') && outputs.length">
        <h4>Uitvoer</h4>
        <table>
          <tr v-for="o in outputs" :key="o.name"><th>{{ o.name }}</th><td class="type">{{ o.type }}</td><td class="src">{{ o.unit }}</td></tr>
        </table>
      </section>

      <section v-if="show.has('regels') && rules.length">
        <h4>Regels</h4>
        <div v-for="r in rules" :key="r.output" class="rule">
          <div class="rule-out">
            <strong>{{ r.output }}</strong>
            <template v-if="r.value !== undefined"> = <span class="num">{{ r.value }}</span></template>
          </div>
          <ol v-if="r.rows.length" class="rule-tree">
            <li v-for="(row, j) in r.rows" :key="j" :style="{ '--depth': row.depth }">
              <span class="op">{{ row.label }}</span>
              <span class="expr">{{ row.text }}</span>
            </li>
          </ol>
        </div>
      </section>
    </template>
    <p v-else class="wet-loading">Laden…</p>
  </figure>
</template>
