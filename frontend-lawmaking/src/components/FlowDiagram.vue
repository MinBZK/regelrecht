<template>
  <div class="flow-diagram" ref="container">
    <svg
      :viewBox="`0 0 ${svgWidth} ${svgHeight}`"
      :width="svgWidth"
      :height="svgHeight"
      class="flow-diagram__svg"
    >
      <!-- Branch lines (background) -->
      <line
        v-for="branch in branches"
        :key="branch.id"
        :x1="80 + branch.col * columnWidth"
        :y1="50 + branch.startRow * rowHeight"
        :x2="80 + branch.col * columnWidth"
        :y2="50 + branch.endRow * rowHeight"
        :stroke="branch.color"
        stroke-width="3"
        stroke-linecap="round"
        :opacity="branchOpacity(branch)"
        class="flow-diagram__branch-line"
      />

      <!-- Branch labels -->
      <g
        v-for="branch in branches"
        :key="'label-' + branch.id"
        :opacity="branchOpacity(branch)"
      >
        <text
          :x="80 + branch.col * columnWidth"
          :y="50 + branch.startRow * rowHeight - 20"
          text-anchor="middle"
          font-size="12"
          font-weight="700"
          :fill="branch.color"
          class="flow-diagram__branch-label"
        >{{ branch.gitLabel }}</text>
      </g>

      <!-- Connections -->
      <FlowConnection
        v-for="conn in connections"
        :key="conn.from + '-' + conn.to"
        :connection="conn"
        :is-active="isConnectionActive(conn)"
        :column-width="columnWidth"
        :row-height="rowHeight"
      />

      <!-- Nodes -->
      <FlowNode
        v-for="stage in stages"
        :key="stage.id"
        :stage="stage"
        :is-active="stage.step <= activeStep"
        :is-selected="selectedId === stage.id"
        :column-width="columnWidth"
        :row-height="rowHeight"
        @select="$emit('select-stage', $event)"
      />
    </svg>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { stages, branches, connections } from '../data/flowData.js';
import FlowNode from './FlowNode.vue';
import FlowConnection from './FlowConnection.vue';

const props = defineProps({
  activeStep: { type: Number, default: -1 },
  selectedId: { type: String, default: null },
});

defineEmits(['select-stage']);

const columnWidth = 220;
const rowHeight = 80;

const svgWidth = computed(() => 80 + 3 * columnWidth + 80);
const svgHeight = computed(() => 50 + 13 * rowHeight);

function branchOpacity(branch) {
  // Show branch line when any node on it is active
  const branchStages = stages.filter((s) => s.branch === branch.id);
  const minStep = Math.min(...branchStages.map((s) => s.step));
  return props.activeStep >= minStep ? 1 : 0;
}

function isConnectionActive(conn) {
  const fromStage = stages.find((s) => s.id === conn.from);
  const toStage = stages.find((s) => s.id === conn.to);
  if (!fromStage || !toStage) return false;
  return props.activeStep >= toStage.step;
}
</script>

<style>
.flow-diagram {
  display: flex;
  justify-content: center;
  overflow-x: auto;
  padding: var(--spacing-4);
}

.flow-diagram__svg {
  max-width: 100%;
  height: auto;
}

.flow-diagram__branch-line {
  transition: opacity 0.5s ease;
}

.flow-diagram__branch-label {
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  transition: opacity 0.5s ease;
}
</style>
