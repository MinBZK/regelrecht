<script setup>
import { computed, ref, watch } from 'vue';
import { humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { describeEffect, emptyForm } from '../world/snapshot.js';

// Eén actie uit het wereldbestand: wie haar doet, wat ze uitwerkt, wat de actor
// invult, en of ze nu kan.
//
// Het formulier komt uit `form` in het beeld en niet uit deze code: een veld
// heet zoals de wereld het noemt en heeft het type dat de cel accepteert. Een
// actie die nu niet kan blijft staan met de reden erbij; ze is uit te voeren
// zodra de wereld zegt dat het kan.

const props = defineProps({
  /** De actie uit het beeld. */
  action: { type: Object, required: true },
  /** Staat er een wijziging onderweg? */
  busy: { type: Boolean, default: false },
});

const emit = defineEmits(['run']);

const values = ref(emptyForm(props.action));
/** De velden die nog leeg zijn en bij de laatste poging ingevuld hadden moeten zijn. */
const missing = ref([]);

// Een nieuw beeld geeft dezelfde actie opnieuw; het formulier hoort dan leeg te
// beginnen wanneer de velden zelf veranderen, en anders te blijven staan zoals de
// bezoeker het invulde.
watch(
  () => (props.action.form ?? []).map((field) => `${field.name}:${field.type}`).join(','),
  () => {
    values.value = emptyForm(props.action);
    missing.value = [];
  },
);

const fields = computed(() => props.action.form ?? []);
const effect = computed(() => describeEffect(props.action.effect));

function errorId(field) {
  return `${props.action.id}-${field.name}-fout`;
}

function isMissing(field) {
  return missing.value.includes(field.name);
}

/** Leeg is niet ingevuld; `false` bij een ja/nee-veld is wél een antwoord. */
function isEmpty(field) {
  const value = values.value[field.name];
  if (field.type === 'boolean') return false;
  return value === '' || value === null || value === undefined;
}

function setValue(field, event) {
  const raw = fieldValue(event, values.value[field.name]);
  values.value = {
    ...values.value,
    [field.name]: field.type === 'number' ? (raw === '' || raw === null ? null : Number(raw)) : raw,
  };
  missing.value = missing.value.filter((name) => name !== field.name);
}

function setChecked(field, event) {
  values.value = { ...values.value, [field.name]: Boolean(event?.detail?.checked ?? event?.target?.checked) };
}

function submit() {
  const incomplete = fields.value.filter(isEmpty).map((field) => field.name);
  missing.value = incomplete;
  if (incomplete.length === 0) emit('run', { action: props.action, values: values.value });
}
</script>

<template>
  <nldd-card :accessible-label="action.label">
    <nldd-container slot="header" layout="stack" gap="8" padding="16" padding-bottom="8">
      <nldd-title size="6">
        <span slot="overline">{{ action.id }}</span>
        <span>{{ action.label }}</span>
        <span slot="subtitle">{{ effect }}</span>
      </nldd-title>
      <nldd-container layout="wrap" gap="4">
        <nldd-tag
          size="sm"
          :color="action.available ? 'success' : 'warning'"
          :icon="action.available ? 'check-mark-circle' : 'clock'"
          :text="action.available ? 'kan nu' : 'kan nu niet'"
        ></nldd-tag>
      </nldd-container>
    </nldd-container>

    <nldd-container layout="stack" gap="12" padding="16" padding-top="8">
      <nldd-text v-if="action.doc" size="sm" color="secondary">{{ action.doc }}</nldd-text>

      <nldd-banner
        v-if="!action.available && action.unavailable_reason"
        variant="warning"
        text="Deze actie kan nu niet"
        :supporting-text="action.unavailable_reason"
      ></nldd-banner>

      <!-- Eigen <form> binnen nldd-form: de door het ontwerpsysteem aanbevolen
           modus voor frameworks, zodat de component geen kinderen verplaatst
           die Vue zelf plaatst. -->
      <nldd-form>
        <form @submit.prevent="submit">
          <template v-for="field in fields" :key="field.name">
            <!-- Een ja/nee-veld draagt zijn label zelf; het zou anders twee keer
                 boven hetzelfde veld staan. -->
            <nldd-switch-field
              v-if="field.type === 'boolean'"
              :label="humanize(field.name)"
              :checked="values[field.name] || undefined"
              @change="setChecked(field, $event)"
            ></nldd-switch-field>
            <nldd-form-field v-else :label="humanize(field.name)" :supporting-label="field.type">
              <nldd-number-field
                v-if="field.type === 'number'"
                :value="values[field.name] ?? undefined"
                width="full"
                :invalid="isMissing(field) || undefined"
                :error-message="isMissing(field) ? errorId(field) : undefined"
                @input="setValue(field, $event)"
                @change="setValue(field, $event)"
              ></nldd-number-field>
              <nldd-text-field
                v-else
                :value="values[field.name] ?? ''"
                :invalid="isMissing(field) || undefined"
                :error-message="isMissing(field) ? errorId(field) : undefined"
                @input="setValue(field, $event)"
                @change="setValue(field, $event)"
              ></nldd-text-field>
              <nldd-form-field-error-text v-if="isMissing(field)" :id="errorId(field)">
                Vul {{ humanize(field.name).toLowerCase() }} in; de cel accepteert alleen een {{ field.type }}.
              </nldd-form-field-error-text>
            </nldd-form-field>
          </template>

          <nldd-form-actions>
            <nldd-button-group>
              <nldd-button
                variant="primary"
                type="submit"
                start-icon="send"
                :text="`${action.label} uitvoeren`"
                :loading="busy || undefined"
              ></nldd-button>
            </nldd-button-group>
          </nldd-form-actions>
        </form>
      </nldd-form>
    </nldd-container>
  </nldd-card>
</template>
