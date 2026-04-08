<script setup>
import { computed, watch, onUnmounted } from 'vue';
import StatusBadge from './StatusBadge.vue';
import ProgressIndicator from './ProgressIndicator.vue';
import { formatDate } from '../formatters.js';

const props = defineProps({
  job: { type: Object, default: null },
  isOpen: { type: Boolean, default: false },
});

const emit = defineEmits(['close']);

const infoFields = computed(() => {
  if (!props.job) return [];
  return [
    ['ID', props.job.id],
    ['Type', props.job.job_type],
    ['Law ID', props.job.law_id],
    ['Status', props.job.status],
    ['Priority', props.job.priority],
    ['Attempts', `${props.job.attempts} / ${props.job.max_attempts}`],
    ['Created', formatDate(props.job.created_at)],
    ['Started', formatDate(props.job.started_at)],
    ['Completed', formatDate(props.job.completed_at)],
  ].filter(([, value]) => value != null);
});

const resultJson = computed(() => {
  if (!props.job?.result) return null;
  return JSON.stringify(props.job.result, null, 2);
});

const payloadJson = computed(() => {
  if (!props.job?.payload) return null;
  return JSON.stringify(props.job.payload, null, 2);
});

function onKeydown(e) {
  if (e.key === 'Escape') emit('close');
}

watch(() => props.isOpen, (open) => {
  if (open) {
    document.addEventListener('keydown', onKeydown);
  } else {
    document.removeEventListener('keydown', onKeydown);
  }
});

onUnmounted(() => document.removeEventListener('keydown', onKeydown));
</script>

<template>
  <Teleport to="body">
    <div
      class="detail-backdrop"
      :class="{ 'is-open': isOpen }"
      :hidden="!isOpen ? '' : undefined"
      @click="emit('close')"
    />
    <aside class="detail-panel" :class="{ 'is-open': isOpen }">
      <div class="detail-panel__header">
        <h2 class="detail-panel__title">Job Details</h2>
        <ndd-icon-button
          variant="neutral-transparent"
          size="sm"
          accessible-label="Close"
          @click="emit('close')"
        >
          <ndd-icon name="dismiss" />
          Close
        </ndd-icon-button>
      </div>
      <div v-if="job" class="detail-panel__body">
        <!-- Info section -->
        <div class="detail-section">
          <h3 class="detail-section__title">Info</h3>
          <dl class="detail-grid">
            <template v-for="[label, value] in infoFields" :key="label">
              <dt>{{ label }}</dt>
              <dd>
                <StatusBadge v-if="label === 'Status'" :status="value" />
                <template v-else>{{ value }}</template>
              </dd>
            </template>
          </dl>
        </div>

        <!-- Progress section -->
        <ProgressIndicator v-if="job.status === 'processing'" :progress="job.progress" />

        <!-- Error section -->
        <div v-if="job.status === 'failed' && job.result?.error" class="detail-section">
          <h3 class="detail-section__title">Error</h3>
          <div class="detail-error">{{ job.result.error }}</div>
        </div>

        <!-- Result section -->
        <div v-if="job.status === 'completed' && resultJson" class="detail-section">
          <h3 class="detail-section__title">Result</h3>
          <div class="detail-json">{{ resultJson }}</div>
        </div>

        <!-- Payload section -->
        <div v-if="payloadJson" class="detail-section">
          <h3 class="detail-section__title">Payload</h3>
          <div class="detail-json">{{ payloadJson }}</div>
        </div>
      </div>
    </aside>
  </Teleport>
</template>
