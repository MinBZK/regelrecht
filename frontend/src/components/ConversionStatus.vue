<script setup>
/**
 * ConversionStatus - presentational list of a traject's in-progress
 * document-conversion jobs, each shown as a spinner. Completed jobs are not
 * shown here - they appear as the actual `.md` in the documents list.
 * Failures are no longer part of this endpoint's job list: a conversion
 * failure now surfaces as a `job_failed` task in the Taken sheet instead
 * (see TasksSheet.vue). Renders nothing when there are no running jobs.
 */
defineProps({
  jobs: { type: Array, default: () => [] },
});

function title(job) {
  const path = job.target_path || 'document';
  return path.replace(/\.md$/, '');
}
</script>

<template>
  <div v-if="jobs.length" class="conversion-status">
    <nldd-activity-indicator
      v-for="job in jobs"
      :key="job.id"
      :text="`Wordt geconverteerd… - ${title(job)}`"
      show-text
    ></nldd-activity-indicator>
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
