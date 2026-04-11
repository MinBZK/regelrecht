<script setup>
import { computed } from 'vue';

const props = defineProps({
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
  /** True when the in-memory machine_readable differs from the saved copy */
  dirty: { type: Boolean, default: false },
  /** True while a save PUT is in flight */
  saving: { type: Boolean, default: false },
  /** Error from the most recent save attempt (Error instance or null) */
  saveError: { type: Object, default: null },
});

const emit = defineEmits([
  'open-action',
  'open-edit',
  'init-mr',
  'add-action',
  'save',
  /**
   * Delete a single item from the machine_readable. Payload shape mirrors
   * `open-edit` so the parent's `handleSave`/`handleDelete` can dispatch on
   * `section`. Examples:
   *   { section: 'definition', key: 'drempelinkomen_alleenstaande' }
   *   { section: 'parameter', index: 0 }
   *   { section: 'input', index: 2 }
   *   { section: 'output', index: 1 }
   *   { section: 'action', index: 0 }
   */
  'delete',
]);

const mr = computed(() => props.article?.machine_readable ?? null);
const execution = computed(() => mr.value?.execution ?? null);

const definitions = computed(() => {
  const defs = mr.value?.definitions;
  if (!defs) return [];
  return Object.entries(defs).map(([name, def]) => {
    const val = typeof def === 'object' ? def.value : def;
    const unit = typeof def === 'object' ? def.type_spec?.unit : undefined;
    return { name, value: val, unit };
  });
});

const produces = computed(() => execution.value?.produces ?? null);

const parameters = computed(() =>
  (execution.value?.parameters ?? []).map((p) => ({
    name: p.name,
    type: p.type,
    required: p.required ?? false,
  }))
);

const inputs = computed(() =>
  (execution.value?.input ?? []).map((i) => ({
    name: i.name,
    type: i.type,
    source: i.source?.regulation ?? i.source?.output ?? null,
  }))
);

const outputs = computed(() =>
  (execution.value?.output ?? []).map((o) => ({
    name: o.name,
    type: o.type,
  }))
);

const actions = computed(() => execution.value?.actions ?? []);

