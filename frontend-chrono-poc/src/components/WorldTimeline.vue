<script setup>
import { computed, ref, watch } from 'vue';
import { formatMoment } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { clockIndex, gramKind, timelineMoments } from '../world/snapshot.js';

// De tijdlijn onder de kolommen: waar de klok staat, elk moment waarop er iets
// ligt als punt op één lijn, en de bediening om de tijd te laten verstrijken.
//
// Eén punt per dag, niet per gram: de dag is de korrel van deze wereld, en op
// een dag waarop drie dingen gebeurden zou drie keer dezelfde datum op de lijn
// staan. Het punt zegt wel wát er die dag ligt — de naam bij één gram, het
// aantal bij meer — en de grammen zelf staan in de kolom van hun cel.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /** Aantal grammen per kroniek vóór de laatste stap; `null` = niets nieuw. */
  previousCounts: { type: Map, default: null },
  /** Staat er een wijziging onderweg? */
  busy: { type: Boolean, default: false },
});

const emit = defineEmits(['advance']);

const clock = computed(() => props.snapshot?.clock ?? null);
const moments = computed(() => timelineMoments(props.snapshot, { newGramCounts: props.previousCounts }));
const current = computed(() => clockIndex(moments.value));

/** De dag waar naartoe gespoeld wordt. Begint op de klok: vooruit, nooit terug. */
const until = ref(clock.value ?? '');
watch(clock, (value) => { until.value = value ?? ''; });

const canAdvance = computed(() => Boolean(until.value) && Boolean(clock.value) && until.value >= clock.value);

/** Wat er op dit punt ligt: de naam bij één gram, het aantal bij meer. */
function label(point) {
  const date = formatMoment(point.moment);
  if (point.grams.length === 1) return `${date} · ${point.grams[0].name}`;
  if (point.grams.length > 1) return `${date} · ${point.grams.length} grammen`;
  return `${date} · klok`;
}

/**
 * Het icoon op de schijf: de klok waar de klok staat, het nieuwe waar iets
 * bijkwam, anders het gram-soort — en bij meer soorten op één dag de stapel.
 */
function icon(point) {
  if (point.isClock) return 'clock';
  if (point.hasNew) return 'new';
  const kinds = new Set(point.grams.map((gram) => gram.kind));
  return kinds.size === 1 ? gramKind([...kinds][0]).icon : 'stack';
}
</script>

<template>
  <nldd-container layout="stack" gap="12" padding="12">
    <nldd-container layout="wrap" gap="16" vertical-alignment="bottom">
      <nldd-container layout="row" gap="8" vertical-alignment="center">
        <nldd-tag color="accent" icon="clock" :text="`Klok: ${formatMoment(clock)}`"></nldd-tag>
        <nldd-text size="sm" color="secondary">logische tijd, geen wandklok</nldd-text>
      </nldd-container>
      <nldd-form-field label="Spoel vooruit tot" supporting-label="een dag vanaf de klok">
        <nldd-date-field
          :value="until"
          :min="clock || undefined"
          @input="until = fieldValue($event, until)"
          @change="until = fieldValue($event, until)"
        ></nldd-date-field>
      </nldd-form-field>
      <nldd-button
        variant="primary"
        start-icon="forward"
        text="Vooruitspoelen"
        :disabled="!canAdvance || busy || undefined"
        :loading="busy || undefined"
        @click="emit('advance', until)"
      ></nldd-button>
    </nldd-container>

    <nldd-step-indicator :current="current" accessible-label="Tijdlijn van de wereld">
      <nldd-step-indicator-item
        v-for="point in moments"
        :key="point.moment"
        :status="point.step"
        :icon="icon(point)"
        :text="label(point)"
      ></nldd-step-indicator-item>
    </nldd-step-indicator>
  </nldd-container>
</template>
