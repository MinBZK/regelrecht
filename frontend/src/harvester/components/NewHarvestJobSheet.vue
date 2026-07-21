<script setup>
import { nextTick, ref, watch } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import { useNewHarvestJob } from '../composables/useNewHarvestJob.js';

const { isOpen, close, notifyCreated } = useNewHarvestJob();

const sheetRef = ref(null);
const inputRef = ref(null);
const submitting = ref(false);
const errorId = ref('');
const networkError = ref('');
const buttonLabel = ref('Add harvest job');

watch(isOpen, async (open) => {
  if (open) {
    errorId.value = '';
    networkError.value = '';
    sheetRef.value?.show();
    await nextTick();
    inputRef.value?.focus();
  } else {
    sheetRef.value?.hide();
  }
});

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

async function submit({ keepOpen = false } = {}) {
  if (submitting.value) return;
  const el = inputRef.value;
  if (!el) return;
  const lawId = getFieldValue(el).trim();
  if (!lawId) {
    errorId.value = 'harvest-law-id-required';
    el.focus?.();
    return;
  }
  if (!/^BWBR\d{7}$/.test(lawId) && !/^CVDR\d{3,}$/.test(lawId)) {
    errorId.value = 'harvest-law-id-format';
    el.focus?.();
    return;
  }
  errorId.value = '';

  submitting.value = true;
  buttonLabel.value = 'Submitting…';

  try {
    const response = await apiFetch('/api/harvest-admin/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ law_id: lawId }),
      allowStatuses: [401, 409],
    });
    // 401 is handled by the editor's global apiAuthGuard (redirect to login).
    if (response.status === 401) return;
    if (response.status === 409) {
      errorId.value = 'harvest-law-id-conflict';
      el.focus?.();
      return;
    }
    await response.json();
    setFieldValue(el, '');
    notifyCreated();
    if (keepOpen) {
      el.focus?.();
    } else {
      close();
    }
  } catch (err) {
    networkError.value = err.message || 'Unknown error';
    errorId.value = 'harvest-network-error';
    el.focus?.();
  } finally {
    submitting.value = false;
    buttonLabel.value = 'Add harvest job';
  }
}

function onSubmit() { submit(); }
function onSubmitAndAddAnother() { submit({ keepOpen: true }); }

function onInput() {
  if (errorId.value) errorId.value = '';
  if (networkError.value) networkError.value = '';
}

function onKeydown(e) {
  if (e.key === 'Enter') onSubmit();
}

function onSheetClose() {
  if (isOpen.value) close();
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetRef"
      placement="right"
      accessible-label="New harvest job"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="New harvest job"
          dismiss-text="Cancel"
          @dismiss="close"
        />
        <nldd-simple-section>
          <nldd-form>
            <nldd-form-field
              label="Law ID"
              supporting-label="BWB or CVDR ID (e.g. BWBR0018451, CVDR681386)"
            >
              <nldd-text-field
                ref="inputRef"
                size="md"
                :invalid="errorId ? '' : undefined"
                :error-message="errorId || undefined"
                @input="onInput"
                @keydown="onKeydown"
              />
              <nldd-form-field-error-text id="harvest-law-id-required">
                Law ID is required.
              </nldd-form-field-error-text>
              <nldd-form-field-error-text id="harvest-law-id-format">
                Expected a BWB ID (e.g. BWBR0018451) or CVDR ID (e.g. CVDR681386).
              </nldd-form-field-error-text>
              <nldd-form-field-error-text id="harvest-law-id-conflict">
                A harvest job for this law is already pending or processing.
              </nldd-form-field-error-text>
              <nldd-form-field-error-text id="harvest-network-error">
                Failed to submit harvest job: {{ networkError }}
              </nldd-form-field-error-text>
            </nldd-form-field>
            <nldd-form-actions>
              <nldd-button-group>
                <nldd-button
                  variant="accent-filled"
                  :text="buttonLabel"
                  :disabled="submitting ? '' : undefined"
                  @click="onSubmit"
                />
                <nldd-button
                  variant="secondary"
                  text="Add and add another"
                  :disabled="submitting ? '' : undefined"
                  @click="onSubmitAndAddAnother"
                />
              </nldd-button-group>
            </nldd-form-actions>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
