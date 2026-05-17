<script setup>
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { buildOperationTree } from '../utils/operationTree.js';
import OperationSettings from './OperationSettings.vue';

const props = defineProps({
  action: { type: Object, default: null },
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
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

watch(() => props.action, async (action) => {
  selectedOpIndex.value = 0;
  actionKey.value = action ? String(++actionSeq) : 'none';

  if (!action) {
    sheetEl.value?.hide();
    return;
  }
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
  <nldd-sheet ref="sheetEl" placement="right" width="480px" @close="emit('close')">
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
        <!-- Output binding (editable view only).
             The `&& action` guard prevents a render-time TypeError when the
             sheet is mounted but `action` hasn't been set yet — the parent
             always mounts the sheet eagerly and toggles visibility via the
             web-component's show()/hide() methods, so `action` is null
             whenever the sheet isn't actively editing something. -->
        <template v-if="editable && action">
          <nldd-list variant="box" class="settings-list" data-testid="action-output-binding">
            <nldd-list-item size="md">
              <nldd-text-cell text="Output" width="fit-content"></nldd-text-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-cell>
                <nldd-text-field size="md" :value="action.output" @input="action.output = $event.target?.value ?? $event.detail?.value ?? action.output" data-testid="action-output-field"></nldd-text-field>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>

          <nldd-spacer size="8"></nldd-spacer>
        </template>

        <!-- Section A: Bovenliggende operaties -->
        <template v-if="parentOperations.length">
          <nldd-list variant="box">
            <nldd-list-item
              v-for="op in parentOperations"
              :key="op.number"
              size="md"
              :data-testid="`parent-op-${op.number}`"
              :type="editable ? undefined : 'button'"
              @click="!editable && selectOperation(op)"
            >
              <template v-if="!editable">
                <nldd-icon-cell size="20">
                  <nldd-icon name="chevron-left"></nldd-icon>
                </nldd-icon-cell>
                <nldd-spacer-cell size="12"></nldd-spacer-cell>
              </template>
              <nldd-text-cell :text="`${op.number}. ${op.title}`" :supporting-text="op.subtitle">
              </nldd-text-cell>
              <nldd-spacer-cell v-if="editable" size="8"></nldd-spacer-cell>
              <nldd-cell v-if="editable">
                <nldd-button :data-testid="`parent-op-${op.number}-edit-btn`" @click="selectOperation(op)" text="Bewerk"></nldd-button>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </template>

        <!-- Section B: Operation Settings -->
        <nldd-spacer v-if="parentOperations.length && selectedOperation" size="24"></nldd-spacer>
        <OperationSettings v-if="selectedOperation" :operation="selectedOperation" :article="article" :editable="editable" @select-operation="selectOperationByNode" />

        <!-- Direct value: action has no operation, just outputs a value
             (literal or $VAR reference). Mirror OperationSettings' Titel +
             Waarde layout so the sheet body isn't blank and the user sees
             which action they're looking at. -->
        <nldd-list v-if="directValue" variant="box">
          <nldd-list-item size="md">
            <nldd-text-cell text="Output" width="fit-content"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-text-cell horizontal-alignment="right" :text="action.output || '(leeg)'"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="md">
            <nldd-text-cell text="Waarde" width="fit-content"></nldd-text-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-text-cell horizontal-alignment="right" :text="directValue.label"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </nldd-simple-section>

      <nldd-container slot="footer" padding="16">
        <nldd-button v-if="editable" variant="primary" size="md" width="full" data-testid="action-sheet-save-btn" @click="emit('save')" text="Opslaan"></nldd-button>
        <nldd-button v-else variant="secondary" size="md" width="full" data-testid="action-sheet-edit-btn" @click="emit('edit')" text="Bewerken"></nldd-button>
      </nldd-container>
    </nldd-page>
  </nldd-sheet>
</template>
