export const LAW_STATUSES = [
  'unknown', 'queued', 'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
  'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted',
];

export const JOB_STATUSES = ['pending', 'processing', 'completed', 'failed'];

export const JOB_TYPES = ['harvest', 'enrich'];

export const ENRICHABLE_STATUSES = ['harvested', 'enriched', 'enrich_failed'];
export const RE_HARVESTABLE_STATUSES = ['unknown', 'queued', 'harvest_failed', 'harvested', 'enriched', 'enrich_failed'];

export const LAW_ENTRY_COLUMNS = [
  { key: 'law_id', label: 'Law ID', sortable: true },
  { key: 'law_name', label: 'Name', sortable: true },
  { key: 'status', label: 'Status', sortable: true, filter: { options: LAW_STATUSES } },
  { key: 'coverage_score', label: 'Coverage', sortable: true },
  { key: 'updated_at', label: 'Updated', sortable: true },
  { key: '_actions', label: 'Actions', sortable: false },
];

export const JOB_COLUMNS = [
  { key: 'id', label: 'ID', sortable: true },
  { key: 'job_type', label: 'Type', sortable: true, filter: { options: JOB_TYPES } },
  { key: 'law_id', label: 'Law ID', sortable: true, filter: { type: 'text' } },
  { key: 'status', label: 'Status', sortable: true, filter: { options: JOB_STATUSES } },
  { key: '_error', label: 'Error', sortable: false },
  { key: 'priority', label: 'Priority', sortable: true },
  { key: 'attempts', label: 'Attempts', sortable: true },
  { key: 'created_at', label: 'Created', sortable: true },
];

export const GROUPED_COLUMNS = [
  { key: 'law_id', label: 'Law ID', sortable: true },
  { key: 'total_jobs', label: 'Jobs', sortable: true, filter: { key: 'status', options: JOB_STATUSES, label: 'Status' } },
  { key: 'pending', label: 'Pending', sortable: false },
  { key: 'processing', label: 'Processing', sortable: false },
  { key: 'completed', label: 'Completed', sortable: false },
  { key: 'failed', label: 'Failed', sortable: false },
  { key: 'latest_created_at', label: 'Latest', sortable: true, filter: { key: 'job_type', options: JOB_TYPES, label: 'Type' } },
];

// Sort allowlists derived from sortable column definitions (defence in depth)
export const LAW_ENTRY_SORT_KEYS = new Set(LAW_ENTRY_COLUMNS.filter((c) => c.sortable).map((c) => c.key));
export const JOB_SORT_KEYS = new Set(JOB_COLUMNS.filter((c) => c.sortable).map((c) => c.key));
export const GROUPED_SORT_KEYS = new Set(GROUPED_COLUMNS.filter((c) => c.sortable).map((c) => c.key));

export const STATUS_BADGE_MAP = {
  completed: 'green',
  harvested: 'green',
  enriched: 'green',
  failed: 'red',
  harvest_failed: 'red',
  enrich_failed: 'red',
  harvest_exhausted: 'red',
  enrich_exhausted: 'red',
  processing: 'yellow',
  harvesting: 'yellow',
  enriching: 'yellow',
  pending: 'grey',
  unknown: 'grey',
  queued: 'grey',
};

export const DATE_FORMATTER = new Intl.DateTimeFormat('nl-NL', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
});

export const PHASE_LABELS = {
  mvt_research: 'MvT Research',
  generating: 'Generating',
  validating: 'Validating',
  reverse_validating: 'Reverse Validating',
};
