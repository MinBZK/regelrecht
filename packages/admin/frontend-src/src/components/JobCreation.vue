<script setup>
import { ref } from 'vue';
import { redirectToLogin } from '../composables/useAuth.js';

const emit = defineEmits(['job-created']);
const inputRef = ref(null);
const submitting = ref(false);
const buttonLabel = ref('Harvest');

function getFieldValue(el) {
  if (el.value != null && el.value !== '') return el.value;
  const inner = el.shadowRoot?.querySelector('input');
  return inner?.value ?? '';
}

function setFieldValue(el, val) {
  el.value = val;
  const inner = el.shadowRoot?.querySelector('input');
  if (inner) inner.value = val;
}

async function onSubmit() {
  if (submitting.value) return;
  const el = inputRef.value;
  if (!el) return;
  const lawId = getFieldValue(el).trim();
  if (!lawId) return;
  if (!/^BWBR\d{7}$/.test(lawId) && !/^CVDR\d{3,}$/.test(lawId)) {
    alert('Expected a BWB ID (e.g. BWBR0018451) or CVDR ID (e.g. CVDR681386)');
    return;
  }

  submitting.value = true;
  buttonLabel.value = 'Submitting\u2026';

  try {
    const response = await fetch('api/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ law_id: lawId }),
    });
    if (response.status === 401) {
      redirectToLogin();
      return;
    }
    if (response.status === 409) {
      alert('A harvest job for this law is already pending or processing.');
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    await response.json();
    setFieldValue(el, '');
    buttonLabel.value = 'Queued \u2713';
    setTimeout(() => { buttonLabel.value = 'Harvest'; }, 2000);
    emit('job-created');
  } catch (err) {
    alert('Harvest failed: ' + err.message);
    buttonLabel.value = 'Harvest';
  } finally {
    submitting.value = false;
  }
}

function onKeydown(e) {
  if (e.key === 'Enter') onSubmit();
}
</script>

<template>
  <div class="job-creation">
    <span class="job-creation__label">New harvest job</span>
    <ndd-text-field
      ref="inputRef"
      size="md"
      placeholder="BWB or CVDR ID (e.g. BWBR0018451, CVDR681386)"
      @keydown="onKeydown"
    />
    <ndd-button
      variant="accent-filled"
      size="md"
      :text="buttonLabel"
      :disabled="submitting ? '' : undefined"
      @click="onSubmit"
    />
  </div>
</template>
