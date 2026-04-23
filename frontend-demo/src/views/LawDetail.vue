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
  <nldd-page>
    <nldd-container padding="24">
      <nldd-button
        variant="secondary"
        text="← Terug"
        @click="router.push({ name: 'dashboard' })"
      ></nldd-button>
      <nldd-spacer size="8"></nldd-spacer>

      <template v-if="lawEntry">
        <nldd-title size="2"><h1>{{ lawEntry.label }}</h1></nldd-title>
        <p>{{ lawEntry.summary }}</p>
        <nldd-spacer size="16"></nldd-spacer>

        <nldd-card>
          <nldd-title slot="header" size="5"><h5>Invoer</h5></nldd-title>
          <nldd-container padding="16">
            <nldd-form-field label="Peildatum">
              <nldd-text-field
                type="date"
                :value="calculationDate"
                @input="updateDate"
              ></nldd-text-field>
            </nldd-form-field>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-button text="Bereken opnieuw" @click="run"></nldd-button>
          </nldd-container>
        </nldd-card>

        <nldd-spacer size="16"></nldd-spacer>

        <nldd-card>
          <nldd-title slot="header" size="5"><h5>Uitkomst</h5></nldd-title>
          <nldd-container padding="16">
            <p v-if="loading">Rekenen…</p>
            <p v-else-if="error" class="error">{{ error }}</p>
            <template v-else-if="primaryOutput !== null && primaryOutput !== undefined">
              <p class="amount">
                <strong>{{ formattedAmount ?? primaryOutput }}</strong>
                <small>per {{ lawEntry.output }}</small>
              </p>
              <nldd-toggle-button
                :pressed="showTrace"
                text="Toon redenering"
                @click="showTrace = !showTrace"
              ></nldd-toggle-button>
              <nldd-button
                variant="secondary"
                :text="explanationLoading ? 'Uitleg laden…' : 'Vraag uitleg'"
                :disabled="explanationLoading"
                @click="requestExplanation"
              ></nldd-button>
              <nldd-spacer size="8"></nldd-spacer>
              <div v-if="showTrace" class="trace">
                <h6>Gebruikte invoer</h6>
                <pre>{{ JSON.stringify(lastResult?.resolved_inputs ?? {}, null, 2) }}</pre>
                <h6>Trace</h6>
                <pre>{{ lastResult?.trace_text ?? JSON.stringify(lastResult?.trace ?? {}, null, 2) }}</pre>
              </div>
              <div v-if="explanationError" class="error">{{ explanationError }}</div>
              <nldd-box v-if="explanation">
                <nldd-title size="6"><h6>Uitleg</h6></nldd-title>
                <p class="explanation-text">{{ explanation }}</p>
              </nldd-box>
            </template>
            <p v-else>Nog niets berekend.</p>
          </nldd-container>
        </nldd-card>

        <nldd-spacer size="16"></nldd-spacer>
        <nldd-button
          variant="secondary"
          text="Simuleer voor 1000 burgers"
          @click="goToSimulation"
        ></nldd-button>
      </template>

      <template v-else>
        <p>Wet niet gevonden.</p>
      </template>
    </nldd-container>
  </nldd-page>
</template>

<style scoped>
.amount { font-size: 1.5rem; margin: 0.5em 0; }
.amount small { display: block; font-size: 0.8rem; color: var(--primitives-color-neutral-600, #666); }
.error { color: var(--primitives-color-danger-600, #b00020); }
.trace pre { font-size: 0.75rem; white-space: pre-wrap; word-break: break-word; background: var(--primitives-color-neutral-50, #fafafa); padding: 8px; border-radius: 4px; }
.explanation-text { margin: 0; white-space: pre-wrap; }
</style>
