<script setup>
// The form, built from GET /api/form of the process: the fields of the event
// in the stream of the cell. Labels, types and order come from the form file
// the process provides; without that file the label and the type come from
// the law. What the channel or a register supplies (`supplied`) is shown
// filled in and read-only, and is not sent along: the cell takes it from the
// channel. A field the law lets the applicant leave out says so.
import { computed, inject, onMounted, provide, ref } from 'vue';
import { external, getPath, setPath, suppliedText, withoutSupplied } from '../form.js';
import { provenanceRows, routesFrom, sourceStatusText } from '../text.js';
import InputField from '../components/InputField.vue';
import TableInput from '../components/TableInput.vue';
import FieldWhy from '../components/FieldWhy.vue';
import WhySteps from '../components/WhySteps.vue';
import { fragmentCache } from '../why.js';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const api = inject('api');
// The YAML fragments behind the steps of the "waarom?", fetched once each.
provide('fragment', fragmentCache((path) => api.fragment(path)));
// The application example of the process (`external`), or null.
const examples = inject('examples');
const example = computed(() => examples.value.application);

const props = defineProps({
  // Values already fixed, for example the window of the chosen possibility.
  prefilled: { type: Object, default: () => ({}) },
  // How the form is submitted: by default by the logged-in applicant
  // (POST /api/application); the counter passes its own route.
  send: { type: Function, default: null },
  // Whether the assessment before submitting is available: it assesses the
  // concept of the logged-in applicant, so not at the counter.
  withAssessment: { type: Boolean, default: true },
  title: { type: String, default: null },
});

const emit = defineEmits(['submitted']);

const form = ref(null);
const values = ref({});
const error = ref('');
const assessment = ref(null);
const busy = ref('');

onMounted(async () => {
  try {
    form.value = await api.form();
    for (const f of form.value.fields) {
      values.value[f.name] = props.prefilled[f.name] ?? (f.type === 'table' ? [{}] : null);
    }
  } catch (e) {
    error.value = e.message;
  }
});

// Fields per group, in the order of the form.
const groups = computed(() => {
  const out = [];
  for (const f of form.value?.fields ?? []) {
    const title = f.group ?? '';
    let g = out.find((x) => x.title === title);
    if (!g) out.push((g = { title, fields: [] }));
    g.fields.push(f);
  }
  return out;
});

function set(name, value) {
  values.value = { ...values.value, [name]: value };
  assessment.value = null;
}

async function assess() {
  error.value = '';
  busy.value = 'assessment';
  try {
    assessment.value = await api.assess(external(withoutSupplied(values.value, form.value.fields)));
  } catch (e) {
    error.value = e.message;
  } finally {
    busy.value = '';
  }
}

// An nldd dropdown only updates its displayed value after a choice by the
// user or a new select, not when the value changes from outside. After
// filling in the example, the key rebuilds the form.
const version = ref(0);

// Fill the form with the example; what is fixed beforehand (the chosen
// window) wins.
function fillExample() {
  const out = {};
  for (const f of form.value.fields) {
    const v = props.prefilled[f.name] ?? getPath(example.value, f.name);
    out[f.name] = v ?? (f.type === 'table' ? [{}] : null);
  }
  values.value = out;
  assessment.value = null;
  version.value++;
}

// The example as it is, with what is fixed beforehand on top.
function exampleExternal() {
  const out = withoutSupplied(JSON.parse(JSON.stringify(example.value)), form.value.fields);
  for (const [name, v] of Object.entries(props.prefilled)) setPath(out, name, v);
  return out;
}

async function submit(withExample = false) {
  error.value = '';
  busy.value = withExample ? 'example' : 'submit';
  try {
    const send = props.send ?? api.submit;
    const r = await send(withExample ? exampleExternal() : external(withoutSupplied(values.value, form.value.fields)));
    // The recorded gram with its YAML: {gram, yaml}.
    emit('submitted', r);
  } catch (e) {
    error.value = e.message;
  } finally {
    busy.value = '';
  }
}

const result = computed(() => assessment.value?.result ?? null);

// Per parameter that went to the engine: the value and where it came from,
// the own lexostatus or another cell.
const provenance = computed(() =>
  provenanceRows(assessment.value?.parameters, assessment.value?.provenance, routesFrom(assessment.value)),
);

const sources = computed(() => assessment.value?.sources ?? []);

const resultText = computed(() => {
  const r = result.value;
  if (!r) return '';
  if (!r.to_assess) return 'Niet te beoordelen';
  return `${r.output}: ${r.value === true ? 'ja' : r.value === false ? 'nee' : JSON.stringify(r.value)}`;
});
const resultExplanation = computed(() => {
  const r = result.value;
  if (!r) return '';
  const parts = [];
  if (r.reason) parts.push(r.reason);
  if (r.absent?.length) parts.push(`Ontbreekt: ${r.absent.join(', ')}`);
  const notDerived = assessment.value?.lexostatus?.not_derived ?? [];
  if (notDerived.length) parts.push(`Niet af te leiden uit het concept: ${notDerived.join(', ')}`);
  return parts.join('. ');
});
</script>

