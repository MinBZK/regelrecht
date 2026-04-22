<script setup>
import { ref, computed, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { useEngine } from '../composables/useEngine.js';

const props = defineProps({
  lawId: { type: String, required: true },
});

const router = useRouter();
const { getDemoIndex, getProfile, evaluate, loading, error, lastResult } = useEngine();

const profile = ref(null);
const lawEntry = ref(null);
const calculationDate = ref('2025-06-01');
const showTrace = ref(false);
const explanation = ref(null);
const explanationLoading = ref(false);
const explanationError = ref(null);

onMounted(async () => {
  const [index, merijn] = await Promise.all([getDemoIndex(), getProfile('merijn')]);
  profile.value = merijn;
  lawEntry.value = index.laws.find((l) => l.id === props.lawId);
  if (lawEntry.value) await run();
});

async function run() {
  if (!lawEntry.value || !profile.value) return;
  await evaluate({
    lawEntry: lawEntry.value,
    profile: profile.value,
    parameters: { bsn: profile.value.bsn },
    calculationDate: calculationDate.value,
  });
}

function updateDate(e) {
  calculationDate.value = e.target?.value ?? e.detail?.value ?? calculationDate.value;
}

const primaryOutput = computed(() => {
  const r = lastResult.value;
  if (!r || !lawEntry.value) return null;
  return r.outputs?.[lawEntry.value.output];
});

const formattedAmount = computed(() => {
  const v = primaryOutput.value;
  if (typeof v !== 'number' && typeof v !== 'bigint') return null;
  return (Number(v) / 100).toLocaleString('nl-NL', {
    style: 'currency',
    currency: 'EUR',
  });
});

function goToSimulation() {
  router.push({ name: 'law-simulation', params: { lawId: props.lawId } });
}

async function requestExplanation() {
  if (!lawEntry.value || !lastResult.value) return;
  explanationLoading.value = true;
  explanationError.value = null;
  explanation.value = null;
  try {
    const resp = await fetch('/api/explain', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        law_id: lawEntry.value.id,
        law_label: lawEntry.value.label,
        output_name: lawEntry.value.output,
        parameters: { bsn: profile.value?.bsn },
        result: lastResult.value,
        trace: lastResult.value?.trace ?? null,
        profile_summary: profile.value?.description ?? '',
      }),
    });
    if (!resp.ok) {
      explanationError.value = `Uitleg-service gaf ${resp.status}`;
      return;
    }
    const body = await resp.json();
    explanation.value = body.explanation;
  } catch (e) {
    explanationError.value = e?.message || String(e);
  } finally {
    explanationLoading.value = false;
  }
}
</script>

<template>
  <ndd-page>
    <ndd-container padding="24">
      <ndd-back-button text="Terug" @click="router.push({ name: 'dashboard' })"></ndd-back-button>
      <ndd-spacer size="8"></ndd-spacer>

      <template v-if="lawEntry">
        <ndd-title size="2"><h1>{{ lawEntry.label }}</h1></ndd-title>
        <p>{{ lawEntry.summary }}</p>
        <ndd-spacer size="16"></ndd-spacer>

        <ndd-container padding="16" class="form-card">
          <ndd-title size="5"><h5>Invoer</h5></ndd-title>
          <ndd-spacer size="4"></ndd-spacer>
          <label>Peildatum
            <ndd-text-field
              type="date"
              :value="calculationDate"
              @input="updateDate"
            ></ndd-text-field>
          </label>
          <ndd-spacer size="8"></ndd-spacer>
          <ndd-button text="Bereken opnieuw" @click="run"></ndd-button>
        </ndd-container>

        <ndd-spacer size="16"></ndd-spacer>

        <ndd-container padding="16" class="result-card">
          <ndd-title size="5"><h5>Uitkomst</h5></ndd-title>
          <ndd-spacer size="4"></ndd-spacer>
          <p v-if="loading">Rekenen…</p>
          <p v-else-if="error" class="error">{{ error }}</p>
          <template v-else-if="primaryOutput !== null && primaryOutput !== undefined">
            <p class="amount">
              <strong>{{ formattedAmount ?? primaryOutput }}</strong>
              <small>per {{ lawEntry.output }}</small>
            </p>
            <ndd-toggle-button
              :pressed="showTrace"
              text="Toon redenering"
              @click="showTrace = !showTrace"
            ></ndd-toggle-button>
            <ndd-button
              variant="secondary"
              :text="explanationLoading ? 'Uitleg laden…' : 'Vraag uitleg'"
              :disabled="explanationLoading"
              @click="requestExplanation"
            ></ndd-button>
            <ndd-spacer size="8"></ndd-spacer>
            <div v-if="showTrace" class="trace">
              <h6>Gebruikte invoer</h6>
              <pre>{{ JSON.stringify(lastResult?.resolved_inputs ?? {}, null, 2) }}</pre>
              <h6>Trace</h6>
              <pre>{{ lastResult?.trace_text ?? JSON.stringify(lastResult?.trace ?? {}, null, 2) }}</pre>
            </div>
            <div v-if="explanationError" class="error">{{ explanationError }}</div>
            <div v-if="explanation" class="explanation">
              <h6>Uitleg</h6>
              <p>{{ explanation }}</p>
            </div>
          </template>
          <p v-else>Nog niets berekend.</p>
        </ndd-container>

        <ndd-spacer size="16"></ndd-spacer>
        <ndd-button
          variant="secondary"
          text="Simuleer voor 1000 burgers"
          @click="goToSimulation"
        ></ndd-button>
      </template>

      <template v-else>
        <p>Wet niet gevonden.</p>
      </template>
    </ndd-container>
  </ndd-page>
</template>

<style scoped>
.form-card, .result-card { border: 1px solid var(--ndd-color-neutral-200, #e5e5e5); border-radius: 8px; }
.amount { font-size: 1.5rem; margin: 0.5em 0; }
.amount small { display: block; font-size: 0.8rem; color: var(--ndd-color-neutral-600, #666); }
.error { color: var(--ndd-color-red-600, #b00020); }
.trace pre { font-size: 0.75rem; white-space: pre-wrap; word-break: break-word; background: var(--ndd-color-neutral-50, #fafafa); padding: 8px; border-radius: 4px; }
.explanation { margin-top: 16px; padding: 12px; background: var(--ndd-color-primary-50, #eef3ff); border-left: 4px solid var(--ndd-color-primary-500, #1e5bc6); border-radius: 4px; }
.explanation p { margin: 0; white-space: pre-wrap; }
</style>
