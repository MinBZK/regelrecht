<script setup>
/**
 * ConversionStatus - presentational list of a traject's in-progress and failed
 * document-conversion jobs. A running job shows a spinner; a failed job shows
 * its reason. Completed jobs are not shown here - they appear as the actual
 * `.md` in the documents list. Renders nothing when there are no such jobs.
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
