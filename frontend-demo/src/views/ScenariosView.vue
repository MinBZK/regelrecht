<script setup>
import { computed, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { parseFeature, dispatch, quotedValue, bareValue, ExecutionContext } from '@regelrecht/frontend-shared/gherkin';
import OrgLogo from '../components/OrgLogo.vue';
import { matchStep, renderStepNl, FEATURE_KEYWORDS_NL } from '../data/gherkinNl.js';
import { useDemo } from '../store/demoStore.js';

// The scenario runner: every law's acceptance scenarios (Gherkin, canonical
// grammar) in the sidebar; the chosen feature rendered for a Dutch audience;
// "Uitvoeren" runs a scenario in the browser against the same WASM engine the
// portal uses and shows the engine's full trace. This is the live proof that
// the law behaves as the scenario says.

const route = useRoute();
const router = useRouter();
const demo = useDemo();
const { corpus, profile, engine } = demo;

const features = computed(() => corpus.value?.scenarios ?? []);
const selectedPath = ref(null);
const parsed = ref(null);
const rawText = ref('');
const loadError = ref(null);
const runs = reactive({}); // scenario index -> { status, steps: [{status, error}], trace, outputs }
const query = ref('');

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  const list = q ? features.value.filter((f) => `${f.title} ${f.law_path}`.toLowerCase().includes(q)) : features.value;
  return [...list].sort((a, b) => a.title.localeCompare(b.title));
});

function lawFor(feature) {
  return corpus.value?.laws.find((l) => l.law_path === feature.law_path) ?? null;
}

async function select(path, { replaceRoute = false } = {}) {
  if (!path) return;
  selectedPath.value = path;
  Object.keys(runs).forEach((k) => delete runs[k]);
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
 * Run one scenario. Data steps register scoped sources on the shared engine;
 * afterwards the persona data is restored so the portal is unaffected.
 */
async function run(index) {
  const scenario = parsed.value?.scenarios[index];
  if (!scenario || !engine.value) return;
  const wip = scenario.tags.includes('@wip');
  const state = { status: 'running', steps: [], trace: null, traceText: '', outputs: null, error: null, wip };
  runs[index] = state;
  const ctx = new ExecutionContext();
  const e = engine.value;
  e.clearDataSources();
  try {
    for (const step of stepsOf(scenario)) {
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
      } catch (err) {
        record.status = 'fail';
        record.error = String(err?.message ?? err?.error ?? err);
        break;
      }
    }
    const failed = state.steps.some((s) => s.status === 'fail');
    state.status = failed ? 'fail' : 'pass';
  } finally {
    // Give the portal its persona data back.
    e.clearDataSources();
    demo.reregister();
  }
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
  for (let i = 0; i < (parsed.value?.scenarios.length ?? 0); i += 1) {
    // eslint-disable-next-line no-await-in-loop
    await run(i);
  }
}

const summary = computed(() => {
  const all = Object.values(runs);
  return { total: parsed.value?.scenarios.length ?? 0, pass: all.filter((r) => r.status === 'pass').length, fail: all.filter((r) => r.status === 'fail').length };
});

const activeTrace = ref(null); // scenario index whose trace is shown
const traceScenario = computed(() => (activeTrace.value === null ? null : runs[activeTrace.value] ?? null));

function stepClass(index, stepIndex) {
  const r = runs[index];
  const s = r?.steps?.[stepIndex];
  return s ? s.status : '';
}

const fileName = computed(() => selectedPath.value?.split('/').pop() ?? '');
</script>

