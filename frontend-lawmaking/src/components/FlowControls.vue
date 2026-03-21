<template>
  <div class="flow-controls">
    <rr-button
      variant="neutral-tinted"
      @click="$emit('reset')"
      :disabled="activeStep <= 0"
    >
      <span class="flow-controls__icon">&#x23EE;</span>
    </rr-button>
    <rr-button
      variant="neutral-tinted"
      @click="$emit('back')"
      :disabled="activeStep <= 0"
    >
      <span class="flow-controls__icon">&#x258F;&#x25C0;</span>
    </rr-button>
    <rr-button
      :variant="isPlaying ? 'accent-filled' : 'neutral-tinted'"
      @click="$emit('toggle-play')"
    >
      <span class="flow-controls__icon">{{ isPlaying ? '&#x23F8;' : '&#x25B6;' }}</span>
    </rr-button>
    <rr-button
      variant="neutral-tinted"
      @click="$emit('forward')"
      :disabled="activeStep >= maxStep"
    >
      <span class="flow-controls__icon">&#x25B6;&#x258F;</span>
    </rr-button>
    <rr-button
      variant="neutral-tinted"
      @click="$emit('end')"
      :disabled="activeStep >= maxStep"
    >
      <span class="flow-controls__icon">&#x23ED;</span>
    </rr-button>

    <span class="flow-controls__step-label">
      Stap {{ Math.max(0, activeStep + 1) }} / {{ maxStep + 1 }}
    </span>
  </div>
</template>

<script setup>
defineProps({
  activeStep: { type: Number, default: -1 },
  maxStep: { type: Number, default: 12 },
  isPlaying: { type: Boolean, default: false },
});

defineEmits(['back', 'forward', 'reset', 'end', 'toggle-play']);
</script>

<style>
.flow-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-2);
  padding: var(--spacing-4);
  background: var(--color-surface);
  border-top: 1px solid var(--color-border);
}

.flow-controls__icon {
  font-size: 16px;
  line-height: 1;
}

.flow-controls__step-label {
  margin-left: var(--spacing-4);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  min-width: 100px;
}
</style>
