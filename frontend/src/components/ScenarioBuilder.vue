<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useDependencies } from '../composables/useDependencies.js';
import { useScenarios } from '../composables/useScenarios.js';
import { parseFeature } from '../gherkin/parser.js';
import { mapFeatureToForm, getEffectiveSetup, formStateToGherkin, syncEditedValues } from '../gherkin/formMapper.js';
import { matchStatus } from '../utils/outputFormat.js';
import { buildArticleMap } from '../utils/articleMapping.js';
import ScenarioForm from './ScenarioForm.vue';

const props = defineProps({
  lawId: { type: String, required: true },
  lawYaml: { type: String, default: null },
  engine: { type: Object, default: null },
  ready: { type: Boolean, default: false },
  /** Articles array from useLaw() for article mapping */
  articles: { type: Array, default: () => [] },
});

const emit = defineEmits(['executed', 'dirty-change']);

// --- Article mapping ---
const articleMap = computed(() => buildArticleMap(props.articles));

// --- Dependencies ---
const {
  loading: depsLoading,
  progress: depsProgress,
  error: depsError,
  loadAllDependencies,
} = useDependencies();

// --- Scenario loading ---
const lawIdRef = computed(() => props.lawId);
const {
  scenarios: scenarioFiles,
  selectedScenario: selectedScenarioFile,
  featureText,
  loading: scenariosLoading,
  saving,
  saveError,
  selectScenario: selectScenarioFile,
  saveScenario,
} = useScenarios(lawIdRef);

const formState = ref(null);
const saveSuccess = ref(false);
const isDirty = ref(false);
const selectedScenarioIndex = ref(null);
const scenarioSheetEl = ref(null);

watch(selectedScenarioIndex, async (idx) => {
  await nextTick();
  if (idx !== null) scenarioSheetEl.value?.show();
  else scenarioSheetEl.value?.hide();
});

watch(isDirty, (val) => emit('dirty-change', val));

function markDirty() {
  if (!isDirty.value) isDirty.value = true;
}

function humanize(name) {
  if (typeof name !== 'string') return name;
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}

function scenarioExpectations(index) {
  const assertions = formState.value?.scenarios?.[index]?.assertions || [];
  return assertions
    .filter((a) => a.outputName && a.value !== null && a.value !== undefined)
    .map((a) => ({ name: humanize(a.outputName), value: humanize(String(a.value)) }));
}

// Parse feature file when loaded
watch(featureText, (text) => {
  isDirty.value = false;
  selectedScenarioIndex.value = null;
  if (!text) {
    formState.value = null;
    return;
  }
  try {
    const parsed = parseFeature(text);
    formState.value = mapFeatureToForm(parsed);
  } catch {
    formState.value = null;
  }
});

// Cache for fetched law YAML texts
const yamlCache = {};

async function fetchLawYaml(lawId) {
  if (yamlCache[lawId]) return yamlCache[lawId];
  const res = await fetch(`/api/corpus/laws/${encodeURIComponent(lawId)}`);
  if (!res.ok) throw new Error(`Failed to fetch law '${lawId}': ${res.status}`);
  const text = await res.text();
  yamlCache[lawId] = text;
  return text;
}

// --- Dependencies ready tracking ---
const depsReady = ref(false);

// --- Load dependencies when law YAML changes ---
let watchVersion = 0;

watch(
  [() => props.lawYaml, () => props.ready, formState],
  async ([lawYaml, isReady]) => {
    if (!lawYaml || !isReady || !props.engine) return;
    const version = ++watchVersion;
    depsReady.value = false;

    await loadAllDependencies(lawYaml, props.engine, fetchLawYaml);
    if (version !== watchVersion) return;

    // Also load dependencies from scenario background + per-scenario steps
    if (formState.value) {
      const allDeps = new Set();
      for (const dep of formState.value.background?.dependencies || []) allDeps.add(dep);
      for (const sc of formState.value.scenarios || []) {
        for (const dep of sc.setup?.dependencies || []) allDeps.add(dep);
      }
      for (const depId of allDeps) {
        try {
          if (!props.engine.hasLaw(depId)) {
            const yaml = await fetchLawYaml(depId);
            props.engine.loadLaw(yaml);
          }
        } catch (e) {
          console.warn(`Failed to load scenario dependency '${depId}':`, e);
        }
      }
    }

    if (version === watchVersion) {
      depsReady.value = true;
    }
  },
  { immediate: true },
);

// --- Template refs for ScenarioForm instances ---
const scenarioRefs = ref([]);

// --- Per-scenario result tracking ---
const scenarioResults = ref(new Map());

function onScenarioResult(index, data) {
  const updated = new Map(scenarioResults.value);
  updated.set(index, data);
  scenarioResults.value = updated;
}

