<script setup>
import { computed, reactive, ref, shallowRef, watch } from 'vue';
import OrgLogo from '../components/OrgLogo.vue';
import SimBarChart from '../components/SimBarChart.vue';
import { fieldSpec, formatValue, humanize, intlLocale } from '../data/format.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { BUSINESS_DEFAULTS, CITIZEN_DEFAULTS, MAX_POPULATION } from '../simulation/population.js';
import { definitionKind, overridableDefinitions } from '../simulation/lawParameters.js';
import { runSimulation, simulationLaws } from '../simulation/runner.js';
import { BUSINESS_DIMENSIONS, CITIZEN_DIMENSIONS, breakdown, dimensionLabel, flattenResults, toCsv } from '../simulation/stats.js';
import { disposableIncomeBreakdown, summariseDisposableIncome } from '../simulation/income.js';
import { describeModel, featureLabel, featuresFor, taxLawIds, trainBracketModel, trainingData } from '../simulation/harmonize.js';
import { useDemo } from '../store/demoStore.js';
import { useNarrow } from '../useNarrow.js';
import { useI18n } from '../i18n/index.js';
import { useLocalePath } from '../i18n/useLocalePath.js';

const { t } = useI18n();

// Naar een ander tabblad op naam, niet op pad: onder `/en/` leidt een
// letterlijk Nederlands pad de bezoeker ongemerkt het Nederlandse tabblad in.
const { goTo } = useLocalePath();

// Simulatie: a synthetic population of citizens or businesses, every portal
// law evaluated for each of them by the same engine the portal uses, and the
// outcome in numbers: who qualifies, for how much, split by age, income, type
// of business. Law constants can be changed for a run ("what if the threshold
// goes up?"), and runs sit side by side for comparison.

const demo = useDemo();
const { corpus, engine, ready, state, profile, features } = demo;

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

// De knoppen dragen sleutels, geen zinnen: ze worden hier één keer opgebouwd,
// en een letterlijk label zou na een taalwissel in de oude taal blijven staan.
const CITIZEN_KNOBS = [
  { groupKey: 'sim.knobs.age_distribution', fields: ['18-30', '30-45', '45-67', '67-85', '85+'].map((k) => ({ path: ['ageDistribution', k], labelKey: 'sim.knobs.age_band', vars: { band: k } })) },
  { groupKey: 'sim.knobs.income_distribution', fields: [['low', 'sim.knobs.income.low'], ['middle', 'sim.knobs.income.middle'], ['high', 'sim.knobs.income.high']].map(([k, labelKey]) => ({ path: ['incomeDistribution', k], labelKey })) },
  { groupKey: 'sim.knobs.household', fields: [{ path: ['zeroIncomePct'], labelKey: 'sim.knobs.no_income' }, { path: ['renterPct'], labelKey: 'sim.knobs.renters' }, { path: ['studentPct'], labelKey: 'sim.knobs.students' }] },
];
const BUSINESS_KNOBS = [
  { groupKey: 'sim.knobs.business_type', fields: [{ path: ['horecaPct'], labelKey: 'sim.knobs.hospitality' }, { path: ['foodPct'], labelKey: 'sim.knobs.food' }, { path: ['terracePct'], labelKey: 'sim.knobs.terrace' }, { path: ['employeesPct'], labelKey: 'sim.knobs.employees' }] },
  { groupKey: 'sim.knobs.floor_area', fields: [['small', 'sim.knobs.size.small'], ['medium', 'sim.knobs.size.medium'], ['large', 'sim.knobs.size.large']].map(([k, labelKey]) => ({ path: ['sizeDistribution', k], labelKey })) },
];
const knobs = computed(() =>
  (kind.value === 'ondernemers' ? BUSINESS_KNOBS : CITIZEN_KNOBS).map((g) => ({
    key: g.groupKey,
    group: t(g.groupKey),
    fields: g.fields.map((f) => ({ ...f, label: t(f.labelKey, f.vars) })),
  })),
);
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
  // Op een smal scherm bedekt de sheet het hoofdpaneel, dus de voortgang en de
  // uitkomst zouden erachter verdwijnen.
  splitView.value?.hidePrimarySidebarSheet?.();
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
    result.label = `${runs.value.length + 1}. ${t(kind.value === 'ondernemers' ? 'sim.kind.businesses' : 'sim.kind.citizens')} (${result.subjects.length})`;
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

// ---- harmonisatie -------------------------------------------------------------
// Uit de uitkomsten van deze run één vereenvoudigde regeling afleiden die de
// gekozen wetten benadert, en laten zien hoe dicht die komt. Achter een vlag
// per profiel (de POC's FEATURE_HARMONIZE).
const harmonizeEnabled = computed(() => features.value.HARMONIZE);
/** De wetten die samengenomen worden; standaard alles met een bedrag. */
const harmonizeLaws = ref([]);
const harmonizePrimary = ref('inkomen');
const harmonizeBrackets = ref(5);
const harmonizeModel = shallowRef(null);
const harmonizeError = ref('');

/** Alleen wetten die geld opleveren zijn te harmoniseren. */
const harmonizableLaws = computed(() => {
  const run = activeRun.value;
  if (!run) return [];
  return run.laws.filter((law) => run.summary[law.id]?.withAmount > 0);
});
const harmonizeFeatures = computed(() => featuresFor(activeRun.value?.kind ?? 'burgers'));
const harmonizeNumericFeatures = computed(() => harmonizeFeatures.value.filter((f) => f.kind === 'number'));

