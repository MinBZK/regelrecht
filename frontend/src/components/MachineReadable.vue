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

function humanize(name) {
  if (typeof name !== 'string') return name;
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
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
  <div v-if="!mr" data-testid="no-machine-readable">
    <nldd-inline-dialog text="Geen machine-leesbare gegevens voor dit artikel">
      <nldd-button v-if="editable" slot="actions" variant="primary" size="md" data-testid="init-mr-btn" @click="emit('init-mr')" text="Initialiseer machine readable versie"></nldd-button>
    </nldd-inline-dialog>
  </div>

  <div v-else data-testid="machine-readable">
    <!-- Save error surfaces inline; the actual save button lives in the
         parent pane's footer. -->
    <template v-if="editable && saveError">
      <nldd-inline-dialog
        variant="alert"
        text="Opslaan mislukt"
        :supporting-text="saveError.message || String(saveError)"
        data-testid="save-mr-error"
      ></nldd-inline-dialog>
      <nldd-spacer size="12"></nldd-spacer>
    </template>

    <!-- Metadata: produces -->
    <nldd-list v-if="produces" variant="box">
      <nldd-list-item v-if="produces.legal_character" size="md">
        <nldd-text-cell text="Juridische basis"></nldd-text-cell>
        <nldd-cell v-if="editable">
          <nldd-button size="md" expandable :text="humanize(produces.legal_character)"></nldd-button>
        </nldd-cell>
        <nldd-text-cell v-else horizontal-alignment="right" :text="humanize(produces.legal_character)"></nldd-text-cell>
      </nldd-list-item>
      <nldd-list-item v-if="produces.decision_type" size="md">
        <nldd-text-cell text="Besluit-type"></nldd-text-cell>
        <nldd-cell v-if="editable">
          <nldd-button size="md" expandable :text="humanize(produces.decision_type)"></nldd-button>
        </nldd-cell>
        <nldd-text-cell v-else horizontal-alignment="right" :text="humanize(produces.decision_type)"></nldd-text-cell>
      </nldd-list-item>
    </nldd-list>

    <nldd-spacer v-if="produces" size="12"></nldd-spacer>

    <!-- Definities -->
    <template v-if="definitions.length || editable">
      <nldd-title size="5" data-testid="section-definitions"><h5>Definities</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="def in definitions" :key="def.name" size="md">
          <nldd-text-cell :text="`${humanize(def.name)} = ${formatValue(def.value, def.unit)}`"></nldd-text-cell>
          <nldd-cell v-if="editable">
            <div class="mr-row-actions">
              <nldd-button @click="editDef(def.name)" text="Bewerk"></nldd-button>
              <nldd-icon-button
                icon="minus"
                accessible-label="Verwijder definitie"
                :data-testid="`def-${def.name}-delete-btn`"
                @click="deleteDef(def.name)"
              ></nldd-icon-button>
            </div>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="stretch">
            <nldd-button full-width start-icon="plus-small" data-testid="add-def-btn" @click="addDef" text="Nieuwe definitie"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length || editable">
      <nldd-title size="5" data-testid="section-parameters"><h5>Parameters</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="(param, index) in parameters" :key="param.name" size="md">
          <nldd-text-cell :text="`${humanize(param.name)} (${param.type})`"></nldd-text-cell>
          <nldd-cell v-if="editable">
            <div class="mr-row-actions">
              <nldd-button @click="editParam(index)" text="Bewerk"></nldd-button>
              <nldd-icon-button
                icon="minus"
                accessible-label="Verwijder parameter"
                :data-testid="`param-${param.name}-delete-btn`"
                @click="deleteParam(index)"
              ></nldd-icon-button>
            </div>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="stretch">
            <nldd-button full-width start-icon="plus-small" data-testid="add-param-btn" @click="addParam" text="Nieuwe parameter"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length || editable">
      <nldd-title size="5" data-testid="section-inputs"><h5>Inputs</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="(input, index) in inputs" :key="input.name" :data-testid="`input-row-${input.name}`" size="md">
          <nldd-text-cell :text="`${humanize(input.name)} (${input.type})${input.source ? ` — ${humanize(input.source)}` : ''}`"></nldd-text-cell>
          <nldd-cell v-if="editable">
            <div class="mr-row-actions">
              <nldd-button :data-testid="`input-${input.name}-edit-btn`" @click="editInput(index)" text="Bewerk"></nldd-button>
              <nldd-icon-button
                icon="minus"
                accessible-label="Verwijder input"
                :data-testid="`input-${input.name}-delete-btn`"
                @click="deleteInput(index)"
              ></nldd-icon-button>
            </div>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="stretch">
            <nldd-button full-width start-icon="plus-small" data-testid="add-input-btn" @click="addInput" text="Nieuwe input"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length || editable">
      <nldd-title size="5" data-testid="section-outputs"><h5>Outputs</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="(output, index) in outputs" :key="output.name" size="md">
          <nldd-text-cell :text="`${humanize(output.name)} (${output.type})`"></nldd-text-cell>
          <nldd-cell v-if="editable">
            <div class="mr-row-actions">
              <nldd-button @click="editOutput(index)" text="Bewerk"></nldd-button>
              <nldd-icon-button
                icon="minus"
                accessible-label="Verwijder output"
                :data-testid="`output-${output.name}-delete-btn`"
                @click="deleteOutput(index)"
              ></nldd-icon-button>
            </div>
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="stretch">
            <nldd-button full-width start-icon="plus-small" data-testid="add-output-btn" @click="addOutput" text="Nieuwe output"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>

    <!-- Acties -->
    <template v-if="actions.length || editable">
      <nldd-title size="5" data-testid="section-actions"><h5>Acties</h5></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item
          v-for="(action, index) in actions"
          :key="index"
          size="md"
          :type="editable ? undefined : 'button'"
          @click="!editable && emit('open-action', action)"
        >
          <nldd-text-cell :text="humanize(action.output)"></nldd-text-cell>
          <nldd-cell v-if="editable">
            <div class="mr-row-actions">
              <nldd-button :data-testid="`action-${action.output}-edit-btn`" @click="emit('open-action', action)" text="Bewerk"></nldd-button>
              <nldd-icon-button
                icon="minus"
                accessible-label="Verwijder actie"
                :data-testid="`action-${action.output}-delete-btn`"
                @click="deleteAction(index)"
              ></nldd-icon-button>
            </div>
          </nldd-cell>
          <template v-else>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon name="chevron-right"></nldd-icon>
            </nldd-icon-cell>
          </template>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="stretch">
            <nldd-button full-width start-icon="plus-small" data-testid="add-action-btn" @click="emit('add-action')" text="Voeg actie toe"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-spacer size="16"></nldd-spacer>
    </template>
  </div>
</template>

<style scoped>
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
