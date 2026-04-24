<script setup>
import { computed } from 'vue';
import {
  OPERATION_LABELS,
  collectAvailableVariables,
  describeSubtitle,
  formatValueLabel,
} from '../utils/operationTree.js';

const props = defineProps({
  operation: { type: Object, default: null },
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
});

const emit = defineEmits(['select-operation']);

const availableVariables = computed(() => collectAvailableVariables(props.article));

const typeOptions = computed(() =>
  Object.entries(OPERATION_LABELS).map(([key, label]) => ({ value: key, label }))
);

// Group variables by category with alphabetical sorting within each group,
// matching the optgroup pattern used in EditSheet's source parameter dropdown.
const variableGroups = computed(() => {
  const groups = new Map();
  for (const v of availableVariables.value) {
    if (!groups.has(v.category)) groups.set(v.category, []);
    groups.get(v.category).push({
      value: v.ref,
      label: v.name.replace(/_/g, ' '),
    });
  }
  for (const opts of groups.values()) {
    opts.sort((a, b) => a.label.localeCompare(b.label, 'nl'));
  }
  return groups;
});

const COMPARISON_OPS = new Set([
  'EQUALS', 'NOT_EQUALS', 'GREATER_THAN', 'GREATER_THAN_OR_EQUAL',
  'LESS_THAN', 'LESS_THAN_OR_EQUAL', 'NOT_NULL', 'IN', 'NOT_IN',
]);

const LOGICAL_OPS = new Set(['AND', 'OR']);
const ARITHMETIC_OPS = new Set(['ADD', 'SUBTRACT', 'MULTIPLY', 'DIVIDE', 'MIN', 'MAX', 'CONCAT']);

const isComparisonOp = computed(() => COMPARISON_OPS.has(props.operation?.operation));

const canAddValue = computed(() => {
  if (!props.editable) return false;
  const op = props.operation?.operation;
  if (!op) return false;
  // Structural-slot ops have no concept of "add a value"
  if (op === 'NOT' || op === 'IF' || op === 'SWITCH') return false;
  // NOT_NULL never takes a value field (it only checks the subject is non-null)
  if (op === 'NOT_NULL') return false;
  // AGE has a fixed shape (date_of_birth + reference_date) — no add slot.
  if (op === 'AGE') return false;
  // Comparison ops always have exactly subject + value (or just subject for
  // NOT_NULL); operationValues pushes both unconditionally, so addValue() has
  // nothing to do. Hide the button to avoid a no-op click.
  if (COMPARISON_OPS.has(op)) return false;
  // Logical ops only accept nested operations as conditions; "Voeg waarde toe"
  // would push an EQUALS predicate identical to "Voeg operatie toe", so we
  // suppress the duplicate button.
  if (LOGICAL_OPS.has(op)) return false;
  return true;
});

const canAddNestedOperation = computed(() => {
  if (!props.editable) return false;
  const op = props.operation?.operation;
  if (!op) return false;
  // Same fixed-shape rule as canAddValue: structural-slot ops don't grow.
  if (op === 'NOT' || op === 'IF' || op === 'SWITCH' || op === 'AGE') return false;
  return !isComparisonOp.value;
});

// Required structural fields whose minus button must be hidden so the user
// cannot delete them and silently produce an invalid node.
function canRemoveValue(val) {
  if (!props.editable) return false;
  const node = props.operation?.node;
  const op = props.operation?.operation;
  if (isComparisonOp.value && (val._kind === 'subject' || val._kind === 'value')) return false;
  // NOT needs value
  if (op === 'NOT' && val._kind === 'value') return false;
  // AGE has two fixed structural slots — neither can be deleted.
  if (op === 'AGE' && (val._kind === 'date_of_birth' || val._kind === 'reference_date')) return false;
  // IF and SWITCH share the cases[]/default schema and both need a default branch
  if ((op === 'IF' || op === 'SWITCH') && val._kind === 'default') return false;
  // IF must keep its single case; SWITCH must keep at least one case.
  // Removing the case-when or case-then deletes the whole case entry.
  if ((val._kind === 'case-when' || val._kind === 'case-then') && Array.isArray(node?.cases)) {
    if (op === 'IF') return false;
    if (op === 'SWITCH' && node.cases.length <= 1) return false;
  }
  // AND/OR/arithmetic ops need at least one entry — block removal of the
  // last condition or value so the user can't drain conditions: [] / values: []
  // and produce a semantically undefined node.
  if (val._kind === 'conditions' && Array.isArray(node?.conditions) && node.conditions.length <= 1) return false;
  if (val._kind === 'values' && Array.isArray(node?.values) && node.values.length <= 1) return false;
  return true;
}

