<script setup>
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { parseFeature, dispatch, quotedValue, bareValue, ExecutionContext } from '@regelrecht/frontend-shared/gherkin';
import { matchStep, renderStepNl, FEATURE_KEYWORDS_NL } from '../data/gherkinNl.js';
import { serviceInfo } from '../data/loadCorpus.js';
import { prepareScenarioEngine } from '../engine/useDemoEngine.js';
import { useDemo } from '../store/demoStore.js';

// The scenario runner: every law's acceptance scenarios (Gherkin, canonical
// grammar) in the sidebar; the chosen feature rendered for a Dutch audience;
// "Uitvoeren" runs a scenario in the browser against the same WASM engine the
// portal uses (a second instance, with the same laws) and shows the engine's
// full trace. This is the live proof that the law behaves as the scenario says.

const route = useRoute();
const router = useRouter();
const { corpus, profile } = useDemo();

const features = computed(() => corpus.value?.scenarios ?? []);
// The list of test files is a sheet, closed until asked for.
const splitView = ref(null);
const selectedPath = ref(null);
const parsed = ref(null);
const rawText = ref('');
const loadError = ref(null);
const runs = reactive({}); // scenario index -> { status, steps: [{status, error}], trace, outputs }
// Scenarios start collapsed: a feature with a dozen scenarios is otherwise a
// wall of steps. Each card opens on its own; a failed run opens itself so the
// failing step is in view.
const open = reactive({}); // scenario index -> boolean
const runningAll = ref(false);
const anyRunning = computed(() => runningAll.value || Object.values(runs).some((r) => r.status === 'running'));
const query = ref('');

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  const list = q ? features.value.filter((f) => `${f.title} ${f.law_path}`.toLowerCase().includes(q)) : features.value;
  return [...list].sort((a, b) => a.title.localeCompare(b.title));
});

// One track per column of the Gherkin table, sized to its content, plus a
// trailing filler track: the rows span every track, so the filler carries the
// row dividers to the edge of the box while the cells stay compact. The table
// is its own horizontal scroll container, so a wide table scrolls instead of
// squeezing or overflowing the card.
function dataTableColumns(table) {
  const width = Math.max(...table.map((row) => row.length));
  return `repeat(${width}, max-content) 1fr`;
}

function lawFor(feature) {
  return corpus.value?.laws.find((l) => l.law_path === feature.law_path) ?? null;
}

async function select(path, { replaceRoute = false } = {}) {
  if (!path) return;
  selectedPath.value = path;
  splitView.value?.hidePrimarySidebarSheet?.();
  Object.keys(runs).forEach((k) => delete runs[k]);
  Object.keys(open).forEach((k) => delete open[k]);
  loadError.value = null;
  try {
    const res = await fetch(path);
    if (!res.ok) throw new Error(`Kon ${path} niet laden (${res.status})`);
    rawText.value = await res.text();
    parsed.value = parseFeature(rawText.value);
  } catch (e) {
    loadError.value = e;
    parsed.value = null;
  }
  const target = `/scenarios/${path.replace(/^\/data\/laws\//, '')}`;
  if (route.fullPath !== target) (replaceRoute ? router.replace : router.push).call(router, target);
}

watch(
  () => [route.name, route.params.featurePath, corpus.value, profile.value],
  ([name, featurePath]) => {
    if (name !== 'scenarios' || !corpus.value) return;
    if (featurePath) {
      const path = `/data/laws/${featurePath}`;
      if (path !== selectedPath.value) select(path, { replaceRoute: true });
    } else if (!selectedPath.value) {
      const wanted = profile.value?.default_feature;
      const first = features.value.find((f) => f.law_path === wanted) ?? features.value[0];
      if (first) select(first.path, { replaceRoute: true });
    }
  },
  { immediate: true },
);

const selectedFeature = computed(() => features.value.find((f) => f.path === selectedPath.value) ?? null);
const selectedLaw = computed(() => (selectedFeature.value ? lawFor(selectedFeature.value) : null));

const showText = ref(false);

/** Scenario steps with the background prepended. */
function stepsOf(scenario) {
  return [...(parsed.value?.background ?? []), ...scenario.steps];
}

/**
 * Run one scenario on the scenario engine. Data steps register scoped sources
 * there; the sources are cleared before every run so scenarios never see each
 * other's rows. The portal's engine and its persona data are never touched.
 */
