<script setup>
/**
 * ConversionStatus - presentational list of a traject's in-progress
 * document-conversion jobs. Completed jobs are not shown here - they appear
 * as the actual `.md` in the documents list; a failure surfaces as a
 * `job_failed` task in the taken-lijst (see TasksPane.vue), never here (the
 * jobs endpoint only returns pending/processing rows).
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
