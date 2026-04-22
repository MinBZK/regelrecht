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
  summary.value = null;
  buckets.value = [];
  progress.value = 0;

  const lawBasename = lawEntry.value.path.split('/').pop();
  const lawYaml = await fetch(`/demo-assets/laws/${lawBasename}`).then((r) => r.text());
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
    lawYaml,
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
</script>

<template>
  <ndd-page>
    <ndd-container padding="24">
      <ndd-back-button
        text="Terug"
        @click="router.push({ name: 'law-detail', params: { lawId } })"
      ></ndd-back-button>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-title size="2"><h1>Populatie-simulatie</h1></ndd-title>
      <p v-if="lawEntry">
        Draai <strong>{{ lawEntry.label }}</strong> over een synthetische populatie en kijk
        naar de verdeling van uitkomsten.
      </p>

      <ndd-container padding="16" class="form-card">
        <label>Aantal personen
          <ndd-number-field :value="size" @input="updateSize"></ndd-number-field>
        </label>
        <ndd-spacer size="8"></ndd-spacer>
        <label>Peildatum
          <ndd-text-field type="date" :value="calculationDate" @input="updateDate"></ndd-text-field>
        </label>
        <ndd-spacer size="8"></ndd-spacer>
        <ndd-button
          :text="running ? 'Bezig…' : 'Start simulatie'"
          :disabled="running || !lawEntry"
          @click="run"
        ></ndd-button>
      </ndd-container>

      <ndd-spacer size="16"></ndd-spacer>

      <div v-if="running" class="progress">
        <progress :value="progress" :max="progressTotal"></progress>
        <span>{{ progress }} / {{ progressTotal }}</span>
      </div>

      <p v-if="error" class="error">{{ error }}</p>

      <template v-if="summary">
        <ndd-container padding="16" class="summary-card">
          <ndd-title size="5"><h5>Samenvatting</h5></ndd-title>
          <ndd-spacer size="4"></ndd-spacer>
          <dl>
            <dt>Totaal</dt><dd>{{ summary.total }}</dd>
            <dt>Rechthebbend</dt>
            <dd>{{ summary.eligible }} ({{ (summary.percentageEligible * 100).toFixed(1) }}%)</dd>
            <dt>Gemiddeld bedrag</dt><dd>{{ avgFormatted }}</dd>
            <dt>Mediaan</dt><dd>{{ medianFormatted }}</dd>
          </dl>
        </ndd-container>

        <ndd-spacer size="16"></ndd-spacer>
        <ndd-title size="5"><h5>Verdeling bedragen (€)</h5></ndd-title>
        <ndd-spacer size="4"></ndd-spacer>
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
    </ndd-container>
  </ndd-page>
</template>

<style scoped>
.form-card, .summary-card {
  border: 1px solid var(--ndd-color-neutral-200, #e5e5e5);
  border-radius: 8px;
}
.progress { display: flex; align-items: center; gap: 8px; margin: 16px 0; }
progress { flex: 1; height: 10px; }
dl { display: grid; grid-template-columns: 200px 1fr; row-gap: 4px; margin: 0; }
dt { font-weight: 600; }
.histogram {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 160px;
  padding: 8px;
  background: var(--ndd-color-neutral-50, #fafafa);
  border: 1px solid var(--ndd-color-neutral-200, #e5e5e5);
  border-radius: 4px;
}
.bar { flex: 1; background: var(--ndd-color-primary-500, #1e5bc6); min-height: 2px; }
.error { color: var(--ndd-color-red-600, #b00020); }
</style>
