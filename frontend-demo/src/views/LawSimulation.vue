<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useRouter } from 'vue-router';
import { useEngine } from '../composables/useEngine.js';
import { generatePopulation, histogram } from '../utils/population.js';

const props = defineProps({ lawId: { type: String, required: true } });
const router = useRouter();
const { getDemoIndex } = useEngine();

const lawEntry = ref(null);
const size = ref(1000);
const calculationDate = ref('2025-06-01');
const running = ref(false);
const progress = ref(0);
const progressTotal = ref(0);
const summary = ref(null);
const error = ref(null);
const warning = ref(null);
const buckets = ref([]);
let worker = null;

onMounted(async () => {
  const index = await getDemoIndex();
  lawEntry.value = index.laws.find((l) => l.id === props.lawId);
});

onBeforeUnmount(() => {
  worker?.terminate();
});

async function run() {
  if (!lawEntry.value) return;
  running.value = true;
  error.value = null;
  warning.value = null;
  summary.value = null;
  buckets.value = [];
  progress.value = 0;

  const paths = [...(lawEntry.value.dependencies || []), lawEntry.value.path];
  const lawYamls = await Promise.all(
    paths.map((p) => fetch(`/demo-assets/laws/${p}`).then((r) => {
      if (!r.ok) throw new Error(`law fetch failed (${p}): ${r.status}`);
      return r.text();
    })),
  );
  const population = generatePopulation({
    size: size.value,
    calculationDate: calculationDate.value,
  });
  progressTotal.value = population.length;

  worker?.terminate();
  worker = new Worker(new URL('../workers/simulator.js', import.meta.url), { type: 'module' });
  worker.onmessage = (e) => {
    const msg = e.data;
    if (msg.type === 'progress') {
      progress.value = msg.done;
    } else if (msg.type === 'result') {
      summary.value = msg.summary;
      if (msg.firstError) {
        warning.value = `Let op: ${msg.errorCount} van ${msg.summary.total} berekeningen faalden. Eerste fout: ${msg.firstError}`;
      }
      const amounts = msg.results.filter((r) => r.eligible).map((r) => r.amount / 100);
      buckets.value = histogram(amounts, 20).buckets;
      running.value = false;
    } else if (msg.type === 'error') {
      error.value = msg.message;
      running.value = false;
    }
  };

  worker.postMessage({
    type: 'run',
    lawEntry: { id: lawEntry.value.id, output: lawEntry.value.output },
    lawYamls,
    population,
    calculationDate: calculationDate.value,
  });
}

function updateSize(e) {
  size.value = parseInt(e.target?.value ?? e.detail?.value ?? size.value, 10) || 1000;
}
function updateDate(e) {
  calculationDate.value = e.target?.value ?? e.detail?.value ?? calculationDate.value;
}

const maxBucket = computed(() =>
  buckets.value.reduce((m, b) => Math.max(m, b.count), 0),
);

const avgFormatted = computed(() =>
  summary.value
    ? (summary.value.averageAmount / 100).toLocaleString('nl-NL', {
        style: 'currency',
        currency: 'EUR',
      })
    : '',
);
const medianFormatted = computed(() =>
  summary.value
    ? (summary.value.medianAmount / 100).toLocaleString('nl-NL', {
        style: 'currency',
        currency: 'EUR',
      })
    : '',
);

const summaryRows = computed(() => {
  if (!summary.value) return [];
  return [
    { label: 'Totaal', value: String(summary.value.total) },
    {
      label: 'Rechthebbend',
      value: `${summary.value.eligible} (${(summary.value.percentageEligible * 100).toFixed(1)}%)`,
    },
    { label: 'Gemiddeld bedrag', value: avgFormatted.value },
    { label: 'Mediaan', value: medianFormatted.value },
  ];
});
</script>

<template>
  <nldd-page>
    <nldd-container padding="24">
      <nldd-button
        variant="secondary"
        text="← Terug"
        @click="router.push({ name: 'law-detail', params: { lawId } })"
      ></nldd-button>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-title size="2"><h1>Populatie-simulatie</h1></nldd-title>
      <p v-if="lawEntry">
        Draai <strong>{{ lawEntry.label }}</strong> over een synthetische populatie en kijk
        naar de verdeling van uitkomsten.
      </p>

      <nldd-card>
        <nldd-title slot="header" size="5"><h5>Invoer</h5></nldd-title>
        <nldd-container padding="16">
          <nldd-form-field label="Aantal personen">
            <nldd-number-field
              :value="size"
              @input="updateSize"
            ></nldd-number-field>
          </nldd-form-field>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-form-field label="Peildatum">
            <nldd-text-field
              type="date"
              :value="calculationDate"
              @input="updateDate"
            ></nldd-text-field>
          </nldd-form-field>
          <nldd-spacer size="8"></nldd-spacer>
          <nldd-button
            :text="running ? 'Bezig…' : 'Start simulatie'"
            :disabled="running || !lawEntry"
            @click="run"
          ></nldd-button>
        </nldd-container>
      </nldd-card>

      <nldd-spacer size="16"></nldd-spacer>

      <div v-if="running" class="progress">
        <progress :value="progress" :max="progressTotal"></progress>
        <span>{{ progress }} / {{ progressTotal }}</span>
      </div>

      <p v-if="error" class="error">{{ error }}</p>
      <p v-if="warning" class="warning">{{ warning }}</p>

      <template v-if="summary">
        <nldd-card>
          <nldd-title slot="header" size="5"><h5>Samenvatting</h5></nldd-title>
          <nldd-list variant="box">
            <nldd-list-item v-for="row in summaryRows" :key="row.label" size="md">
              <nldd-description-cell>
                <span slot="title">{{ row.label }}</span>
                <span slot="description">{{ row.value }}</span>
              </nldd-description-cell>
            </nldd-list-item>
          </nldd-list>
        </nldd-card>

        <nldd-spacer size="16"></nldd-spacer>
        <nldd-title size="5"><h5>Verdeling bedragen (€)</h5></nldd-title>
        <nldd-spacer size="4"></nldd-spacer>
        <div class="histogram" :aria-label="`Histogram met ${buckets.length} buckets`">
          <div
            v-for="(b, i) in buckets"
            :key="i"
            class="bar"
            :style="{ height: `${(b.count / (maxBucket || 1)) * 100}%` }"
            :title="`€${b.from.toFixed(0)} – €${b.to.toFixed(0)}: ${b.count}`"
          ></div>
        </div>
      </template>
    </nldd-container>
  </nldd-page>
</template>

<style scoped>
.progress { display: flex; align-items: center; gap: 8px; margin: 16px 0; }
progress { flex: 1; height: 10px; }
.histogram {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 160px;
  padding: 8px;
  background: var(--primitives-color-neutral-50, #fafafa);
  border: 1px solid var(--primitives-color-neutral-200, #e5e5e5);
  border-radius: 4px;
}
.bar { flex: 1; background: var(--primitives-color-accent-500, #1e5bc6); min-height: 2px; }
.error { color: var(--primitives-color-danger-600, #b00020); }
.warning { color: var(--primitives-color-danger-600, #b00020); font-size: 0.9rem; }
</style>
