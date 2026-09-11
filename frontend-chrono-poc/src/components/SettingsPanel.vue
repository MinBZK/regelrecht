<script setup>
import { computed, ref, watch } from 'vue';
import { formatValue, humanize } from '../world/format.js';
import { fieldValue } from '../world/events.js';
import { settingRows } from '../world/snapshot.js';

// De instellingen van deze wereld, en het terugzetten ervan.
//
// Een instelling die een besluit al gebruikte ligt vast; dat staat erbij met de
// cel en het besluit die haar gebruikten, zodat te zien is welke knop nog om kan
// zonder het te hoeven proberen. Welke instellingen er zijn en wat ze heten komt
// uit het wereldbestand — deze code kent er geen enkele bij naam.

const props = defineProps({
  /** Het beeld van de wereld. */
  snapshot: { type: Object, default: null },
  /** Staat er een wijziging onderweg? */
  busy: { type: Boolean, default: false },
});

const emit = defineEmits(['save', 'reset']);

const rows = computed(() => settingRows(props.snapshot));
/** De waarden zoals ze in het formulier staan. */
const draft = ref({});

watch(
  rows,
  (current) => {
    draft.value = Object.fromEntries(current.map((row) => [row.name, row.value]));
  },
  { immediate: true, deep: true },
);

/**
 * Alleen wat veranderd is gaat mee: de server weigert een vaste instelling.
 *
 * Een veld dat leeggemaakt is (`null`) is geen wijziging maar een lege hand: wie
 * dat als waarde zou versturen, zet een instelling op nul zonder dat iemand nul
 * bedoelde.
 */
const changes = computed(() =>
  Object.fromEntries(
    rows.value
      .filter((row) => !row.locked && draft.value[row.name] !== null && draft.value[row.name] !== row.value)
      .map((row) => [row.name, draft.value[row.name]]),
  ),
);
const changed = computed(() => Object.keys(changes.value).length > 0);

function typeOf(value) {
  if (typeof value === 'boolean') return 'boolean';
  if (typeof value === 'number') return 'number';
  return 'string';
}

function setValue(row, event) {
  const raw = fieldValue(event, draft.value[row.name]);
  const empty = raw === '' || raw === null || raw === undefined;
  draft.value = {
    ...draft.value,
    // Een leeggemaakt getalveld is niets ingevuld en niet nul: `Number('')` is 0,
    // en dat zou een instelling op nul zetten alleen omdat het veld leeg was.
    [row.name]: typeOf(row.value) === 'number' ? (empty ? null : Number(raw)) : raw,
  };
}

function setChecked(row, event) {
  draft.value = { ...draft.value, [row.name]: Boolean(event?.detail?.checked ?? event?.target?.checked) };
}

const resetDialog = ref(null);

function confirmReset() {
  resetDialog.value?.hide?.();
  emit('reset');
}
</script>

<template>
  <nldd-container layout="stack" gap="24">
    <nldd-container layout="stack" gap="12">
      <nldd-title size="5">
        <span>Instellingen</span>
        <span slot="subtitle">beleid uit het wereldbestand, geen wet</span>
      </nldd-title>

      <nldd-form>
        <form @submit.prevent="changed && emit('save', changes)">
          <template v-for="row in rows" :key="row.name">
            <nldd-switch-field
              v-if="typeOf(row.value) === 'boolean'"
              :label="humanize(row.name)"
              :checked="draft[row.name] || undefined"
              :disabled="Boolean(row.locked) || undefined"
              @change="setChecked(row, $event)"
            ></nldd-switch-field>
            <nldd-form-field
              v-else
              :label="humanize(row.name)"
              :supporting-label="row.locked ? `vast: gebruikt door besluit '${row.locked.besluit}' van cel '${row.locked.cell}'` : typeOf(row.value)"
            >
              <nldd-number-field
                v-if="typeOf(row.value) === 'number'"
                :value="draft[row.name] ?? undefined"
                width="full"
                :disabled="Boolean(row.locked) || undefined"
                @input="setValue(row, $event)"
                @change="setValue(row, $event)"
              ></nldd-number-field>
              <nldd-text-field
                v-else
                :value="draft[row.name] ?? ''"
                :disabled="Boolean(row.locked) || undefined"
                @input="setValue(row, $event)"
                @change="setValue(row, $event)"
              ></nldd-text-field>
            </nldd-form-field>
          </template>

          <nldd-inline-dialog
            v-if="rows.length === 0"
            icon="settings"
            text="Geen instellingen"
            supporting-text="Dit wereldbestand heeft geen instellingen."
          ></nldd-inline-dialog>

          <nldd-form-actions v-if="rows.length > 0">
            <nldd-button-group>
              <nldd-button
                variant="primary"
                type="submit"
                start-icon="save"
                text="Instellingen opslaan"
                :disabled="!changed || busy || undefined"
                :loading="busy || undefined"
              ></nldd-button>
            </nldd-button-group>
          </nldd-form-actions>
        </form>
      </nldd-form>

      <nldd-list v-if="rows.some((row) => row.locked)" variant="box-tinted" accessible-label="Vaste instellingen">
        <nldd-list-item v-for="row in rows.filter((item) => item.locked)" :key="`vast-${row.name}`" size="sm">
          <nldd-icon-cell icon="locked" size="16" color="secondary"></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell
            size="sm"
            :text="humanize(row.name)"
            :supporting-text="`staat vast op ${formatValue(row.value)}: besluit '${row.locked.besluit}' van cel '${row.locked.cell}' rekende er al mee`"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-container>

    <nldd-container layout="stack" gap="12">
      <nldd-title size="5">
        <span>Opnieuw beginnen</span>
        <span slot="subtitle">de wereld terug naar zijn startstand</span>
      </nldd-title>
      <nldd-button-group>
        <nldd-button
          variant="destructive"
          start-icon="refresh"
          text="Wereld terugzetten…"
          :loading="busy || undefined"
          @click="resetDialog?.show?.()"
        ></nldd-button>
      </nldd-button-group>
    </nldd-container>

    <nldd-modal-dialog
      ref="resetDialog"
      variant="alert"
      text="Wereld terugzetten?"
      supporting-text="Elk gram dat sinds de start is vastgelegd verdwijnt, en de klok gaat terug naar het begin. De fixtures blijven."
      accessible-label="Wereld terugzetten"
    >
      <nldd-button slot="actions" variant="destructive" text="Terugzetten" @click="confirmReset"></nldd-button>
      <nldd-button slot="actions" variant="secondary" text="Annuleren" @click="resetDialog?.hide?.()"></nldd-button>
    </nldd-modal-dialog>
  </nldd-container>
</template>
