import { formatCoverageScore, formatDate, jobSubtitle } from './formatters.js';

export const LAW_STATUSES = [
  'unknown', 'queued', 'harvesting', 'harvested', 'harvest_failed', 'harvest_exhausted',
  'enriching', 'enriched', 'enrich_failed', 'enrich_exhausted',
  'not_harvestable',
];

export const JOB_STATUSES = ['pending', 'processing', 'completed', 'failed'];

// Mirrors the `job_type` database enum (packages/pipeline/src/models.rs).
// The dashboard no longer reads this list; it takes the buckets the API returns.
export const JOB_TYPES = [
  'harvest', 'enrich', 'document_convert', 'law_convert', 'traject_harvest',
];

// The subset whose `law_id` names a law, mirroring
// `JobType::law_id_names_a_law`. Only these belong in a filter over the
// law-grouped view: that endpoint leaves the other two out, so offering them
// there is a dropdown option that always returns nothing.
export const LAW_SCOPED_JOB_TYPES = ['harvest', 'enrich', 'traject_harvest'];

export const ENRICHABLE_STATUSES = ['harvested', 'enriched', 'enrich_failed'];
export const RE_HARVESTABLE_STATUSES = ['unknown', 'queued', 'harvest_failed', 'harvested', 'enriched', 'enrich_failed', 'not_harvestable'];

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
    filter: { key: 'job_type', options: LAW_SCOPED_JOB_TYPES, label: 'Type' },
    text: (g) => g.law_id,
    supportingText: (g) =>
      g.latest_created_at ? `Updated at ${formatDate(g.latest_created_at)}` : undefined,
  },
  { key: 'status_bar', label: 'Status', sortable: false, width: '80px' },
  { key: 'total_jobs', label: 'Jobs', sortable: true, filter: { key: 'status', options: JOB_STATUSES, label: 'Status' }, width: 'fit-content', minWidth: '24px', align: 'right', hideBelow: '640px' },
];

// Untranslatables (RFC-012): one row per legal construct the enrichment agent
// could not express with the engine's current operation set. Atomic grain -
// grouped views (per construct / per law) are aggregation over this same list.
// Keep in sync with ENRICH_PROVIDERS in packages/pipeline/src/enrich.rs - a
// provider added there but not here still displays, only its filter option is missing.
export const UNTRANSLATABLE_PROVIDERS = ['opencode', 'claude'];

export const UNTRANSLATABLE_COLUMNS = [
  {
    key: 'law_name',
    label: 'Law',
    filter: { key: 'law_id', type: 'text', label: 'Law' },
    overline: (row) => row.law_id,
    text: (row) => row.law_name || '—',
    supportingText: (row) =>
      row.created_at ? `Captured at ${formatDate(row.created_at)}` : undefined,
  },
  {
    key: 'article',
    label: 'Article',
    width: 'fit-content',
    minWidth: '60px',
    text: (row) => row.article || '—',
  },
  {
    key: 'construct',
    label: 'Construct',
    filter: { key: 'construct', type: 'text', label: 'Construct' },
    text: (row) => row.construct || '—',
  },
  {
    // Free-text, potentially long: the cell truncates natively; full text lives
    // in the detail panel.
    key: 'reason',
    label: 'Reason',
    hideBelow: '640px',
    text: (row) => row.reason || '—',
  },
  {
    key: 'accepted',
    label: 'Accepted',
    filter: { options: ['true', 'false'] },
    width: 120,
  },
  {
    key: 'provider',
    label: 'Provider',
    filter: { options: UNTRANSLATABLE_PROVIDERS },
    width: 'fit-content',
    minWidth: '80px',
    hideBelow: '640px',
    text: (row) => row.provider || '—',
  },
];

// Markings (schema v0.7.0): one row per construct the format itself cannot
// express. The successor of untranslatables; both are shown because a law
// pinned to schema v0.5.x still carries the old field.
//
// Atomic grain, like the untranslatable list: the cluster view is an
// aggregation over these same rows, not a separate dataset.
export const MARKING_PROVIDERS = UNTRANSLATABLE_PROVIDERS;

// `resolution` is a closed vocabulary in the schema, so it filters as a
// dropdown rather than the free-text search the other fields use.
export const MARKING_RESOLUTIONS = ['operation', 'model'];