// Een nieuwe run betekent een nieuw model; de oude uitkomst hoort niet bij
// deze cijfers.
watch(activeRun, (run) => {
  harmonizeModel.value = null;
  harmonizeError.value = '';
  harmonizeLaws.value = harmonizableLaws.value.map((l) => l.id);
  const numeric = harmonizeNumericFeatures.value;
  if (!numeric.some((f) => f.key === harmonizePrimary.value)) harmonizePrimary.value = numeric[0]?.key ?? 'inkomen';
  if (run?.kind === 'ondernemers' && harmonizePrimary.value === 'inkomen') harmonizePrimary.value = numeric[0]?.key ?? 'oppervlakte';
});

function toggleHarmonizeLaw(id) {
  harmonizeLaws.value = harmonizeLaws.value.includes(id)
    ? harmonizeLaws.value.filter((x) => x !== id)
    : [...harmonizeLaws.value, id];
}

function harmonize() {
  harmonizeError.value = '';
  harmonizeModel.value = null;
  try {
    if (!harmonizeLaws.value.length) throw new Error(t('sim.harmonise.choose_law'));
    // Een belasting telt negatief mee: wat de burger overhoudt is het saldo.
    const data = trainingData(activeRun.value, harmonizeLaws.value, taxLawIds(corpus.value));
    harmonizeModel.value = trainBracketModel(data, {
      primary: harmonizePrimary.value,
      brackets: Number(harmonizeBrackets.value) || 5,
    });
  } catch (e) {
    harmonizeError.value = String(e?.message ?? e);
  }
}

const harmonizeTable = computed(() => (harmonizeModel.value ? describeModel(harmonizeModel.value, money) : []));
/**
 * Hoeveel de vereenvoudiging gemiddeld scheelt, als aandeel van het bedrag.
 *
 * Tegen de absolute waarde van het gemiddelde: zit er een belasting in de som,
 * dan is het saldo negatief (men betaalt per saldo), en een percentage van een
 * negatief getal leest als een fout die het niet is.
 */
const harmonizeRelativeError = computed(() => {
  const m = harmonizeModel.value?.metrics;
  if (!m?.meanAmount) return null;
  return (m.mae / Math.abs(m.meanAmount)) * 100;
});
/** Betaalt de gemiddelde persoon per saldo, in plaats van te ontvangen? */
const harmonizeNetCost = computed(() => (harmonizeModel.value?.metrics?.meanAmount ?? 0) < 0);

// ---- inspector ----------------------------------------------------------------
// De instellingen staan op een smal scherm in een sheet; na het starten van
// een run moet die dicht, anders bedekt hij de uitkomst waar het om gaat.
// Op een breed scherm blijft het paneel gewoon staan (zie de split view in de
// template), dus daar is de knop overbodig.
const splitView = ref(null);
const narrow = useNarrow();

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
  if (k === 'eurocent') return t('sim.inspector.hint.eurocent', { amount: formatValue(def.value, { type: 'amount' }) });
  if (k === 'percentage') {
    return def.value <= 1
      ? t('sim.inspector.hint.fraction', { pct: (def.value * 100).toLocaleString(intlLocale(), { maximumFractionDigits: 3 }) })
      : t('sim.inspector.hint.percentage');
  }
  return t('sim.inspector.hint.number');
}

// ---- presentation helpers -----------------------------------------------------
const euro = { format: (v) => new Intl.NumberFormat(intlLocale(), { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 }).format(v) };
const pct = (v) => (v === null || v === undefined ? '–' : `${Math.round(v)}%`);
const money = (v) => (v === null || v === undefined ? '–' : euro.format(v));
const num = (v, digits = 0) => (v === null || v === undefined ? '–' : new Intl.NumberFormat(intlLocale(), { maximumFractionDigits: digits }).format(v));

