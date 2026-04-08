<script setup>
import { ref } from 'vue';
import { RE_HARVESTABLE_STATUSES, ENRICHABLE_STATUSES } from '../constants.js';

const props = defineProps({
  row: { type: Object, required: true },
});

const emit = defineEmits(['action-complete']);

const harvestSubmitting = ref(false);
const harvestLabel = ref('Harvest');
const enrichSubmitting = ref(false);
const enrichLabel = ref('Enrich');
const resetSubmitting = ref(false);
const resetLabel = ref('Reset');

async function onHarvest() {
  harvestSubmitting.value = true;
  harvestLabel.value = 'Submitting\u2026';

  try {
    const response = await fetch('api/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: props.row.law_id }),
    });
    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }
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
  } finally {
    harvestSubmitting.value = false;
    harvestLabel.value = 'Harvest';
  }
}

async function onEnrich() {
  enrichSubmitting.value = true;
  enrichLabel.value = 'Submitting\u2026';

  try {
    const response = await fetch('api/enrich-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ law_id: props.row.law_id }),
    });
    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }
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
  } finally {
    enrichSubmitting.value = false;
    enrichLabel.value = 'Enrich';
  }
}

async function onResetExhausted() {
  resetSubmitting.value = true;
  resetLabel.value = 'Resetting\u2026';

  try {
    const response = await fetch(`api/law_entries/${encodeURIComponent(props.row.law_id)}/reset-exhausted`, {
      method: 'POST',
    });
    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    resetLabel.value = 'Reset \u2713';
    setTimeout(() => { resetLabel.value = 'Reset'; }, 2000);
    emit('action-complete');
  } catch (err) {
    alert('Reset failed: ' + err.message);
  } finally {
    resetSubmitting.value = false;
  }
}
</script>

<template>
  <span class="action-btns">
    <ndd-button
      v-if="RE_HARVESTABLE_STATUSES.includes(row.status)"
      variant="accent-outlined"
      size="sm"
      :text="harvestLabel"
      :title="'Re-harvest ' + row.law_id"
      :disabled="harvestSubmitting ? '' : undefined"
      @click.stop="onHarvest"
    />
    <ndd-button
      v-if="ENRICHABLE_STATUSES.includes(row.status)"
      variant="neutral-tinted"
      size="sm"
      :text="enrichLabel"
      :title="'Trigger enrichment for ' + row.law_id"
      :disabled="enrichSubmitting ? '' : undefined"
      @click.stop="onEnrich"
    />
    <ndd-button
      v-if="row.status === 'harvest_exhausted' || row.status === 'enrich_exhausted'"
      variant="accent-outlined"
      size="sm"
      :text="resetLabel"
      :title="'Reset exhausted status for ' + row.law_id"
      :disabled="resetSubmitting ? '' : undefined"
      @click.stop="onResetExhausted"
    />
  </span>
</template>