async function run(index) {
  const scenario = parsed.value?.scenarios[index];
  if (!scenario || !corpus.value || runs[index]?.status === 'running') return;
  const wip = scenario.tags.includes('@wip');
  // Reactive on purpose: the run mutates this object step by step, and the card
  // has to follow. A plain object assigned into `runs` would only be tracked
  // through `runs[index]`, not through this reference.
  const state = reactive({ status: 'running', steps: [], trace: null, traceText: '', outputs: null, error: null, wip });
  runs[index] = state;
  // Let the card repaint (status icon, loading button) before the engine work.
  await new Promise((resolve) => setTimeout(resolve));
  const ctx = new ExecutionContext();
  const t0 = performance.now();
  const timings = [];
  const lap = (label, from) => timings.push(`${label} ${(performance.now() - from).toFixed(1)}ms`);
  let t = performance.now();
  let e;
  try {
    e = await prepareScenarioEngine(corpus.value);
  } catch (err) {
    state.error = `Engine niet beschikbaar: ${err?.message ?? err}`;
    state.status = 'fail';
    open[index] = true;
    return;
  }
  lap('engine', t);
  t = performance.now();
  e.clearDataSources();
  lap('clearDataSources', t);
  try {
    for (const step of stepsOf(scenario)) {
      t = performance.now();
      const match = matchStep(step.text);
      const record = { status: 'pending', error: null };
      state.steps.push(record);
      if (!match) {
        record.status = 'fail';
        record.error = 'Onbekende stap (niet in bdd/grammar.yaml)';
        break;
      }
      const { entry, args } = match;
      const typed = args.map((raw, i) => (entry.argTypes[i] === 'number' ? bareValue(raw) : quotedValue(raw)));
      const table = step.dataTable ?? null;
      try {
        if (entry.action === 'evaluate_outputs') {
          const outputs = String(typed[0]).split(',').map((s) => s.trim());
          evaluateWithTrace(ctx, e, typed[1], outputs, state);
        } else if (entry.action === 'evaluate') {
          evaluateWithTrace(ctx, e, typed[1], [typed[0]], state);
        } else {
          await dispatch(ctx, e, entry.action, [...typed, ...entry.literals], table, { loadDependency: async () => {} });
        }
        record.status = 'pass';
        lap(`step:${entry.action}`, t);
      } catch (err) {
        record.status = 'fail';
        record.error = String(err?.message ?? err?.error ?? err);
        break;
      }
    }
    const failed = state.steps.some((s) => s.status === 'fail');
    state.status = failed ? 'fail' : 'pass';
    // A failure opens the card so the failing step is in view; a pass leaves
    // the card as the presenter had it.
    if (failed) open[index] = true;
  } finally {
    e.clearDataSources();
    console.debug(`[scenario-timing] ${scenario.name}: total ${(performance.now() - t0).toFixed(1)}ms | ${timings.join(' | ')}`);
  }
}

/** The 1-based number of the failing step within the scenario's own steps, or 0 when it is a background step. */
function failedStepNumber(index) {
  const r = runs[index];
  const at = r?.steps?.findIndex((s) => s.status === 'fail') ?? -1;
  if (at < 0) return null;
  const bg = parsed.value?.background?.length ?? 0;
  return at < bg ? 0 : at - bg + 1;
}

function resultTag(index) {
  const r = runs[index];
  if (r?.status === 'pass') return { color: 'success', text: 'Geslaagd' };
  if (r?.status === 'fail') {
    const n = failedStepNumber(index);
    const text = n === 0 ? 'Mislukt in de achtergrond' : n ? `Mislukt bij stap ${n}` : 'Mislukt';
    // A @wip scenario is known not to pass yet (the Rust runner skips it); its
    // failure is expected, not a regression.
    return r.wip ? { color: 'warning', text: `${text} (@wip)` } : { color: 'critical', text };
  }
  return null;
}

function statusIcon(index) {
  const s = runs[index]?.status;
  return s === 'pass' ? 'check-mark-circle' : s === 'fail' ? 'dismiss-circle' : s === 'running' ? 'clock' : 'circle-dashed';
}

function statusColor(index) {
  const r = runs[index];
  return r?.status === 'pass' ? 'success' : r?.status === 'fail' ? (r.wip ? 'warning' : 'critical') : 'secondary';
}

function evaluateWithTrace(ctx, e, lawId, outputs, state) {
  if (!ctx.calculationDate) throw new Error('Geen peildatum: voeg "the calculation date is" toe.');
  try {
    const result = e.executeMultipleWithTrace(lawId, outputs, ctx.parameters, ctx.calculationDate);
    ctx.result = result;
    ctx.executed = true;
    ctx.error = null;
    state.trace = result.trace ?? null;
    state.traceText = result.trace_text ?? '';
    state.outputs = result.outputs ?? {};
  } catch (err) {
    ctx.error = err;
    ctx.executed = true;
    ctx.result = null;
    state.traceText = err?.trace ? renderTraceFallback(err) : '';
    state.error = String(err?.error ?? err?.message ?? err);
    // Assertions after a failed run report the failure themselves.
  }
}

function renderTraceFallback(err) {
  return `Uitvoering mislukt: ${err.error ?? err.message ?? err}`;
}