<template>
  <nldd-container layout="row" gap="8" vertical-alignment="center">
    <nldd-title size="2">
      <h1>{{ title ?? form?.title ?? 'Indienen' }}</h1>
      <span slot="subtitle" v-if="form">{{ form.event }} in stroom {{ form.stream?.$id }}</span>
    </nldd-title>
    <TraceKnop
      v-if="form?.why"
      :trace="false"
      titel="Waarom ziet deze aanvraag er zo uit?"
      overline="Waarom?"
      accessible-label="Waarom ziet deze aanvraag er zo uit?"
    >
      <WhySteps :steps="form.why.event" />
      <template v-if="form.why.excluded?.length">
        <nldd-title :size="4"><h3>Niet in de aanvraag</h3></nldd-title>
        <nldd-list>
          <nldd-list-item v-for="x in form.why.excluded" :key="x.article + x.parameter">
            <nldd-container padding-block="8">
              <nldd-rich-text>
                <p><code>{{ x.parameter }}</code> ({{ x.article }}): {{ x.reason }}</p>
              </nldd-rich-text>
            </nldd-container>
          </nldd-list-item>
        </nldd-list>
      </template>
    </TraceKnop>
  </nldd-container>
  <nldd-spacer size="16"></nldd-spacer>
  <template v-if="form && example">
    <nldd-button-group orientation="horizontal">
      <nldd-button variant="secondary" text="Voorbeeld invullen" @click="fillExample"></nldd-button>
      <nldd-button
        variant="secondary"
        text="Direct indienen met voorbeeld"
        :loading="busy === 'example' || undefined"
        @click="submit(true)"
      ></nldd-button>
    </nldd-button-group>
    <nldd-spacer size="16"></nldd-spacer>
  </template>
  <template v-if="form">
    <nldd-form :key="version" novalidate @submit.prevent="submit()">
      <slot name="before"></slot>
      <template v-for="g in groups" :key="g.title">
        <nldd-form-section v-if="g.title" :text="g.title"></nldd-form-section>
        <template v-for="f in g.fields" :key="f.name">
          <nldd-form-field v-if="f.type === 'checkbox'" label="">
            <nldd-container layout="row" gap="8" vertical-alignment="center">
              <InputField :kind="f.type" :label="f.label" :model-value="values[f.name]" @update:model-value="set(f.name, $event)" />
              <FieldWhy :field="f" />
            </nldd-container>
          </nldd-form-field>
          <nldd-form-field v-else-if="f.supplied" :label="f.label" :supporting-label="suppliedText(f)">
            <nldd-container layout="row" gap="8" vertical-alignment="center">
              <nldd-text-field readonly :value="String(f.supplied.value ?? '')" :accessible-label="f.label"></nldd-text-field>
              <FieldWhy :field="f" />
            </nldd-container>
          </nldd-form-field>
          <nldd-form-field
            v-else
            :label="f.label"
            :supporting-label="f.name !== f.label ? f.name : undefined"
            :optional="f.optional || undefined"
            :optional-label="f.optional ? 'niet verplicht' : undefined"
          >
            <nldd-container layout="row" gap="8" vertical-alignment="center">
              <TableInput
                v-if="f.type === 'table'"
                :label="f.label"
                :columns="f.columns ?? []"
                :model-value="values[f.name] ?? []"
                @update:model-value="set(f.name, $event)"
              />
              <InputField
                v-else
                :kind="f.type"
                :label="f.label"
                :choices="f.options"
                :model-value="values[f.name]"
                @update:model-value="set(f.name, $event)"
              />
              <FieldWhy :field="f" />
            </nldd-container>
          </nldd-form-field>
        </template>
      </template>
      <template v-if="result">
        <nldd-container layout="row" gap="8" vertical-alignment="center">
          <nldd-inline-dialog
            :variant="result.to_assess && result.value === true ? 'success' : 'alert'"
            :text="resultText"
            :supporting-text="resultExplanation"
          ></nldd-inline-dialog>
          <TraceKnop v-if="result.trace_text" :trace-text="result.trace_text" :titel="result.output" />
        </nldd-container>
      </template>
      <template v-if="provenance.length">
        <nldd-table columns="minmax(200px,1fr) 160px minmax(200px,1fr)" accessible-label="Herkomst per parameter">
          <nldd-table-row slot="header">
            <nldd-text-cell text="Parameter"></nldd-text-cell>
            <nldd-text-cell text="Waarde"></nldd-text-cell>
            <nldd-text-cell text="Herkomst"></nldd-text-cell>
          </nldd-table-row>
          <nldd-table-row v-for="p in provenance" :key="p.name">
            <nldd-text-cell :text="p.name"></nldd-text-cell>
            <nldd-text-cell :text="p.value"></nldd-text-cell>
            <nldd-text-cell :text="p.source"></nldd-text-cell>
          </nldd-table-row>
        </nldd-table>
        <template v-for="s in sources" :key="s.cell + s.lexostatus">
          <nldd-inline-dialog
            v-if="s.status !== 'queried'"
            variant="alert"
            :text="`Bron ${s.cell}: ${sourceStatusText(s.status)}`"
            :supporting-text="s.error"
          ></nldd-inline-dialog>
        </template>
      </template>
      <template v-if="error">
        <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="error"></nldd-inline-dialog>
      </template>
      <nldd-form-actions>
        <nldd-button-group>
          <nldd-button
            v-if="withAssessment"
            variant="secondary"
            text="Controleer"
            :loading="busy === 'assessment' || undefined"
            @click="assess"
          ></nldd-button>
          <nldd-button
            variant="primary"
            type="submit"
            text="Indienen"
            :loading="busy === 'submit' || undefined"
          ></nldd-button>
        </nldd-button-group>
      </nldd-form-actions>
    </nldd-form>
  </template>
  <template v-else-if="error">
    <nldd-inline-dialog variant="alert" text="De stroom is niet te laden" :supporting-text="error"></nldd-inline-dialog>
  </template>
</template>
