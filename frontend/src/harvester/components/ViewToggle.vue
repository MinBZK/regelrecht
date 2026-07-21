<script setup>
defineProps({
  value: { type: String, required: true },
});

defineEmits(['change']);
</script>

<template>
  <nldd-segmented-control size="sm" width="fit-content" :value="value" @change="$emit('change', $event)">
    <nldd-segmented-control-item value="grouped" text="Grouped" />
    <nldd-segmented-control-item value="flat" text="Flat" />
  </nldd-segmented-control>
</template>
