<template>
  <div
    class="flow-diagram"
    ref="container"
    @wheel.prevent="onWheel"
    @mousedown="onMouseDown"
    @mousemove="onMouseMove"
    @mouseup="onMouseUp"
    @mouseleave="onMouseUp"
  >
    <svg
      :viewBox="`${viewBox.x} ${viewBox.y} ${viewBox.w} ${viewBox.h}`"
      :width="svgWidth"
      :height="svgHeight"
      class="flow-diagram__svg"
      :class="{ 'flow-diagram__svg--grabbing': isDragging }"
    >
      <!-- Branch lines (background) -->
      <line
        v-for="branch in layoutBranches"
        :key="branch.id"
        :x1="80 + branch.col * columnWidth"
        :y1="50 + branch.startRow * rowHeight"
        :x2="80 + branch.col * columnWidth"
        :y2="50 + branch.endRow * rowHeight"
        :stroke="branch.color"
        stroke-width="4"
        stroke-linecap="round"
        :opacity="branchOpacity(branch)"
        class="flow-diagram__branch-line"
      />

      <!-- Branch labels -->
      <g
        v-for="branch in layoutBranches"
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

      <!-- Phase labels (advanced view) -->
      <g v-if="phases">
        <g
          v-for="phase in visiblePhases"
          :key="phase.id"
        >
          <rect
            :x="0"
            :y="50 + phase.startRow * rowHeight - 20"
            :width="svgWidth"
            :height="(phase.endRow - phase.startRow + 1) * rowHeight"
            :fill="phase.color || 'transparent'"
            opacity="0.04"
          />
          <text
            :x="16"
            :y="50 + phase.startRow * rowHeight - 4"
            font-size="11"
            font-weight="700"
            :fill="'var(--color-text-muted)'"
            text-transform="uppercase"
            class="flow-diagram__phase-label"
          >{{ phase.label }}</text>
        </g>
      </g>

      <!-- Connections -->
      <FlowConnection
        v-for="conn in connections"
        :key="conn.from + '-' + conn.to"
        :connection="conn"
        :stages="layoutStages"
        :is-active="isConnectionActive(conn)"
        :column-width="columnWidth"
        :row-height="rowHeight"
      />

      <!-- Nodes -->
      <FlowNode
        v-for="stage in layoutStages"
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
import { computed, ref, reactive, watch } from 'vue';
import FlowNode from './FlowNode.vue';
import FlowConnection from './FlowConnection.vue';

const props = defineProps({
  stages: { type: Array, required: true },
  branches: { type: Array, required: true },
  connections: { type: Array, required: true },
  phases: { type: Array, default: null },
  timeline: { type: Array, default: null },
  activeStep: { type: Number, default: -1 },
  selectedId: { type: String, default: null },
});

defineEmits(['select-stage']);

const columnWidth = 220;
const rowHeight = 80;

// Minimum row gap: 1 for same-column, 2 for cross-column (nicer S-curves)
const SAME_COL_GAP = 1;
const CROSS_COL_GAP = 2;

/**
 * Auto-layout: compute row positions from step order and connections.
 * Each stage is placed at max(predecessor rows) + gap.
 * Cross-column connections get a larger gap for nice S-curves.
 */
const layoutStages = computed(() => {
  const sorted = [...props.stages].sort((a, b) => a.step - b.step);
  const rowMap = new Map(); // stageId → computed row
  const colLastRow = new Map(); // col → last used row in that column
  let globalLastRow = -1; // track the highest row assigned so far

  for (const stage of sorted) {
    // Find all connections where this stage is the target
    const inbound = props.connections.filter((c) => c.to === stage.id);
    let row = 0;

    for (const conn of inbound) {
      const fromRow = rowMap.get(conn.from);
      if (fromRow === undefined) continue;
      const fromStage = sorted.find((s) => s.id === conn.from);
      const crossColumn = fromStage && fromStage.col !== stage.col;
      const gap = crossColumn ? CROSS_COL_GAP : SAME_COL_GAP;
      row = Math.max(row, fromRow + gap);
    }

    // Nodes without inbound connections (except the first) should be placed
    // after all previously laid-out nodes to respect step ordering
    if (inbound.length === 0 && globalLastRow >= 0) {
      row = Math.max(row, globalLastRow + SAME_COL_GAP);
    }

    // Ensure no overlap within the same column
    const lastInCol = colLastRow.get(stage.col);
    if (lastInCol !== undefined) {
      row = Math.max(row, lastInCol + SAME_COL_GAP);
    }

    rowMap.set(stage.id, row);
    colLastRow.set(stage.col, row);
    globalLastRow = Math.max(globalLastRow, row);
  }

  return sorted.map((s) => ({ ...s, row: rowMap.get(s.id) ?? s.row ?? 0 }));
});

/**
 * Auto-compute branch startRow/endRow from the stages on each branch.
 */
const layoutBranches = computed(() => {
  return props.branches.map((branch) => {
    const branchStages = layoutStages.value.filter((s) => s.branch === branch.id);
    if (branchStages.length === 0) return branch;
    const rows = branchStages.map((s) => s.row);
    return {
      ...branch,
      startRow: Math.min(...rows),
      endRow: Math.max(...rows),
    };
  });
});

const maxCol = computed(() => Math.max(0, ...layoutStages.value.map((s) => s.col)));
const maxRow = computed(() => Math.max(0, ...layoutStages.value.map((s) => s.row)));

