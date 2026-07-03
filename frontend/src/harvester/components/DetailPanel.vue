<script setup>
import { computed, ref, watch } from 'vue';
import StatusBadge from './StatusBadge.vue';
import { formatDate } from '../formatters.js';

const props = defineProps({
  job: { type: Object, default: null },
  isOpen: { type: Boolean, default: false },
});

const emit = defineEmits(['close']);

const sheetRef = ref(null);

watch(() => props.isOpen, (open) => {
  if (open) sheetRef.value?.show();
  else sheetRef.value?.hide();
});

const infoFields = computed(() => {
  if (!props.job) return [];
  return [
    ['Job ID', props.job.id],
    ['Law ID', props.job.law_id],
    ['Type', props.job.job_type],
    ['Status', props.job.status],
    ['Priority', props.job.priority],
    ['Attempts', `${props.job.attempts} / ${props.job.max_attempts}`],
    ['Created', formatDate(props.job.created_at)],
    ['Started', formatDate(props.job.started_at)],
    ['Completed', formatDate(props.job.completed_at)],
  ].filter(([, value]) => value != null);
});

const resultJson = computed(() =>
  props.job?.result ? JSON.stringify(props.job.result, null, 2) : null,
);

const payloadJson = computed(() =>
  props.job?.payload ? JSON.stringify(props.job.payload, null, 2) : null,
);

const codeSections = computed(() => {
  const j = props.job;
  if (!j) return [];
  const out = [];
  if (j.status === 'failed' && j.result?.error) {
    out.push({ title: 'Error', code: j.result.error, wrap: true });
  }
  if (j.status === 'completed' && resultJson.value) {
    out.push({ title: 'Result', code: resultJson.value, language: 'json' });
  }
  if (payloadJson.value) {
    out.push({ title: 'Payload', code: payloadJson.value, language: 'json' });
  }
  return out;
});

function onSheetClose() {
  if (props.isOpen) emit('close');
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetRef"
      placement="right"
      accessible-label="Job details"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Job details"
          dismiss-text="Close"
          @dismiss="$emit('close')"
        />
        <nldd-simple-section v-if="job">
          <nldd-list variant="simple">
            <nldd-list-item v-for="[label, value] in infoFields" :key="label">
              <nldd-text-cell :text="label" color="secondary" width="fit-content" />
              <nldd-spacer-cell size="12" />
              <nldd-cell
                v-if="label === 'Status'"
                width="full"
                style="align-items: flex-end"
              >
                <StatusBadge :status="value" size="md" />
              </nldd-cell>
              <nldd-text-cell v-else :text="String(value)" horizontal-alignment="right" />
            </nldd-list-item>
          </nldd-list>

          <template v-for="section in codeSections" :key="section.title">
            <nldd-spacer size="16" />
            <nldd-title size="6"><h3>{{ section.title }}</h3></nldd-title>
            <nldd-spacer size="4" />
            <nldd-code-viewer :wrap="section.wrap || undefined" :language="section.language || undefined">{{ section.code }}</nldd-code-viewer>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
