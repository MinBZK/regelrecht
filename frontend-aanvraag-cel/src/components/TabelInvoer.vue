<script setup>
// Een tabelveld: regels met de kolommen uit het formulier.
import Invoer from './Invoer.vue';

const props = defineProps({
  label: { type: String, required: true },
  kolommen: { type: Array, required: true },
  modelValue: { type: Array, default: () => [] },
});
const emit = defineEmits(['update:modelValue']);

function zet(i, kolom, waarde) {
  const regels = props.modelValue.map((r) => ({ ...r }));
  regels[i][kolom] = waarde;
  emit('update:modelValue', regels);
}

function erbij() {
  emit('update:modelValue', [...props.modelValue, {}]);
}

function weg(i) {
  emit('update:modelValue', props.modelValue.filter((_, j) => j !== i));
}

const kolomBreedtes = () => [...props.kolommen.map(() => 'minmax(120px,1fr)'), '56px'].join(' ');
</script>

<template>
  <nldd-table :columns="kolomBreedtes()" :accessible-label="label" empty-text="Nog geen regels">
    <nldd-table-row slot="header">
      <nldd-text-cell v-for="k in kolommen" :key="k.id" :text="k.label ?? k.id"></nldd-text-cell>
      <nldd-text-cell text=""></nldd-text-cell>
    </nldd-table-row>
    <nldd-table-row v-for="(r, i) in modelValue" :key="i">
      <nldd-cell v-for="k in kolommen" :key="k.id">
        <Invoer
          :soort="k.type"
          :label="`${k.label ?? k.id}, regel ${i + 1}`"
          :keuzes="k.opties"
          :model-value="r[k.id] ?? null"
          @update:model-value="zet(i, k.id, $event)"
        />
      </nldd-cell>
      <nldd-cell>
        <nldd-icon-button
          icon="delete"
          variant="neutral-transparent"
          :text="`Regel ${i + 1} verwijderen`"
          @click="weg(i)"
        ></nldd-icon-button>
      </nldd-cell>
    </nldd-table-row>
  </nldd-table>
  <nldd-spacer size="8"></nldd-spacer>
  <nldd-button variant="secondary" size="sm" start-icon="add" text="Regel toevoegen" @click="erbij"></nldd-button>
</template>