export const MARKING_COLUMNS = [
  {
    key: 'law_name',
    label: 'Wet',
    filter: { key: 'law_id', type: 'text', label: 'Wet' },
    overline: (row) => row.law_id,
    text: (row) => row.law_name || '—',
    supportingText: (row) =>
      row.created_at ? `Gevonden op ${formatDate(row.created_at)}` : undefined,
  },
  {
    key: 'article',
    label: 'Artikel',
    width: 'fit-content',
    minWidth: '60px',
    text: (row) => row.article || '—',
  },
  {
    key: 'about',
    label: 'Constructie',
    filter: { key: 'about', type: 'text', label: 'Constructie' },
    text: (row) => row.about || '—',
  },
  {
    // What has to change before this article can be translated in full. The
    // field the backlog is grouped by, so it earns a column even though it is
    // free text and often long; the full value sits in the detail panel.
    key: 'resolved_by',
    label: 'Wat het oplost',
    hideBelow: '640px',
    text: (row) => row.resolved_by || '—',
  },
  {
    key: 'resolution',
    label: 'Soort',
    filter: { options: MARKING_RESOLUTIONS },
    width: 'fit-content',
    minWidth: '90px',
  },
  {
    // A TEXT[] on the wire. No generic cell renderer handles an array, so the
    // view fills this through a `cell-target` slot; without one the fallback
    // would print a stringified list.
    key: 'target',
    label: 'Blokkeert',
    hideBelow: '900px',
    width: 'fit-content',
    minWidth: '100px',
  },
  {
    key: 'accepted',
    label: 'Beoordeeld',
    filter: { options: ['true', 'false'] },
    width: 120,
  },
  {
    key: 'provider',
    label: 'Provider',
    filter: { options: MARKING_PROVIDERS },
    width: 'fit-content',
    minWidth: '80px',
    hideBelow: '640px',
    text: (row) => row.provider || '—',
  },
];

// Sort menus are independent from visible columns - users should be able to
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

// Sort options are ordered by relevance - most useful pivots first.
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

// Keys must match the backend sort allowlist (ALLOWED_SORT_COLUMNS_UNTRANSLATABLE):
// all real untranslatables columns. The "Law" pivot sorts by law_id (a real
// column), not the joined law_name.
export const UNTRANSLATABLE_SORT_OPTIONS = [
  { key: 'created_at', label: 'Recent changes' },
  { key: 'law_id', label: 'Law', directionLabels: DIR_TEXT },
  { key: 'construct', label: 'Construct', directionLabels: DIR_TEXT },
  { key: 'article', label: 'Article', directionLabels: DIR_TEXT },
  { key: 'accepted', label: 'Accepted' },
  { key: 'provider', label: 'Provider', directionLabels: DIR_TEXT },
];

// Sort allowlists for server-side validation (defence in depth)
export const LAW_ENTRY_SORT_KEYS = new Set(LAW_ENTRY_SORT_OPTIONS.map((o) => o.key));
export const JOB_SORT_KEYS = new Set(JOB_SORT_OPTIONS.map((o) => o.key));
export const GROUPED_SORT_KEYS = new Set(GROUPED_SORT_OPTIONS.map((o) => o.key));
// Keys must match the backend allowlist (ALLOWED_SORT_COLUMNS_MARKING): real
// columns on `markings` only. `law_name` is joined, so it is not sortable.
export const MARKING_SORT_OPTIONS = [
  { key: 'created_at', label: 'Recent gevonden' },
  { key: 'law_id', label: 'Wet', directionLabels: DIR_TEXT },
  { key: 'about', label: 'Constructie', directionLabels: DIR_TEXT },
  { key: 'resolution', label: 'Soort', directionLabels: DIR_TEXT },
  { key: 'resolved_by', label: 'Wat het oplost', directionLabels: DIR_TEXT },
  { key: 'article', label: 'Artikel', directionLabels: DIR_TEXT },
  { key: 'accepted', label: 'Beoordeeld' },
  { key: 'provider', label: 'Provider', directionLabels: DIR_TEXT },
];

export const MARKING_SORT_KEYS = new Set(MARKING_SORT_OPTIONS.map((o) => o.key));

export const UNTRANSLATABLE_SORT_KEYS = new Set(UNTRANSLATABLE_SORT_OPTIONS.map((o) => o.key));

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
  // No consolidated text to harvest - informational, terminal, not a failure.
  not_harvestable: 'neutral',
  // Untranslatables (RFC-012): the human-review `accepted` flag rendered as a
  // badge. `accepted` is a review gate that defaults to false, so false means
  // "not yet reviewed / open" (neutral), NOT "rejected" (which would wrongly
  // paint every fresh untranslatable red).
  accepted: 'success',
  open: 'neutral',
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
