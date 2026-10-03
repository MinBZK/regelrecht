<script setup>
// An action in a case: its form, a trial and recording. What the form asks
// for comes from the runtime (from the law: the verdicts, what the stage
// requires, or the fields of the event), not from code. A trial records
// nothing: it says which outputs the engine gives, or what is still missing,
// and per parameter where it came from. The cell does the recording; if it
// refuses, the chronicle stays as it was. If the trial says no on the content
// (`reportable`), the process does not take the action by itself; if the fact
// happened anyway, the handler reports it and the cell records it.
import { computed, inject, ref } from 'vue';
import InputField from './InputField.vue';
import { kindOf, outputText, provenanceRows, routesFrom, sourceStatusText } from '../text.js';
import { fieldLabel, inputKind, toForm, toLaw } from '../form.js';
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';

const props = defineProps({
  root: { type: String, required: true },
  // As the case describes it: name, label, kind, form, trial ...
  action: { type: Object, required: true },
});
const emit = defineEmits(['recorded']);
const api = inject('api');
const examples = inject('examples');
// The example form of this action, or null.
const example = computed(() => examples.value.actions?.[props.action.name] ?? null);

const values = ref(Object.fromEntries(props.action.form.map((f) => [f.name, null])));
const trial = ref(props.action.trial?.error ? null : props.action.trial);
const taken = ref(null);
const error = ref(props.action.trial?.error ?? '');
const busy = ref('');

const groups = computed(() => {
  const out = [];
  for (const f of props.action.form) {
    const title = f.group ?? (f.kind === 'fact' ? 'Wat er gebeurde' : '');
    let g = out.find((x) => x.title === title);
    if (!g) out.push((g = { title, fields: [] }));
    g.fields.push(f);
  }
  return out;
});

function set(name, value) {
  values.value = { ...values.value, [name]: value === '' ? null : value };
}

// What is filled in, with amounts in the unit of the law.
function formData(source) {
  const out = {};
  for (const f of props.action.form) {
    const v = source[f.name];
    if (v === null || v === undefined) continue;
    out[f.name] = toLaw(f, v);
  }
  return out;
}

// An nldd dropdown only updates its displayed value after a choice by the
// user; after filling in the example, the key rebuilds the form.
const version = ref(0);

// The example in the units of the form.
function exampleValues() {
  return Object.fromEntries(
    props.action.form.map((f) => [f.name, toForm(f, example.value?.[f.name] ?? null)]),
  );
}

function fillExample() {
  values.value = exampleValues();
  trial.value = null;
  version.value++;
}

async function runTrial() {
  error.value = '';
  busy.value = 'trial';
  try {
    trial.value = await api.trialAction(props.root, props.action.name, formData(values.value));
  } catch (e) {
    error.value = e.message;
  } finally {
    busy.value = '';
  }
}

async function record(withExample = false, happened = false) {
  error.value = '';
  busy.value = withExample ? 'example' : happened ? 'report' : 'record';
  try {
    const result = await api.takeAction(
      props.root,
      props.action.name,
      formData(withExample ? exampleValues() : values.value),
      happened,
    );
    taken.value = result;
    trial.value = result.trial;
    emit('recorded', result);
  } catch (e) {
    error.value = e.message;
  } finally {
    busy.value = '';
  }
}

const outputs = computed(() =>
  Object.entries({ ...(trial.value?.outputs ?? {}), ...(trial.value?.assessments ?? {}) }).map(([name, v]) => ({
    name,
    value: outputText(v, trial.value?.types?.[name]),
  })),
);
const provenance = computed(() =>
  provenanceRows(trial.value?.parameters, trial.value?.provenance, routesFrom(trial.value)),
);
const notDelivered = computed(() => trial.value?.not_delivered ?? []);
// The kind comes as {kind, ...} (for a follow-up with the action of the
// decision included).
const kind = computed(() => kindOf(props.action));
const kindLabel = computed(() => {
  const a = props.action;
  if (kind.value === 'decision') return `besluit, stage ${a.stage}`;
  if (kind.value === 'follow_up') return `stage ${a.stage} van het besluit (${a.kind.decision})`;
  return 'feit uit het verloop van de zaak';
});
</script>