<template>
  <nldd-navigation-split-view sidebar-accessible-label="Scenario's" inspector-accessible-label="Uitvoering">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12" gap="8">
          <nldd-top-title-bar text="Scenario's" :supporting-text="`${features.length} testbestanden`"></nldd-top-title-bar>
          <nldd-search-field placeholder="Zoek een scenario" size="sm" :value="query" @input="query = $event.detail?.value ?? $event.target.value"></nldd-search-field>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <nldd-list type="navigation" accessible-label="Testbestanden">
            <nldd-list-item v-for="f in filtered" :key="f.path" size="sm" button :selected="f.path === selectedPath || undefined" @click="select(f.path)">
              <nldd-cell v-if="lawFor(f)?.service"><OrgLogo :service="lawFor(f).service" size="sm" /></nldd-cell>
              <nldd-spacer-cell v-if="lawFor(f)?.service" size="8"></nldd-spacer-cell>
              <nldd-text-cell size="sm" :text="f.title" :supporting-text="f.law_path"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page sticky-header>
        <nldd-container slot="header" padding="8" v-if="parsed">
          <nldd-toolbar size="sm">
            <nldd-toolbar-title slot="start" :text="parsed.feature" :supporting-text="fileName"></nldd-toolbar-title>
            <nldd-toolbar-item slot="end" v-if="summary.pass + summary.fail > 0">
              <nldd-tag :color="summary.fail ? 'critical' : 'success'" :text="`${summary.pass} geslaagd${summary.fail ? `, ${summary.fail} mislukt` : ''}`"></nldd-tag>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-segmented-control size="sm" width="fit-content" :value="showText ? 'text' : 'steps'" @change="showText = $event.detail?.value === 'text'">
                <nldd-segmented-control-item value="steps" text="Scenario's"></nldd-segmented-control-item>
                <nldd-segmented-control-item value="text" text="Bestand"></nldd-segmented-control-item>
              </nldd-segmented-control>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button size="sm" variant="primary" start-icon="play" text="Alles uitvoeren" @click="runAll"></nldd-button>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="selectedLaw">
              <nldd-button size="sm" variant="neutral-tinted" start-icon="book" text="Wettekst" @click="router.push(`/wetten/${encodeURIComponent(selectedLaw.id)}`)"></nldd-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>

        <nldd-simple-section v-if="loadError" width="full">
          <nldd-banner variant="critical" text="Kon het scenario niet laden" :supporting-text="String(loadError)"></nldd-banner>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="!parsed" height="60vh">
          <nldd-inline-dialog icon="checklist" text="Kies een scenario" supporting-text="Kies links een testbestand."></nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else-if="showText" width="full">
          <nldd-code-viewer language="gherkin" wrap>{{ rawText }}</nldd-code-viewer>
        </nldd-simple-section>
        <nldd-simple-section v-else width="full">
          <nldd-box>
            <nldd-container padding="16">
              <div class="gherkin">
                <div><span class="kw">{{ FEATURE_KEYWORDS_NL.Feature }}:</span> {{ parsed.feature }}</div>
                <template v-if="parsed.background?.length">
                  <div>&nbsp;</div>
                  <div><span class="kw">{{ FEATURE_KEYWORDS_NL.Background }}:</span></div>
                  <div v-for="(step, i) in parsed.background" :key="`bg-${i}`" class="step">
                    <span class="kw">{{ renderStepNl(step).keyword }}</span> {{ renderStepNl(step).text }}
                  </div>
                </template>
              </div>
            </nldd-container>
          </nldd-box>
          <nldd-spacer size="16"></nldd-spacer>
          <nldd-card v-for="(scenario, index) in parsed.scenarios" :key="index" :accessible-label="scenario.name">
            <nldd-container slot="header" padding="12" layout="row" gap="12" vertical-alignment="center">
              <nldd-icon-cell
                :icon="runs[index]?.status === 'pass' ? 'check-mark-circle' : runs[index]?.status === 'fail' ? 'dismiss-circle' : runs[index]?.status === 'running' ? 'clock' : 'circle-dashed'"
                :color="runs[index]?.status === 'pass' ? 'success' : runs[index]?.status === 'fail' ? 'critical' : 'secondary'"
              ></nldd-icon-cell>
              <nldd-title-cell size="5" :text="scenario.name" :supporting-text="scenario.tags.join(' ') || undefined"></nldd-title-cell>
              <nldd-button size="sm" variant="secondary" start-icon="play" text="Uitvoeren" @click="run(index)"></nldd-button>
              <nldd-button v-if="runs[index]?.traceText" size="sm" variant="neutral-tinted" start-icon="list" text="Trace" @click="activeTrace = index"></nldd-button>
            </nldd-container>
            <nldd-container padding-inline="16" padding-bottom="12">
              <div class="gherkin">
                <div v-for="(step, si) in scenario.steps" :key="si" :class="['step', stepClass(index, (parsed.background?.length ?? 0) + si)]">
                  <span class="kw">{{ renderStepNl(step).keyword }}</span> {{ renderStepNl(step).text }}
                  <div v-if="step.dataTable" class="table-wrap"><table>
                    <tr v-for="(row, ri) in step.dataTable.slice(0, 6)" :key="ri">
                      <component :is="ri === 0 ? 'th' : 'td'" v-for="(cell, ci) in row.slice(0, 8)" :key="ci">{{ cell }}</component>
                      <td v-if="row.length > 8">… {{ row.length - 8 }} meer</td>
                    </tr>
                    <tr v-if="step.dataTable.length > 6"><td :colspan="Math.min(step.dataTable[0].length, 9)">… {{ step.dataTable.length - 6 }} rijen meer</td></tr>
                  </table></div>
                  <div v-if="runs[index]?.steps?.[(parsed.background?.length ?? 0) + si]?.error" class="str">✗ {{ runs[index].steps[(parsed.background?.length ?? 0) + si].error }}</div>
                </div>
              </div>
            </nldd-container>
          </nldd-card>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="inspector" :has-content="traceScenario ? true : undefined">
      <nldd-page v-if="traceScenario">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar text="Uitvoering door de engine" :supporting-text="parsed?.scenarios[activeTrace]?.name" dismiss-text="Sluiten" @dismiss="activeTrace = null"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="12">
          <nldd-banner v-if="traceScenario.error" variant="critical" text="Uitvoering mislukt" :supporting-text="traceScenario.error"></nldd-banner>
          <nldd-list v-if="traceScenario.outputs" variant="box" accessible-label="Uitkomsten">
            <nldd-list-item v-for="(v, k) in traceScenario.outputs" :key="k" size="sm">
              <nldd-text-cell size="sm" :text="String(k)"></nldd-text-cell>
              <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="JSON.stringify(v)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-code-viewer variant="box" no-copy>{{ traceScenario.traceText }}</nldd-code-viewer>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
