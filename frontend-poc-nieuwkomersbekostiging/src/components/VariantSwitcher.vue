<template>
  <div class="variant-switcher">
    <!-- Meerkeuze (beleid-view): kolommen naast huidig recht, max `max`. -->
    <nldd-toggle-button-group
      v-if="multiple"
      type="checkbox"
      size="sm"
      accessible-label="Beleidsvarianten naast huidig recht"
    >
      <nldd-toggle-button
        v-for="v in variants"
        :key="v.id"
        :text="chipTitel(v)"
        :accessible-label="shortTitle(v)"
        :title="shortTitle(v)"
        :value="v.id"
        :selected="modelValue.includes(v.id) ? true : undefined"
        :disabled="!modelValue.includes(v.id) && modelValue.length >= max ? true : undefined"
        @change="onToggle(v.id, $event)"
      ></nldd-toggle-button>
    </nldd-toggle-button-group>

    <!-- Enkelkeuze (casus- en schoolview): één variant naast huidig recht. -->
    <nldd-dropdown v-else :key="`vs:${modelValue}:${variants.length}`" size="sm" width="360px" @change="onSelect">
      <select :value="modelValue ?? ''" aria-label="Beleidsvariant om te vergelijken">
        <option value="">Geen variant</option>
        <option v-for="v in variants" :key="v.id" :value="v.id">{{ shortTitle(v) }}</option>
      </select>
    </nldd-dropdown>

    <p v-if="!variants.length" class="vs-leeg">
      Geen varianten gevonden: maak een <code>variant/nk-*</code>-branch die het corpus wijzigt en draai <code>just dev</code> opnieuw.
    </p>
  </div>
</template>

<script setup>
import { useLawStore } from '../engine/lawStore.js';
import { shortTitle } from '../composables/useSimulation.js';
import { chipTitel } from '../engine/lawStore.js';

const props = defineProps({
  multiple: { type: Boolean, default: false },
  modelValue: { type: [Array, String], default: () => [] },
  max: { type: Number, default: 3 },
});
const emit = defineEmits(['update:modelValue', 'change']);

const { variants } = useLawStore();

function onToggle(id, event) {
  const selected = event.detail?.selected ?? !props.modelValue.includes(id);
  let next;
  if (selected) {
    if (props.modelValue.includes(id)) return;
    if (props.modelValue.length >= props.max) return;
    next = [...props.modelValue, id];
  } else {
    next = props.modelValue.filter((x) => x !== id);
  }
  emit('update:modelValue', next);
  emit('change', next);
}

function onSelect(event) {
  const id = event.detail?.value ?? event.target?.value ?? '';
  emit('update:modelValue', id || null);
  emit('change', id || null);
}
</script>

<style scoped>
.variant-switcher { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
/* De knoppen breken hun tekst niet af; op een smal scherm zouden lange
   labels buiten het paneel steken. De groep mag wel afbreken. */
.variant-switcher nldd-toggle-button-group { max-width: 100%; }
.vs-leeg { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
