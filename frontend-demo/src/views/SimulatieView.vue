<script setup>
import { computed, reactive, ref, shallowRef, watch } from 'vue';
import OrgLogo from '../components/OrgLogo.vue';
import SimBarChart from '../components/SimBarChart.vue';
import { fieldSpec, formatValue, humanize } from '../data/format.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { BUSINESS_DEFAULTS, CITIZEN_DEFAULTS } from '../simulation/population.js';
import { definitionKind, overridableDefinitions } from '../simulation/lawParameters.js';
import { runSimulation, simulationLaws } from '../simulation/runner.js';
import { BUSINESS_DIMENSIONS, CITIZEN_DIMENSIONS, breakdown, flattenResults, toCsv } from '../simulation/stats.js';
import { disposableIncomeBreakdown, summariseDisposableIncome } from '../simulation/income.js';
import { useDemo } from '../store/demoStore.js';

// Simulatie: a synthetic population of citizens or businesses, every portal
// law evaluated for each of them by the same engine the portal uses, and the
// outcome in numbers: who qualifies, for how much, split by age, income, type
// of business. Law constants can be changed for a run ("what if the threshold
// goes up?"), and runs sit side by side for comparison.

const demo = useDemo();
const { corpus, engine, ready, state } = demo;

const kind = ref('burgers');
const citizenParams = reactive(JSON.parse(JSON.stringify(CITIZEN_DEFAULTS)));
const businessParams = reactive(JSON.parse(JSON.stringify(BUSINESS_DEFAULTS)));
const params = computed(() => (kind.value === 'ondernemers' ? businessParams : citizenParams));
const referenceDate = ref(state.referenceDate);
/** law id -> definition key -> value (only the ones the presenter touched). */
const overrides = reactive({});

// The simulation runs every law of the audience that the demo shows at all;
// a profile's own `disabled_laws` (what Merijn's portal hides) does not apply.
function notHidden(law) {
  const hidden = corpus.value?.config?.hidden_laws ?? {};
  return !(hidden[law.service] ?? []).some((p) => law.law_path === p || law.law_path.startsWith(`${p}/`));
}
const lawSet = computed(() => (corpus.value ? simulationLaws(corpus.value, kind.value, notHidden) : { runnable: [], skipped: [] }));
const definitionsByLaw = computed(() => Object.fromEntries(lawSet.value.runnable.map((law) => [law.id, overridableDefinitions(law.doc)])));
function overrideCount(lawId) {
  const own = Object.fromEntries((definitionsByLaw.value[lawId] ?? []).map((d) => [d.key, d.value]));
  return Object.entries(overrides[lawId] ?? {}).filter(([k, v]) => k in own && v !== own[k]).length;
}
const totalOverrides = computed(() => lawSet.value.runnable.reduce((n, law) => n + overrideCount(law.id), 0));

// ---- sections in the sidebar ------------------------------------------------
const open = reactive({ populatie: false, wetgeving: false });

