<script setup>
// The "waarom?" of one field: why it is on the form, why it has its value,
// and for a value from a register the trace of the run that supplied it.
import TraceKnop from '@regelrecht/frontend-shared/components/TraceKnop.vue';
import WhySteps from './WhySteps.vue';
import { formUnit, formValue, suppliedText } from '../form.js';

defineProps({ field: { type: Object, required: true } });
</script>

<template>
  <TraceKnop
    v-if="field.why"
    :show-trace="field.supplied?.source === 'register'"
    :trace-text="field.supplied?.trace_text ?? null"
    :title="field.label"
    overline="Waarom?"
    :accessible-label="`Waarom: ${field.label}`"
  >
    <nldd-title :size="4"><h3>Waarom dit veld hier staat</h3></nldd-title>
    <WhySteps :steps="field.why.here" />
    <nldd-title :size="4"><h3>Waarom deze waarde</h3></nldd-title>
    <WhySteps :steps="field.why.value" />
    <nldd-rich-text v-if="field.supplied">
      <p>Waarde: {{ formValue(field, field.supplied.value) }}{{ formUnit(field) ? ` ${formUnit(field)}` : '' }}. {{ suppliedText(field) }}.</p>
    </nldd-rich-text>
  </TraceKnop>
</template>
