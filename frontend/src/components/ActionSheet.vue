<script setup>
import { computed } from 'vue';

const props = defineProps({
  action: { type: Object, default: null },
});

const emit = defineEmits(['close']);

const operations = computed(() => {
  if (!props.action) return [];
  return flattenOperations(props.action);
});

function flattenOperations(node, list = []) {
  if (!node) return list;
  if (node.operation) {
    list.push({ operation: node.operation, values: node.values, subject: node.subject, value: node.value });
  }
  if (node.value && typeof node.value === 'object') {
    flattenOperations(node.value, list);
  }
  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      if (typeof v === 'object') flattenOperations(v, list);
    }
  }
  if (node.then && typeof node.then === 'object') flattenOperations(node.then, list);
  if (node.else !== undefined && typeof node.else === 'object') flattenOperations(node.else, list);
  if (node.when && typeof node.when === 'object') flattenOperations(node.when, list);
  return list;
}

function describeOperation(op) {
  if (!op.operation) return '—';
  const args = [];
  if (op.subject) args.push(formatArg(op.subject));
  if (op.value !== undefined) args.push(formatArg(op.value));
  if (op.values) {
    for (const v of op.values) args.push(formatArg(v));
  }
  return `${op.operation}(${args.join(', ')})`;
}

function formatArg(v) {
  if (typeof v === 'string') return v.startsWith('$') ? v.slice(1) : v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (typeof v === 'object' && v !== null && v.operation) return v.operation + '(...)';
  return '...';
}
</script>

<template>
  <div v-if="action" class="action-sheet-overlay" @click.self="emit('close')">
    <div class="action-sheet-backdrop" @click="emit('close')"></div>
    <div class="action-sheet-panel">
      <rr-toolbar size="md">
        <rr-toolbar-start-area>
          <rr-toolbar-item>
            <span class="machine-section-title" style="margin:0">Actie: {{ action.output }}</span>
          </rr-toolbar-item>
        </rr-toolbar-start-area>
        <rr-toolbar-end-area>
          <rr-toolbar-item>
            <rr-button variant="accent-transparent" size="md" @click="emit('close')">Annuleer</rr-button>
          </rr-toolbar-item>
        </rr-toolbar-end-area>
      </rr-toolbar>

      <div class="action-sheet-body">
        <rr-simple-section>
          <h3 class="machine-section-title">Operaties</h3>
          <rr-list variant="box">
            <rr-list-item v-for="(op, i) in operations" :key="i" size="md">
              <rr-text-cell>{{ describeOperation(op) }}</rr-text-cell>
            </rr-list-item>
            <rr-list-item v-if="!operations.length" size="md">
              <rr-text-cell v-if="action.value && typeof action.value === 'string'">{{ action.value }}</rr-text-cell>
              <rr-text-cell v-else-if="action.resolve">resolve: {{ action.resolve.type }} / {{ action.resolve.output }}</rr-text-cell>
              <rr-text-cell v-else>Geen operaties</rr-text-cell>
            </rr-list-item>
          </rr-list>
        </rr-simple-section>
      </div>

      <div class="action-sheet-footer">
        <rr-button variant="accent-filled" size="md" style="width: 100%;" @click="emit('close')">Sluiten</rr-button>
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
  width: 480px;
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
.action-sheet-body {
  flex: 1;
  overflow-y: auto;
}
.action-sheet-footer {
  padding: 0 16px 16px;
}
</style>