const svgWidth = computed(() => 80 + (maxCol.value + 2) * columnWidth);
const svgHeight = computed(() => 50 + (maxRow.value + 2) * rowHeight);

// Pan & zoom state
const viewBox = reactive({ x: 0, y: 0, w: svgWidth.value, h: svgHeight.value });
const isDragging = ref(false);
let dragStart = { x: 0, y: 0, vbX: 0, vbY: 0 };

// Reset viewBox when data changes (toggle)
watch([svgWidth, svgHeight], () => {
  viewBox.x = 0;
  viewBox.y = 0;
  viewBox.w = svgWidth.value;
  viewBox.h = svgHeight.value;
});

function onWheel(e) {
  const delta = e.ctrlKey ? e.deltaY * 0.01 : e.deltaY * 0.002;
  const zoomFactor = 1 + Math.max(-0.15, Math.min(0.15, delta));
  const rect = e.currentTarget.getBoundingClientRect();
  const mx = (e.clientX - rect.left) / rect.width;
  const my = (e.clientY - rect.top) / rect.height;
  const cx = viewBox.x + viewBox.w * mx;
  const cy = viewBox.y + viewBox.h * my;
  const newW = viewBox.w * zoomFactor;
  const newH = viewBox.h * zoomFactor;
  viewBox.x = cx - newW * mx;
  viewBox.y = cy - newH * my;
  viewBox.w = newW;
  viewBox.h = newH;
}

function onMouseDown(e) {
  if (e.button !== 0) return;
  isDragging.value = true;
  dragStart = { x: e.clientX, y: e.clientY, vbX: viewBox.x, vbY: viewBox.y };
}

function onMouseMove(e) {
  if (!isDragging.value) return;
  const container = e.currentTarget;
  const scale = viewBox.w / container.clientWidth;
  viewBox.x = dragStart.vbX - (e.clientX - dragStart.x) * scale;
  viewBox.y = dragStart.vbY - (e.clientY - dragStart.y) * scale;
}

function onMouseUp() {
  isDragging.value = false;
}

function zoomIn() {
  const cx = viewBox.x + viewBox.w / 2;
  const cy = viewBox.y + viewBox.h / 2;
  viewBox.w *= 0.8;
  viewBox.h *= 0.8;
  viewBox.x = cx - viewBox.w / 2;
  viewBox.y = cy - viewBox.h / 2;
}

function zoomOut() {
  const cx = viewBox.x + viewBox.w / 2;
  const cy = viewBox.y + viewBox.h / 2;
  viewBox.w *= 1.25;
  viewBox.h *= 1.25;
  viewBox.x = cx - viewBox.w / 2;
  viewBox.y = cy - viewBox.h / 2;
}

function resetView() {
  viewBox.x = 0;
  viewBox.y = 0;
  viewBox.w = svgWidth.value;
  viewBox.h = svgHeight.value;
}

defineExpose({ zoomIn, zoomOut, resetView });

/**
 * Resolve phase row ranges from stage IDs (startStage/endStage)
 * or fall back to hardcoded startRow/endRow for legacy data.
 */
const resolvedPhases = computed(() => {
  if (!props.phases) return [];
  return props.phases.map((p) => {
    let startRow = p.startRow;
    let endRow = p.endRow;
    if (p.startStage) {
      const s = layoutStages.value.find((st) => st.id === p.startStage);
      if (s) startRow = s.row;
    }
    if (p.endStage) {
      const s = layoutStages.value.find((st) => st.id === p.endStage);
      if (s) endRow = s.row;
    }
    return { ...p, startRow, endRow };
  });
});

const visiblePhases = computed(() => {
  return resolvedPhases.value.filter((p) => {
    if (p.startRow === undefined || p.endRow === undefined) return false;
    const phaseStages = layoutStages.value.filter(
      (s) => s.row >= p.startRow && s.row <= p.endRow
    );
    if (phaseStages.length === 0) return false;
    const minStep = Math.min(...phaseStages.map((s) => s.step));
    return props.activeStep >= minStep;
  });
});

function branchOpacity(branch) {
  const branchStages = layoutStages.value.filter((s) => s.branch === branch.id);
  if (branchStages.length === 0) return 0;
  const minStep = Math.min(...branchStages.map((s) => s.step));
  return props.activeStep >= minStep ? 1 : 0;
}

function isConnectionActive(conn) {
  const fromStage = layoutStages.value.find((s) => s.id === conn.from);
  const toStage = layoutStages.value.find((s) => s.id === conn.to);
  if (!fromStage || !toStage) return false;
  return props.activeStep >= toStage.step;
}
</script>

<style scoped>
.flow-diagram {
  display: flex;
  justify-content: center;
  overflow: hidden;
  padding: var(--spacing-4);
  position: relative;
  cursor: grab;
}

.flow-diagram__svg {
  max-width: 100%;
  height: auto;
}

.flow-diagram__svg--grabbing {
  cursor: grabbing;
}

.flow-diagram__branch-line {
  transition: opacity 0.5s ease;
}

.flow-diagram__branch-label {
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  transition: opacity 0.5s ease;
}

.flow-diagram__phase-label {
  font-family: var(--font-family);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.flow-diagram__timeline-label {
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-variant-numeric: tabular-nums;
}
</style>
