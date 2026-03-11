<script setup>
import { computed } from 'vue';

const props = defineProps({
  article: { type: Object, default: null },
});

const emit = defineEmits(['open-action']);

const mr = computed(() => props.article?.machine_readable ?? null);
const execution = computed(() => mr.value?.execution ?? null);

const definitions = computed(() => {
  const defs = mr.value?.definitions;
  if (!defs) return [];
  return Object.entries(defs).map(([name, def]) => {
    const val = typeof def === 'object' ? def.value : def;
    return { name, value: val };
  });
});

const produces = computed(() => execution.value?.produces ?? null);

const parameters = computed(() =>
  (execution.value?.parameters ?? []).map((p) => ({
    name: p.name,
    type: p.type,
    required: p.required ?? false,
  }))
);

const inputs = computed(() =>
  (execution.value?.input ?? []).map((i) => ({
    name: i.name,
    type: i.type,
    source: i.source?.regulation ?? i.source?.output ?? null,
  }))
);

const outputs = computed(() =>
  (execution.value?.output ?? []).map((o) => ({
    name: o.name,
    type: o.type,
  }))
);

const actions = computed(() => execution.value?.actions ?? []);

function formatValue(val) {
  if (typeof val === 'number') {
    if (val >= 1 && val % 1 === 0 && val > 100) {
      return (val / 100).toLocaleString('nl-NL', { style: 'currency', currency: 'EUR' });
    }
    if (val > 0 && val < 1) {
      return (val * 100).toLocaleString('nl-NL', { maximumFractionDigits: 3 }) + '%';
    }
  }
  return String(val);
}
</script>

<template>
  <div v-if="!mr" style="padding: 32px; color: #999; text-align: center;">
    Geen machine-leesbare gegevens voor dit artikel
  </div>

  <template v-else>
    <!-- Metadata: produces -->
    <rr-list v-if="produces" variant="box">
      <rr-list-item v-if="produces.legal_character" size="md">
        <rr-label-cell>Juridische basis</rr-label-cell>
        <rr-button-cell slot="end">
          <rr-button variant="neutral-tinted" size="md">
            {{ produces.legal_character }}
            <img src="/assets/icons/chevron-down-small.svg" alt="" width="16" height="16">
          </rr-button>
        </rr-button-cell>
      </rr-list-item>
      <rr-list-item v-if="produces.decision_type" size="md">
        <rr-label-cell>Besluit-type</rr-label-cell>
        <rr-button-cell slot="end">
          <rr-button variant="neutral-tinted" size="md">
            {{ produces.decision_type }}
            <img src="/assets/icons/chevron-down-small.svg" alt="" width="16" height="16">
          </rr-button>
        </rr-button-cell>
      </rr-list-item>
    </rr-list>

    <rr-spacer v-if="produces" size="24"></rr-spacer>

    <!-- Definities -->
    <template v-if="definitions.length">
      <h3 class="machine-section-title">Definities</h3>
      <rr-list variant="box">
        <rr-list-item v-for="def in definitions" :key="def.name" size="md">
          <rr-text-cell>{{ def.name }} = {{ formatValue(def.value) }}</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="24"></rr-spacer>
    </template>

    <!-- Parameters -->
    <template v-if="parameters.length">
      <h3 class="machine-section-title">Parameters</h3>
      <rr-list variant="box">
        <rr-list-item v-for="param in parameters" :key="param.name" size="md">
          <rr-text-cell>{{ param.name }} ({{ param.type }})</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="24"></rr-spacer>
    </template>

    <!-- Inputs -->
    <template v-if="inputs.length">
      <h3 class="machine-section-title">Inputs</h3>
      <rr-list variant="box">
        <rr-list-item v-for="input in inputs" :key="input.name" size="md">
          <rr-text-cell>{{ input.name }} ({{ input.type }})<template v-if="input.source"> — {{ input.source }}</template></rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="24"></rr-spacer>
    </template>

    <!-- Outputs -->
    <template v-if="outputs.length">
      <h3 class="machine-section-title">Outputs</h3>
      <rr-list variant="box">
        <rr-list-item v-for="output in outputs" :key="output.name" size="md">
          <rr-text-cell>{{ output.name }} ({{ output.type }})</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="24"></rr-spacer>
    </template>

    <!-- Acties -->
    <template v-if="actions.length">
      <h3 class="machine-section-title">Acties</h3>
      <rr-list variant="box">
        <rr-list-item v-for="action in actions" :key="action.output" size="md">
          <rr-text-cell>{{ action.output }}</rr-text-cell>
          <rr-button-cell slot="end">
            <rr-button variant="neutral-tinted" size="sm" @click="emit('open-action', action)">Bewerk</rr-button>
          </rr-button-cell>
        </rr-list-item>
      </rr-list>
      <rr-spacer size="32"></rr-spacer>
    </template>
  </template>
</template>

<style>
.machine-section-title {
  font-family: var(--rr-font-family-title, 'RijksSansVF', sans-serif);
  font-weight: 550;
  font-size: 20px;
  line-height: 1.4;
  color: var(--semantics-text-primary-color, #333B44);
  margin: 0 0 8px 0;
}
rr-list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  min-height: 44px;
}
rr-list[variant="box"] rr-list-item + rr-list-item {
  border-top: 1px solid var(--semantics-dividers-color, #E0E3E8);
}
</style>