const CITIZEN_KNOBS = [
  { group: 'Leeftijdsverdeling (%)', fields: ['18-30', '30-45', '45-67', '67-85', '85+'].map((k) => ({ path: ['ageDistribution', k], label: `${k} jaar` })) },
  { group: 'Inkomensverdeling (%)', fields: [['low', 'Laag (tot € 30k)'], ['middle', 'Midden (€ 30-60k)'], ['high', 'Hoog (€ 60k+)']].map(([k, label]) => ({ path: ['incomeDistribution', k], label })) },
  { group: 'Huishouden en werk (%)', fields: [{ path: ['zeroIncomePct'], label: 'Zonder inkomen' }, { path: ['renterPct'], label: 'Huurders' }, { path: ['studentPct'], label: 'Studenten onder 30' }] },
];
const BUSINESS_KNOBS = [
  { group: 'Bedrijfstype (%)', fields: [{ path: ['horecaPct'], label: 'Horeca' }, { path: ['foodPct'], label: 'Bereidt of verkoopt voedsel' }, { path: ['terracePct'], label: 'Horeca met terras' }, { path: ['employeesPct'], label: 'Met werknemers' }] },
  { group: 'Vloeroppervlakte (%)', fields: [['small', 'Klein (tot 50 m²)'], ['medium', 'Middel (50-100 m²)'], ['large', 'Groot (100+ m²)']].map(([k, label]) => ({ path: ['sizeDistribution', k], label })) },
];
const knobs = computed(() => (kind.value === 'ondernemers' ? BUSINESS_KNOBS : CITIZEN_KNOBS));
function getKnob(path) {
  return path.reduce((o, k) => o?.[k], params.value);
}
function setKnob(path, value) {
  const n = Number(value);
  if (!Number.isFinite(n)) return;
  const target = path.slice(0, -1).reduce((o, k) => o[k], params.value);
  target[path[path.length - 1]] = n;
}
function numberFrom(e) {
  const v = e.detail?.value ?? e.target?.value;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

// ---- runs ---------------------------------------------------------------------
const runs = shallowRef([]);
const activeTab = ref(null); // run id or 'vergelijking'
const running = ref(false);
const progress = reactive({ done: 0, total: 0 });
const runError = ref(null);
const signal = reactive({ cancelled: false });
const mainView = ref('overzicht');
const dimensionId = ref(null);

const activeRun = computed(() => runs.value.find((r) => r.id === activeTab.value) ?? null);
const dimensions = computed(() => (activeRun.value?.kind === 'ondernemers' ? BUSINESS_DIMENSIONS : CITIZEN_DIMENSIONS));
const dimension = computed(() => dimensions.value.find((d) => d.id === dimensionId.value) ?? dimensions.value[0]);
watch(activeRun, () => {
  if (!dimensions.value.some((d) => d.id === dimensionId.value)) dimensionId.value = dimensions.value[0]?.id ?? null;
});

async function run() {
  if (!ready.value || running.value) return;
  running.value = true;
  runError.value = null;
  signal.cancelled = false;
  progress.done = 0;
  progress.total = Number(params.value.count) || 0;
  try {
    const result = await runSimulation({
      engine: engine.value,
      corpus: corpus.value,
      kind: kind.value,
      params: JSON.parse(JSON.stringify(params.value)),
      overrides: JSON.parse(JSON.stringify(overrides)),
      referenceDate: referenceDate.value,
      isLawEnabled: notHidden,
      onProgress: (done, total) => {
        progress.done = done;
        progress.total = total;
      },
      signal,
    });
    result.label = `${runs.value.length + 1}. ${kind.value === 'ondernemers' ? 'Ondernemers' : 'Burgers'} (${result.subjects.length})`;
    runs.value = [...runs.value, result];
    activeTab.value = result.id;
    mainView.value = 'overzicht';
  } catch (e) {
    runError.value = String(e?.message ?? e?.error ?? e);
  } finally {
    running.value = false;
    // The simulation replaced the personas' data; give the portal its data back.
    demo.reregister();
  }
}
function cancel() {
  signal.cancelled = true;
}
function removeRun(id) {
  runs.value = runs.value.filter((r) => r.id !== id);
  if (activeTab.value === id) activeTab.value = runs.value.at(-1)?.id ?? null;
  if (activeTab.value === 'vergelijking' && runs.value.length < 2) activeTab.value = runs.value.at(-1)?.id ?? null;
}
function onTab(e) {
  const id = e.detail?.item?.dataset?.run;
  if (id) activeTab.value = id;
}

// ---- inspector ----------------------------------------------------------------
const inspector = ref(null); // { type: 'params', lawId } | { type: 'law', lawId }
function editParameters(law) {
  inspector.value = { type: 'params', lawId: law.id };
}
function showLaw(lawId) {
  inspector.value = { type: 'law', lawId };
}
const inspectorLaw = computed(() => (inspector.value ? corpus.value?.lawById(inspector.value.lawId) : null));
function setOverride(lawId, key, value) {
  overrides[lawId] ??= {};
  const n = Number(value);
  if (Number.isFinite(n)) overrides[lawId][key] = n;
}
function resetOverrides(lawId) {
  delete overrides[lawId];
}
function definitionHint(def) {
  const k = definitionKind(def.key, def.value);
  if (k === 'eurocent') return `${formatValue(def.value, { type: 'amount' })} · in eurocent`;
  if (k === 'percentage') return def.value <= 1 ? `${(def.value * 100).toLocaleString('nl-NL', { maximumFractionDigits: 3 })}% · als fractie` : 'percentage';
  return 'getal';
}

// ---- presentation helpers -----------------------------------------------------
const euro = new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 });
const pct = (v) => (v === null || v === undefined ? '–' : `${Math.round(v)}%`);
const money = (v) => (v === null || v === undefined ? '–' : euro.format(v));
const num = (v, digits = 0) => (v === null || v === undefined ? '–' : new Intl.NumberFormat('nl-NL', { maximumFractionDigits: digits }).format(v));

function amountLabel(run, lawId) {
  const law = run.laws.find((l) => l.id === lawId);
  const name = run.results.find((r) => r.laws[lawId]?.amountName)?.laws[lawId]?.amountName ?? law?.primaryName;
  return name ? humanize(name) : 'Bedrag';
}
function amountIsMoney(run, lawId) {
  const doc = corpus.value?.lawById(lawId)?.doc;
  const name = run.results.find((r) => r.laws[lawId]?.amountName)?.laws[lawId]?.amountName;
  if (!name || !doc) return false;
  const spec = fieldSpec(doc, name);
  return spec?.type === 'amount' || spec?.type_spec?.unit === 'eurocent';
}
const fmtAmount = (run, lawId, v) => (amountIsMoney(run, lawId) ? money(v) : num(v, 1));

