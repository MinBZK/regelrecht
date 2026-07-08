/**
 * useTrajectDocumentJobs — poll a traject's document-conversion jobs (running +
 * failed) for the werkdocumenten conversion-status block.
 *
 * Mirrors the harvester `usePollingFetch` pattern (initial-load-only `loading`,
 * keep-stale on poll failure, auto-stop on unmount) but for the `{ jobs: [...] }`
 * response shape of `GET .../corpus/documents/jobs`. The consumer starts polling
 * when its surface opens and stops when it closes.
 */
import { ref, onUnmounted } from 'vue';
import { apiFetch } from '@regelrecht/frontend-shared';
import { documentJobsUrl } from './corpusUrls.js';

const DEFAULT_INTERVAL_MS = 4000;

export function useTrajectDocumentJobs(trajectRef, { interval = DEFAULT_INTERVAL_MS } = {}) {
  const jobs = ref([]);
  const loading = ref(false);
  const error = ref(null);

  let timer = null;
  let initialLoad = true;

  async function refresh() {
    const ref_ = trajectRef.value;
    if (!ref_) {
      jobs.value = [];
      return;
    }
    if (initialLoad) loading.value = true;
    try {
      // 401 handled by the global apiAuthGuard (redirect); return early so we
      // don't flash an error before it fires.
      const res = await apiFetch(documentJobsUrl(ref_), { allowStatuses: [401] });
      if (res.status === 401) return;
      const json = await res.json();
      jobs.value = Array.isArray(json?.jobs) ? json.jobs : [];
      error.value = null;
    } catch (e) {
      // Keep stale jobs on a poll failure; only clear on the very first load.
      if (initialLoad) jobs.value = [];
      error.value = e;
    } finally {
      loading.value = false;
      initialLoad = false;
    }
  }

  function startPolling() {
    stopPolling();
    refresh();
    timer = setInterval(refresh, interval);
  }

  function stopPolling() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  function reset() {
    initialLoad = true;
    jobs.value = [];
    error.value = null;
  }

  onUnmounted(stopPolling);

  return { jobs, loading, error, refresh, startPolling, stopPolling, reset };
}
