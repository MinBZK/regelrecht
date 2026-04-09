export const LAW_STATUSES = [
  'unknown', 'queued', 'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
  'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted',
];

export const JOB_STATUSES = ['pending', 'processing', 'completed', 'failed'];

export const JOB_TYPES = ['harvest', 'enrich'];

export const ENRICHABLE_STATUSES = ['harvested', 'enriched', 'enrich_failed'];
export const RE_HARVESTABLE_STATUSES = ['unknown', 'queued', 'harvest_failed', 'harvested', 'enriched', 'enrich_failed'];

export const LAW_ENTRY_COLUMNS = [
  { key: 'law_id', label: 'Law ID', sortable: true, width: 140 },
  { key: 'law_name', label: 'Name', sortable: true, width: 300 },
  { key: 'status', label: 'Status', sortable: true, filter: { options: LAW_STATUSES }, width: 140 },
  { key: 'coverage_score', label: 'Coverage', sortable: true, width: 90 },
  { key: 'updated_at', label: 'Updated', sortable: true, width: 160 },
  { key: '_actions', label: 'Actions', sortable: false, width: 160 },
];

export const JOB_COLUMNS = [
  { key: 'id', label: 'ID', sortable: true, width: 100 },
  { key: 'job_type', label: 'Type', sortable: true, filter: { options: JOB_TYPES }, width: 80 },
  { key: 'law_id', label: 'Law ID', sortable: true, filter: { type: 'text' }, width: 140 },
  { key: 'status', label: 'Status', sortable: true, filter: { options: JOB_STATUSES }, width: 110 },
  { key: '_error', label: 'Error', sortable: false, width: 200 },
  { key: 'priority', label: 'Priority', sortable: true, width: 70 },
  { key: 'attempts', label: 'Attempts', sortable: true, width: 80 },
  { key: 'created_at', label: 'Created', sortable: true, width: 160 },
];

export const GROUPED_COLUMNS = [
  { key: 'law_id', label: 'Law ID', sortable: true, width: 200 },
  { key: 'total_jobs', label: 'Jobs', sortable: true, filter: { key: 'status', options: JOB_STATUSES, label: 'Status' }, width: 60 },
  { key: 'pending', label: 'Pending', sortable: false, width: 80 },
  { key: 'processing', label: 'Processing', sortable: false, width: 90 },
  { key: 'completed', label: 'Completed', sortable: false, width: 90 },
  { key: 'failed', label: 'Failed', sortable: false, width: 70 },
  { key: 'latest_created_at', label: 'Latest', sortable: true, filter: { key: 'job_type', options: JOB_TYPES, label: 'Type' }, width: 160 },
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
