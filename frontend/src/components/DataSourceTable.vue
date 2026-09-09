<script setup>
import { ref, computed, useId } from 'vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import { NOT_NULLABLE_MESSAGE, nullAllowed, isNullText } from '../utils/nullability.js';

let nextRowId = 0;

const props = defineProps({
  title: { type: String, required: true },
  // The key column of a data source. `null` for a table that has no key,
  // such as the elements of a collection-valued parameter (RFC-016).
  keyField: { type: String, default: 'bsn' },
  fields: { type: Array, required: true },
  modelValue: { type: Array, default: () => [] },
  defaultExpanded: { type: Boolean, default: false },
  readonly: { type: Boolean, default: false },
  // When the table is shown one level deep in a drill-in sheet there's no
  // accordion: the title is a plain heading and the body is always visible.
  drilledIn: { type: Boolean, default: false },
});

const emit = defineEmits(['update:modelValue']);

const expanded = ref(props.defaultExpanded);

const rows = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
});

function toggleExpand() {
  expanded.value = !expanded.value;
}

function addRow() {
  const newRow = { _id: ++nextRowId };
  if (props.keyField) {
    newRow[props.keyField] = rows.value.length > 0
      ? rows.value[0][props.keyField] || ''
      : '';
  }
  for (const field of props.fields) {
    if (!(field.name in newRow)) {
      newRow[field.name] = defaultForType(field.type);
    }
  }
  rows.value = [...rows.value, newRow];
}

function removeRow(index) {
  const rk = rowKey(rows.value[index], index);
  const updated = [...rows.value];
  updated.splice(index, 1);
  rows.value = updated;
  for (const key of [...rejectedNull.value]) {
    if (key.startsWith(`${rk}:`)) rejectedNull.value.delete(key);
  }
}

function updateCell(rowIndex, fieldName, value) {
  const updated = rows.value.map((row, i) => {
    if (i !== rowIndex) return row;
    return { ...row, [fieldName]: value };
  });
  rows.value = updated;
}

function defaultForType(type) {
  switch (type) {
    case 'number':
    case 'amount':
      return '';
    case 'boolean':
      return 'false';
    default:
      return '';
  }
}

// Cell contract (RFC-036). A cell holds what the table says: blank means "no
// value stated" (the runner leaves the field out, the engine reports the
// input as unknown), the word `null` is an absence the author stated. The
// two must stay apart in the form, so a blank shows as blank and a null (a
// JS null from a typed collection record, or the text `null` from a
// data-source row) shows as the word `null`, in a text control whatever the
// column type - a number field cannot show it. Clearing a field stores a
// blank; stating an absence is done by typing `null` in a text column or
// picking it in a boolean column.
//
// Whether an absence may be stated at all is the column's `nullable` (the
// field's declaration in the law, schema v0.5.8). Three states: `true` (the
// law allows null), `false` (the law declares the field as never absent, the
// engine rejects a null there), `undefined` (no declaration known - the
// element fields of a collection, or a source column the law never names).
// Only `false` restricts: an unknown declaration makes no claim, the same
// rule the engine's type checker follows (see utils/nullability.js).
const isNullCell = isNullText;
function cellDisplay(v) {
  if (isNullCell(v)) return 'null';
  return v === undefined ? '' : v;
}
function cellType(col, v) {
  return isNullCell(v) ? 'string' : col.type;
}

// Cells whose `null` was refused, keyed `<row key>:<column>`. The refusal
// stores a blank (the record says "no value stated", the honest fallback
// for a statement the law does not allow) and marks the cell invalid until
// the author types something else; the field itself keeps showing what was
// typed, so the message points at the text it is about.
const rejectedNull = ref(new Set());
const errorIdPrefix = useId();

function rowKey(row, rowIndex) {
  return row?._id ?? rowIndex;
}
function cellKey(row, rowIndex, col) {
  return `${rowKey(row, rowIndex)}:${col.name}`;
}
function cellErrorId(row, rowIndex, col) {
  return `${errorIdPrefix}-${cellKey(row, rowIndex, col)}`;
}
function cellStore(rowIndex, col, v) {
  const key = cellKey(rows.value[rowIndex], rowIndex, col);
  if (isNullCell(v) && !nullAllowed(col)) {
    rejectedNull.value.add(key);
    updateCell(rowIndex, col.name, '');
    return;
  }
  rejectedNull.value.delete(key);
  updateCell(rowIndex, col.name, v == null ? '' : v);
}
// A cell is invalid when its `null` was just refused, or when it already
// holds a `null` (read from the feature file, or the column was re-typed as
// non-nullable after the law's declaration arrived) that the law does not
// allow. The stored value stays: it came from the file, a human decides.
function cellInvalid(row, rowIndex, col) {
  return rejectedNull.value.has(cellKey(row, rowIndex, col))
    || (isNullCell(row[col.name]) && !nullAllowed(col));
}
// The boolean dropdown: `null` is the stated absence, the empty option a
// blank cell. The `null` option is offered only where the law allows it, and
// kept while the cell holds one so the invalid state has something to show.
function booleanCellValue(v) {
  if (isNullCell(v)) return 'null';
  return v === undefined ? '' : String(v);
}
function offersNullOption(row, col) {
  return nullAllowed(col) || isNullCell(row[col.name]);
}

