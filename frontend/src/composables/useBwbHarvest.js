/**
 * useBwbHarvest — request and track harvesting of external laws.
 *
 * Manages the lifecycle of harvest requests: submitting, polling for status,
 * and determining when a law becomes available. Works with the pipeline-api
 * (proxied through editor-api at /api/harvest/*).
 *
 * State is shared at module level so all callers see the same harvest status.
 */
import { ref, computed } from 'vue';

const TERMINAL_STATUSES = new Set([
  'harvest_failed', 'harvest_exhausted', 'enrich_failed', 'enrich_exhausted', 'error', 'timeout', 'unauthorized',
]);
const AVAILABLE_STATUSES = new Set(['harvested', 'enriched']);
const POLLING_STATUSES = new Set(['queued', 'already_queued', 'harvesting', 'enriching']);

const POLL_INTERVAL_MS = 5000;
const POLL_MAX_MS = 10 * 60 * 1000; // 10 minutes

// Module-level shared state — all callers of useBwbHarvest() share these refs.
/** @type {import('vue').Ref<Record<string, string>>} Status per BWB ID */
const harvestStatus = ref({});
/** @type {import('vue').Ref<Record<string, string>>} Resolved slug per BWB ID */
const harvestSlugs = ref({});

let pollInterval = null;
let pollStart = null;

const hasActiveHarvests = computed(() =>
  Object.values(harvestStatus.value).some(s => POLLING_STATUSES.has(s)),
);

// --- Polling (module-level, singleton) ---

function startPolling() {
  if (pollInterval) return;
  pollStart = Date.now();
  pollInterval = setInterval(pollHarvestStatus, POLL_INTERVAL_MS);
}

function stopPolling() {
  if (pollInterval) {
    clearInterval(pollInterval);
    pollInterval = null;
    pollStart = null;
  }
}

async function pollHarvestStatus() {
  // Timeout check
  if (pollStart && Date.now() - pollStart > POLL_MAX_MS) {
    const updated = { ...harvestStatus.value };
    for (const [id, status] of Object.entries(updated)) {
      if (POLLING_STATUSES.has(status)) updated[id] = 'timeout';
    }
    harvestStatus.value = updated;
    stopPolling();
    return;
  }

  const activeIds = Object.entries(harvestStatus.value)
    .filter(([, status]) => POLLING_STATUSES.has(status))
    .map(([id]) => id);

  if (activeIds.length === 0) {
    stopPolling();
    return;
  }

  try {
    const res = await fetch(`/api/harvest/status?ids=${activeIds.join(',')}`);
    if (!res.ok) return;
    const data = await res.json();

    const updatedStatus = { ...harvestStatus.value };
    const updatedSlugs = { ...harvestSlugs.value };

    for (const entry of data.results) {
      updatedStatus[entry.bwb_id] = entry.status;
      if (entry.slug) updatedSlugs[entry.bwb_id] = entry.slug;
    }

    harvestStatus.value = updatedStatus;
    harvestSlugs.value = updatedSlugs;

    // Stop polling if no active IDs remain
    const stillActive = Object.values(updatedStatus).some(s => POLLING_STATUSES.has(s));
    if (!stillActive) stopPolling();
  } catch {
    // Poll is best-effort
  }
}

export function useBwbHarvest() {
  // --- Harvest requests ---

  /**
   * Request harvest for a single law by BWB ID.
   * @param {string} bwbId - The BWB identifier (e.g. "BWBR0018451")
   */
  async function requestHarvest(bwbId) {
    harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'loading' };
    try {
      const res = await fetch('/api/harvest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bwb_id: bwbId }),
      });
      if (res.ok) {
        const data = await res.json();
        harvestStatus.value = { ...harvestStatus.value, [bwbId]: data.status };
        if (data.status === 'queued' || data.status === 'already_queued') {
          startPolling();
        }
      } else if (res.status === 401) {
        harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'unauthorized' };
      } else {
        harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'error' };
      }
    } catch {
      harvestStatus.value = { ...harvestStatus.value, [bwbId]: 'error' };
    }
  }

  /**
   * Request harvest for multiple laws by slug (for dependency walker).
   * @param {string[]} lawIds - Law slugs to harvest
   * @returns {Promise<object|null>} Batch response or null on failure
   */
  async function requestHarvestBatch(lawIds) {
    try {
      const res = await fetch('/api/harvest/batch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ law_ids: lawIds }),
      });
      if (res.ok) {
        return await res.json();
      }
    } catch {
      // Batch harvest is best-effort
    }
    return null;
  }

  // --- Status helpers ---

  const isTerminal = (s) => TERMINAL_STATUSES.has(s);
  const isAvailable = (s) => AVAILABLE_STATUSES.has(s);
  const isPolling = (s) => POLLING_STATUSES.has(s);

  function statusText(bwbId, fallback) {
    const s = harvestStatus.value[bwbId];
    if (!s) return fallback || '';
    switch (s) {
      case 'loading': return 'Aanvragen...';
      case 'queued':
      case 'already_queued': return 'Harvest aangevraagd';
      case 'harvesting': return 'Wordt opgehaald...';
      case 'enriching': return 'Wordt verwerkt...';
      case 'harvested':
      case 'enriched': return 'Beschikbaar \u2014 klik om te openen';
      case 'harvest_failed':
      case 'harvest_exhausted':
      case 'enrich_failed':
      case 'enrich_exhausted': return 'Ophalen mislukt';
      case 'timeout': return 'Timeout \u2014 probeer later opnieuw';
      case 'unauthorized': return 'Log in om te harvesten';
      case 'error': return 'Fout bij aanvragen';
      default: return fallback || '';
    }
  }

  function statusIcon(bwbId) {
    const s = harvestStatus.value[bwbId];
    if (!s) return 'arrow-down-to-line';
    if (AVAILABLE_STATUSES.has(s)) return 'arrow-right';
    if (s === 'loading' || POLLING_STATUSES.has(s)) return 'arrow-clockwise';
    if (TERMINAL_STATUSES.has(s)) return 'x-circle';
    return 'arrow-down-to-line';
  }

  return {
    harvestStatus,
    harvestSlugs,
    hasActiveHarvests,
    requestHarvest,
    requestHarvestBatch,
    isTerminal,
    isAvailable,
    isPolling,
    statusText,
    statusIcon,
  };
}