const operationValues = computed(() => {
  const node = props.operation?.node;
  if (!node) return [];

  if (isComparisonOp.value) {
    const vals = [];
    vals.push({ _label: 'Onderwerp', _value: node.subject ?? '', _kind: 'subject' });
    if (node.operation !== 'NOT_NULL') {
      vals.push({ _label: 'Waarde', _value: node.value ?? '', _kind: 'value' });
    }
    return vals;
  }

  // AGE: two structural slots, similar in shape to a comparison op (fixed,
  // named) but with date semantics. Returns the age in whole years.
  if (node.operation === 'AGE') {
    return [
      { _label: 'Geboortedatum', _value: node.date_of_birth ?? '', _kind: 'date_of_birth' },
      { _label: 'Peildatum', _value: node.reference_date ?? '', _kind: 'reference_date' },
    ];
  }

  // IF and SWITCH share the same cases[]/default schema. The only difference
  // is semantic (IF is single-case, SWITCH is multi-case) and how the user
  // labels each branch.
  if (node.operation === 'IF' || node.operation === 'SWITCH') {
    const vals = [];
    const isSwitch = node.operation === 'SWITCH';
    if (Array.isArray(node.cases)) {
      node.cases.forEach((c, i) => {
        const prefix = isSwitch ? `Geval ${i + 1} — ` : '';
        if (c?.when !== undefined) vals.push({ _label: `${prefix}als`, _value: c.when, _kind: 'case-when', _caseIndex: i });
        if (c?.then !== undefined) vals.push({ _label: `${prefix}dan`, _value: c.then, _kind: 'case-then', _caseIndex: i });
      });
    }
    if (node.default !== undefined) vals.push({ _label: isSwitch ? 'Standaard' : 'Anders', _value: node.default, _kind: 'default' });
    return vals;
  }

  if (Array.isArray(node.values)) {
    return node.values.map((v, i) => ({ _label: `Waarde ${i + 1}`, _value: v, _kind: 'values', _index: i }));
  }
  if (Array.isArray(node.conditions)) {
    return node.conditions.map((c, i) => ({ _label: `Conditie ${i + 1}`, _value: c, _kind: 'conditions', _index: i }));
  }

  const vals = [];
  if (node.subject != null) vals.push({ _label: 'Onderwerp', _value: node.subject, _kind: 'subject' });
  if (node.value !== undefined) vals.push({ _label: 'Waarde', _value: node.value, _kind: 'value' });
  return vals;
});

const hasClickableRow = computed(() =>
  !props.editable && operationValues.value.some(v => isNestedOperation(v._value))
);

function isNestedOperation(val) {
  return val != null && typeof val === 'object' && val.operation;
}

function isLiteralValue(val) {
  return val === null || typeof val === 'number' || typeof val === 'boolean' || (typeof val === 'string' && !val.startsWith('$'));
}

function valueDropdownNestedOption(val) {
  if (!isNestedOperation(val)) return null;
  return { value: '__nested__', label: formatValueLabel(val) + ' (operatie)' };
}

function currentDropdownValue(val) {
  if (isNestedOperation(val)) return '__nested__';
  if (typeof val === 'string' && val.startsWith('$')) return val;
  return String(val);
}

// --- Mutation helpers ---

function parseInputValue(str) {
  if (str === 'true') return true;
  if (str === 'false') return false;
  const n = Number(str);
  if (!isNaN(n) && str.trim() !== '') return n;
  return str;
}