async function runAll() {
  if (runningAll.value) return;
  runningAll.value = true;
  try {
    for (let i = 0; i < (parsed.value?.scenarios.length ?? 0); i += 1) {
      // eslint-disable-next-line no-await-in-loop
      await run(i);
    }
  } finally {
    runningAll.value = false;
  }
}

const summary = computed(() => {
  const all = Object.values(runs);
  return { total: parsed.value?.scenarios.length ?? 0, pass: all.filter((r) => r.status === 'pass').length, fail: all.filter((r) => r.status === 'fail').length };
});

const activeTrace = ref(null); // scenario index whose trace is shown
const traceScenario = computed(() => (activeTrace.value === null ? null : runs[activeTrace.value] ?? null));
const traceSheet = ref(null);
watch(activeTrace, async (index) => {
  if (index === null) return traceSheet.value?.hide?.();
  await nextTick();
  traceSheet.value?.show?.();
});

function stepClass(index, stepIndex) {
  const r = runs[index];
  const s = r?.steps?.[stepIndex];
  return s ? s.status : '';
}

const fileName = computed(() => selectedPath.value?.split('/').pop() ?? '');
</script>

<template>
  <nldd-navigation-split-view ref="splitView" primary-sidebar-as-sheet primary-sidebar-accessible-label="Scenario's">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12" gap="8">
          <nldd-top-title-bar text="Scenario's" :supporting-text="`${features.length} testbestanden`"></nldd-top-title-bar>
          <nldd-search-field placeholder="Zoek een scenario" size="sm" :value="query" @input="query = $event.detail?.value ?? $event.target.value"></nldd-search-field>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <nldd-list type="navigation" accessible-label="Testbestanden">
            <nldd-list-item v-for="f in filtered" :key="f.path" size="sm" button :selected="f.path === selectedPath || undefined" @click="select(f.path)">
              <nldd-text-cell size="sm" :text="f.title" :supporting-text="lawFor(f) ? serviceInfo(corpus, lawFor(f).service).name : f.law_path"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="checklist" text="Scenario's" :supporting-text="`${features.length}`" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-title v-if="parsed" slot="start" :text="parsed.feature" :supporting-text="fileName"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end" v-if="parsed">
              <nldd-segmented-control size="sm" width="fit-content" :value="showText ? 'text' : 'steps'" @change="showText = $event.detail?.value === 'text'">
                <nldd-segmented-control-item value="steps" text="Scenario's"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="text" text="Bestand"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="parsed">
              <nldd-button size="sm" variant="primary" start-icon="play" text="Alles uitvoeren" :loading="runningAll || undefined" :disabled="(anyRunning && !runningAll) || undefined" @click="runAll"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="parsed && selectedLaw">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="book" text="Wettekst" @click="router.push(`/wetten/${encodeURIComponent(selectedLaw.id)}`)"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>

        <nldd-simple-section v-if="loadError" width="full">
          <nldd-banner variant="critical" text="Kon het scenario niet laden" :supporting-text="String(loadError)"></nldd-banner>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="!parsed" height="60vh">
          <nldd-inline-dialog icon="checklist" text="Kies een scenario" supporting-text="Open de lijst met testbestanden.">
            <nldd-button slot="actions" variant="primary" size="sm" text="Scenario's" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="showText" width="full">
          <nldd-code-viewer language="gherkin" wrap>{{ rawText }}</nldd-code-viewer>
        </nldd-simple-section>
        <nldd-simple-section v-else width="full">
          <nldd-container gap="16">
          <nldd-box>
            <nldd-container padding="12" layout="row" gap="12" vertical-alignment="center">
              <nldd-icon-cell icon="checklist" color="secondary"></nldd-icon-cell>
              <nldd-title-cell size="5" :text="parsed.feature" :overline="FEATURE_KEYWORDS_NL.Feature" :supporting-text="parsed.background?.length ? `${FEATURE_KEYWORDS_NL.Background}: ${parsed.background.length} ${parsed.background.length === 1 ? 'stap' : 'stappen'}` : undefined"></nldd-title-cell>
              <!-- The run summary sits with the feature, next to its title, as the result
                   tag does on each scenario card. In the toolbar it stood between 32px
                   controls; no tag or badge size reaches that height. -->
              <nldd-tag v-if="summary.pass + summary.fail > 0" :color="summary.fail ? 'critical' : 'success'" :text="`${summary.pass} geslaagd${summary.fail ? `, ${summary.fail} mislukt` : ''}`"></nldd-tag>
            </nldd-container>
            <!-- The background is the shared premise of every scenario below, so it
                 stays in view; only the scenario cards collapse. -->
            <nldd-container v-if="parsed.background?.length" padding-inline="16" padding-bottom="12">
              <div class="gherkin">
                <div><span class="kw">{{ FEATURE_KEYWORDS_NL.Background }}:</span></div>
                <div v-for="(step, i) in parsed.background" :key="`bg-${i}`" class="step">
                  <span class="kw">{{ renderStepNl(step).keyword }}</span> {{ renderStepNl(step).text }}
                </div>
              </div>
            </nldd-container>
          </nldd-box>
          <nldd-card v-for="(scenario, index) in parsed.scenarios" :key="index" :accessible-label="scenario.name">
            <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center">
              <nldd-icon-cell :icon="statusIcon(index)" :color="statusColor(index)"></nldd-icon-cell>
              <nldd-title-cell size="5" :text="scenario.name" :supporting-text="scenario.tags.join(' ') || undefined"></nldd-title-cell>
              <!-- One height across the action row: an md tag and xs buttons are both
                   24px; no tag size matches an sm button. -->
              <nldd-tag v-if="resultTag(index)" :color="resultTag(index).color" :text="resultTag(index).text"></nldd-tag>
              <nldd-button size="xs" variant="secondary" start-icon="play" text="Uitvoeren" :loading="runs[index]?.status === 'running' || undefined" :disabled="(anyRunning && runs[index]?.status !== 'running') || undefined" @click="run(index)"></nldd-button>
              <nldd-button v-if="runs[index]?.traceText" size="xs" variant="neutral-tinted" start-icon="list" text="Trace" @click="activeTrace = index"></nldd-button>
              <nldd-icon-button size="xs" variant="neutral-transparent" :icon="open[index] ? 'chevron-up' : 'chevron-down'" :text="open[index] ? 'Stappen verbergen' : 'Stappen tonen'" :expanded="open[index] || undefined" @click="open[index] = !open[index]"></nldd-icon-button>
            </nldd-container>
            <nldd-container v-if="open[index]" padding-inline="16" padding-bottom="12" gap="12">
              <nldd-banner v-if="runs[index]?.error && !runs[index]?.steps?.length" variant="critical" text="Uitvoering mislukt" :supporting-text="runs[index].error"></nldd-banner>
              <div class="gherkin">
                <div v-for="(step, si) in scenario.steps" :key="si" :class="['step', stepClass(index, (parsed.background?.length ?? 0) + si)]">
                  <span class="kw">{{ renderStepNl(step).keyword }}</span> {{ renderStepNl(step).text }}
                  <nldd-container v-if="step.dataTable" padding-block="4">
                    <nldd-table :columns="dataTableColumns(step.dataTable)" :accessible-label="`Tabel bij ${renderStepNl(step).text}`">
                      <nldd-table-row slot="header">
                        <nldd-text-cell v-for="(cell, ci) in step.dataTable[0]" :key="ci" size="sm" :text="cell"></nldd-text-cell>
                      </nldd-table-row>
                      <nldd-table-row v-for="(row, ri) in step.dataTable.slice(1)" :key="ri">
                        <nldd-text-cell v-for="(cell, ci) in row" :key="ci" size="sm" :text="cell"></nldd-text-cell>
                      </nldd-table-row>
                    </nldd-table>
                  </nldd-container>
                  <div v-if="runs[index]?.steps?.[(parsed.background?.length ?? 0) + si]?.error" class="str">✗ {{ runs[index].steps[(parsed.background?.length ?? 0) + si].error }}</div>
                </div>
              </div>
            </nldd-container>
          </nldd-card>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <!-- The engine's trace is wide; a 320px inspector column cuts every line,
         so it opens in a broad sheet, as the tile's "Berekening" does. -->
    <Teleport to="body">
      <nldd-sheet ref="traceSheet" placement="right" width="760px" accessible-label="Uitvoering door de engine" @close="activeTrace = null">
        <nldd-page v-if="traceScenario">
          <nldd-container slot="header" padding="12">
            <nldd-top-title-bar text="Uitvoering door de engine" :supporting-text="parsed?.scenarios[activeTrace]?.name" dismiss-text="Sluiten" @dismiss="activeTrace = null"></nldd-top-title-bar>
          </nldd-container>
          <nldd-container padding="16" gap="16">
            <nldd-banner v-if="traceScenario.error" variant="critical" text="Uitvoering mislukt" :supporting-text="traceScenario.error"></nldd-banner>
            <nldd-list v-if="traceScenario.outputs" variant="box-tinted" accessible-label="Uitkomsten">
              <nldd-list-item v-for="(v, k) in traceScenario.outputs" :key="k" size="sm">
                <nldd-text-cell size="sm" :text="String(k)"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="JSON.stringify(v)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-code-viewer variant="box-tinted" no-copy>{{ traceScenario.traceText }}</nldd-code-viewer>
          </nldd-container>
        </nldd-page>
      </nldd-sheet>
    </Teleport>
  </nldd-navigation-split-view>
</template>