function amountLabel(run, lawId) {
  const law = run.laws.find((l) => l.id === lawId);
  const name = run.results.find((r) => r.laws[lawId]?.amountName)?.laws[lawId]?.amountName ?? law?.primaryName;
  return name ? humanize(name) : t('sim.amount.fallback');
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
  series: [{ name: t('sim.eligible.short'), values: eligibleRows.value.map((l) => l.s.eligiblePct) }],
}));
function outcomeLine(run, law) {
  const s = law.s;
  const head = s.hasEligibility
    ? t('sim.outcome.eligible', { eligible: s.eligible, evaluated: s.evaluated })
    : t('sim.outcome.computed', { n: s.evaluated });
  const amount = s.withAmount ? t('sim.outcome.average', { amount: fmtAmount(run, law.id, s.avgAmount) }) : '';
  const undecided = s.undecided ? t('sim.outcome.undecided', { n: s.undecided }) : '';
  const errors = s.errors ? t('sim.outcome.errors', { n: s.errors }) : '';
  return head + amount + undecided + errors;
}
const amountChart = computed(() => {
  const rows = lawRows.value.filter((l) => l.s.withAmount > 0 && amountIsMoney(activeRun.value, l.id));
  return { categories: rows.map((l) => l.name), series: [{ name: t('sim.amount.title'), values: rows.map((l) => l.s.avgAmount) }] };
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
      [t('sim.population.businesses_count'), num(p.count)],
      [t('sim.population.hospitality'), pct(p.horecaPct)],
      [t('sim.population.prepares_food'), pct(p.voedselPct)],
      [t('sim.population.terrace'), pct(p.terrasPct)],
      [t('sim.population.avg_floor_area'), `${num(p.gemOppervlakte)} m²`],
      [t('sim.population.with_employees'), pct(p.metWerknemersPct)],
    ];
  }
  return [
    [t('sim.population.persons'), num(p.count)],
    [t('sim.population.avg_age'), t('format.years', { n: num(p.gemLeeftijd) })],
    [t('sim.population.with_partner'), pct(p.partnerPct)],
    [t('sim.population.renters'), pct(p.huurderPct)],
    [t('sim.population.with_children'), pct(p.kinderenPct)],
    [t('sim.population.students'), pct(p.studentPct)],
    [t('sim.population.avg_income'), money(p.gemInkomen)],
    [t('sim.population.median_income'), money(p.medInkomen)],
  ];
});
// De kolomkoppen zijn veldnamen, dus ze gaan door `humanize()` en de
// woordenlijst — op `huurder` na, die daar niet in staat, en op de twee koppen
// die bewust korter zijn dan de veldnaam.
const subjectColumns = computed(() => {
  if (!activeRun.value) return [];
  return activeRun.value.kind === 'ondernemers'
    ? [['kvk_nummer', humanize('kvk_nummer')], ['type', humanize('type')], ['oppervlakte', 'm²'], ['rechtsvorm', humanize('rechtsvorm')], ['werknemers', humanize('werknemers')], ['terras_m2', `${humanize('terras')} m²`], ['voedsel', humanize('voedsel')]]
    : [['bsn', humanize('bsn')], ['leeftijd', humanize('leeftijd')], ['inkomen', humanize('inkomen')], ['partner', humanize('partner')], ['kinderen', humanize('kinderen')], ['huurder', t('sim.subjects.renter')], ['student', humanize('student')]];
});
function subjectCell(subject, key) {
  const v = subject[key];
  if (key === 'inkomen') return money(v);
  return formatValue(v);
}
function eligibleCount(result) {
  return Object.values(result.laws).filter((l) => l.ok && (l.met === true || l.met === null)).length;
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
  <!-- Alleen op een smal scherm wordt het zijpaneel een sheet. Daar viel het
       anders helemaal weg en was er geen enkele manier meer om een simulatie
       te starten. Op een breed scherm blijft het staan, anders dan bij Wetten
       en Scenario's: tijdens een demo stel je hier voortdurend parameters bij,
       en dat zou telkens een extra klik kosten. -->
  <nldd-navigation-split-view
    ref="splitView"
    :primary-sidebar-as-sheet="narrow || undefined"
    :primary-sidebar-accessible-label="t('sim.settings.label')"
  >
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <!-- Kop, keuze en zoekveld horen in de header, zoals Wetten het doet: de
           burger/ondernemer-keuze is de hoofdknop van dit paneel, want alles
           eronder gaat over de gekozen soort.
           `sticky-header` staat er bewust niet op. Een plakkende header zweeft
           over de scroll-container (gemeten: 61px, en precies evenveel bij
           Wetten), wat daar onzichtbaar blijft omdat een lijst begint waar hier
           meteen een invoerveld staat. -->
      <nldd-page background="inherit">
        <nldd-container slot="header" padding="12" gap="8">
          <nldd-top-title-bar :text="t('sim.title')" :supporting-text="t('sim.title.laws', { n: lawSet.runnable.length })"></nldd-top-title-bar>
          <nldd-segmented-control width="full" :value="kind" @change="kind = $event.detail?.value ?? kind">
            <nldd-segmented-control-item value="burgers" :text="t('sim.kind.citizens')" icon="person"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="ondernemers" :text="t('sim.kind.businesses')" icon="business-suitcase"></nldd-segmented-control-item>
          </nldd-segmented-control>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-form-field :label="t(kind === 'ondernemers' ? 'sim.count.businesses' : 'sim.count.citizens')">
            <nldd-number-field :value="params.count" min="1" :max="MAX_POPULATION" step="10" width="full" @change="params.count = numberFrom($event) ?? params.count"></nldd-number-field>
          </nldd-form-field>
          <nldd-form-field :label="t('sim.reference_date')">
            <nldd-date-field :value="referenceDate" width="full" @change="referenceDate = $event.detail?.value || referenceDate"></nldd-date-field>
          </nldd-form-field>

          <nldd-button width="full" variant="neutral-transparent" horizontal-alignment="left" :end-icon="open.populatie ? 'chevron-up' : 'chevron-down'" :text="t(kind === 'ondernemers' ? 'sim.population.businesses' : 'sim.population.citizens')" :expanded="open.populatie || undefined" @click="open.populatie = !open.populatie"></nldd-button>
          <template v-if="open.populatie">
            <nldd-form-field :label="t('sim.seed')">
              <nldd-number-field size="sm" :value="params.seed" min="1" step="1" width="full" hide-spin-buttons @change="params.seed = numberFrom($event) ?? params.seed"></nldd-number-field>
              <nldd-form-field-help-text>{{ t('sim.seed.help') }}</nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-container v-for="group in knobs" :key="group.key" gap="8">
              <nldd-text-cell size="sm" color="secondary" :text="group.group"></nldd-text-cell>
              <nldd-form-field v-for="field in group.fields" :key="field.path.join('.')" :label="field.label">
                <nldd-number-field size="sm" :value="getKnob(field.path)" min="0" max="100" step="5" width="full" hide-spin-buttons @change="setKnob(field.path, numberFrom($event))"></nldd-number-field>
              </nldd-form-field>
            </nldd-container>
          </template>

          <nldd-button width="full" variant="neutral-transparent" horizontal-alignment="left" :end-icon="open.wetgeving ? 'chevron-up' : 'chevron-down'" :text="totalOverrides ? t('sim.params.title_changed', { n: totalOverrides }) : t('sim.params.title')" :expanded="open.wetgeving || undefined" @click="open.wetgeving = !open.wetgeving"></nldd-button>
          <nldd-list v-if="open.wetgeving" variant="box-tinted" :accessible-label="t('sim.params.list_label')">
            <nldd-list-item v-for="law in lawSet.runnable" :key="law.id" size="sm" button :disabled="definitionsByLaw[law.id].length === 0 || undefined" @click="editParameters(law)">
              <nldd-cell><OrgLogo :service="law.service" size="sm" /></nldd-cell>
              <nldd-spacer-cell size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="law.name" :supporting-text="t('sim.params.count', { n: definitionsByLaw[law.id].length })"></nldd-text-cell>
              <nldd-cell v-if="overrideCount(law.id)"><nldd-tag size="sm" color="warning" :text="t('sim.params.changed', { n: overrideCount(law.id) })"></nldd-tag></nldd-cell>
              <nldd-icon-cell v-else-if="definitionsByLaw[law.id].length" icon="chevron-right" color="secondary"></nldd-icon-cell>
            </nldd-list-item>
          </nldd-list>

          <nldd-button width="full" variant="primary" start-icon="play" :text="t(running ? 'sim.run.busy' : 'sim.run')" :disabled="!ready || running || undefined" @click="run"></nldd-button>
          <template v-if="running">
            <nldd-progress-bar :value="progress.done" :max="progress.total || 1" value-format="fraction" value-display="inline" :text="t(kind === 'ondernemers' ? 'sim.run.progress.businesses' : 'sim.run.progress.citizens')"></nldd-progress-bar>
            <nldd-button width="full" variant="secondary" size="sm" :text="t('sim.run.stop')" @click="cancel"></nldd-button>
          </template>
          <nldd-banner v-if="runError" variant="critical" :text="t('sim.run.failed')" :supporting-text="runError"></nldd-banner>
          <nldd-rich-text v-if="lawSet.skipped.length" size="sm">
            <p><small>{{ t('sim.skipped', { laws: lawSet.skipped.map((s) => t('sim.skipped.law', { name: s.law.name, missing: s.missing.join(', ') })).join('; ') }) }}</small></p>
          </nldd-rich-text>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <!-- De koptekst staat er altijd, ook zonder run: hij draagt de knop
             naar de instellingen, en die is op een smal scherm de enige
             ingang naar het zijpaneel. Stond hij achter `runs.length`, dan
             was er zonder simulatie geen enkele manier om er een te
             starten. -->
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start" v-if="narrow">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="settings" :text="t('sim.settings.open')" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="start" v-if="runs.length">
              <nldd-tab-bar size="sm" @tabchange="onTab">
                <nldd-tab-bar-item v-for="r in runs" :key="r.id" :data-run="r.id" :current="activeTab === r.id || undefined" :text="r.label"></nldd-tab-bar-item>
                <nldd-tab-bar-item v-if="runs.length > 1" data-run="vergelijking" :current="activeTab === 'vergelijking' || undefined" :text="t('sim.tab.comparison')" icon="arrow-left-right"></nldd-tab-bar-item>
              </nldd-tab-bar>
            </nldd-toolbar-item>
            <template v-if="activeRun">
              <nldd-toolbar-item slot="end">
                <nldd-segmented-control size="sm" width="fit-content" :value="mainView" @change="mainView = $event.detail?.value ?? mainView">
                  <nldd-segmented-control-item value="overzicht" :text="t('sim.view.overview')"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="uitsplitsing" :text="t('sim.view.breakdown')"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="populatie" :text="t('sim.view.population')"></nldd-segmented-control-item>
                  <nldd-segmented-control-item v-if="harmonizeEnabled" value="harmonisatie" :text="t('sim.view.harmonisation')"></nldd-segmented-control-item>
                </nldd-segmented-control>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end">
                <nldd-icon-button size="sm" variant="neutral-tinted" icon="menu" :text="t('sim.export')" tooltip-timing="never" expandable>
                  <nldd-menu>
                    <nldd-menu-item text="CSV" icon="file-text" @click="exportCsv"></nldd-menu-item>
                    <nldd-menu-item text="JSON" icon="brackets-ellipsis" @click="exportJson"></nldd-menu-item>
                    <nldd-menu-divider></nldd-menu-divider>
                    <nldd-menu-item :text="t('sim.export.delete')" icon="trash" @click="removeRun(activeRun.id)"></nldd-menu-item>
                  </nldd-menu>
                </nldd-icon-button>
              </nldd-toolbar-item>
            </template>
          </nldd-toolbar>
        </nldd-container>

        <!-- Harmonisatie rekent op de uitkomsten van een run, dus zonder run is
             er niets te harmoniseren. Dat hier zeggen, want anders lijkt de
             feature niet te werken terwijl de vlag aan staat. -->
        <nldd-simple-section v-if="!runs.length" height="60vh">
          <nldd-inline-dialog
            icon="chart-x-y-axis-line"
            :text="t('sim.empty.title')"
            :supporting-text="t('sim.empty.body', { where: t(narrow ? 'sim.empty.where.narrow' : 'sim.empty.where.wide'), harmonize: harmonizeEnabled ? t('sim.empty.harmonize') : '' })"
          ></nldd-inline-dialog>
        </nldd-simple-section>

        <!-- One run -->
        <template v-else-if="activeRun">
          <nldd-simple-section v-if="mainView === 'overzicht'" width="full">
            <nldd-container gap="16">
              <nldd-banner v-if="activeRun.cancelled" variant="warning" :text="t('sim.cancelled')" :supporting-text="t('sim.cancelled.body', { done: activeRun.results.length, total: activeRun.subjects.length })"></nldd-banner>
              <nldd-banner v-if="Object.keys(activeRun.overrides).length" variant="accent" :text="t('sim.overrides.title')" :supporting-text="Object.entries(activeRun.overrides).map(([id, o]) => `${corpus.lawById(id)?.name ?? id}: ${Object.entries(o).map(([k, v]) => `${k} = ${v}`).join(', ')}`).join(' · ')"></nldd-banner>

              <nldd-card :accessible-label="t('sim.population.title')">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.population.title')" :supporting-text="t('sim.population.meta', { date: formatValue(activeRun.referenceDate), seed: activeRun.params.seed, seconds: (activeRun.durationMs / 1000).toFixed(1) })"></nldd-title-cell></nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12">
                  <nldd-list variant="simple" :accessible-label="t('sim.population.facts_label')">
                    <nldd-list-item v-for="[label, value] in populationFacts" :key="label" size="sm">
                      <nldd-text-cell size="sm" color="secondary" :text="label"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="value"></nldd-text-cell>
                    </nldd-list-item>
                  </nldd-list>
                </nldd-container>
              </nldd-card>

              <nldd-card v-if="disposable" background="tinted" :accessible-label="t('sim.disposable.title')">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center">
                  <nldd-icon name="euro-sign" size="24"></nldd-icon>
                  <nldd-title-cell size="4" :text="t('sim.disposable.title')" :supporting-text="t('sim.disposable.lead')"></nldd-title-cell>
                </nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12" gap="12">
                  <nldd-container layout="grid" column-count="3" sm-column-count="1" gap="12">
                    <nldd-list variant="box-base" :accessible-label="t('sim.disposable.avg')"><nldd-list-item size="md"><nldd-title-cell size="3" :overline="t('sim.disposable.avg')" :text="money(disposable.avgDisposable)"></nldd-title-cell></nldd-list-item></nldd-list>
                    <nldd-list variant="box-base" :accessible-label="t('sim.disposable.median')"><nldd-list-item size="md"><nldd-title-cell size="3" :overline="t('sim.disposable.median')" :text="money(disposable.medianDisposable)"></nldd-title-cell></nldd-list-item></nldd-list>
                    <nldd-list variant="box-base" :accessible-label="t('sim.disposable.after_housing')"><nldd-list-item size="md"><nldd-title-cell size="3" :overline="t('sim.disposable.after_housing')" :text="money(disposable.avgAfterHousing)" :supporting-text="t('sim.disposable.housing_cost', { amount: money(disposable.avgHousing) })"></nldd-title-cell></nldd-list-item></nldd-list>
                  </nldd-container>
                  <nldd-list variant="box-base" :accessible-label="t('sim.disposable.build_up')">
                    <nldd-list-item size="sm">
                      <nldd-text-cell size="sm" :text="t('sim.disposable.income')" :supporting-text="t('sim.disposable.income.help')"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="money(disposable.avgIncome)"></nldd-text-cell>
                    </nldd-list-item>
                    <nldd-list-item v-for="c in componentRows" :key="`${c.component.law}#${c.component.output}`" size="sm">
                      <nldd-text-cell size="sm" :text="componentLabel(c)" :supporting-text="t('sim.disposable.component.meta', { n: c.withValue, total: disposable.count, output: humanize(c.component.output) })"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="c.component.kind === 'tax' ? 'critical' : 'success'" :text="`${c.component.kind === 'tax' ? '−' : '+'} ${money(c.avg)}`"></nldd-text-cell>
                    </nldd-list-item>
                    <nldd-list-item size="sm">
                      <nldd-text-cell size="sm" :text="t('sim.disposable.total')" :supporting-text="t('sim.disposable.total.help')"></nldd-text-cell>
                      <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="`**${money(disposable.avgDisposable)}**`"></nldd-text-cell>
                    </nldd-list-item>
                  </nldd-list>
                </nldd-container>
              </nldd-card>

              <nldd-card :accessible-label="t('sim.eligible.short')">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.eligible.title')" :supporting-text="t('sim.eligible.lead')"></nldd-title-cell></nldd-container>
                <nldd-container padding="12">
                  <SimBarChart :categories="eligibleChart.categories" :series="eligibleChart.series" unit="percent" horizontal :height="`${Math.max(200, 34 * eligibleChart.categories.length + 40)}px`" />
                </nldd-container>
              </nldd-card>

              <nldd-card v-if="amountChart.categories.length" :accessible-label="t('sim.amount.title')">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.amount.title')" :supporting-text="t('sim.amount.lead')"></nldd-title-cell></nldd-container>
                <nldd-container padding="12">
                  <SimBarChart :categories="amountChart.categories" :series="amountChart.series" unit="euro" horizontal :height="`${Math.max(160, 34 * amountChart.categories.length + 40)}px`" />
                </nldd-container>
              </nldd-card>

              <nldd-list variant="box-tinted" :accessible-label="t('sim.laws.label')">
                <nldd-list-item v-for="law in lawRows" :key="law.id" size="md" button @click="showLaw(law.id)">
                  <nldd-cell><OrgLogo :service="law.service" /></nldd-cell>
                  <nldd-spacer-cell size="12"></nldd-spacer-cell>
                  <nldd-text-cell :text="law.name" :supporting-text="outcomeLine(activeRun, law)"></nldd-text-cell>
                  <nldd-cell v-if="law.s.hasEligibility"><nldd-tag size="sm" :color="law.s.eligiblePct >= 50 ? 'success' : law.s.eligiblePct > 0 ? 'neutral' : 'critical'" :text="pct(law.s.eligiblePct)"></nldd-tag></nldd-cell>
                  <nldd-cell v-else><nldd-tag size="sm" color="neutral" :text="t('sim.law.computed')"></nldd-tag></nldd-cell>
                  <!-- Een tag pal tegen de chevron leest als een knop; scheid ze. -->
                  <nldd-spacer-cell size="8"></nldd-spacer-cell>
                  <nldd-icon-cell icon="chevron-right" color="secondary"></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
            </nldd-container>
          </nldd-simple-section>

          <nldd-simple-section v-else-if="mainView === 'uitsplitsing'" width="full">
            <nldd-container gap="16">
              <nldd-segmented-control size="sm" width="fit-content" :value="dimension.id" @change="dimensionId = $event.detail?.value ?? dimensionId">
                <nldd-segmented-control-item v-for="d in dimensions" :key="d.id" :value="d.id" :text="dimensionLabel(d)"></nldd-segmented-control-item>
              </nldd-segmented-control>
              <nldd-card v-if="disposable" background="tinted" :accessible-label="t('sim.disposable.per_group')">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center">
                  <nldd-icon name="euro-sign" size="24"></nldd-icon>
                  <nldd-title-cell size="5" :text="t('sim.disposable.title')" :supporting-text="t('sim.disposable.per_month_by', { dimension: dimensionLabel(dimension).toLowerCase() })"></nldd-title-cell>
                </nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12">
                  <nldd-table columns="minmax(140px, 1fr) 90px 130px 130px 130px" :accessible-label="t('sim.disposable.per_group')">
                    <nldd-table-row slot="header">
                      <nldd-text-cell size="sm" :text="dimensionLabel(dimension)"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="t('sim.breakdown.persons')" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="t('sim.breakdown.average')" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="t('sim.breakdown.median')" horizontal-alignment="right"></nldd-text-cell>
                      <nldd-text-cell size="sm" :text="t('sim.breakdown.after_housing')" horizontal-alignment="right"></nldd-text-cell>
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
                  <nldd-title-cell size="5" :text="law.name" :supporting-text="t('sim.breakdown.law_meta', { outcome: law.s.hasEligibility ? t('sim.breakdown.law_eligible', { pct: pct(law.s.eligiblePct) }) : t('sim.breakdown.law_all'), dimension: dimensionLabel(dimension).toLowerCase() })"></nldd-title-cell>
                </nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12">
                <nldd-table columns="minmax(140px, 1fr) 110px 90px 130px" :accessible-label="t('sim.breakdown.table_label', { name: law.name, dimension: dimensionLabel(dimension) })">
                  <nldd-table-row slot="header">
                    <nldd-text-cell size="sm" :text="dimensionLabel(dimension)"></nldd-text-cell>
                    <nldd-text-cell size="sm" :text="t('sim.breakdown.eligible')" horizontal-alignment="right"></nldd-text-cell>
                    <nldd-text-cell size="sm" :text="t('sim.breakdown.share')" horizontal-alignment="right"></nldd-text-cell>
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

          <nldd-simple-section v-else-if="mainView === 'populatie'" width="full">
            <nldd-card :accessible-label="t('sim.population.title')">
              <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.population.title')" :supporting-text="t('sim.subjects.meta', { shown: Math.min(activeRun.results.length, 200), total: activeRun.results.length })"></nldd-title-cell></nldd-container>
              <nldd-container padding-inline="12" padding-bottom="12">
              <nldd-table :columns="`repeat(${subjectColumns.length}, minmax(90px, 1fr)) 90px`" :accessible-label="t('sim.subjects.table_label')">
                <nldd-table-row slot="header">
                  <nldd-text-cell v-for="[key, label] in subjectColumns" :key="key" size="sm" :text="label"></nldd-text-cell>
                  <nldd-text-cell size="sm" :text="t('sim.breakdown.eligible')" horizontal-alignment="right"></nldd-text-cell>
                </nldd-table-row>
                <nldd-table-row v-for="r in activeRun.results.slice(0, 200)" :key="r.subject.id">
                  <nldd-text-cell v-for="[key] in subjectColumns" :key="key" size="sm" :text="subjectCell(r.subject, key)"></nldd-text-cell>
                  <nldd-text-cell size="sm" :text="`${eligibleCount(r)} / ${Object.keys(r.laws).length}`" horizontal-alignment="right"></nldd-text-cell>
                </nldd-table-row>
              </nldd-table>
              </nldd-container>
            </nldd-card>
          </nldd-simple-section>

          <!-- Harmonisatie: uit de uitkomsten van deze run één vereenvoudigde
               regeling afleiden, en laten zien hoe dicht die komt. Het verschil
               is het interessante deel: waar de staffel de wet niet kan volgen,
               zit de regel die niemand meer uitlegt. -->
          <nldd-simple-section v-else-if="mainView === 'harmonisatie'" width="full">
            <nldd-container gap="16">
              <nldd-card :accessible-label="t('sim.harmonise.settings_label')">
                <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.harmonise.title')" :supporting-text="t('sim.harmonise.lead')"></nldd-title-cell></nldd-container>
                <nldd-container padding-inline="12" padding-bottom="12" gap="12">
                  <nldd-form-field :label="t('sim.harmonise.laws')">
                    <nldd-container layout="wrap" gap="8">
                      <!-- checkbox-field, niet checkbox: die laatste is alleen
                           het vinkje en toont zijn label niet. -->
                      <nldd-checkbox-field
                        v-for="law in harmonizableLaws"
                        :key="law.id"
                        :label="law.name"
                        :checked="harmonizeLaws.includes(law.id) || undefined"
                        @change="toggleHarmonizeLaw(law.id)"
                      ></nldd-checkbox-field>
                    </nldd-container>
                    <nldd-form-field-help-text v-if="!harmonizableLaws.length">{{ t('sim.harmonise.no_laws') }}</nldd-form-field-help-text>
                  </nldd-form-field>
                  <nldd-container layout="row" gap="12" layout-wrap>
                    <nldd-form-field :label="t('sim.harmonise.primary')">
                      <nldd-dropdown>
                        <select :value="harmonizePrimary" @change="harmonizePrimary = $event.target.value">
                          <option v-for="f in harmonizeNumericFeatures" :key="f.key" :value="f.key">{{ featureLabel(f) }}</option>
                        </select>
                      </nldd-dropdown>
                    </nldd-form-field>
                    <nldd-form-field :label="t('sim.harmonise.brackets')">
                      <nldd-number-field :value="String(harmonizeBrackets)" hide-spin-buttons min="2" max="10" @change="harmonizeBrackets = $event.detail?.value ?? harmonizeBrackets"></nldd-number-field>
                    </nldd-form-field>
                  </nldd-container>
                  <nldd-banner v-if="harmonizeError" variant="critical" :text="harmonizeError"></nldd-banner>
                  <nldd-form-actions>
                    <nldd-button variant="primary" start-icon="play" :text="t('sim.harmonise.run')" :disabled="!harmonizableLaws.length || undefined" @click="harmonize"></nldd-button>
                  </nldd-form-actions>
                </nldd-container>
              </nldd-card>

              <template v-if="harmonizeModel">
                <nldd-card :accessible-label="t('sim.harmonise.fit_label')">
                  <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.harmonise.fit_title')" :supporting-text="t('sim.harmonise.fit_lead')"></nldd-title-cell></nldd-container>
                  <nldd-container padding-inline="12" padding-bottom="12">
                    <nldd-list variant="box-tinted" :accessible-label="t('sim.harmonise.metrics_label')">
                      <nldd-list-item size="md">
                        <nldd-text-cell :text="t('sim.harmonise.mae')" :supporting-text="t('sim.harmonise.mae.help')"></nldd-text-cell>
                        <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="money(harmonizeModel.metrics.mae)"></nldd-text-cell>
                      </nldd-list-item>
                      <nldd-list-item v-if="harmonizeRelativeError !== null" size="md">
                        <nldd-text-cell :text="t('sim.harmonise.relative')" :supporting-text="harmonizeNetCost ? t('sim.harmonise.relative.cost', { amount: money(-harmonizeModel.metrics.meanAmount) }) : t('sim.harmonise.relative.grant', { amount: money(harmonizeModel.metrics.meanAmount) })"></nldd-text-cell>
                        <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="`${num(harmonizeRelativeError, 1)}%`"></nldd-text-cell>
                      </nldd-list-item>
                      <nldd-list-item size="md">
                        <nldd-text-cell :text="t('sim.harmonise.r2')" :supporting-text="t('sim.harmonise.r2.help')"></nldd-text-cell>
                        <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="num(harmonizeModel.metrics.r2, 3)"></nldd-text-cell>
                      </nldd-list-item>
                    </nldd-list>
                  </nldd-container>
                </nldd-card>

                <nldd-card v-for="(group, gi) in harmonizeTable" :key="gi" :accessible-label="t('sim.harmonise.bracket_label')">
                  <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="group.group" :supporting-text="t('sim.harmonise.bracket_meta', { count: group.count, total: activeRun.results.length })"></nldd-title-cell></nldd-container>
                  <nldd-container padding-inline="12" padding-bottom="12">
                    <nldd-table columns="2fr 1fr 1fr" :accessible-label="t('sim.harmonise.bracket_table')">
                      <nldd-table-row slot="header">
                        <nldd-text-cell size="sm" :text="featureLabel(harmonizeModel.primary)"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="t('sim.harmonise.from')" horizontal-alignment="right"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="t('sim.harmonise.to')" horizontal-alignment="right"></nldd-text-cell>
                      </nldd-table-row>
                      <nldd-table-row v-for="(step, si) in group.steps" :key="si">
                        <nldd-text-cell size="sm" :text="step.range"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="step.from" horizontal-alignment="right"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="step.to" horizontal-alignment="right"></nldd-text-cell>
                      </nldd-table-row>
                    </nldd-table>
                  </nldd-container>
                </nldd-card>

                <nldd-card :accessible-label="t('sim.harmonise.worst_label')">
                  <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.harmonise.worst_title')" :supporting-text="t('sim.harmonise.worst_lead')"></nldd-title-cell></nldd-container>
                  <nldd-container padding-inline="12" padding-bottom="12">
                    <nldd-table columns="2fr 1fr 1fr 1fr" :accessible-label="t('sim.harmonise.worst_table')">
                      <nldd-table-row slot="header">
                        <nldd-text-cell size="sm" :text="t('sim.harmonise.person')"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="t('sim.harmonise.by_law')" horizontal-alignment="right"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="t('sim.harmonise.by_model')" horizontal-alignment="right"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="t('sim.harmonise.difference')" horizontal-alignment="right"></nldd-text-cell>
                      </nldd-table-row>
                      <nldd-table-row v-for="(w, wi) in harmonizeModel.metrics.worst" :key="wi">
                        <nldd-text-cell size="sm" :text="harmonizeFeatures.filter((f) => f.kind === 'boolean' && w.values[f.key]).map((f) => featureLabel(f)).join(', ') || '—'" :supporting-text="`${featureLabel(harmonizeModel.primary).toLowerCase()}: ${num(w.values[harmonizeModel.primary.key])}`"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="money(w.actual)" horizontal-alignment="right"></nldd-text-cell>
                        <nldd-text-cell size="sm" :text="money(w.predicted)" horizontal-alignment="right"></nldd-text-cell>
                        <nldd-text-cell size="sm" :color="Math.abs(w.error) > 100 ? 'critical' : 'content'" :text="money(w.error)" horizontal-alignment="right"></nldd-text-cell>
                      </nldd-table-row>
                    </nldd-table>
                  </nldd-container>
                </nldd-card>
              </template>
            </nldd-container>
          </nldd-simple-section>
        </template>

        <!-- Comparison -->
        <nldd-simple-section v-else width="full">
          <nldd-container gap="16">
            <nldd-card :accessible-label="t('sim.comparison.label')">
              <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center"><nldd-title-cell size="5" :text="t('sim.comparison.title')" :supporting-text="t('sim.comparison.lead', { n: comparable.length })"></nldd-title-cell></nldd-container>
              <nldd-container padding="12">
                <SimBarChart :categories="comparisonChart.categories" :series="comparisonChart.series" unit="percent" horizontal :height="`${Math.max(220, (18 * comparable.length + 12) * comparisonChart.categories.length + 60)}px`" />
              </nldd-container>
            </nldd-card>
            <nldd-table :columns="`minmax(160px, 1fr) repeat(${comparable.length}, 150px)`" :accessible-label="t('sim.comparison.table_label')">
              <nldd-table-row slot="header">
                <nldd-text-cell size="sm" :text="t('sim.comparison.law')"></nldd-text-cell>
                <nldd-text-cell v-for="r in comparable" :key="r.id" size="sm" :text="r.label" :supporting-text="t(Object.keys(r.overrides).length ? 'sim.comparison.overridden' : 'sim.comparison.default')" horizontal-alignment="right"></nldd-text-cell>
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
          <nldd-top-title-bar :text="inspectorLaw.name" :supporting-text="inspector.type === 'params' ? t('sim.inspector.params') : serviceInfo(corpus, inspectorLaw.service).name" :dismiss-text="t('sim.inspector.close')" @dismiss="inspector = null"></nldd-top-title-bar>
        </nldd-container>

        <!-- Law parameters -->
        <nldd-container v-if="inspector.type === 'params'" padding="12" gap="12">
          <!-- De zin staat in twee stukken in het woordenboek, met het
               <code>-element ertussen: dat scheelt een v-html voor één tag. -->
          <nldd-rich-text><p>{{ t('sim.inspector.definitions.lead') }} (<code>definitions</code>). {{ t('sim.inspector.definitions.scope') }}</p></nldd-rich-text>
          <nldd-form-field v-for="def in definitionsByLaw[inspectorLaw.id]" :key="def.key" :label="humanize(def.key)">
            <nldd-number-field :value="overrides[inspectorLaw.id]?.[def.key] ?? def.value" :step="Number.isInteger(def.value) ? '1' : '0.001'" width="full" hide-spin-buttons @change="setOverride(inspectorLaw.id, def.key, numberFrom($event))"></nldd-number-field>
            <nldd-form-field-help-text>{{ t('sim.inspector.definition_help', { article: def.article, value: def.value, hint: definitionHint(def) }) }}</nldd-form-field-help-text>
          </nldd-form-field>
          <nldd-button v-if="overrideCount(inspectorLaw.id)" variant="secondary" start-icon="arrow-2-counter-clockwise" :text="t('sim.inspector.reset')" @click="resetOverrides(inspectorLaw.id)"></nldd-button>
        </nldd-container>

        <!-- One law in the active run -->
        <nldd-container v-else-if="activeRun && activeRun.summary[inspector.lawId]" padding="12" gap="12">
          <nldd-list variant="box-tinted" :accessible-label="t('sim.inspector.outcome')">
            <nldd-list-item size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('sim.inspector.evaluated')"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="num(activeRun.summary[inspector.lawId].evaluated)"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].hasEligibility" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('sim.inspector.eligible')"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="`${num(activeRun.summary[inspector.lawId].eligible)} (${pct(activeRun.summary[inspector.lawId].eligiblePct)})`"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].withAmount" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('sim.inspector.average', { name: amountLabel(activeRun, inspector.lawId) })"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="fmtAmount(activeRun, inspector.lawId, activeRun.summary[inspector.lawId].avgAmount)"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].withAmount && amountIsMoney(activeRun, inspector.lawId)" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('sim.inspector.total')"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="money(activeRun.summary[inspector.lawId].totalAmount)"></nldd-text-cell></nldd-list-item>
            <nldd-list-item v-if="activeRun.summary[inspector.lawId].errors" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('sim.inspector.not_computed')"></nldd-text-cell><nldd-text-cell size="sm" width="fit-content" :text="num(activeRun.summary[inspector.lawId].errors)"></nldd-text-cell></nldd-list-item>
          </nldd-list>
          <nldd-banner v-if="activeRun.summary[inspector.lawId].errors" variant="warning" :text="t('sim.inspector.first_error')" :supporting-text="activeRun.results.find((r) => r.laws[inspector.lawId] && !r.laws[inspector.lawId].ok)?.laws[inspector.lawId].error"></nldd-banner>
          <nldd-form-field :label="t('sim.inspector.split_by')">
            <nldd-dropdown width="full">
              <select :value="dimension.id" @change="dimensionId = $event.target.value">
                <option v-for="d in dimensions" :key="d.id" :value="d.id">{{ dimensionLabel(d) }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <SimBarChart :categories="breakdownFor(inspector.lawId).map((r) => humanize(r.group))" :series="[{ name: t('sim.inspector.meets'), values: breakdownFor(inspector.lawId).map((r) => r.eligiblePct) }]" unit="percent" height="220px" />
          <nldd-button variant="secondary" start-icon="book" :text="t('sim.inspector.law_text')" @click="goTo('wetten', { lawId: inspector.lawId })"></nldd-button>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
