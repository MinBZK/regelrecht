<script setup>
defineProps({
  data: { type: Object, required: true },
});
</script>

<template>
  <button
    type="button"
    class="close"
    aria-label="Verberg wet"
  >
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
      <path
        fill="none"
        stroke="currentColor"
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M18 6L6 18M6 6l12 12"
      />
    </svg>
  </button>

  <div class="label">{{ data.label }}</div>
</template>

<style scoped>
.close {
  float: right;
  cursor: pointer;
  border-radius: 6px;
  background: rgba(2, 6, 23, 0.65);
  padding: 4px;
  color: white;
  border: 0;
  line-height: 0;
}

.close svg {
  width: 20px;
  height: 20px;
}

.label {
  text-align: center;
}
</style>