function scenarioStatus(index) {
  const data = scenarioResults.value.get(index);
  if (!data) return null;
  if (data.error) return 'failed';
  if (!data.result) return null;

  const scenario = formState.value?.scenarios[index];
  if (!scenario) return null;

  const checkable = (scenario.assertions || []).filter(
    (a) => a.outputName && a.value != null,
  );
  if (checkable.length === 0) return null;

  for (const a of checkable) {
    const status = matchStatus(
      a.outputName,
      data.result.outputs?.[a.outputName],
      { [a.outputName]: String(a.value) },
    );
    if (status === 'failed') return 'failed';
  }
  return 'passed';
}

// Reset results and refs when formState changes (new scenario file loaded)
watch(formState, () => {
  scenarioResults.value = new Map();
  scenarioRefs.value = [];
});

// --- Auto-execute all scenarios sequentially when deps are ready ---
let autoExecuteVersion = 0;

watch(
  [depsReady, formState],
  async ([ready, state]) => {
    if (!ready || !state || !state.scenarios?.length) return;
    const version = ++autoExecuteVersion;

    // Wait one tick so ScenarioForm refs are mounted
    await nextTick();
    if (version !== autoExecuteVersion) return;

    for (let i = 0; i < state.scenarios.length; i++) {
      if (version !== autoExecuteVersion) return;
      const formRef = scenarioRefs.value[i];
      if (formRef?.execute) {
        formRef.execute();
        // Collect result after execution
        const data = formRef.getExecutionData?.();
        if (data) onScenarioResult(i, data);
      }
    }
  },
);

// --- Details handler: emit to right panel ---
function onShowDetails(index) {
  // Prefer fresh data from the form ref, but its state may have been reset
  // after a save/reload — fall back to the cached result in that case.
  const formRef = scenarioRefs.value[index];
  const fresh = formRef?.getExecutionData?.();
  const hasFresh = fresh && (fresh.result || fresh.traceText || fresh.error);
  const data = hasFresh ? fresh : scenarioResults.value.get(index);
  if (data) {
    const scenarioName = formState.value?.scenarios[index]?.name || '';
    emit('executed', {
      result: data.result,
      traceText: data.traceText,
      error: data.error,
      expectations: data.expectations || {},
      scenarioName,
    });
  }
}

// Memoized setup per scenario (avoids new object on every render)
const scenarioSetups = computed(() => {
  if (!formState.value) return [];
  return formState.value.scenarios.map((_, i) => getEffectiveSetup(formState.value, i));
});

function onScenarioFileSelect(event) {
  const filename = event.target.value;
  if (filename) selectScenarioFile(filename);
}

async function onSave() {
  if (!formState.value || !selectedScenarioFile.value) return;

  saveSuccess.value = false;
  try {
    // Sync edited input values back to formState before serializing.
    // This must be inside the try so that a throw from syncEditedValues
    // (e.g. unexpected form shape) surfaces as a save error rather than
    // failing silently.
    for (let i = 0; i < (formState.value.scenarios || []).length; i++) {
      const formRef = scenarioRefs.value[i];
      if (!formRef?.getFormValues) continue;
      const values = formRef.getFormValues();
      syncEditedValues(formState.value, i, values);
    }

    const gherkin = formStateToGherkin(formState.value);
    await saveScenario(selectedScenarioFile.value, gherkin);
    saveSuccess.value = true;
    isDirty.value = false;
    selectedScenarioIndex.value = null;
    setTimeout(() => { saveSuccess.value = false; }, 3000);
  } catch (e) {
    // The composable sets saveError on its own failures. For sync/serialise
    // errors that happen before saveScenario, set it manually so the user
    // still sees the banner instead of an unexplained no-op.
    if (!saveError.value) saveError.value = e;
  }
}

async function onSaveAndShow() {
  const idx = selectedScenarioIndex.value;
  await onSave();
  if (saveError.value || idx === null) return;
  // Saving reloads featureText which resets each ScenarioForm's local result
  // via its setup/scenario watcher. Re-execute so the result sheet reflects
  // the saved scenario rather than a stale cache or an empty state.
  await nextTick();
  const formRef = scenarioRefs.value[idx];
  if (formRef?.execute) {
    formRef.execute();
    const data = formRef.getExecutionData?.();
    if (data) onScenarioResult(idx, data);
  }
  onShowDetails(idx);
}

function cancelEdits() {
  // Discard edits by re-parsing the last-loaded feature text into formState.
  // Each ScenarioForm's deep watcher resets its local inputs when its
  // `scenario`/`setup` props are replaced.
  const text = featureText.value;
  if (text) {
    try {
      const parsed = parseFeature(text);
      formState.value = mapFeatureToForm(parsed);
    } catch { /* keep the previous state */ }
  }
  isDirty.value = false;
  selectedScenarioIndex.value = null;
}

defineExpose({ save: onSave });
</script>

