<script setup>
import { computed, ref, watch, onMounted, onUnmounted } from 'vue';
import { buildOperationTree } from '../utils/operationTree.js';
import OperationSettings from './OperationSettings.vue';

const props = defineProps({
  action: { type: Object, default: null },
  article: { type: Object, default: null },
});

const outputOptions = computed(() => {
  const outputs = props.article?.machine_readable?.execution?.output;
  if (!Array.isArray(outputs)) return [];
  return outputs.map(o => ({
    value: o.name,
    label: `${o.name.replace(/_/g, ' ')} (${o.type})`,
  }));
});

const emit = defineEmits(['close']);

const operationTree = computed(() => props.action ? buildOperationTree(props.action) : []);

const selectedOpIndex = ref(0);

watch(() => props.action, () => {
  const tree = operationTree.value;
  selectedOpIndex.value = tree.length > 0 ? tree.length - 1 : 0;
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
  <div v-if="action" class="action-sheet-overlay" @click.self="emit('close')">
    <div class="action-sheet-backdrop" @click="emit('close')"></div>
    <div class="action-sheet-panel">
      <!-- Header -->
      <rr-toolbar size="md">
        <rr-toolbar-start-area>
          <rr-toolbar-item>
            <span class="action-sheet-header-title">Actie</span>
          </rr-toolbar-item>
        </rr-toolbar-start-area>
        <rr-toolbar-end-area>
          <rr-toolbar-item>
            <rr-button variant="accent-transparent" size="md" @click="emit('close')">Annuleer</rr-button>
          </rr-toolbar-item>
        </rr-toolbar-end-area>
      </rr-toolbar>

      <!-- Body -->
      <div class="action-sheet-body">
        <rr-simple-section>
          <!-- Output binding -->
          <h3 class="section-title">Output</h3>
          <rr-list variant="box">
            <rr-list-item size="md">
              <rr-label-cell>Verbonden aan</rr-label-cell>
              <rr-button-cell slot="end">
                <rr-drop-down-field size="md" :value="action?.output" .options="outputOptions"></rr-drop-down-field>
              </rr-button-cell>
            </rr-list-item>
          </rr-list>

          <rr-spacer size="16"></rr-spacer>

          <!-- Section A: Bovenliggende operaties -->
          <template v-if="parentOperations.length">
            <h3 class="section-title">Bovenliggende operaties</h3>
            <rr-list variant="box">
              <rr-list-item v-for="op in parentOperations" :key="op.number" size="md">
                <div class="op-cell">
                  <div class="op-cell-title">{{ op.number }}. {{ op.title }}</div>
                  <div class="op-cell-subtitle">{{ op.subtitle }}</div>
                </div>
                <rr-button-cell slot="end">
                  <rr-button variant="neutral-tinted" size="sm" @click="selectOperation(op)">Bewerk</rr-button>
                </rr-button-cell>
              </rr-list-item>
            </rr-list>

            <rr-spacer size="16"></rr-spacer>
          </template>

          <!-- Section B: Operation Settings -->
          <OperationSettings v-if="selectedOperation" :operation="selectedOperation" :article="article" @select-operation="selectOperationByNode" />
        </rr-simple-section>
      </div>

      <!-- Footer -->
      <div class="action-sheet-footer">
        <rr-button variant="accent-filled" size="md" style="width: 100%;" @click="emit('close')">
          Sluiten
        </rr-button>
      </div>
    </div>
  </div>
</template>

<style>
.action-sheet-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  justify-content: flex-end;
}
.action-sheet-backdrop {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.1);
}
.action-sheet-panel {
  position: relative;
  width: 640px;
  background: #fff;
  display: flex;
  flex-direction: column;
  height: 100%;
  box-shadow: 0px 16px 64px 0px rgba(0, 0, 0, 0.11),
              0px 8px 32px 0px rgba(0, 0, 0, 0.09),
              0px 4px 16px 0px rgba(0, 0, 0, 0.06),
              0px 2px 8px 0px rgba(0, 0, 0, 0.04),
              0px 1px 4px 0px rgba(0, 0, 0, 0.03),
              0px 0px 2px 0px rgba(0, 0, 0, 0.02);
}
.action-sheet-header-title {
  font-family: var(--rr-font-family-title, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 20px;
  line-height: 1.4;
  color: var(--semantics-text-primary-color, #333B44);
}
.action-sheet-body {
  flex: 1;
  overflow-y: auto;
}
.action-sheet-footer {
  padding: 0 16px 16px;
}
.section-title {
  font-family: var(--rr-font-family-title, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 20px;
  line-height: 1.4;
  color: var(--semantics-text-primary-color, #333B44);
  margin: 0 0 8px 0;
}
.op-cell {
  flex: 1;
  min-width: 0;
}
.op-cell-title {
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 16px;
  line-height: 1.4;
  color: var(--semantics-text-primary-color, #333B44);
}
.op-cell-subtitle {
  font-family: var(--rr-font-family-body, 'RijksSansVF', sans-serif);
  font-weight: 400;
  font-size: 14px;
  line-height: 1.25;
  color: var(--semantics-text-secondary-color, #545D68);
}
</style>
