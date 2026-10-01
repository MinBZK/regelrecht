<script setup>
// A table field: rows with the columns from the form. One root, so the
// caller can place it as a single block; the default slot sits next to the
// add button (for example the "waarom?" icon of the field).
import InputField from './InputField.vue';

const props = defineProps({
  label: { type: String, required: true },
  columns: { type: Array, required: true },
  modelValue: { type: Array, default: () => [] },
});
const emit = defineEmits(['update:modelValue']);

function set(i, column, value) {
  const rows = props.modelValue.map((r) => ({ ...r }));
  rows[i][column] = value;
  emit('update:modelValue', rows);
}

function addRow() {
  emit('update:modelValue', [...props.modelValue, {}]);
}

function removeRow(i) {
  emit('update:modelValue', props.modelValue.filter((_, j) => j !== i));
}

const columnWidths = () => ['56px', ...props.columns.map(() => 'minmax(120px,1fr)')].join(' ');
</script>

<template>
  <nldd-container gap="8">
    <nldd-table :columns="columnWidths()" :accessible-label="label" empty-text="Nog geen regels">
      <nldd-table-row slot="header">
        <nldd-text-cell text=""></nldd-text-cell>
        <nldd-text-cell v-for="c in columns" :key="c.id" :text="c.label ?? c.id"></nldd-text-cell>
      </nldd-table-row>
      <nldd-table-row v-for="(r, i) in modelValue" :key="i">
        <nldd-cell>
          <nldd-icon-button
            icon="delete"
            variant="neutral-transparent"
            :text="`Regel ${i + 1} verwijderen`"
            @click="removeRow(i)"
          ></nldd-icon-button>
        </nldd-cell>
        <nldd-cell v-for="c in columns" :key="c.id">
          <InputField
            :kind="c.type"
            :label="`${c.label ?? c.id}, regel ${i + 1}`"
            :choices="c.options"
            :model-value="r[c.id] ?? null"
            @update:model-value="set(i, c.id, $event)"
          />
        </nldd-cell>
      </nldd-table-row>
    </nldd-table>
    <nldd-container layout="row" gap="8" vertical-alignment="center">
      <nldd-button variant="secondary" size="sm" start-icon="add" text="Regel toevoegen" @click="addRow"></nldd-button>
      <slot></slot>
    </nldd-container>
  </nldd-container>
</template>