const lawRows = computed(() => {
  const run = activeRun.value;
  if (!run) return [];
  return run.laws
    .map((law) => ({ ...law, s: run.summary[law.id], info: serviceInfo(corpus.value, law.service) }))
    .sort((a, b) => b.s.eligiblePct - a.s.eligiblePct);
});
const eligibleRows = computed(() => lawRows.value.filter((l) => l.s.hasEligibility));
const eligibleChart = computed(() => ({
  categories: eligibleRows.value.map((l) => l.name),
  series: [{ name: 'Voldoet aan de voorwaarden', values: eligibleRows.value.map((l) => l.s.eligiblePct) }],
}));
function outcomeLine(run, law) {
  const s = law.s;
  const head = s.hasEligibility ? `${s.eligible} van ${s.evaluated} voldoet` : `${s.evaluated} berekend`;
  const amount = s.withAmount ? ` · gemiddeld ${fmtAmount(run, law.id, s.avgAmount)}` : '';
  const errors = s.errors ? ` · ${s.errors} niet berekend` : '';
  return head + amount + errors;
}
const amountChart = computed(() => {
  const rows = lawRows.value.filter((l) => l.s.withAmount > 0 && amountIsMoney(activeRun.value, l.id));
  return { categories: rows.map((l) => l.name), series: [{ name: 'Gemiddeld bedrag', values: rows.map((l) => l.s.avgAmount) }] };
});
function breakdownFor(lawId) {
  return breakdown(activeRun.value.results, lawId, dimension.value.groupOf, dimension.value.order);
}
const populationFacts = computed(() => {
  const run = activeRun.value;
  if (!run) return [];
  const p = run.population;
  if (run.kind === 'ondernemers') {
    return [
      ['Bedrijven', num(p.count)],
      ['Horeca', pct(p.horecaPct)],
      ['Bereidt voedsel', pct(p.voedselPct)],
      ['Met terras', pct(p.terrasPct)],
      ['Gemiddelde oppervlakte', `${num(p.gemOppervlakte)} m²`],
      ['Met werknemers', pct(p.metWerknemersPct)],
    ];
  }
  return [
    ['Personen', num(p.count)],
    ['Gemiddelde leeftijd', `${num(p.gemLeeftijd)} jaar`],
    ['Met partner', pct(p.partnerPct)],
    ['Huurders', pct(p.huurderPct)],
    ['Met kinderen', pct(p.kinderenPct)],
    ['Studenten', pct(p.studentPct)],
    ['Gemiddeld inkomen', money(p.gemInkomen)],
    ['Mediaan inkomen', money(p.medInkomen)],
  ];
});
const subjectColumns = computed(() => {
  if (!activeRun.value) return [];
  return activeRun.value.kind === 'ondernemers'
    ? [['kvk_nummer', 'KVK'], ['type', 'Type'], ['oppervlakte', 'm²'], ['rechtsvorm', 'Rechtsvorm'], ['werknemers', 'Werknemers'], ['terras_m2', 'Terras m²'], ['voedsel', 'Voedsel']]
    : [['bsn', 'BSN'], ['leeftijd', 'Leeftijd'], ['inkomen', 'Inkomen'], ['partner', 'Partner'], ['kinderen', 'Kinderen'], ['huurder', 'Huurder'], ['student', 'Student']];
});
function subjectCell(subject, key) {
  const v = subject[key];
  if (key === 'inkomen') return money(v);
  return formatValue(v);
}
function eligibleCount(result) {
  return Object.values(result.laws).filter((l) => l.ok && l.met !== false).length;
}

// ---- disposable income (citizens) --------------------------------------------------
// The POC's closing figure: what a person keeps per month after taxes and
// benefits, and after housing. Which outputs count is demo configuration.
const incomeComponents = computed(() => (corpus.value?.config?.simulation?.disposable_income ?? []).filter((c) => corpus.value.latestById.has(c.law)));
const disposable = computed(() => (activeRun.value?.kind === 'burgers' && incomeComponents.value.length ? summariseDisposableIncome(activeRun.value.results, incomeComponents.value) : null));
const disposableRows = computed(() => (disposable.value ? disposableIncomeBreakdown(activeRun.value.results, incomeComponents.value, dimension.value.groupOf, dimension.value.order) : []));
const componentRows = computed(() => (disposable.value ? disposable.value.components.filter((c) => c.withValue > 0) : []));
function componentLabel(c) {
  return c.component.label ?? corpus.value?.lawById(c.component.law)?.name ?? c.component.law;
}

// ---- comparison ---------------------------------------------------------------
const comparable = computed(() => runs.value.filter((r) => r.kind === (runs.value.find((x) => x.id === activeTab.value)?.kind ?? runs.value[0]?.kind)));
const comparisonLaws = computed(() => {
  const ids = new Map();
  for (const r of comparable.value) for (const law of r.laws) ids.set(law.id, law);
  return [...ids.values()].sort((a, b) => a.name.localeCompare(b.name));
});
const comparisonChart = computed(() => ({
  categories: comparisonLaws.value.map((l) => l.name),
  series: comparable.value.map((r) => ({ name: r.label, values: comparisonLaws.value.map((l) => r.summary[l.id]?.eligiblePct ?? null) })),
}));

