import { formatCoverageScore, formatDate, jobSubtitle } from './formatters.js';

export const LAW_STATUSES = [
  'unknown', 'queued', 'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
  'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted',
  'withdrawn', 'not_yet_in_force', 'announced',
];

export const JOB_STATUSES = ['pending', 'processing', 'completed', 'failed'];

export const JOB_TYPES = ['harvest', 'enrich'];

export const ENRICHABLE_STATUSES = ['harvested', 'enriched', 'enrich_failed'];
export const RE_HARVESTABLE_STATUSES = ['unknown', 'queued', 'harvest_failed', 'harvested', 'enriched', 'enrich_failed', 'withdrawn', 'not_yet_in_force', 'announced'];

export const LAW_ENTRY_COLUMNS = [
  {
    key: 'law_name',
    label: 'Name',
    sortable: true,
    overline: (row) => row.law_id,
    text: (row) => row.law_name || '—',
    supportingText: (row) =>
      row.updated_at ? `Updated at ${formatDate(row.updated_at)}` : undefined,
  },
  { key: 'status', label: 'Status', sortable: true, filter: { options: LAW_STATUSES }, width: 140 },
  {
    key: 'coverage_score',
    label: 'Coverage',
    sortable: true,
    width: 'fit-content',
    minWidth: '40px',
    align: 'right',
    overline: () => 'Coverage',
    text: (row) => formatCoverageScore(row.coverage_score) || '0%',
  },
  { key: '_actions', label: 'Actions', sortable: false, width: 60 },
];

export const JOB_COLUMNS = [
  {
    key: 'id',
    label: 'Job',
    sortable: true,
    filter: { key: 'law_id', type: 'text', label: 'Law ID' },
    overline: (row) => `${row.law_id} › ${row.id}`,
    // Every row has the same shape: subtitle as the main text; a failed
    // job's error message goes underneath as supporting text.
    text: (row) => jobSubtitle(row),
    supportingText: (row) => row.result?.error || undefined,
  },
  { key: 'status', label: 'Status', sortable: true, filter: { options: JOB_STATUSES }, width: 110 },
  {
    key: 'priority',
    label: 'Priority',
    sortable: true,
    width: 'fit-content',
    minWidth: '40px',
    align: 'right',
    hideBelow: '640px',
    text: (row) => `Prio ${row.priority ?? '—'}`,
  },
];

export const GROUPED_COLUMNS = [
  {
    key: 'law_id',
    label: 'Law ID',
    sortable: true,
    filter: { key: 'job_type', options: JOB_TYPES, label: 'Type' },
    text: (g) => g.law_id,
    supportingText: (g) =>
      g.latest_created_at ? `Updated at ${formatDate(g.latest_created_at)}` : undefined,
  },
  { key: 'status_bar', label: 'Status', sortable: false, width: '80px' },
  { key: 'total_jobs', label: 'Jobs', sortable: true, filter: { key: 'status', options: JOB_STATUSES, label: 'Status' }, width: 'fit-content', minWidth: '24px', align: 'right', hideBelow: '640px' },
];

// Sort menus are independent from visible columns — users should be able to
// sort by fields that aren't shown as a separate column.
// `directionLabels` controls which directions appear in the menu:
//  - both keys → menu has both ascending + descending items
//  - one key   → only that direction is offered
//  - omitted   → single item, no direction shown (backend decides order)
// Within a single field, the first listed direction is the more useful one
// (most-recent / highest / a-z first) so the menu reads top-down by relevance.
const DIR_DATE    = { desc: 'new - old', asc: 'old - new' };
const DIR_NUMERIC = { desc: 'high - low', asc: 'low - high' };
const DIR_TEXT    = { asc: 'a - z', desc: 'z - a' };

// Sort options are ordered by relevance — most useful pivots first.
export const JOB_SORT_OPTIONS = [
  { key: 'created_at', label: 'Recent changes' },
  { key: 'status', label: 'Status' },
  { key: 'priority', label: 'Priority', directionLabels: DIR_NUMERIC },
  { key: 'attempts', label: 'Attempts', directionLabels: DIR_NUMERIC },
  { key: 'law_id', label: 'Law ID', directionLabels: DIR_TEXT },
  { key: 'job_type', label: 'Type', directionLabels: DIR_TEXT },
  { key: 'id', label: 'Job ID', directionLabels: DIR_TEXT },
];

export const GROUPED_SORT_OPTIONS = [
  { key: 'latest_created_at', label: 'Recent changes' },
  { key: 'status', label: 'Status' },
  { key: 'total_jobs', label: 'Jobs', directionLabels: DIR_NUMERIC },
  { key: 'law_id', label: 'Law ID', directionLabels: DIR_TEXT },
];

export const LAW_ENTRY_SORT_OPTIONS = [
  { key: 'updated_at', label: 'Recent changes' },
  { key: 'status', label: 'Status' },
  { key: 'coverage_score', label: 'Coverage', directionLabels: DIR_NUMERIC },
  { key: 'law_name', label: 'Name', directionLabels: DIR_TEXT },
];

// Sort allowlists for server-side validation (defence in depth)
export const LAW_ENTRY_SORT_KEYS = new Set(LAW_ENTRY_SORT_OPTIONS.map((o) => o.key));
export const JOB_SORT_KEYS = new Set(JOB_SORT_OPTIONS.map((o) => o.key));
export const GROUPED_SORT_KEYS = new Set(GROUPED_SORT_OPTIONS.map((o) => o.key));

export const STATUS_BADGE_MAP = {
  completed: 'success',
  harvested: 'success',
  enriched: 'success',
  failed: 'critical',
  harvest_failed: 'critical',
  enrich_failed: 'critical',
  harvest_exhausted: 'critical',
  enrich_exhausted: 'critical',
  processing: 'accent',
  harvesting: 'accent',
  enriching: 'accent',
  pending: 'neutral',
  unknown: 'neutral',
  queued: 'neutral',
  // No consolidated text to harvest — informational, terminal, not a failure.
  withdrawn: 'neutral',
  not_yet_in_force: 'neutral',
  announced: 'neutral',
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
