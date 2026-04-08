import { ref, onUnmounted } from 'vue';

export function useJobDetail() {
  const job = ref(null);
  const isOpen = ref(false);

  let progressTimer = null;

  function open(initialJob) {
    job.value = { ...initialJob };
    isOpen.value = true;
    stopProgressPoll();
    if (initialJob.status === 'processing') {
      startProgressPoll();
    }
  }

  function close() {
    isOpen.value = false;
    stopProgressPoll();
    setTimeout(() => {
      if (!isOpen.value) job.value = null;
    }, 300);
  }

  function startProgressPoll() {
    stopProgressPoll();
    progressTimer = setInterval(async () => {
      if (!job.value) return;
      try {
        const resp = await fetch(`api/jobs/${encodeURIComponent(job.value.id)}`);
        if (!resp.ok) return;
        const updated = await resp.json();
        job.value = updated;
        if (updated.status !== 'processing') {
          stopProgressPoll();
        }
      } catch {
        // ignore fetch errors during polling
      }
    }, 10_000);
  }

  function stopProgressPoll() {
    if (progressTimer) {
      clearInterval(progressTimer);
      progressTimer = null;
    }
  }

  onUnmounted(() => {
    stopProgressPoll();
  });

  return { job, isOpen, open, close };
}