// ---- export -------------------------------------------------------------------
function download(name, text, type) {
  const blob = new Blob([text], { type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
function exportCsv() {
  const run = activeRun.value;
  download(`simulatie-${run.kind}-${run.subjects.length}.csv`, toCsv(flattenResults(run.results, run.laws.map((l) => l.id))), 'text/csv;charset=utf-8');
}
function exportJson() {
  const run = activeRun.value;
  const { results, ...rest } = run;
  download(`simulatie-${run.kind}-${run.subjects.length}.json`, JSON.stringify({ ...rest, results: flattenResults(results, run.laws.map((l) => l.id)) }, null, 2), 'application/json');
}
</script>

<template>
  <nldd-navigation-split-view>
    <nldd-split-view-pane slot="sidebar" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-title slot="start" text="Simulatie" :supporting-text="`${lawSet.runnable.length} regelingen`"></nldd-toolbar-title>
          </nldd-toolbar>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-segmented-control width="full" :value="kind" @change="kind = $event.detail?.value ?? kind">
            <nldd-segmented-control-item value="burgers" text="Burgers" icon="person"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="ondernemers" text="Ondernemers" icon="business-suitcase"></nldd-segmented-control-item>
          </nldd-segmented-control>

          <nldd-form-field :label="kind === 'ondernemers' ? 'Aantal bedrijven' : 'Aantal personen'">
            <nldd-number-field :value="params.count" min="1" max="2000" step="10" width="full" @change="params.count = numberFrom($event) ?? params.count"></nldd-number-field>
          </nldd-form-field>
          <nldd-form-field label="Peildatum">
            <nldd-date-field :value="referenceDate" width="full" @change="referenceDate = $event.detail?.value || referenceDate"></nldd-date-field>
          </nldd-form-field>

          <nldd-button width="full" variant="neutral-transparent" horizontal-alignment="left" :end-icon="open.populatie ? 'chevron-up' : 'chevron-down'" :text="kind === 'ondernemers' ? 'Samenstelling bedrijven' : 'Demografie'" :expanded="open.populatie || undefined" @click="open.populatie = !open.populatie"></nldd-button>
          <template v-if="open.populatie">
            <nldd-form-field label="Seed">
              <nldd-number-field size="sm" :value="params.seed" min="1" step="1" width="full" hide-spin-buttons @change="params.seed = numberFrom($event) ?? params.seed"></nldd-number-field>
              <nldd-form-field-help-text>Dezelfde seed geeft dezelfde populatie.</nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-container v-for="group in knobs" :key="group.group" gap="8">
              <nldd-text-cell size="sm" color="secondary" :text="group.group"></nldd-text-cell>
              <nldd-form-field v-for="field in group.fields" :key="field.label" :label="field.label">
                <nldd-number-field size="sm" :value="getKnob(field.path)" min="0" max="100" step="5" width="full" hide-spin-buttons @change="setKnob(field.path, numberFrom($event))"></nldd-number-field>
              </nldd-form-field>
            </nldd-container>
          </template>

          <nldd-button width="full" variant="neutral-transparent" horizontal-alignment="left" :end-icon="open.wetgeving ? 'chevron-up' : 'chevron-down'" :text="totalOverrides ? `Parameters van wetgeving (${totalOverrides} aangepast)` : 'Parameters van wetgeving'" :expanded="open.wetgeving || undefined" @click="open.wetgeving = !open.wetgeving"></nldd-button>
          <nldd-list v-if="open.wetgeving" variant="box-tinted" accessible-label="Wetten met aanpasbare parameters">
            <nldd-list-item v-for="law in lawSet.runnable" :key="law.id" size="sm" button :disabled="definitionsByLaw[law.id].length === 0 || undefined" @click="editParameters(law)">
              <nldd-cell><OrgLogo :service="law.service" size="sm" /></nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="law.name" :supporting-text="`${definitionsByLaw[law.id].length} parameters`"></nldd-text-cell>
              <nldd-cell v-if="overrideCount(law.id)"><nldd-tag size="sm" color="warning" :text="`${overrideCount(law.id)} aangepast`"></nldd-tag></nldd-cell>
              <nldd-icon-cell v-else-if="definitionsByLaw[law.id].length" icon="chevron-right" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>

          <nldd-button width="full" variant="primary" start-icon="play" :text="running ? 'Bezig…' : 'Simuleren'" :disabled="!ready || running || undefined" @click="run"></nldd-button>
          <template v-if="running">
            <nldd-progress-bar :value="progress.done" :max="progress.total || 1" value-format="fraction" value-display="inline" :text="kind === 'ondernemers' ? 'Bedrijven doorgerekend' : 'Personen doorgerekend'"></nldd-progress-bar>
            <nldd-button width="full" variant="secondary" size="sm" text="Stoppen" @click="cancel"></nldd-button>
          </template>
          <nldd-banner v-if="runError" variant="critical" text="Simulatie mislukt" :supporting-text="runError"></nldd-banner>
          <nldd-rich-text v-if="lawSet.skipped.length" size="sm">
            <p><small>Niet gesimuleerd: {{ lawSet.skipped.map((s) => `${s.law.name} (vraagt ${s.missing.join(', ')})`).join('; ') }}.</small></p>
          </nldd-rich-text>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container v-if="runs.length" slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="sm" @tabchange="onTab">
                <nldd-tab-bar-item v-for="r in runs" :key="r.id" :data-run="r.id" :selected="activeTab === r.id || undefined" :text="r.label"></nldd-tab-bar-item>
                <nldd-tab-bar-item v-if="runs.length > 1" data-run="vergelijking" :selected="activeTab === 'vergelijking' || undefined" text="Vergelijking" icon="arrow-left-right"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <template v-if="activeRun">
              <nldd-toolbar-item slot="end">
                <nldd-segmented-control size="sm" width="fit-content" :value="mainView" @change="mainView = $event.detail?.value ?? mainView">
                  <nldd-segmented-control-item value="overzicht" text="Overzicht"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="uitsplitsing" text="Uitsplitsing"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="populatie" text="Populatie"></nldd-segmented-control-item>
                </nldd-segmented-control>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end">
                <nldd-icon-button size="sm" variant="neutral-tinted" icon="menu" text="Exporteren" tooltip-timing="never" expandable>
                  <nldd-menu>
                    <nldd-menu-item text="CSV" icon="file-text" @click="exportCsv"></nldd-menu-item>
                    <nldd-menu-item text="JSON" icon="brackets-ellipsis" @click="exportJson"></nldd-menu-item>
                    <nldd-menu-divider></nldd-menu-divider>
                    <nldd-menu-item text="Verwijderen" icon="trash" @click="removeRun(activeRun.id)"></nldd-menu-item>
                  </nldd-menu>
                </nldd-icon-button>
              </nldd-toolbar-item>
            </template>
          </nldd-toolbar>
        </nldd-container>

        <nldd-simple-section v-if="!runs.length" height="60vh">
          <nldd-inline-dialog icon="chart-x-y-axis-line" text="Nog geen simulatie" supporting-text="Kies links een populatie en druk op Simuleren. Elke persoon of elk bedrijf wordt door de engine door alle regelingen gehaald."></nldd-inline-dialog>
        </nldd-simple-section>

        <!-- One run -->
        <template v-else-if="activeRun">
          <nldd-simple-section v-if="mainView === 'overzicht'" width="full">
            <nldd-container gap="16">
              <nldd-banner v-if="activeRun.cancelled" variant="warning" text="Gestopt" :supporting-text="`${activeRun.results.length} van ${activeRun.subjects.length} doorgerekend.`"></nldd-banner>
              <nldd-banner v-if="Object.keys(activeRun.overrides).length" variant="info" text="Met aangepaste parameters" :supporting-text="Object.entries(activeRun.overrides).map(([id, o]) => `${corpus.lawById(id)?.name ?? id}: ${Object.entries(o).map(([k, v]) => `${k} = ${v}`).join(', ')}`).join(' · ')"></nldd-banner>

              <nldd-card accessible-label="Populatie">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" text="Populatie" :supporting-text="`Peildatum ${formatValue(activeRun.referenceDate)} · seed ${activeRun.params.seed} · ${(activeRun.durationMs / 1000).toFixed(1)} s`"></nldd-title-cell></nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12">
                  <nldd-list variant="simple" accessible-label="Kenmerken van de populatie">
                    <nldd-list-item v-for="[label, value] in populationFacts" :key="label" size="sm">
                      <nldd-text-cell size="sm" color="secondary" :text="label"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="value"></nldd-text-cell>
                    </nldd-list-item>
                  </nldd-list>
                </nldd-container>
              </nldd-card>

              <nldd-card v-if="disposable" background="tinted" accessible-label="Besteedbaar inkomen">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center">
                  <nldd-icon name="euro-sign" size="24"></nldd-icon>
                  <nldd-title-cell size="4" text="Besteedbaar inkomen" supporting-text="Wat een persoon per maand overhoudt: inkomen min belastingen, plus toeslagen en uitkeringen die de wetten toekennen"></nldd-title-cell>
                </nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12" gap="12">
                  <nldd-container layout="grid" column-count="3" sm-column-count="1" gap="12">
                    <nldd-list variant="box-base" accessible-label="Gemiddeld per maand"><nldd-list-item size="md"><nldd-title-cell size="3" overline="Gemiddeld per maand" :text="money(disposable.avgDisposable)"></nldd-title-cell></nldd-list-item></nldd-list>
                    <nldd-list variant="box-base" accessible-label="Mediaan per maand"><nldd-list-item size="md"><nldd-title-cell size="3" overline="Mediaan per maand" :text="money(disposable.medianDisposable)"></nldd-title-cell></nldd-list-item></nldd-list>
                    <nldd-list variant="box-base" accessible-label="Na woonkosten"><nldd-list-item size="md"><nldd-title-cell size="3" overline="Na woonkosten" :text="money(disposable.avgAfterHousing)" :supporting-text="`gemiddeld ${money(disposable.avgHousing)} huur of hypotheek`"></nldd-title-cell></nldd-list-item></nldd-list>
                  </nldd-container>
                  <nldd-list variant="box-base" accessible-label="Opbouw van het besteedbaar inkomen">
                    <nldd-list-item size="sm">
                      <nldd-text-cell size="sm" text="Inkomen" supporting-text="bruto, per maand"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="money(disposable.avgIncome)"></nldd-text-cell>
                    </nldd-list-item>
                    <nldd-list-item v-for="c in componentRows" :key="`${c.component.law}#${c.component.output}`" size="sm">
                      <nldd-text-cell size="sm" :text="componentLabel(c)" :supporting-text="`${c.withValue} van ${disposable.count} personen · ${humanize(c.component.output)}`"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="c.component.kind === 'tax' ? 'critical' : 'success'" :text="`${c.component.kind === 'tax' ? '−' : '+'} ${money(c.avg)}`"></nldd-text-cell>
                    </nldd-list-item>
                    <nldd-list-item size="sm">
                      <nldd-text-cell size="sm" text="**Besteedbaar**" supporting-text="gemiddeld per maand"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="`**${money(disposable.avgDisposable)}**`"></nldd-text-cell>
                    </nldd-list-item>
                  </nldd-list>
                </nldd-container>
              </nldd-card>

              <nldd-card accessible-label="Voldoet aan de voorwaarden">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" text="Wie voldoet aan de voorwaarden" supporting-text="Per regeling, als percentage van de populatie"></nldd-title-cell></nldd-container>
                <nldd-container padding="12">
                  <SimBarChart :categories="eligibleChart.categories" :series="eligibleChart.series" unit="percent" horizontal :height="`${Math.max(200, 34 * eligibleChart.categories.length + 40)}px`" />
                </nldd-container>
              </nldd-card>

              <nldd-card v-if="amountChart.categories.length" accessible-label="Gemiddeld bedrag">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" text="Gemiddeld bedrag" supporting-text="Voor wie aan de voorwaarden voldoet, in euro"></nldd-title-cell></nldd-container>
                <nldd-container padding="12">
                  <SimBarChart :categories="amountChart.categories" :series="amountChart.series" unit="euro" horizontal :height="`${Math.max(160, 34 * amountChart.categories.length + 40)}px`" />
                </nldd-container>
              </nldd-card>

              <nldd-list variant="box-tinted" accessible-label="Regelingen">
                <nldd-list-item v-for="law in lawRows" :key="law.id" size="md" button @click="showLaw(law.id)">
                  <nldd-cell><OrgLogo :service="law.service" /></nldd-cell>
                  <nldd-spacer-cell size="12"></nldd-spacer-cell>
                  <nldd-text-cell :text="law.name" :supporting-text="outcomeLine(activeRun, law)"></nldd-text-cell>
                  <nldd-cell v-if="law.s.hasEligibility"><nldd-tag size="sm" :color="law.s.eligiblePct >= 50 ? 'success' : law.s.eligiblePct > 0 ? 'neutral' : 'critical'" :text="pct(law.s.eligiblePct)"></nldd-tag></nldd-cell>
                  <nldd-cell v-else><nldd-tag size="sm" color="neutral" text="berekend"></nldd-tag></nldd-cell>
                  <nldd-icon-cell icon="chevron-right" color="secondary"></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
            </nldd-container>
          </nldd-simple-section>

          <nldd-simple-section v-else-if="mainView === 'uitsplitsing'" width="full">
            <nldd-container gap="16">
              <nldd-segmented-control size="sm" width="fit-content" :value="dimension.id" @change="dimensionId = $event.detail?.value ?? dimensionId">
                <nldd-segmented-control-item v-for="d in dimensions" :key="d.id" :value="d.id" :text="d.label"></nldd-segmented-control-item>
              </nldd-segmented-control>
              <nldd-card v-if="disposable" background="tinted" accessible-label="Besteedbaar inkomen per groep">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center">
                  <nldd-icon name="euro-sign" size="24"></nldd-icon>
                  <nldd-title-cell size="5" text="Besteedbaar inkomen" :supporting-text="`per maand · naar ${dimension.label.toLowerCase()}`"></nldd-title-cell>
                </nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12">
                  <nldd-table columns="minmax(140px, 1fr) 90px 130px 130px 130px" accessible-label="Besteedbaar inkomen per groep">
                    <nldd-table-row slot="header">
                      <nldd-text-cell size="sm" :text="dimension.label"></nldd-text-cell>
                      <nldd-text-cell size="sm" text="Personen" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" text="Gemiddeld" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" text="Mediaan" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" text="Na woonkosten" horizontal-alignment="right"></nldd-text-cell>
                    </nldd-table-row>
                    <nldd-table-row v-for="row in disposableRows" :key="row.group">
                      <nldd-text-cell size="sm" :text="humanize(row.group)"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="num(row.count)" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="money(row.avgDisposable)" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="money(row.medianDisposable)" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="money(row.avgAfterHousing)" horizontal-alignment="right"></nldd-text-cell>
                    </nldd-table-row>
                  </nldd-table>
                </nldd-container>
              </nldd-card>
              <nldd-card v-for="law in lawRows" :key="law.id" :accessible-label="law.name">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="top">
                  <OrgLogo :service="law.service" size="sm" />
                  <nldd-title-cell size="5" :text="law.name" :supporting-text="`${law.s.hasEligibility ? `${pct(law.s.eligiblePct)} voldoet` : 'berekend voor iedereen'} · naar ${dimension.label.toLowerCase()}`"></nldd-title-cell>
                </nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12">
                <nldd-table columns="minmax(140px, 1fr) 110px 90px 130px" :accessible-label="`${law.name} naar ${dimension.label}`">
                  <nldd-table-row slot="header">
                    <nldd-text-cell size="sm" :text="dimension.label"></nldd-text-cell>
                    <nldd-text-cell size="sm" text="Voldoet" horizontal-alignment="right"></nldd-text-cell>
                    <nldd-text-cell size="sm" text="Aandeel" horizontal-alignment="right"></nldd-text-cell>
                    <nldd-text-cell size="sm" :text="amountLabel(activeRun, law.id)" horizontal-alignment="right"></nldd-text-cell>
                  </nldd-table-row>
                  <nldd-table-row v-for="row in breakdownFor(law.id)" :key="row.group">
                    <nldd-text-cell size="sm" :text="humanize(row.group)"></nldd-text-cell>
                    <nldd-text-cell size="sm" :text="`${row.eligible} / ${row.evaluated}`" horizontal-alignment="right"></nldd-text-cell>
                    <nldd-text-cell size="sm" :text="pct(row.eligiblePct)" horizontal-alignment="right"></nldd-text-cell>
                    <nldd-text-cell size="sm" :text="row.withAmount ? fmtAmount(activeRun, law.id, row.avgAmount) : '–'" horizontal-alignment="right"></nldd-text-cell>
                  </nldd-table-row>
                </nldd-table>
                </nldd-container>
              </nldd-card>
            </nldd-container>
          </nldd-simple-section>

          <nldd-simple-section v-else width="full">
            <nldd-card accessible-label="Populatie">
              <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" text="Populatie" :supporting-text="`${Math.min(activeRun.results.length, 200)} van ${activeRun.results.length} getoond · laatste kolom: regelingen waaraan wordt voldaan`"></nldd-title-cell></nldd-container>
              <nldd-container padding-inline="12" padding-bottom="12">
              <nldd-table :columns="`repeat(${subjectColumns.length}, minmax(90px, 1fr)) 90px`" accessible-label="Gesimuleerde populatie">
                <nldd-table-row slot="header">
                  <nldd-text-cell v-for="[key, label] in subjectColumns" :key="key" size="sm" :text="label"></nldd-text-cell>
                  <nldd-text-cell size="sm" text="Voldoet" horizontal-alignment="right"></nldd-text-cell>
                </nldd-table-row>
                <nldd-table-row v-for="r in activeRun.results.slice(0, 200)" :key="r.subject.id">
                  <nldd-text-cell v-for="[key] in subjectColumns" :key="key" size="sm" :text="subjectCell(r.subject, key)"></nldd-text-cell>
                  <nldd-text-cell size="sm" :text="`${eligibleCount(r)} / ${Object.keys(r.laws).length}`" horizontal-alignment="right"></nldd-text-cell>
                </nldd-table-row>
              </nldd-table>
              </nldd-container>
            </nldd-card>
          </nldd-simple-section>
        </template>

        <!-- Comparison -->
        <nldd-simple-section v-else width="full">
          <nldd-container gap="16">
            <nldd-card accessible-label="Vergelijking">
              <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" text="Wie voldoet, per simulatie" :supporting-text="`${comparable.length} simulaties van dezelfde soort`"></nldd-title-cell></nldd-container>
              <nldd-container padding="12">
                <SimBarChart :categories="comparisonChart.categories" :series="comparisonChart.series" unit="percent" horizontal :height="`${Math.max(220, (18 * comparable.length + 12) * comparisonChart.categories.length + 60)}px`" />
              </nldd-container>
            </nldd-card>
            <nldd-table :columns="`minmax(160px, 1fr) repeat(${comparable.length}, 150px)`" accessible-label="Vergelijking per regeling">
              <nldd-table-row slot="header">
                <nldd-text-cell size="sm" text="Regeling"></nldd-text-cell>
                <nldd-text-cell v-for="r in comparable" :key="r.id" size="sm" :text="r.label" :supporting-text="Object.keys(r.overrides).length ? 'aangepaste parameters' : 'standaard'" horizontal-alignment="right"></nldd-text-cell>
              </nldd-table-row>
              <nldd-table-row v-for="law in comparisonLaws" :key="law.id">
                <nldd-text-cell size="sm" :text="law.name"></nldd-text-cell>
                <nldd-text-cell v-for="r in comparable" :key="r.id" size="sm" :text="pct(r.summary[law.id]?.eligiblePct)" :supporting-text="r.summary[law.id]?.withAmount ? fmtAmount(r, law.id, r.summary[law.id].avgAmount) : ''" horizontal-alignment="right"></nldd-text-cell>
              </nldd-table-row>
            </nldd-table>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane v-if="inspector && inspectorLaw" slot="inspector" has-content>
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="inspectorLaw.name" :supporting-text="inspector.type === 'params' ? 'Parameters voor deze simulatie' : serviceInfo(corpus, inspectorLaw.service).name" dismiss-text="Sluiten" @dismiss="inspector = null"></nldd-top-title-bar>
        </nldd-container>

        <!-- Law parameters -->
        <nldd-container v-if="inspector.type === 'params'" padding="12" gap="12">
          <nldd-rich-text><p>De constanten uit de wettekst (<code>definitions</code>). Een gewijzigde waarde geldt alleen voor de volgende simulatie; de wet in het corpus blijft zoals hij is.</p></nldd-rich-text>
          <nldd-form-field v-for="def in definitionsByLaw[inspectorLaw.id]" :key="def.key" :label="humanize(def.key)">
            <nldd-number-field :value="overrides[inspectorLaw.id]?.[def.key] ?? def.value" :step="Number.isInteger(def.value) ? '1' : '0.001'" width="full" hide-spin-buttons @change="setOverride(inspectorLaw.id, def.key, numberFrom($event))"></nldd-number-field>
            <nldd-form-field-help-text>Artikel {{ def.article }} · standaard {{ def.value }} · {{ definitionHint(def) }}</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-button v-if="overrideCount(inspectorLaw.id)" variant="secondary" start-icon="arrow-2-counter-clockwise" text="Terug naar de wet" @click="resetOverrides(inspectorLaw.id)"></nldd-button>
        </nldd-container>

        <!-- One law in the active run -->
        <nldd-container v-else-if="activeRun && activeRun.summary[inspector.lawId]" padding="12" gap="12">
          <nldd-list variant="box-tinted" accessible-label="Uitkomst">
            <nldd-list-item size="sm"><nldd-text-cell size="sm" color="secondary" text="Doorgerekend"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="num(activeRun.summary[inspector.lawId].evaluated)"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].hasEligibility" size="sm"><nldd-text-cell size="sm" color="secondary" text="Voldoet aan de voorwaarden"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="`${num(activeRun.summary[inspector.lawId].eligible)} (${pct(activeRun.summary[inspector.lawId].eligiblePct)})`"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].withAmount" size="sm"><nldd-text-cell size="sm" color="secondary" :text="`Gemiddeld · ${amountLabel(activeRun, inspector.lawId)}`"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="fmtAmount(activeRun, inspector.lawId, activeRun.summary[inspector.lawId].avgAmount)"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].withAmount && amountIsMoney(activeRun, inspector.lawId)" size="sm"><nldd-text-cell size="sm" color="secondary" text="Totaal"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="money(activeRun.summary[inspector.lawId].totalAmount)"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].errors" size="sm"><nldd-text-cell size="sm" color="secondary" text="Niet berekend"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="num(activeRun.summary[inspector.lawId].errors)"></nldd-text-cell></nldd-list-item>
          </nldd-list>
          <nldd-banner v-if="activeRun.summary[inspector.lawId].errors" variant="warning" text="Eerste fout" :supporting-text="activeRun.results.find((r) => r.laws[inspector.lawId] && !r.laws[inspector.lawId].ok)?.laws[inspector.lawId].error"></nldd-banner>
          <nldd-form-field label="Uitsplitsing naar">
            <nldd-dropdown width="full">
              <select :value="dimension.id" @change="dimensionId = $event.target.value">
                <option v-for="d in dimensions" :key="d.id" :value="d.id">{{ d.label }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <SimBarChart :categories="breakdownFor(inspector.lawId).map((r) => humanize(r.group))" :series="[{ name: 'Voldoet', values: breakdownFor(inspector.lawId).map((r) => r.eligiblePct) }]" unit="percent" height="220px" />
          <nldd-button variant="secondary" start-icon="book" text="Wettekst" @click="$router.push(`/wetten/${encodeURIComponent(inspector.lawId)}`)"></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
