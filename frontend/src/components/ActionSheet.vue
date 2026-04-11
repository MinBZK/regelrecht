<script setup>
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { buildOperationTree } from '../utils/operationTree.js';
import OperationSettings from './OperationSettings.vue';

const props = defineProps({
  action: { type: Object, default: null },
  article: { type: Object, default: null },
  editable: { type: Boolean, default: false },
});

const emit = defineEmits(['close', 'save']);

const sheetEl = ref(null);

const operationTree = computed(() => props.action ? buildOperationTree(props.action) : []);

const selectedOpIndex = ref(0);

watch(() => props.action, async (action) => {
  const tree = operationTree.value;
  selectedOpIndex.value = tree.length > 0 ? tree.length - 1 : 0;

  if (!action) {
    sheetEl.value?.hide();
    return;
  }
  await nextTick();
  sheetEl.value?.show();
}, { immediate: true });

const selectedOperation = computed(() => operationTree.value[selectedOpIndex.value] ?? null);

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
  <ndd-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <ndd-page sticky-header>
      <ndd-top-title-bar slot="header" text="Actie" dismiss-text="Annuleer" @dismiss="emit('close')"></ndd-top-title-bar>

      <ndd-simple-section>
        <!-- Output binding (editable view only).
             The `&& action` guard prevents a render-time TypeError when the
             sheet is mounted but `action` hasn't been set yet — the parent
             always mounts the sheet eagerly and toggles visibility via the
             web-component's show()/hide() methods, so `action` is null
             whenever the sheet isn't actively editing something. -->
        <template v-if="editable && action">
          <ndd-list variant="box" class="settings-list" data-testid="action-output-binding">
            <ndd-list-item size="md">
              <ndd-text-cell text="Output"></ndd-text-cell>
              <ndd-cell>
                <ndd-text-field size="md" :value="action.output" @input="action.output = $event.target?.value ?? $event.detail?.value ?? action.output" data-testid="action-output-field"></ndd-text-field>
              </ndd-cell>
            </ndd-list-item>
          </ndd-list>

          <ndd-spacer size="8"></ndd-spacer>
        </template>

        <!-- Section A: Bovenliggende operaties -->
        <template v-if="parentOperations.length">
          <ndd-title size="5"><h5>Bovenliggende operaties</h5></ndd-title>
          <ndd-spacer size="4"></ndd-spacer>
          <ndd-list variant="box">
            <ndd-list-item v-for="op in parentOperations" :key="op.number" size="md">
              <ndd-text-cell :text="`${op.number}. ${op.title}`" :supporting-text="op.subtitle">
              </ndd-text-cell>
              <ndd-cell>
                <ndd-button @click="selectOperation(op)" text="Bewerk"></ndd-button>
              </ndd-cell>
            </ndd-list-item>
          </ndd-list>

          <ndd-spacer size="8"></ndd-spacer>
        </template>

        <!-- Section B: Operation Settings -->
        <OperationSettings v-if="selectedOperation" :operation="selectedOperation" :article="article" :editable="editable" @select-operation="selectOperationByNode" />
      </ndd-simple-section>

      <ndd-container slot="footer" padding="16">
        <ndd-button v-if="editable" variant="primary" size="md" full-width @click="emit('save')" text="Opslaan"></ndd-button>
        <ndd-button v-else variant="primary" size="md" full-width @click="emit('close')" text="Sluiten"></ndd-button>
      </ndd-container>
    </ndd-page>
  </ndd-sheet>
</template>
