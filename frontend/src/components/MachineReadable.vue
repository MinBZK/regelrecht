<script setup>
import { computed, ref, watch, nextTick } from 'vue';
import { humanize } from '../utils/outputFormat.js';
import { useCorpusLaws } from '../composables/useCorpusLaws.js';
import BreakableName from './BreakableName.vue';
import RowActionsMenu from './RowActionsMenu.vue';

const { displayName: lawDisplayName } = useCorpusLaws();

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
    // Per-token parts for read-only display: a list keeps its raw items
    // (each wrapped in <wbr>-breakable tokens), a scalar is the formatted
    // single value. Lets long underscore identifiers wrap instead of
    // overflowing the (no longer fit-content) value cell.
    const isList = Array.isArray(val);
    const parts = isList ? val.map((v) => String(v)) : [formatValue(val, unit)];
    return { name, value: val, unit, isList, parts };
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
  // A list definition value (YAML sequence) renders here; String([...])
  // joins with a bare comma ("a,b,c") which has no break opportunity and
  // pushes the column width. Use comma + space so it can wrap.
  if (Array.isArray(val)) return val.join(', ');
  return String(val);
}

// structuredClone throws DataCloneError on Vue reactive proxies; YAML data is JSON-safe.
function snapshot(value) {
  return JSON.parse(JSON.stringify(value));
}

// Open edit sheet for existing items
function editDef(name) {
  const rawDef = mr.value?.definitions?.[name];
  if (rawDef == null) return;
  emit('open-edit', { section: 'definition', key: name, rawDef: snapshot(rawDef) });
}

function editParam(index) {
  const p = execution.value?.parameters?.[index];
  if (p) emit('open-edit', { section: 'parameter', index, data: snapshot(p) });
}

function editInput(index) {
  const raw = execution.value?.input?.[index];
  if (raw) emit('open-edit', { section: 'input', index, data: snapshot(raw) });
}

function editOutput(index) {
  const raw = execution.value?.output?.[index];
  if (raw) emit('open-edit', { section: 'output', index, data: snapshot(raw) });
}

// Delete handlers — stage a confirmation in `pendingDelete`, then on
// confirm emit the delete event with the section + identity of the row.
// The parent (EditorApp) is the source of truth for machineReadable,
// so all mutations live there. The modal-dialog confirmation is here
// because there is no undo yet.
const SECTION_LABELS = {
  definition: 'definitie',
  parameter: 'parameter',
  input: 'input',
  output: 'output',
  action: 'actie',
};

const pendingDelete = ref(null);
const deleteModalEl = ref(null);
const pendingSectionLabel = computed(
  () => (pendingDelete.value ? SECTION_LABELS[pendingDelete.value.section] ?? '' : ''),
);

watch(pendingDelete, async (val) => {
  await nextTick();
  // Guard against test envs where the modal-dialog custom element isn't
  // upgraded — the ref then holds a plain HTMLElement without show/hide.
  // Optional-chaining on `?.show()` would still throw because it only
  // skips when `.value` is nullish, not when `.show` itself is undefined.
  const el = deleteModalEl.value;
  if (val) {
    if (typeof el?.show === 'function') el.show();
  } else {
    if (typeof el?.hide === 'function') el.hide();
  }
});

function deleteDef(name) {
  pendingDelete.value = { section: 'definition', key: name, label: `definitie '${name}'` };
}

function deleteParam(index) {
  const p = parameters.value[index];
  pendingDelete.value = { section: 'parameter', index, label: `parameter '${p?.name ?? index}'` };
}

function deleteInput(index) {
  const i = inputs.value[index];
  pendingDelete.value = { section: 'input', index, label: `input '${i?.name ?? index}'` };
}

function deleteOutput(index) {
  const o = outputs.value[index];
  pendingDelete.value = { section: 'output', index, label: `output '${o?.name ?? index}'` };
}

function deleteAction(index) {
  const a = actions.value[index];
  pendingDelete.value = { section: 'action', index, label: `actie '${a?.output ?? index}'` };
}

function confirmDelete() {
  if (!pendingDelete.value) return;
  const { label, ...payload } = pendingDelete.value;
  emit('delete', payload);
  pendingDelete.value = null;
}