function formatValue(val, unit) {
  if (typeof val === 'number') {
    if (unit === 'eurocent') {
      return (val / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR' });
    }
    if (val > 0 && val < 1 && !unit) {
      return (val * 100).toLocaleString('nl-NL', { maximumFractionDigits: 3 }) + '%';
    }
  }
  return String(val);
}

// Open edit sheet for existing items
function editDef(name) {
  const rawDef = mr.value?.definitions?.[name];
  if (rawDef == null) return;
  emit('open-edit', { section: 'definition', key: name, rawDef: JSON.parse(JSON.stringify(rawDef)) });
}

function editParam(index) {
  const p = execution.value?.parameters?.[index];
  if (p) emit('open-edit', { section: 'parameter', index, data: JSON.parse(JSON.stringify(p)) });
}

function editInput(index) {
  const raw = execution.value?.input?.[index];
  if (raw) emit('open-edit', { section: 'input', index, data: JSON.parse(JSON.stringify(raw)) });
}

function editOutput(index) {
  const raw = execution.value?.output?.[index];
  if (raw) emit('open-edit', { section: 'output', index, data: JSON.parse(JSON.stringify(raw)) });
}

// Delete handlers — emit a delete event with the section + identity of
// the row. The parent (EditorApp) is the source of truth for
// machineReadable, so all mutations live there.
function deleteDef(name) {
  emit('delete', { section: 'definition', key: name });
}

function deleteParam(index) {
  emit('delete', { section: 'parameter', index });
}

function deleteInput(index) {
  emit('delete', { section: 'input', index });
}

function deleteOutput(index) {
  emit('delete', { section: 'output', index });
}

function deleteAction(index) {
  emit('delete', { section: 'action', index });
}

// Open edit sheet for new items
function addDef() {
  emit('open-edit', { section: 'add-definition', isNew: true });
}

function addParam() {
  emit('open-edit', { section: 'add-parameter', isNew: true });
}

function addInput() {
  emit('open-edit', { section: 'add-input', isNew: true });
}

function addOutput() {
  emit('open-edit', { section: 'add-output', isNew: true });
}
</script>

<template>
  <ndd-simple-section v-if="!mr" align="center" data-testid="no-machine-readable">
    <ndd-inline-dialog text="Geen machine-leesbare gegevens voor dit artikel"></ndd-inline-dialog>
    <ndd-spacer v-if="editable" size="8"></ndd-spacer>
    <ndd-button v-if="editable" variant="primary" size="md" data-testid="init-mr-btn" @click="emit('init-mr')" text="Initialiseer machine_readable"></ndd-button>
  </ndd-simple-section>

  <ndd-simple-section v-else data-testid="machine-readable">
    <!-- Save bar: visible only when the user can edit. The button itself is
         disabled when there's nothing to save so it still acts as a clear
         "everything's saved" signal instead of disappearing. -->
    <template v-if="editable">
      <div class="mr-save-bar">
        <ndd-button
          variant="primary"
          size="md"
          data-testid="save-mr-btn"
          :disabled="!dirty || saving"
          :text="saving ? 'Opslaan…' : dirty ? 'Opslaan' : 'Opgeslagen'"
          @click="emit('save')"
        ></ndd-button>
      </div>
      <ndd-inline-dialog
        v-if="saveError"
        variant="alert"
        text="Opslaan mislukt"
        :supporting-text="saveError.message || String(saveError)"
        data-testid="save-mr-error"
      ></ndd-inline-dialog>
      <ndd-spacer size="12"></ndd-spacer>
    </template>

    <!-- Metadata: produces -->
    <ndd-list v-if="produces" variant="box">
      <ndd-list-item v-if="produces.legal_character" size="md">
        <ndd-text-cell text="Juridische basis"></ndd-text-cell>
        <ndd-cell>
          <ndd-button size="md" expandable :text="produces.legal_character"></ndd-button>
        </ndd-cell>
      </ndd-list-item>
      <ndd-list-item v-if="produces.decision_type" size="md">
        <ndd-text-cell text="Besluit-type"></ndd-text-cell>
        <ndd-cell>
          <ndd-button size="md" expandable :text="produces.decision_type"></ndd-button>
        </ndd-cell>
      </ndd-list-item>
    </ndd-list>

    <ndd-spacer v-if="produces" size="12"></ndd-spacer>

    <!-- Definities -->
    <template v-if="definitions.length || editable">
      <ndd-title size="5" data-testid="section-definitions"><h5>Definities</h5></ndd-title>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-list variant="box">
        <ndd-list-item v-for="def in definitions" :key="def.name" size="md">
          <ndd-text-cell :text="`${def.name} = ${formatValue(def.value, def.unit)}`"></ndd-text-cell>
          <ndd-cell v-if="editable">
            <div class="mr-row-actions">
              <ndd-button @click="editDef(def.name)" text="Bewerk"></ndd-button>
              <ndd-icon-button
                icon="minus"
                accessible-label="Verwijder definitie"
                :data-testid="`def-${def.name}-delete-btn`"
                @click="deleteDef(def.name)"
              ></ndd-icon-button>
            </div>
          </ndd-cell>
        </ndd-list-item>
        <ndd-list-item v-if="editable" size="md">
          <ndd-button start-icon="plus-small" data-testid="add-def-btn" @click="addDef" text="Nieuwe definitie"></ndd-button>
        </ndd-list-item>
      </ndd-list>
      <ndd-spacer size="16"></ndd-spacer>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length || editable">
      <ndd-title size="5" data-testid="section-parameters"><h5>Parameters</h5></ndd-title>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-list variant="box">
        <ndd-list-item v-for="(param, index) in parameters" :key="param.name" size="md">
          <ndd-text-cell :text="`${param.name} (${param.type})`"></ndd-text-cell>
          <ndd-cell v-if="editable">
            <div class="mr-row-actions">
              <ndd-button @click="editParam(index)" text="Bewerk"></ndd-button>
              <ndd-icon-button
                icon="minus"
                accessible-label="Verwijder parameter"
                :data-testid="`param-${param.name}-delete-btn`"
                @click="deleteParam(index)"
              ></ndd-icon-button>
            </div>
          </ndd-cell>
        </ndd-list-item>
        <ndd-list-item v-if="editable" size="md">
          <ndd-button start-icon="plus-small" data-testid="add-param-btn" @click="addParam" text="Nieuwe parameter"></ndd-button>
        </ndd-list-item>
      </ndd-list>
      <ndd-spacer size="16"></ndd-spacer>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length || editable">
      <ndd-title size="5" data-testid="section-inputs"><h5>Inputs</h5></ndd-title>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-list variant="box">
        <ndd-list-item v-for="(input, index) in inputs" :key="input.name" :data-testid="`input-row-${input.name}`" size="md">
          <ndd-text-cell :text="`${input.name} (${input.type})${input.source ? ` — ${input.source}` : ''}`"></ndd-text-cell>
          <ndd-cell v-if="editable">
            <div class="mr-row-actions">
              <ndd-button :data-testid="`input-${input.name}-edit-btn`" @click="editInput(index)" text="Bewerk"></ndd-button>
              <ndd-icon-button
                icon="minus"
                accessible-label="Verwijder input"
                :data-testid="`input-${input.name}-delete-btn`"
                @click="deleteInput(index)"
              ></ndd-icon-button>
            </div>
          </ndd-cell>
        </ndd-list-item>
        <ndd-list-item v-if="editable" size="md">
          <ndd-button start-icon="plus-small" data-testid="add-input-btn" @click="addInput" text="Nieuwe input"></ndd-button>
        </ndd-list-item>
      </ndd-list>
      <ndd-spacer size="16"></ndd-spacer>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length || editable">
      <ndd-title size="5" data-testid="section-outputs"><h5>Outputs</h5></ndd-title>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-list variant="box">
        <ndd-list-item v-for="(output, index) in outputs" :key="output.name" size="md">
          <ndd-text-cell :text="`${output.name} (${output.type})`"></ndd-text-cell>
          <ndd-cell v-if="editable">
            <div class="mr-row-actions">
              <ndd-button @click="editOutput(index)" text="Bewerk"></ndd-button>
              <ndd-icon-button
                icon="minus"
                accessible-label="Verwijder output"
                :data-testid="`output-${output.name}-delete-btn`"
                @click="deleteOutput(index)"
              ></ndd-icon-button>
            </div>
          </ndd-cell>
        </ndd-list-item>
        <ndd-list-item v-if="editable" size="md">
          <ndd-button start-icon="plus-small" data-testid="add-output-btn" @click="addOutput" text="Nieuwe output"></ndd-button>
        </ndd-list-item>
      </ndd-list>
      <ndd-spacer size="16"></ndd-spacer>
    </template>

    <!-- Acties -->
    <template v-if="actions.length || editable">
      <ndd-title size="5" data-testid="section-actions"><h5>Acties</h5></ndd-title>
      <ndd-spacer size="8"></ndd-spacer>
      <ndd-list variant="box">
        <ndd-list-item v-for="(action, index) in actions" :key="index" size="md">
          <ndd-text-cell :text="action.output"></ndd-text-cell>
          <ndd-cell>
            <div class="mr-row-actions">
              <ndd-button :data-testid="`action-${action.output}-edit-btn`" @click="emit('open-action', action)" :text="editable ? 'Bewerk' : 'Bekijk'"></ndd-button>
              <ndd-icon-button
                v-if="editable"
                icon="minus"
                accessible-label="Verwijder actie"
                :data-testid="`action-${action.output}-delete-btn`"
                @click="deleteAction(index)"
              ></ndd-icon-button>
            </div>
          </ndd-cell>
        </ndd-list-item>
        <ndd-list-item v-if="editable" size="md">
          <ndd-button start-icon="plus-small" data-testid="add-action-btn" @click="emit('add-action')" text="Voeg actie toe"></ndd-button>
        </ndd-list-item>
      </ndd-list>
      <ndd-spacer size="16"></ndd-spacer>
    </template>
  </ndd-simple-section>
</template>

<style scoped>
.mr-save-bar {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 8px;
}

/* Row-level actions cluster: Bewerk button + minus icon button. flex-end
 * keeps them right-aligned within the row's value cell, and the gap matches
 * the spacing used in OperationSettings' value-row pattern. */
.mr-row-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}
</style>
