<script setup>
/**
 * ArchEdge — a rolled-up relationship line with a clickable count badge.
 *
 * The line itself is a plain `<BaseEdge>` on a bezier path, identical in shape
 * to Vue Flow's built-in `default` edge, so the visuals match the pre-lifting
 * edges. When the edge aggregates more than one underlying relation
 * (`weight > 1`) it also renders a small badge in the `<EdgeLabelRenderer>`
 * layer showing the count; clicking the badge reveals the underlying relations.
 *
 * Only the badge is a click target. Vue Flow's invisible 20px interaction path
 * under every edge keeps `pointer-events: none` (see styles.css) so it cannot
 * steal clicks from the node expand toggles it crosses (regression from #982);
 * the label layer as a whole is click-through and only the badge re-enables
 * pointer events.
 */
import { computed, inject } from 'vue';
import { BaseEdge, EdgeLabelRenderer, getBezierPath } from '@vue-flow/core';

const props = defineProps({
  id: { type: String, required: true },
  sourceX: { type: Number, required: true },
  sourceY: { type: Number, required: true },
  targetX: { type: Number, required: true },
  targetY: { type: Number, required: true },
  sourcePosition: { type: String, default: undefined },
  targetPosition: { type: String, default: undefined },
  data: { type: Object, default: () => ({}) },
  markerEnd: { type: String, default: undefined },
  style: { type: Object, default: () => ({}) },
});

const revealEdge = inject('revealEdge', null);

const path = computed(() =>
  getBezierPath({
    sourceX: props.sourceX,
    sourceY: props.sourceY,
    sourcePosition: props.sourcePosition,
    targetX: props.targetX,
    targetY: props.targetY,
    targetPosition: props.targetPosition,
  }),
);

const weight = computed(() => props.data?.weight ?? 1);
const labelStyle = computed(() => ({
  transform: `translate(-50%, -50%) translate(${path.value[1]}px, ${path.value[2]}px)`,
}));

function onReveal(event) {
  event.stopPropagation();
  revealEdge?.(props.data);
}
</script>

<template>
  <BaseEdge :id="id" :path="path[0]" :marker-end="markerEnd" :style="style" />
  <EdgeLabelRenderer v-if="weight > 1">
    <button
      type="button"
      class="arch-edge__badge nodrag nopan"
      :class="`arch-edge__badge--${data.kind}`"
      :style="labelStyle"
      :title="`${weight} onderliggende relaties — klik om te ontvouwen`"
      @click="onReveal"
    >
      {{ weight }}
    </button>
  </EdgeLabelRenderer>
</template>
