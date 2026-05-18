<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { authedFetch } from '../composables/useAuth.js';
import { useLawJobsSheet } from '../composables/useLawJobsSheet.js';
import StatusBadge from './StatusBadge.vue';
import { formatDate, jobSubtitle } from '../formatters.js';

const { isOpen, lawId, close } = useLawJobsSheet();

const JOBS_LIMIT = 100;

const sheetRef = ref(null);
const jobs = ref([]);
const totalJobs = ref(0);
const loading = ref(false);
const error = ref(null);
const selectedJob = ref(null);

watch(isOpen, (open) => {
  if (open) {
    sheetRef.value?.show();
    selectedJob.value = null;
    loadJobs();
  } else {
    sheetRef.value?.hide();
  }
});

onMounted(async () => {
  if (isOpen.value) {
    await nextTick();
    sheetRef.value?.show();
    selectedJob.value = null;
    loadJobs();
  }
});

watch(lawId, () => {
  if (isOpen.value) {
    selectedJob.value = null;
    loadJobs();
  }
});

async function loadJobs() {
  if (!lawId.value) return;
  loading.value = true;
  error.value = null;
  try {
    const params = new URLSearchParams({
      law_id: lawId.value,
      sort: 'created_at',
      order: 'desc',
      limit: String(JOBS_LIMIT),
    });
    const response = await authedFetch(`api/jobs?${params}`);
    if (!response) return;
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    const body = await response.json();
    jobs.value = body.data ?? [];
    totalJobs.value = body.total ?? jobs.value.length;
  } catch (err) {
    error.value = err.message;
  } finally {
    loading.value = false;
  }
}

function onSheetClose() {
  if (isOpen.value) close();
}

function onBack() {
  selectedJob.value = null;
}

const infoFields = computed(() => {
  if (!selectedJob.value) return [];
  const j = selectedJob.value;
  return [
    ['Job ID', j.id],
    ['Law ID', j.law_id],
    ['Type', j.job_type],
    ['Status', j.status],
    ['Priority', j.priority],
    ['Attempts', `${j.attempts} / ${j.max_attempts}`],
    ['Created', formatDate(j.created_at)],
    ['Started', formatDate(j.started_at)],
    ['Completed', formatDate(j.completed_at)],
  ].filter(([, value]) => value != null);
});

const resultJson = computed(() =>
  selectedJob.value?.result ? JSON.stringify(selectedJob.value.result, null, 2) : null,
);

const payloadJson = computed(() =>
  selectedJob.value?.payload ? JSON.stringify(selectedJob.value.payload, null, 2) : null,
);

const codeSections = computed(() => {
  const j = selectedJob.value;
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
</script>

<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetRef"
      placement="right"
      accessible-label="Jobs"
      @close="onSheetClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          v-if="!selectedJob"
          slot="header"
          :text="`Jobs for ${lawId || ''}`"
          dismiss-text="Close"
          @dismiss="close"
        />
        <nldd-top-title-bar
          v-else
          slot="header"
          :back-text="`Jobs for ${lawId}`"
          dismiss-text="Close"
          @back="onBack"
          @dismiss="close"
        />

        <nldd-simple-section v-if="!selectedJob">
          <nldd-inline-dialog
            v-if="loading && jobs.length === 0"
            text="Loading…"
          />
          <nldd-inline-dialog
            v-else-if="error"
            :text="'Failed to load jobs: ' + error"
          />
          <nldd-inline-dialog
            v-else-if="jobs.length === 0"
            text="No jobs"
          />
          <nldd-list v-else variant="simple">
            <nldd-list-item
              v-for="job in jobs"
              :key="job.id"
              size="md"
              type="button"
              @click="selectedJob = job"
            >
              <nldd-text-cell
                :overline="job.id"
                :text="jobSubtitle(job)"
                :supporting-text="job.result?.error || undefined"
              />
              <nldd-spacer-cell size="12" />
              <nldd-cell width="fit-content">
                <StatusBadge :status="job.status || 'unknown'" />
              </nldd-cell>
              <nldd-spacer-cell size="12" />
              <nldd-icon-cell icon="chevron-right" size="20" />
            </nldd-list-item>
          </nldd-list>
          <nldd-inline-dialog
            v-if="totalJobs > jobs.length"
            :text="`Showing first ${jobs.length} of ${totalJobs} jobs`"
          />
        </nldd-simple-section>

        <nldd-simple-section v-else>
          <nldd-title><h2>Job details</h2></nldd-title>
          <nldd-spacer size="8" />

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
            <nldd-code :wrap="section.wrap || undefined" :language="section.language || undefined">{{ section.code }}</nldd-code>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
