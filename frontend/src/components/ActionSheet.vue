<script setup>
import { computed, ref, watch, nextTick } from 'vue';
import { buildOperationTree } from '../utils/operationTree.js';
import OperationSettings from './OperationSettings.vue';

const props = defineProps({
  action: { type: Object, default: null },
  article: { type: Object, default: null },
});

const emit = defineEmits(['close']);

const sheetEl = ref(null);

const operationTree = computed(() => props.action ? buildOperationTree(props.action) : []);

const selectedOpIndex = ref(0);

watch(() => props.action, async (val) => {
  const tree = operationTree.value;
  selectedOpIndex.value = tree.length > 0 ? tree.length - 1 : 0;
  await nextTick();
  if (val) sheetEl.value?.show();
  else sheetEl.value?.hide();
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
</script>

<template>
  <rr-sheet ref="sheetEl" placement="right" @close="emit('close')">
    <div class="action-sheet-content">
      <!-- Header -->
      <rr-toolbar size="md">
        <rr-toolbar-start-area>
          <rr-toolbar-item>
            <rr-title-bar size="4">Actie</rr-title-bar>
          </rr-toolbar-item>
        </rr-toolbar-start-area>
        <rr-toolbar-end-area>
          <rr-toolbar-item>
            <rr-button variant="accent-transparent" size="md" @click="emit('close')">Annuleer</rr-button>
          </rr-toolbar-item>
        </rr-toolbar-end-area>
      </rr-toolbar>

      <!-- Body -->
      <div class="action-sheet-body" v-if="action">
        <rr-simple-section>
          <!-- Section A: Bovenliggende operaties -->
          <template v-if="parentOperations.length">
            <rr-title-bar size="5">Bovenliggende operaties</rr-title-bar>
            <rr-spacer size="4"></rr-spacer>
            <rr-list variant="box">
              <rr-list-item v-for="op in parentOperations" :key="op.number" size="md">
                <rr-text-cell>
                  <span slot="text">{{ op.number }}. {{ op.title }}</span>
                  <span slot="supporting-text">{{ op.subtitle }}</span>
                </rr-text-cell>
                <rr-cell>
                  <rr-button variant="neutral-tinted" size="sm" @click="selectOperation(op)">Bewerk</rr-button>
                </rr-cell>
              </rr-list-item>
            </rr-list>

            <rr-spacer size="8"></rr-spacer>
          </template>

          <!-- Section B: Operation Settings -->
          <OperationSettings v-if="selectedOperation" :operation="selectedOperation" :article="article" @select-operation="selectOperationByNode" />
        </rr-simple-section>
      </div>

      <!-- Footer -->
      <div class="action-sheet-footer">
        <rr-button variant="accent-filled" size="md" full-width @click="emit('close')">
          Opslaan
        </rr-button>
      </div>
    </div>
  </rr-sheet>
</template>

<style>
.action-sheet-content {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 640px;
  max-width: 100vw;
}
.action-sheet-body {
  flex: 1;
  overflow-y: auto;
}
.action-sheet-footer {
  padding: 0 16px 16px;
}
</style>
