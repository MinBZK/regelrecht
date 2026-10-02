<script setup>
// A labelled box inside a law: Bronnen, Invoer or Uitvoer. Its items are
// child nodes, so this only draws the frame and the label.
const props = defineProps({ data: { type: Object, required: true } });
</script>

<template>
  <div :class="['graph-box', `graph-box--${props.data.kind}`]">
    <span class="graph-box__label">{{ props.data.label }}</span>
  </div>
</template>