<template>
  <nldd-title size="3">
    <h2>{{ action.label }}</h2>
    <span slot="subtitle">{{ kindLabel }}; {{ action.article }}</span>
  </nldd-title>
  <nldd-spacer size="8"></nldd-spacer>
  <template v-if="error">
    <nldd-inline-dialog variant="alert" text="Dat lukte niet" :supporting-text="error"></nldd-inline-dialog>
    <nldd-spacer size="8"></nldd-spacer>
  </template>
  <template v-if="!action.available && !taken">
    <nldd-inline-dialog text="Niet in deze stand van de zaak" :supporting-text="action.reason"></nldd-inline-dialog>
  </template>
  <template v-else>
    <template v-if="example && taken === null">
      <nldd-button-group orientation="horizontal">
        <nldd-button variant="secondary" text="Voorbeeld invullen" @click="fillExample"></nldd-button>
        <nldd-button
          variant="secondary"
          text="Direct vastleggen met voorbeeld"
          :loading="busy === 'example' || undefined"
          @click="record(true)"
        ></nldd-button>
      </nldd-button-group>
      <nldd-spacer size="16"></nldd-spacer>
    </template>
    <nldd-form :key="version" novalidate @submit.prevent="runTrial">
      <template v-for="g in groups" :key="g.title">
        <nldd-form-section v-if="g.title" :text="g.title"></nldd-form-section>
        <nldd-form-field
          v-for="f in g.fields"
          :key="f.name"
          :label="fieldLabel(f)"
          :supporting-label="f.name"
        >
          <InputField :kind="inputKind(f)" :label="f.label" :model-value="values[f.name]" @update:model-value="set(f.name, $event)" />
        </nldd-form-field>
      </template>
      <nldd-form-actions>
        <nldd-button variant="primary" type="submit" text="Op proef" :loading="busy === 'trial' || undefined"></nldd-button>
        <nldd-button
          variant="secondary"
          type="button"
          text="Vastleggen"
          :loading="busy === 'record' || undefined"
          :disabled="(taken !== null && kind !== 'fact') || undefined"
          @click="record()"
        ></nldd-button>
      </nldd-form-actions>
    </nldd-form>
  </template>

  <template v-if="trial">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="4">
      <h3>{{ taken ? 'Uitgerekend bij het vastleggen' : 'Op proef' }}</h3>
      <span slot="subtitle">
        {{ trial.article }}, peildatum {{ trial.reference_date }} ({{ trial.reference_date_from }}).{{ taken ? '' : ' Er is niets vastgelegd.' }}
      </span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-inline-dialog
        :variant="trial.takeable ? 'success' : 'alert'"
        :text="trial.takeable ? 'Te nemen' : 'Niet te nemen'"
        :supporting-text="trial.reason"
      ></nldd-inline-dialog>
      <TraceKnop v-if="trial.trace_text" :trace-text="trial.trace_text" :title="trial.article" />
    </nldd-container>
    <template v-if="trial.reportable && !taken">
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-inline-dialog
        icon="info"
        text="Het proces doet dit niet uit zichzelf"
        supporting-text="Is het toch gebeurd, meld het dan: de cel legt het vast, en de zaak toont de gevolgen."
      ></nldd-inline-dialog>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-button
        variant="secondary"
        text="Het is gebeurd: vastleggen"
        :loading="busy === 'report' || undefined"
        @click="record(false, true)"
      ></nldd-button>
    </template>
    <template v-if="outputs.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-table columns="minmax(240px,1fr) minmax(160px,1fr)" accessible-label="Uitkomsten">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Uitkomst"></nldd-text-cell>
          <nldd-text-cell text="Waarde"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="o in outputs" :key="o.name">
          <nldd-text-cell :text="o.name"></nldd-text-cell>
          <nldd-text-cell :text="o.value"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </template>
    <template v-for="r in trial.rows ?? []" :key="r.parameter">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Samengesteld per regel: {{ r.parameter }}</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(200px,1fr) minmax(200px,1fr) 120px 140px" accessible-label="Bronnen per regel">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Cel"></nldd-text-cell>
          <nldd-text-cell text="Lexostatus"></nldd-text-cell>
          <nldd-text-cell text="Regels"></nldd-text-cell>
          <nldd-text-cell text="Status"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="s in r.sources" :key="s.cell + s.lexostatus">
          <nldd-text-cell :text="s.cell" :supporting-text="s.transport"></nldd-text-cell>
          <nldd-text-cell
            :text="s.lexostatus"
            :supporting-text="s.reduction ? `reductie via ${s.reduction === 'engine' ? 'de engine' : s.reduction}` : undefined"
          ></nldd-text-cell>
          <nldd-text-cell :text="String(s.queried)"></nldd-text-cell>
          <nldd-text-cell :text="sourceStatusText(s.status)" :supporting-text="s.error"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
      <template v-if="r.missing?.length">
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-inline-dialog
          icon="warning"
          icon-color="warning"
          :text="`Kolommen zonder waarde: ${r.missing.join(', ')}`"
          supporting-text="Er wordt niets aangevuld."
        ></nldd-inline-dialog>
      </template>
    </template>
    <template v-for="s in trial.sources" :key="s.cell + s.lexostatus">
      <nldd-inline-dialog
        v-if="s.status !== 'queried'"
        variant="alert"
        :text="`Bron ${s.cell}: ${sourceStatusText(s.status)}`"
        :supporting-text="s.error"
      ></nldd-inline-dialog>
    </template>
    <template v-if="notDelivered.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Niet geleverd</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(220px,1fr) minmax(200px,1fr) minmax(300px,2fr)" accessible-label="Niet geleverde parameters">
        <nldd-table-row slot="header">
          <nldd-text-cell text="Parameter"></nldd-text-cell>
          <nldd-text-cell text="Artikel"></nldd-text-cell>
          <nldd-text-cell text="Herkomst volgens het model"></nldd-text-cell>
        </nldd-table-row>
        <nldd-table-row v-for="n in notDelivered" :key="n.name">
          <nldd-text-cell :text="n.name" :supporting-text="n.type"></nldd-text-cell>
          <nldd-text-cell :text="n.article"></nldd-text-cell>
          <nldd-text-cell :text="n.description ?? ''"></nldd-text-cell>
        </nldd-table-row>
      </nldd-table>
    </template>
    <template v-if="provenance.length">
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-title size="4"><h3>Herkomst per parameter</h3></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-table columns="minmax(220px,1fr) 160px minmax(220px,1fr)" accessible-label="Herkomst per parameter">
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
    </template>
  </template>

  <template v-if="taken">
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="4">
      <h3>Vastgelegd</h3>
      <span slot="subtitle">{{ taken.gram.type }}{{ taken.gram.stage ? `, stage ${taken.gram.stage}` : '' }}, op {{ taken.gram.effective_at }}</span>
    </nldd-title>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-inline-dialog variant="success" text="De cel heeft het gram vastgelegd" :supporting-text="taken.gram.name"></nldd-inline-dialog>
    <nldd-inline-dialog
      v-for="w in taken.warnings ?? []"
      :key="w"
      icon="warning"
      icon-color="warning"
      text="Waarschuwing"
      :supporting-text="w"
    ></nldd-inline-dialog>
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-code-viewer language="yaml" wrap>{{ taken.yaml }}</nldd-code-viewer>
  </template>
</template>
