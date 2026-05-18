<script setup>
import { computed, useId } from 'vue';
import { useRouter } from 'vue-router';
import { RE_HARVESTABLE_STATUSES, ENRICHABLE_STATUSES } from '../constants.js';
import { authedFetch } from '../composables/useAuth.js';

const props = defineProps({
  row: { type: Object, required: true },
});

const emit = defineEmits(['action-complete']);

const router = useRouter();

function onViewJobs() {
  router.push({ name: 'jobs', query: { law_id: props.row.law_id } });
}

const uid = useId();
const menuAnchor = computed(() => `row-actions-${uid}`);

const canHarvest = computed(() => RE_HARVESTABLE_STATUSES.includes(props.row.status));
const canEnrich = computed(() => ENRICHABLE_STATUSES.includes(props.row.status));
const canReset = computed(
  () => props.row.status === 'harvest_exhausted' || props.row.status === 'enrich_exhausted',
);

async function onHarvest() {
  try {
    const response = await authedFetch('api/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: props.row.law_id }),
    });
    if (!response) return;
    if (response.status === 409) {
      const text = await response.text().catch(() => '');
      alert(text || 'A harvest job for this law is already pending or processing.');
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    const result = await response.json();
    alert(`Created harvest job: ${result.job_id}`);
    emit('action-complete');
  } catch (err) {
    alert('Harvest failed: ' + err.message);
  }
}

async function onEnrich() {
  try {
    const response = await authedFetch('api/enrich-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ law_id: props.row.law_id }),
    });
    if (!response) return;
    if (response.status === 409) {
      const text = await response.text().catch(() => '');
      alert(text || 'Enrich jobs for this law are already pending or processing.');
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    const result = await response.json();
    alert(`Created ${result.job_ids.length} enrich job(s) for ${result.providers.join(', ')}`);
    emit('action-complete');
  } catch (err) {
    alert('Enrich failed: ' + err.message);
  }
}

async function onResetExhausted() {
  try {
    const response = await authedFetch(
      `api/law_entries/${encodeURIComponent(props.row.law_id)}/reset-exhausted`,
      { method: 'POST' },
    );
    if (!response) return;
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    emit('action-complete');
  } catch (err) {
    alert('Reset failed: ' + err.message);
  }
}
</script>

<template>
  <nldd-icon-button
    :id="menuAnchor"
    icon="ellipsis"
    text="Actions"
    tooltip-timing="never"
    variant="neutral-tinted"
    size="md"
  />
  <nldd-menu :anchor="menuAnchor">
    <nldd-menu-item v-if="canHarvest" text="Harvest" @click.stop="onHarvest" />
    <nldd-menu-item v-if="canEnrich" text="Enrich" @click.stop="onEnrich" />
    <nldd-menu-item v-if="canReset" text="Reset exhausted" @click.stop="onResetExhausted" />
    <nldd-menu-item text="View job details" @click.stop="onViewJobs" />
  </nldd-menu>
</template>
