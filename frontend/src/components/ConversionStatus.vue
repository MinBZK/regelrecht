<script setup>
/**
 * ConversionStatus - presentational list of a traject's in-progress and
 * failed document-conversion jobs. Completed jobs are not shown here - they
 * appear as the actual `.md` in the documents list.
 *
 * With `tasks.job_review` ON, the jobs endpoint only ever returns
 * pending/processing rows (a failure surfaces as a `job_failed` task in the
 * Taken sheet instead, see TasksSheet.vue), so the failed-branch below is
 * inert - `job.status` never equals `'failed'`. With the flag OFF, the
 * endpoint falls back to including failed rows (with `error`), and this is
 * the old inline failure UI from before the taken-mechanisme existed.
 */
import { deslugifyDocPath } from '../lib/docTitle.js';

defineProps({
  jobs: { type: Array, default: () => [] },
});

// A pending conversion has no body (and thus no frontmatter title) yet, so
// the de-slugged target path is the best available name.
function title(job) {
  return deslugifyDocPath(job.target_path || 'document');
}
</script>

<template>
  <div v-if="jobs.length" class="conversion-status">
    <template v-for="job in jobs" :key="job.id">
      <nldd-inline-dialog
        v-if="job.status === 'failed'"
        variant="alert"
        :text="`Conversie mislukt: ${title(job)}`"
        :supporting-text="job.error || 'Onbekende fout'"
      ></nldd-inline-dialog>
      <nldd-activity-indicator
        v-else
        :text="`Wordt geconverteerd… - ${title(job)}`"
        show-text
      ></nldd-activity-indicator>
    </template>
  </div>
</template>

<style scoped>
/* Stack the status rows with a small gap. Custom spacing on top of the design
   system - reported in the PR. */
.conversion-status {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 12px;
}
</style>
