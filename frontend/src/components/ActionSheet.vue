<script setup>
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { buildOperationTree } from '../utils/operationTree.js';
import OperationSettings from './OperationSettings.vue';

const props = defineProps({
  action: { type: Object, default: null },
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
  /** A freshly added action — Save is always offered (you opened the
   *  sheet to create it), regardless of the dirty snapshot. */
  isNew: { type: Boolean, default: false },
});

const emit = defineEmits(['close', 'save', 'edit']);

const sheetEl = ref(null);

const operationTree = computed(() => props.action ? buildOperationTree(props.action) : []);

const selectedOpIndex = ref(0);

// Monotonic counter used as :key for the nldd-page element so it remounts
// every time a new action opens (fixing the sticky-header height
// measurement) without remounting on every keystroke in the output field.
let actionSeq = 0;
const actionKey = ref('none');

// Snapshot the action when it opens so we can show "Opslaan" only when
// something actually changed. Edits mutate the action in place, so a
// JSON compare of the live action vs this baseline is the dirty signal.
const actionBaseline = ref('');
const isDirty = computed(() => {
  if (!props.action) return false;
  try {
    return JSON.stringify(props.action) !== actionBaseline.value;
  } catch {
    return true;
  }
});

watch(() => props.action, async (action) => {
  selectedOpIndex.value = 0;
  actionKey.value = action ? String(++actionSeq) : 'none';

  if (!action) {
    sheetEl.value?.hide();
    return;
  }
  actionBaseline.value = JSON.stringify(action);
  await nextTick();
  sheetEl.value?.show();
}, { immediate: true });

const selectedOperation = computed(() => operationTree.value[selectedOpIndex.value] ?? null);

// When the action has no operation (e.g. `value: $PERCENTAGE_LID_5`), the
// operation tree is empty and the sheet would otherwise show a blank body.
// Show the value verbatim from the YAML — `$VAR` refs stay in their CAPS +
// underscore form because the `$` is a code-marker that pairs visually
// with code-style identifiers (humanizing them to `$percentage lid 5`
// reads inconsistently).
const directValue = computed(() => {
  if (selectedOperation.value) return null;
  const v = props.action?.value;
  if (v == null) return null;
  return { label: String(v) };
});

const parentOperations = computed(() => {
  const selected = selectedOperation.value;
  if (!selected) return [];
  return operationTree.value.filter(op =>
    op !== selected && selected.number.startsWith(op.number + '.')
  );
});

function selectOperation(op) {
  const idx = operationTree.value.indexOf(op);
  if (idx >= 0) selectedOpIndex.value = idx;
}

function selectOperationByNode(node) {
  const idx = operationTree.value.findIndex(op => op.node === node);
  if (idx >= 0) selectedOpIndex.value = idx;
}

function handleKeydown(e) {
  if (e.key === 'Escape' && props.action) {
    emit('close');
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <nldd-sheet ref="sheetEl" placement="right" :width="editable ? '640px' : '480px'" @close="emit('close')">
    <!-- :key forces nldd-page to remount whenever a NEW action opens.
         nldd-page captures the sticky-header height ONCE per mount via
         requestAnimationFrame; when the sheet opens with a new action the
         header may already be rendered but the measurement happened while
         the sheet was still hidden, producing a zero-height offset that
         lets the body content slide up under the title bar (visible as a
         fade). Remounting fixes the measurement.

         actionKey is captured once when the action changes (in the
         watcher), NOT reactively bound to action.output, so editing the
         output text field does not trigger a remount on every keystroke. -->
    <nldd-page :key="actionKey" sticky-header>
      <nldd-top-title-bar slot="header" text="Actie" :dismiss-text="editable ? 'Annuleer' : 'Sluit'" @dismiss="emit('close')"></nldd-top-title-bar>

      <nldd-simple-section>
        <!-- Output binding lives inside the operation's settings list (via
             OperationSettings' lead-row slot) at the top level, or in the
             direct-value list below — so it sits in the same box as the
             other root rows instead of a separate box. -->

        <!-- Section A: Bovenliggende operaties -->
        <template v-if="parentOperations.length">
          <nldd-list variant="box">
            <!-- Back/up navigation — clickable parent rows with a
                 chevron-left, identical in view and edit: click any
                 ancestor to jump up one or more levels. -->
            <nldd-list-item
              v-for="op in parentOperations"
              :key="op.number"
              size="md"
              :data-testid="`parent-op-${op.number}`"
              type="button"
              @click="selectOperation(op)"
            >
              <nldd-icon-cell size="20">
                <nldd-icon name="chevron-left"></nldd-icon>
              </nldd-icon-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-text-cell :text="`${op.number}. ${op.title}`" :supporting-text="op.subtitle">
              </nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
        </template>

        <!-- Section B: Operation Settings -->
        <nldd-spacer v-if="parentOperations.length && selectedOperation" size="24"></nldd-spacer>
        <OperationSettings v-if="selectedOperation" :operation="selectedOperation" :article="article" :editable="editable" :hide-title-row="editable && !parentOperations.length" @select-operation="selectOperationByNode">
          <template #lead-row>
            <nldd-list-item v-if="editable && action && !parentOperations.length" size="md">
              <nldd-text-cell text="Output" width="120px"></nldd-text-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field size="md" :value="action.output" @input="action.output = $event.target?.value ?? $event.detail?.value ?? action.output" data-testid="action-output-field"></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
          </template>
        </OperationSettings>

        <!-- Direct value: action has no operation, just outputs a value
             (literal or $VAR reference). Mirror OperationSettings' Titel +
             Waarde layout so the sheet body isn't blank and the user sees
             which action they're looking at. -->
        <nldd-list v-if="directValue" variant="box">
          <nldd-list-item size="md">
            <nldd-text-cell text="Output" :width="editable ? '120px' : 'fit-content'"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-cell v-if="editable">
              <nldd-text-field size="md" :value="action.output" @input="action.output = $event.target?.value ?? $event.detail?.value ?? action.output" data-testid="action-output-field"></nldd-text-field>
            </nldd-cell>
            <nldd-text-cell v-else horizontal-alignment="right" :text="action.output || '(leeg)'"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-text-cell text="Waarde" width="fit-content"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-text-cell horizontal-alignment="right" :text="directValue.label"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </nldd-simple-section>

      <nldd-container slot="footer" padding="16">
        <nldd-button v-if="editable && (isNew || isDirty)" variant="primary" size="md" width="full" data-testid="action-sheet-save-btn" @click="emit('save')" text="Opslaan"></nldd-button>
        <nldd-button v-else-if="!editable" variant="secondary" size="md" width="full" data-testid="action-sheet-edit-btn" @click="emit('edit')" text="Bewerken"></nldd-button>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>
