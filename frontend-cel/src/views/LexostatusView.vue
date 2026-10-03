<script setup>
// Query a lexostatus of the cell: pick the lexostatus, fill in the inputs,
// and see the parameters the reduction yields.
import { computed, inject, ref } from 'vue';
import { routeText, valueText } from '../text.js';
import { fieldText } from '../form.js';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const props = defineProps({ lexostatuses: { type: Array, required: true } });
// The chronicle and the lexostatuses of the cell, through the inspection of a process.
const api = inject('cellApi');

const name = ref(props.lexostatuses[0]?.name ?? '');
const input = ref({});
const result = ref(null);
const error = ref('');
const busy = ref(false);

const definition = computed(() => props.lexostatuses.find((l) => l.name === name.value) ?? null);

function choose(e) {
  name.value = e.target.value;
  input.value = {};
  result.value = null;
}

async function query() {
  error.value = '';
  busy.value = true;
  try {
    // Through the engine (experiment A): ask for the trace of the engine run too.
    const trace = definition.value?.reduction?.route === 'engine' ? { engine_trace: '1' } : {};
    result.value = await api.lexostatus(name.value, { ...input.value, ...trace });
  } catch (e) {
    result.value = null;
    error.value = e.message;
  } finally {
    busy.value = false;
  }
}

// Along which route the cell reduced, if the runtime says so.
const route = computed(() => routeText(result.value?.reduction ?? definition.value?.reduction));

// A list lexostatus: a row per case, with the columns as fields.
const list = computed(() => result.value?.list ?? null);
const columns = computed(() => definition.value?.columns ?? []);

const rows = computed(() => {
  const r = result.value;
  if (!r) return [];
  const out = Object.entries(r.parameters ?? {}).map(([n, v]) => ({ name: n, value: valueText(v), kind: 'parameter' }));
  for (const [n, v] of Object.entries(r.extra_fields ?? {})) out.push({ name: n, value: valueText(v), kind: 'extra veld (niet naar de engine)' });
  for (const n of r.not_derived ?? []) out.push({ name: n, value: '', kind: 'niet af te leiden' });
  return out;
});
</script>

<template>
  <nldd-title size="2"><h1>Lexostatus</h1></nldd-title>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="route">
    <nldd-inline-dialog icon="info" text="Route van de reductie" :supporting-text="route"></nldd-inline-dialog>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <nldd-form novalidate @submit.prevent="query">
    <nldd-form-field label="Lexostatus">
      <nldd-dropdown accessible-label="Lexostatus">
        <select :value="name" @change="choose">
          <option v-for="l in lexostatuses" :key="l.name" :value="l.name">{{ l.name }}</option>
        </select>
      </nldd-dropdown>
    </nldd-form-field>
    <nldd-form-field v-for="i in definition?.inputs ?? []" :key="i.name" :label="i.name" :supporting-label="i.type">
      <nldd-text-field
        :value="input[i.name] ?? ''"
        :accessible-label="i.name"
        @input="input = { ...input, [i.name]: fieldText($event) }"
      ></nldd-text-field>
    </nldd-form-field>
    <template v-if="error">
      <nldd-inline-dialog variant="alert" text="De lexostatus is niet op te vragen" :supporting-text="error"></nldd-inline-dialog>
    </template>
    <nldd-form-actions>
      <nldd-button variant="primary" type="submit" text="Opvragen" :loading="busy || undefined"></nldd-button>
    </nldd-form-actions>
  </nldd-form>
  <template v-if="list">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-table
      :columns="['minmax(280px,1.4fr)', ...columns.map(() => 'minmax(120px,1fr)')].join(' ')"
      accessible-label="Regels van de lijst"
      empty-text="Geen regels"
      empty-supporting-text="Een lijst gaat nooit naar de engine."
    >
      <nldd-table-row slot="header">
        <nldd-text-cell text="Zaak (gram-id)"></nldd-text-cell>
        <nldd-text-cell v-for="c in columns" :key="c" :text="c"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="r in list" :key="r.root">
        <nldd-text-cell :text="r.root"></nldd-text-cell>
        <nldd-text-cell v-for="c in columns" :key="c" :text="valueText(r.fields[c])"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </template>
  <template v-else-if="result">
    <nldd-spacer size="24"></nldd-spacer>
    <template v-if="result.reduction?.trace_text">
      <TraceKnop :trace-text="result.reduction.trace_text" :title="`${name} (${result.reduction.regulation})`" />
      <nldd-spacer size="16"></nldd-spacer>
    </template>
    <nldd-table columns="minmax(200px,1fr) minmax(160px,1fr) minmax(200px,1fr)" accessible-label="Parameters van de lexostatus">
      <nldd-table-row slot="header">
        <nldd-text-cell text="Naam"></nldd-text-cell>
        <nldd-text-cell text="Waarde"></nldd-text-cell>
        <nldd-text-cell text="Soort"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="r in rows" :key="r.name">
        <nldd-text-cell :text="r.name"></nldd-text-cell>
        <nldd-text-cell :text="r.value"></nldd-text-cell>
        <nldd-text-cell :text="r.kind"></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
  </template>
</template>