<template>
  <!-- Overview -->
  <nldd-simple-section>
      <div v-if="scenariosLoading" class="sb-loading">Scenario's laden...</div>
      <select
        v-else-if="scenarioFiles.length > 1"
        class="sb-select"
        :value="selectedScenarioFile"
        @change="onScenarioFileSelect"
      >
        <option v-for="sf in scenarioFiles" :key="sf.filename" :value="sf.filename">
          {{ sf.filename }}
        </option>
      </select>

      <nldd-inline-dialog v-if="saveSuccess" text="Opgeslagen"></nldd-inline-dialog>
      <nldd-inline-dialog v-if="saveError" variant="alert" text="Opslaan mislukt" :supporting-text="saveError.message || String(saveError)"></nldd-inline-dialog>

      <template v-if="depsLoading">
        <nldd-spacer size="8"></nldd-spacer>
        <div class="sb-section-title">Afhankelijkheden laden</div>
        <div class="sb-loading">{{ depsProgress }}</div>
      </template>
      <nldd-inline-dialog v-else-if="depsError" variant="alert" text="Fout" :supporting-text="String(depsError)"></nldd-inline-dialog>

      <template v-if="formState">
        <nldd-collection layout="grid" item-width="320px">
          <nldd-card v-for="(scenario, i) in formState.scenarios" :key="i">
            <nldd-container slot="header" padding-top="16" padding-inline="16">
              <nldd-title size="4"><h3>{{ scenario.name }}</h3></nldd-title>
            </nldd-container>
            <nldd-container padding="16">
              <template v-if="scenarioExpectations(i).length">
                <nldd-title size="6"><h4>Verwachte uitkomsten</h4></nldd-title>
                <nldd-spacer size="4"></nldd-spacer>
                <nldd-list variant="simple">
                  <nldd-list-item v-for="(exp, j) in scenarioExpectations(i)" :key="j" size="sm">
                    <nldd-text-cell size="sm" :text="exp.name"></nldd-text-cell>
                    <nldd-text-cell size="sm" horizontal-alignment="right" :text="exp.value"></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </template>
              <template v-else>
                <span class="sb-no-expectations">Geen verwachte uitkomsten gedefinieerd</span>
              </template>
            </nldd-container>
            <nldd-container slot="footer" padding-inline="16" padding-bottom="16">
              <nldd-button-group orientation="horizontal">
                <nldd-button
                  variant="primary"
                  :disabled="!scenarioResults.get(i) || undefined"
                  text="Toon resultaat"
                  @click="onShowDetails(i)"
                ></nldd-button>
                <nldd-button
                  text="Bewerk"
                  @click="selectedScenarioIndex = i"
                ></nldd-button>
              </nldd-button-group>
            </nldd-container>
          </nldd-card>
        </nldd-collection>
      </template>

      <nldd-inline-dialog
        v-else-if="!scenariosLoading && !depsLoading"
        text="Geen scenario's beschikbaar voor deze wet."
      ></nldd-inline-dialog>
    </nldd-simple-section>

  <!-- Edit scenario in a side sheet. All ScenarioForm instances stay
       mounted so auto-execute can cache results for the overview cards. -->
  <Teleport to="body">
    <nldd-sheet
      v-if="formState"
      ref="scenarioSheetEl"
      placement="right"
      width="640px"
      @close="cancelEdits"
    >
      <nldd-page sticky-header :sticky-footer="isDirty || undefined">
        <nldd-top-title-bar
          slot="header"
          :text="selectedScenarioIndex !== null ? formState.scenarios[selectedScenarioIndex].name : ''"
          dismiss-text="Annuleer"
          @dismiss="cancelEdits"
        ></nldd-top-title-bar>
        <nldd-simple-section>
          <ScenarioForm
            v-for="(scenario, i) in formState.scenarios"
            v-show="selectedScenarioIndex === i"
            :key="i"
            :ref="(el) => { scenarioRefs[i] = el; }"
            :scenario="scenario"
            :setup="scenarioSetups[i]"
            :engine="engine"
            :ready="ready"
            :law-id="lawId"
            :article-map="articleMap"
            @show-details="() => onShowDetails(i)"
            @executed="(data) => onScenarioResult(i, data)"
            @change="markDirty"
          />
        </nldd-simple-section>
        <nldd-container v-if="isDirty" slot="footer" padding="16">
          <nldd-button-group orientation="vertical">
            <nldd-button
              variant="primary"
              size="md"
              data-testid="save-scenarios-btn"
              :disabled="saving || undefined"
              :text="saving ? 'Opslaan…' : 'Opslaan'"
              @click="onSave"
            ></nldd-button>
            <nldd-button
              size="md"
              :disabled="saving || undefined"
              text="Opslaan en toon resultaat"
              @click="onSaveAndShow"
            ></nldd-button>
          </nldd-button-group>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<style scoped>
.sb-section-title {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 4px;
  color: var(--semantics-text-color-primary, #1C2029);
}

.sb-select {
  padding: 6px 8px;
  border: 1px solid var(--semantics-dividers-color, #E0E3E8);
  border-radius: 6px;
  font-size: 13px;
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  background: white;
}

.sb-loading {
  font-size: 12px;
  color: var(--semantics-text-color-secondary, #666);
  font-style: italic;
}

/* Card collection */
.sb-no-expectations {
  font-size: 13px;
  color: var(--semantics-text-color-secondary, #545D68);
  font-style: italic;
}
</style>