function changeOperationType(event) {
  const node = props.operation?.node;
  if (!node) return;
  const newType = event.target.value;
  const oldType = node.operation;
  if (newType === oldType) return;

  node.operation = newType;

  // Every non-AGE branch must also strip `date_of_birth` / `reference_date`
  // so a transition out of AGE doesn't leave the slots dangling on the
  // node. The schema marks every operation type as
  // `additionalProperties: false`, so a leaked field would fail
  // validation on save and corrupt the law.
  if (COMPARISON_OPS.has(newType)) {
    if (node.subject === undefined) node.subject = '';
    if (newType === 'NOT_NULL') {
      delete node.value;
    } else {
      // For all other comparison ops (including IN/NOT_IN), seed value as
      // an empty string. The UI dropdown lets the user pick a variable
      // reference that resolves to a list at runtime; arrays as literal
      // values can be entered through the YAML pane.
      if (node.value === undefined || node.value === null) node.value = '';
    }
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (LOGICAL_OPS.has(newType)) {
    if (!Array.isArray(node.conditions)) {
      node.conditions = [];
    }
    delete node.values;
    delete node.subject;
    delete node.value;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (newType === 'IF') {
    // IF uses the same cases[]/default schema as SWITCH but is single-case.
    // Truncate any extra cases when transitioning from SWITCH so we don't
    // produce an IF with 2+ cases that the schema would reject.
    if (!Array.isArray(node.cases) || node.cases.length === 0) {
      node.cases = [{ when: { operation: 'EQUALS', subject: '', value: '' }, then: 0 }];
    } else if (node.cases.length > 1) {
      node.cases = node.cases.slice(0, 1);
    }
    if (node.default === undefined) node.default = 0;
    delete node.values;
    delete node.conditions;
    delete node.subject;
    delete node.value;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (newType === 'NOT') {
    if (node.value === undefined) {
      node.value = node.subject ?? '';
    }
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.subject;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (newType === 'SWITCH') {
    // Schema requires at least one case
    if (!Array.isArray(node.cases) || node.cases.length === 0) {
      node.cases = [{ when: { operation: 'EQUALS', subject: '', value: '' }, then: 0 }];
    }
    if (node.default === undefined) node.default = 0;
    delete node.values;
    delete node.conditions;
    delete node.subject;
    delete node.value;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (ARITHMETIC_OPS.has(newType)) {
    if (!Array.isArray(node.values)) {
      node.values = [];
    }
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.subject;
    delete node.value;
    delete node.when;
    delete node.then;
    delete node.else;
    delete node.date_of_birth;
    delete node.reference_date;
  } else if (newType === 'AGE') {
    // AGE has two fixed structural slots — seed both as empty strings so
    // the user can fill them via the form. Strip every other slot so the
    // node is shaped exactly like the engine's `ActionOperation::Age`.
    if (node.date_of_birth === undefined) node.date_of_birth = '';
    if (node.reference_date === undefined) node.reference_date = '';
    delete node.subject;
    delete node.value;
    delete node.values;
    delete node.conditions;
    delete node.cases;
    delete node.default;
    delete node.when;
    delete node.then;
    delete node.else;
  }
}

function applyValueMutation(val, newVal) {
  const node = props.operation?.node;
  if (!node) return;
  if (val._kind === 'subject') node.subject = newVal;
  else if (val._kind === 'value') node.value = newVal;
  else if (val._kind === 'date_of_birth') node.date_of_birth = newVal;
  else if (val._kind === 'reference_date') node.reference_date = newVal;
  else if (val._kind === 'values' && val._index !== undefined) node.values[val._index] = newVal;
  else if (val._kind === 'conditions' && val._index !== undefined) node.conditions[val._index] = newVal;
  else if (val._kind === 'default') node.default = newVal;
  else if (val._kind === 'case-when' && val._caseIndex !== undefined && Array.isArray(node.cases)) {
    node.cases[val._caseIndex].when = newVal;
  } else if (val._kind === 'case-then' && val._caseIndex !== undefined && Array.isArray(node.cases)) {
    node.cases[val._caseIndex].then = newVal;
  }
}

function updateValue(val, event) {
  const newVal = parseInputValue(event.target?.value ?? event.detail?.value ?? '');
  applyValueMutation(val, newVal);
}

function updateDropdownValue(val, event) {
  const selected = event.target.value;
  if (selected === '__nested__') return;
  if (isNestedOperation(val._value)) return;
  const newVal = selected.startsWith('$') ? selected : parseInputValue(selected);
  applyValueMutation(val, newVal);
}

function humanize(name) {
  if (typeof name !== 'string') return name;
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}

function variableLabel(ref) {
  for (const opts of variableGroups.value.values()) {
    const hit = opts.find(o => o.value === ref);
    if (hit) return humanize(hit.label);
  }
  return humanize(ref);
}

function readonlyValueText(val) {
  const v = val._value;
  if (v === null || v === undefined || v === '') return '—';
  if (isNestedOperation(v)) return formatValueLabel(v) + ' (operatie)';
  if (typeof v === 'string' && v.startsWith('$')) return variableLabel(v);
  return humanize(String(v));
}

function removeValue(val) {
  const node = props.operation?.node;
  if (!node) return;

  if (val._kind === 'values' && val._index !== undefined && Array.isArray(node.values)) {
    node.values.splice(val._index, 1);
  } else if (val._kind === 'conditions' && val._index !== undefined && Array.isArray(node.conditions)) {
    node.conditions.splice(val._index, 1);
  } else if (val._kind === 'subject') {
    delete node.subject;
  } else if (val._kind === 'value') {
    delete node.value;
  } else if ((val._kind === 'case-when' || val._kind === 'case-then') && val._caseIndex !== undefined && Array.isArray(node.cases)) {
    // Removing either side of a case entry removes the whole case
    node.cases.splice(val._caseIndex, 1);
  } else if (val._kind === 'default') {
    delete node.default;
  }
}

function addValue() {
  const node = props.operation?.node;
  if (!node) return;

  // Don't inject values[] into nodes with structural value slots
  // (NOT uses single 'value', IF uses when/then/else, SWITCH uses cases/default)
  if (node.operation === 'NOT' || node.operation === 'IF' || node.operation === 'SWITCH') return;
  // NOT_NULL is a unary check on subject only — never a value
  if (node.operation === 'NOT_NULL') return;

  if (Array.isArray(node.values)) {
    node.values.push(0);
  } else if (Array.isArray(node.conditions)) {
    node.conditions.push({ operation: 'EQUALS', subject: '', value: '' });
  } else if (isComparisonOp.value) {
    if (node.subject === undefined) node.subject = '';
    else if (node.value === undefined) node.value = '';
  } else {
    if (!node.values) node.values = [];
    node.values.push(0);
  }
}

function addNestedOperation() {
  const node = props.operation?.node;
  if (!node || isComparisonOp.value) return;
  if (node.operation === 'NOT' || node.operation === 'IF' || node.operation === 'SWITCH') return;

  if (Array.isArray(node.conditions)) {
    node.conditions.push({ operation: 'EQUALS', subject: '', value: '' });
  } else if (Array.isArray(node.values)) {
    node.values.push({ operation: 'ADD', values: [] });
  }
}
</script>

<template>
  <template v-if="operation">
    <nldd-title size="4" class="operation-settings__title">
      <h4>Instellingen operatie {{ operation.number }}</h4>
    </nldd-title>
    <nldd-spacer size="12"></nldd-spacer>
    <nldd-list variant="box" class="settings-list">
      <!-- Titel -->
      <nldd-list-item size="md">
        <nldd-text-cell text="Titel" max-width="120"></nldd-text-cell>
        <nldd-cell v-if="editable">
          <nldd-text-field size="md" :value="operation.title" readonly></nldd-text-field>
        </nldd-cell>
        <template v-else>
          <nldd-text-cell horizontal-alignment="right" :text="operation.title || '—'"></nldd-text-cell>
          <template v-if="hasClickableRow">
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
          </template>
        </template>
      </nldd-list-item>

      <!-- Type -->
      <nldd-list-item size="md">
        <nldd-text-cell text="Type" max-width="120"></nldd-text-cell>
        <nldd-cell v-if="editable">
          <nldd-dropdown size="md" data-testid="operation-type-dropdown">
            <select aria-label="Operatie type" :value="operation.operation" @change="changeOperationType($event)">
              <option v-for="opt in typeOptions" :key="opt.value" :value="opt.value" :selected="opt.value === operation.operation">{{ opt.label }}</option>
            </select>
          </nldd-dropdown>
        </nldd-cell>
        <template v-else>
          <nldd-text-cell horizontal-alignment="right" :text="OPERATION_LABELS[operation.operation] || operation.operation || '—'"></nldd-text-cell>
          <template v-if="hasClickableRow">
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
          </template>
        </template>
      </nldd-list-item>

      <!-- Waarde rows -->
      <nldd-list-item
        v-for="(val, i) in operationValues"
        :key="i"
        size="md"
        :data-testid="`op-value-${i}`"
        :type="!editable && isNestedOperation(val._value) ? 'button' : undefined"
        @click="!editable && isNestedOperation(val._value) && emit('select-operation', val._value)"
      >
        <nldd-text-cell :text="val._label" max-width="120"></nldd-text-cell>
        <nldd-cell v-if="editable">
          <div class="value-row">
            <!-- Subject/date fields show a dropdown of available variables.
                 If the field already holds a literal value (e.g. "2025-01-01"),
                 it's included as the first option so it stays visible and
                 selectable. -->
            <template v-if="val._kind === 'subject' || val._kind === 'date_of_birth' || val._kind === 'reference_date'">
              <nldd-dropdown size="md" style="flex: 1; min-width: 0;">
                <select :aria-label="val._label" :value="currentDropdownValue(val._value)" @change="updateDropdownValue(val, $event)">
                  <option value="">Selecteer...</option>
                  <option v-if="isLiteralValue(val._value) && val._value !== '' && val._value !== null" :value="String(val._value)" :selected="true">{{ String(val._value) }}</option>
                  <optgroup v-for="[category, opts] in variableGroups" :key="category" :label="category">
                    <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === currentDropdownValue(val._value)">{{ opt.label }}</option>
                  </optgroup>
                </select>
              </nldd-dropdown>
            </template>
            <!-- Literal values show a text field -->
            <template v-else-if="isLiteralValue(val._value)">
              <nldd-text-field size="md" :value="String(val._value)" is-full-width @input="updateValue(val, $event)"></nldd-text-field>
            </template>
            <!-- Variable references and nested operations show a full dropdown -->
            <template v-else>
              <nldd-dropdown size="md" style="flex: 1; min-width: 0;">
                <select :aria-label="val._label" :value="currentDropdownValue(val._value)" @change="updateDropdownValue(val, $event)">
                  <template v-for="nestedOpt in [valueDropdownNestedOption(val._value)]" :key="'nested'">
                    <option v-if="nestedOpt" :value="nestedOpt.value" :selected="currentDropdownValue(val._value) === '__nested__'">{{ nestedOpt.label }}</option>
                  </template>
                  <optgroup v-for="[category, opts] in variableGroups" :key="category" :label="category">
                    <option v-for="opt in opts" :key="opt.value" :value="opt.value" :selected="opt.value === currentDropdownValue(val._value)">{{ opt.label }}</option>
                  </optgroup>
                </select>
              </nldd-dropdown>
            </template>
            <nldd-icon-button v-if="canRemoveValue(val)" icon="minus" title="Verwijder waarde" @click="removeValue(val)">
            </nldd-icon-button>
          </div>
          <p v-if="isNestedOperation(val._value)" class="value-help-text">
            {{ describeSubtitle(val._value) }}
            <a href="#" @click.prevent="emit('select-operation', val._value)">Bewerk</a>
          </p>
        </nldd-cell>
        <template v-else>
          <nldd-text-cell
            horizontal-alignment="right"
            :text="readonlyValueText(val)"
            :supporting-text="isNestedOperation(val._value) ? describeSubtitle(val._value) : ''"
          ></nldd-text-cell>
          <template v-if="hasClickableRow">
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon v-if="isNestedOperation(val._value)" name="chevron-down"></nldd-icon>
            </nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
          </template>
        </template>
      </nldd-list-item>

      <!-- Add value -->
      <nldd-list-item v-if="canAddValue || canAddNestedOperation" size="md">
        <div class="add-value-buttons">
          <nldd-button v-if="canAddValue" size="md" start-icon="plus-small" data-testid="add-value-btn" @click="addValue" text="Voeg waarde toe"></nldd-button>
          <nldd-button v-if="canAddNestedOperation" size="md" start-icon="plus-small" data-testid="add-nested-op-btn" @click="addNestedOperation" text="Voeg operatie toe"></nldd-button>
        </div>
      </nldd-list-item>
    </nldd-list>
  </template>
</template>

<style>
/* Reset browser default h4 margin so the nldd-spacer below the title is the
 * only source of vertical gap. Without this the h4's default bottom margin
 * (~1em) collapses into the spacer unpredictably, which caused the
 * "textboxes falling under the title" visual in the action panel.
 *
 * Keyed off a `.operation-settings__title` BEM class on the nldd-title in
 * this component. The rule is in an unscoped `<style>` block alongside the
 * rest of the file's selectors (Vue scoped styles can't reach into NLDD
 * shadow DOM), so the class name is the only thing preventing bleed into
 * other components — keep the class unique to this component. */
.operation-settings__title h4 {
  margin: 0;
}

.settings-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.settings-list nldd-text-field,
.settings-list nldd-dropdown {
  width: 100%;
}

.value-row {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
}
.value-row nldd-text-field,
.value-row nldd-dropdown {
  flex: 1;
  min-width: 0;
}

.add-value-buttons {
  display: flex;
  gap: 8px;
  width: 100%;
}

.value-help-text {
  font-family: var(--primitives-font-family-body, 'RijksSansVF', sans-serif);
  font-size: 14px;
  font-weight: 400;
  line-height: 1.25;
  color: var(--semantics-text-secondary-color, #545D68);
  margin: 2px 0 0 0;
}

.value-help-text a {
  color: var(--semantics-text-accent-color, #007BC7);
  text-decoration: none;
}

.value-help-text a:hover {
  text-decoration: underline;
}
</style>
