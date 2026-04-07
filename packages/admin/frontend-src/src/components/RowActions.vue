<script setup>
import { RE_HARVESTABLE_STATUSES, ENRICHABLE_STATUSES } from '../constants.js';

const props = defineProps({
  row: { type: Object, required: true },
});

const emit = defineEmits(['action-complete']);

async function onHarvest(event) {
  const btn = event.currentTarget;
  btn.setAttribute('disabled', '');
  btn.textContent = 'Submitting\u2026';

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
    btn.removeAttribute('disabled');
    btn.textContent = 'Harvest';
  }
}

async function onEnrich(event) {
  const btn = event.currentTarget;
  btn.setAttribute('disabled', '');
  btn.textContent = 'Submitting\u2026';

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
    btn.removeAttribute('disabled');
    btn.textContent = 'Enrich';
  }
}

async function onResetExhausted(event) {
  const btn = event.currentTarget;
  btn.setAttribute('disabled', '');
  btn.textContent = 'Resetting\u2026';

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
    btn.textContent = 'Reset \u2713';
    setTimeout(() => { btn.textContent = 'Reset'; }, 2000);
    emit('action-complete');
  } catch (err) {
    alert('Reset failed: ' + err.message);
  } finally {
    btn.removeAttribute('disabled');
  }
}
</script>

<template>
  <span class="action-btns">
    <rr-button
      v-if="RE_HARVESTABLE_STATUSES.includes(row.status)"
      variant="accent-outlined"
      size="sm"
      :title="'Re-harvest ' + row.law_id"
      @click.stop="onHarvest"
    >Harvest</rr-button>
    <rr-button
      v-if="ENRICHABLE_STATUSES.includes(row.status)"
      variant="neutral-tinted"
      size="sm"
      :title="'Trigger enrichment for ' + row.law_id"
      @click.stop="onEnrich"
    >Enrich</rr-button>
    <rr-button
      v-if="row.status === 'harvest_exhausted' || row.status === 'enrich_exhausted'"
      variant="accent-outlined"
      size="sm"
      :title="'Reset exhausted status for ' + row.law_id"
      @click.stop="onResetExhausted"
    >Reset</rr-button>
  </span>
</template>