function cancelDelete() {
  // Idempotent guard: the Behoud button's @click and the modal's @close
  // both wire to this handler. Clicking Behoud sets pendingDelete = null,
  // the watcher then calls el.hide(), which fires @close → cancelDelete
  // again. Vue's ref equality check makes the second assignment a no-op
  // today, but a future side-effect would silently double-fire without
  // this early return.
  if (pendingDelete.value === null) return;
  pendingDelete.value = null;
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
  <nldd-inline-dialog v-if="!mr" data-testid="no-machine-readable" text="Geen machine-leesbare gegevens voor dit artikel">
    <nldd-button v-if="editable" slot="actions" variant="primary" size="md" data-testid="init-mr-btn" @click="emit('init-mr')" text="Initialiseer machine readable versie"></nldd-button>
  </nldd-inline-dialog>

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

    <!-- Definities -->
    <template v-if="definitions.length || editable">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" data-testid="section-definitions"><h5>Definities</h5></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="def in definitions" :key="def.name" size="md">
          <template v-if="editable">
            <nldd-text-cell :text="`${def.name} = ${formatValue(def.value, def.unit)}`"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
          </template>
          <template v-else>
            <nldd-text-cell min-width="120px"><BreakableName :name="def.name" /></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell
              v-if="def.isList"
              horizontal-alignment="right"
            ><template
              v-for="(part, i) in def.parts"
              :key="i"
            ><span v-if="i > 0">, </span><BreakableName :name="part" /></template></nldd-text-cell>
            <nldd-text-cell
              v-else
              width="fit-content"
              horizontal-alignment="right"
              :text="def.parts[0]"
            ></nldd-text-cell>
          </template>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor definitie ${def.name}`"
              :delete-testid="`def-${def.name}-delete-btn`"
              @edit="editDef(def.name)"
              @delete="deleteDef(def.name)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-def-btn" @click="addDef" text="Definitie toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length || editable">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" data-testid="section-parameters"><h5>Parameters</h5></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="(param, index) in parameters" :key="param.name" size="md">
          <nldd-text-cell><BreakableName :name="param.name" /> <nldd-tag size="sm" :text="param.type"></nldd-tag></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor parameter ${param.name}`"
              :delete-testid="`param-${param.name}-delete-btn`"
              @edit="editParam(index)"
              @delete="deleteParam(index)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-param-btn" @click="addParam" text="Parameter toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length || editable">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" data-testid="section-inputs"><h5>Inputs</h5></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="(input, index) in inputs" :key="input.name" :data-testid="`input-row-${input.name}`" size="md">
          <nldd-text-cell
            :supporting-text="input.source ? lawDisplayName(input.source) : undefined"
          ><BreakableName :name="input.name" /> <nldd-tag size="sm" :text="input.type"></nldd-tag></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor input ${input.name}`"
              :edit-testid="`input-${input.name}-edit-btn`"
              :delete-testid="`input-${input.name}-delete-btn`"
              @edit="editInput(index)"
              @delete="deleteInput(index)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-input-btn" @click="addInput" text="Input toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length || editable">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" data-testid="section-outputs"><h5>Outputs</h5></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item v-for="(output, index) in outputs" :key="output.name" size="md">
          <nldd-text-cell><BreakableName :name="output.name" /> <nldd-tag size="sm" :text="output.type"></nldd-tag></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor output ${output.name}`"
              :delete-testid="`output-${output.name}-delete-btn`"
              @edit="editOutput(index)"
              @delete="deleteOutput(index)"
            />
          </nldd-cell>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-output-btn" @click="addOutput" text="Output toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>

    <!-- Acties -->
    <template v-if="actions.length || editable">
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" data-testid="section-actions"><h5>Acties</h5></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-list variant="box">
        <nldd-list-item
          v-for="(action, index) in actions"
          :key="index"
          size="md"
          :type="editable ? undefined : 'button'"
          @click="!editable && emit('open-action', action)"
        >
          <nldd-text-cell :text="action.output"></nldd-text-cell>
          <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
          <nldd-cell v-if="editable">
            <RowActionsMenu
              :accessible-label="`Acties voor actie ${action.output}`"
              :edit-testid="`action-${action.output}-edit-btn`"
              :delete-testid="`action-${action.output}-delete-btn`"
              @edit="emit('open-action', action)"
              @delete="deleteAction(index)"
            />
          </nldd-cell>
          <template v-else>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon name="chevron-right"></nldd-icon>
            </nldd-icon-cell>
          </template>
        </nldd-list-item>
        <nldd-list-item v-if="editable" size="md">
          <nldd-cell width="full">
            <nldd-button width="full" start-icon="plus-small" data-testid="add-action-btn" @click="emit('add-action')" text="Actie toevoegen"></nldd-button>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
    </template>
  </div>

  <nldd-modal-dialog
    ref="deleteModalEl"
    variant="alert"
    :text="pendingDelete ? `Weet je zeker dat je ${pendingDelete.label} wilt verwijderen?` : ''"
    supporting-text="Deze actie kan niet ongedaan gemaakt worden."
    @close="cancelDelete"
  >
    <nldd-button slot="actions" variant="primary" :text="`Behoud ${pendingSectionLabel}`" @click="cancelDelete"></nldd-button>
    <nldd-button slot="actions" variant="destructive" text="Verwijder" @click="confirmDelete"></nldd-button>
  </nldd-modal-dialog>
</template>