// All columns: key field + declared fields (deduplicated)
const allColumns = computed(() => {
  const cols = [];
  const seen = new Set();

  if (props.keyField) {
    seen.add(props.keyField);
    cols.push({ name: props.keyField, type: 'string', unit: null, isKey: true });
  }

  for (const field of props.fields) {
    if (!seen.has(field.name)) {
      seen.add(field.name);
      cols.push({ ...field, isKey: false });
    }
  }

  return cols;
});

const rowCount = computed(() => rows.value.length);

// Drilled-in tables have no toggle, so the body is always shown.
const showBody = computed(() => props.drilledIn || expanded.value);
</script>

<template>
  <div class="ds-root">
    <!-- Header: accordion toggle, or a plain heading when drilled in -->
    <button v-if="!drilledIn" class="ds-block-toggle" :aria-expanded="expanded" @click="toggleExpand" type="button">
      <span class="ds-block-chevron" :class="{ 'ds-block-chevron--open': expanded }">&#9656;</span>
      <nldd-title size="5" style="flex: 1; text-align: left;">
        <span>{{ title }}</span>
      </nldd-title>
      <span class="ds-block-badge" v-if="rowCount > 0">{{ rowCount }}</span>
    </button>
    <template v-else>
      <nldd-title size="3">
        <h2>{{ title }}</h2>
      </nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
    </template>

    <template v-if="showBody">
      <nldd-inline-dialog v-if="rows.length === 0" text="Geen gegevens - vul in indien relevant">
        <nldd-button v-if="!readonly" slot="actions" size="md" start-icon="plus-small" @click="addRow" text="Voeg toe"></nldd-button>
      </nldd-inline-dialog>

      <!-- One box list per row - identical layout regardless of row count,
           with the delete button at the bottom of each list. Spacers (not a
           flex-gap container) separate the stacked lists. -->
      <template v-for="(row, ri) in rows" :key="row._id ?? ri">
        <nldd-spacer v-if="ri > 0" size="12"></nldd-spacer>
        <nldd-list variant="box-tinted">
          <nldd-list-item v-for="col in allColumns" :key="col.name" size="md">
            <nldd-text-cell :text="col.name" min-width="120px" max-width="200px"></nldd-text-cell>
            <nldd-spacer-cell v-if="!readonly" size="8"></nldd-spacer-cell>
            <template v-if="readonly">
              <nldd-text-cell :text="String(row[col.name] ?? '')"></nldd-text-cell>
            </template>
            <nldd-cell v-else-if="col.type === 'boolean'" width="full" min-width="120px">
              <nldd-dropdown size="md" :invalid="cellInvalid(row, ri, col) || undefined">
                <select
                  :aria-label="col.name"
                  :value="booleanCellValue(row[col.name])"
                  :aria-invalid="cellInvalid(row, ri, col) || undefined"
                  :aria-describedby="cellInvalid(row, ri, col) ? cellErrorId(row, ri, col) : undefined"
                  @change="updateCell(ri, col.name, $event.target.value)"
                >
                  <option value="true">true</option>
                  <option value="false">false</option>
                  <option v-if="offersNullOption(row, col)" value="null">null</option>
                  <option value="">(leeg)</option>
                </select>
              </nldd-dropdown>
              <nldd-form-field-error-text v-if="cellInvalid(row, ri, col)" :id="cellErrorId(row, ri, col)" invalid>
                {{ NOT_NULLABLE_MESSAGE }}
              </nldd-form-field-error-text>
            </nldd-cell>
            <nldd-cell v-else width="full" min-width="120px">
              <ScenarioParameterInput
                :type="cellType(col, row[col.name])"
                :unit="col.unit"
                :name="col.name"
                :value="cellDisplay(row[col.name])"
                :invalid="cellInvalid(row, ri, col)"
                :error-message-ids="cellErrorId(row, ri, col)"
                @update="cellStore(ri, col, $event)"
              />
              <nldd-form-field-error-text v-if="cellInvalid(row, ri, col)" :id="cellErrorId(row, ri, col)" invalid>
                {{ NOT_NULLABLE_MESSAGE }}
              </nldd-form-field-error-text>
            </nldd-cell>
          </nldd-list-item>

          <nldd-list-item v-if="!readonly" size="md">
            <nldd-cell width="full">
              <nldd-button variant="destructive" size="md" width="full" start-icon="delete" @click="removeRow(ri)" text="Verwijder"></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>

      <!-- Empty state offers "Voeg toe" inside the inline-dialog instead. -->
      <template v-if="!readonly && rows.length > 0">
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-list variant="box-tinted">
          <nldd-list-item size="md">
            <nldd-cell width="full">
              <nldd-button size="md" width="full" start-icon="plus-small" @click="addRow" text="Voeg toe"></nldd-button>
            </nldd-cell>
          </nldd-list-item>
        </nldd-list>
      </template>
    </template>
  </div>
</template>

<style scoped>
/* The component needs a single root, but it must not generate a box -
 * otherwise it blocks the nldd flex layout (flex-grow / centering of the
 * empty-state inline-dialog) of the enclosing simple-section. */
.ds-root {
  display: contents;
}

.ds-block-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
  width: 100%;
  margin-bottom: 4px;
}

.ds-block-chevron {
  display: inline-block;
  font-size: 12px;
  transition: transform 0.15s ease;
  flex-shrink: 0;
}

.ds-block-chevron--open {
  transform: rotate(90deg);
}

.ds-block-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--color-primary);
  color: white;
  flex-shrink: 0;
}

</style>
