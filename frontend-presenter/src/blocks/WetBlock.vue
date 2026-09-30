<script setup>
import { computed, inject, ref, watch } from 'vue';
import { fetchArticle } from '../lib/api.js';
import { renderHtml } from '../lib/renderMarkdown.js';
import { directRefs, selectArticle } from '../lib/wetView.js';
import { OPERATION_LABELS, buildOperationTree } from '@editor-utils/operationTree.js';
import { humanize, formatValue } from '@editor-utils/outputFormat.js';

// A ```wet block: one article, in words instead of YAML.
//   law: wet_op_de_zorgtoeslag   ($id, the folder name in the corpus), or
//   bestand: variant.yaml        (a law YAML in the deck folder)
//   article: '2'
//   date: 2025-01-01             (corpus only; newest version on or before it)
// plus the anchors that pick parts of the article; see lib/wetView.js.

const props = defineProps({ spec: { type: Object, required: true } });
const deckCtx = inject('presenter:deck', null);

const data = ref(null);
const error = ref(null);

watch(
  () => [JSON.stringify(props.spec), deckCtx?.name.value, deckCtx?.wetVersion.value],
  async () => {
    error.value = null;
    try {
      const s = props.spec;
      const article = s.article != null ? String(s.article) : '';
      data.value = s.bestand
        ? await fetchArticle({ deck: deckCtx?.name.value, file: String(s.bestand), article })
        : await fetchArticle({ law: s.law, article, date: s.date ? String(s.date).slice(0, 10) : '' });
    } catch (e) {
      data.value = null;
      error.value = e.message;
    }
  },
  { immediate: true },
);

const view = computed(() => selectArticle(data.value?.article, props.spec));
const show = computed(() => view.value.show);
const lit = (name) => view.value.highlight.has(name);

const lawName = computed(() => {
  const law = data.value?.law;
  if (!law) return props.spec.law ?? props.spec.bestand;
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
  view.value.definitions.map(({ name, def }) => ({
    key: name,
    name: humanize(name),
    value: def && typeof def === 'object' && 'value' in def ? literal(def.value, unitOf(def)) : literal(def),
  })),
);

const inputs = computed(() =>
  view.value.inputs.map((i) => ({
    key: i.name,
    name: humanize(i.name),
    type: i.type,
    source: i.isParameter
      ? 'parameter'
      : i.source?.regulation
        ? `${humanize(i.source.regulation)} → ${humanize(i.source.output ?? '')}`
        : i.source?.output
          ? `dit artikel → ${humanize(i.source.output)}`
          : 'gegeven',
  })),
);

const outputs = computed(() =>
  view.value.outputs.map((o) => ({ key: o.name, name: humanize(o.name), type: o.type, unit: unitOf(o) ?? '' })),
);

const outputUnits = computed(() => Object.fromEntries((data.value?.article?.machine_readable?.execution?.output ?? []).map((o) => [o.name, unitOf(o)])));

/** Each action as rows: a plain value is one line, an operation its tree. */
const rules = computed(() =>
  view.value.actions.map((action) => {
    const tree = buildOperationTree(action);
    if (tree.length) {
      return {
        key: action.output,
        output: humanize(action.output),
        rows: tree.map((n) => ({
          depth: n.number.split('.').length - 1,
          label: OPERATION_LABELS[n.operation] ?? n.operation?.toLowerCase(),
          text: n.number === '1' ? n.subtitle : n.title,
          lit: [...directRefs(n.node)].some(lit),
        })),
      };
    }
    const v = action.value;
    const ref = typeof v === 'string' && v.startsWith('$') ? v.slice(1) : null;
    return {
      key: action.output,
      output: humanize(action.output),
      value: ref ? humanize(ref) : literal(v, outputUnits.value[action.output]),
      valueLit: ref ? lit(ref) : false,
      rows: [],
    };
  }),
);

const warnings = computed(() =>
  data.value ? view.value.unknown.map((u) => `${u.kind} "${u.name}" komt in dit artikel niet voor`) : [],
);
</script>

<template>
  <figure class="card wet">
    <figcaption class="wet-head">
      <span class="wet-law">{{ lawName }}</span>
      <span v-if="spec.article != null" class="wet-art">artikel {{ spec.article }}</span>
      <span v-if="data" class="wet-date">
        <template v-if="spec.bestand">{{ spec.bestand }} · </template>geldig vanaf {{ data.law.valid_from }}
      </span>
    </figcaption>

    <p v-if="error" class="wet-error">{{ error }}</p>
    <template v-else-if="data">
      <ul v-if="warnings.length" class="wet-warnings">
        <li v-for="w in warnings" :key="w">{{ w }}</li>
      </ul>

      <div v-if="show.has('tekst') && data.article?.text" class="wet-text" lang="nl" v-html="textHtml"></div>

      <section v-if="show.has('definities') && definitions.length">
        <h4>Vaste waarden</h4>
        <table>
          <tr v-for="d in definitions" :key="d.key" :class="{ lit: lit(d.key) }"><th>{{ d.name }}</th><td class="num">{{ d.value }}</td></tr>
        </table>
      </section>

      <section v-if="show.has('invoer') && inputs.length">
        <h4>Invoer</h4>
        <table>
          <tr v-for="i in inputs" :key="i.key" :class="{ lit: lit(i.key) }"><th>{{ i.name }}</th><td class="type">{{ i.type }}</td><td class="src">{{ i.source }}</td></tr>
        </table>
      </section>

      <section v-if="show.has('uitvoer') && outputs.length">
        <h4>Uitvoer</h4>
        <table>
          <tr v-for="o in outputs" :key="o.key" :class="{ lit: lit(o.key) }"><th>{{ o.name }}</th><td class="type">{{ o.type }}</td><td class="src">{{ o.unit }}</td></tr>
        </table>
      </section>

      <section v-if="show.has('regels') && rules.length">
        <h4>Regels</h4>
        <div v-for="r in rules" :key="r.key" class="rule">
          <div class="rule-out" :class="{ lit: lit(r.key) }">
            <strong>{{ r.output }}</strong>
            <template v-if="r.value !== undefined"> = <span class="num" :class="{ lit: r.valueLit }">{{ r.value }}</span></template>
          </div>
          <ol v-if="r.rows.length" class="rule-tree">
            <li v-for="(row, j) in r.rows" :key="j" :class="{ lit: row.lit }" :style="{ '--depth': row.depth }">
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
