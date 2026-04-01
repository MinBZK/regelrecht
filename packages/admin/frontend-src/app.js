/**
 * RegelRecht Admin - Application Logic
 *
 * Handles tab switching, data fetching, table rendering with sorting,
 * filtering, and pagination for the admin database viewer.
 */

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LAW_STATUSES = [
  'unknown', 'queued', 'harvesting', 'harvested', 'harvest_failed',
  'enriching', 'enriched', 'enrich_failed',
];

const JOB_STATUSES = ['pending', 'processing', 'completed', 'failed'];

const JOB_TYPES = ['harvest', 'enrich'];

const ENRICHABLE_STATUSES = ['harvested', 'enriched', 'enrich_failed'];
const RE_HARVESTABLE_STATUSES = ['unknown', 'queued', 'harvest_failed', 'harvested', 'enriched', 'enrich_failed'];

const TAB_CONFIG = {
  law_entries: {
    label: 'Law Entries',
    endpoint: 'api/law_entries',
    columns: [
      { key: 'law_id', label: 'Law ID', sortable: true },
      { key: 'law_name', label: 'Name', sortable: true },
      { key: 'status', label: 'Status', sortable: true, filter: { options: LAW_STATUSES } },
      { key: 'coverage_score', label: 'Coverage', sortable: true },
      { key: 'updated_at', label: 'Updated', sortable: true },
      { key: '_actions', label: 'Actions', sortable: false },
    ],
    defaultSort: 'updated_at',
  },
  jobs: {
    label: 'Jobs',
    endpoint: 'api/jobs',
    columns: [
      { key: 'id', label: 'ID', sortable: true },
      { key: 'job_type', label: 'Type', sortable: true, filter: { options: JOB_TYPES } },
      { key: 'law_id', label: 'Law ID', sortable: true, filter: { type: 'text' } },
      { key: 'status', label: 'Status', sortable: true, filter: { options: JOB_STATUSES } },
      { key: '_error', label: 'Error', sortable: false },
      { key: 'priority', label: 'Priority', sortable: true },
      { key: 'attempts', label: 'Attempts', sortable: true },
      { key: 'created_at', label: 'Created', sortable: true },
    ],
    defaultSort: 'created_at',
  },
};

const GROUPED_COLUMNS = [
  { key: 'law_id', label: 'Law ID', sortable: true },
  { key: 'total_jobs', label: 'Jobs', sortable: true },
  { key: 'pending', label: 'Pending', sortable: false },
  { key: 'processing', label: 'Processing', sortable: false },
  { key: 'completed', label: 'Completed', sortable: false },
  { key: 'failed', label: 'Failed', sortable: false },
  { key: 'latest_created_at', label: 'Latest', sortable: true },
];

const STATUS_BADGE_MAP = {
  // Green
  completed: 'green',
  harvested: 'green',
  enriched: 'green',
  // Red
  failed: 'red',
  harvest_failed: 'red',
  enrich_failed: 'red',
  // Yellow
  processing: 'yellow',
  harvesting: 'yellow',
  enriching: 'yellow',
  // Grey
  pending: 'grey',
  unknown: 'grey',
  queued: 'grey',
};

const DATE_FORMATTER = new Intl.DateTimeFormat('nl-NL', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
});


// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const state = {
  activeTab: 'law_entries',
  sort: 'updated_at',
  order: 'desc',
  limit: 50,
  offset: 0,
  filters: {},
  totalCount: 0,
  data: [],
  loading: false,
  error: null,
  // Jobs grouped view
  viewMode: 'grouped', // 'flat' or 'grouped' (only for jobs tab)
  expandedLawIds: new Set(),
  expandedJobsCache: {}, // { [law_id]: Job[] }
  jobCreationOpen: true,
};


// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------

function $(selector, parent = document) {
  return parent.querySelector(selector);
}

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}


// ---------------------------------------------------------------------------
// Cell formatters
// ---------------------------------------------------------------------------

function formatCell(value, key) {
  if (value === null || value === undefined || value === '') {
    return '<span class="cell-null">\u2014</span>';
  }

  // Status badge
  if (key === 'status') {
    const variant = STATUS_BADGE_MAP[value] || 'grey';
    return `<span class="badge badge--${variant}">${escapeHtml(value)}</span>`;
  }

  // UUID: truncate to first 8 chars
  if (key === 'id') {
    const str = String(value);
    if (str.length > 8) {
      return `<span class="cell-mono" title="${escapeHtml(str)}">${escapeHtml(str.substring(0, 8))}</span>`;
    }
    return `<span class="cell-mono">${escapeHtml(str)}</span>`;
  }

  // Quality score as percentage
  if (key === 'coverage_score') {
    const num = Number(value);
    if (Number.isFinite(num)) {
      return `${Math.round(num * 100)}%`;
    }
    return escapeHtml(String(value));
  }

  // Dates
  if (key.endsWith('_at')) {
    const date = new Date(value);
    if (!isNaN(date.getTime())) {
      return escapeHtml(DATE_FORMATTER.format(date));
    }
    return escapeHtml(String(value));
  }

  // law_id in monospace
  if (key === 'law_id') {
    return `<span class="cell-mono">${escapeHtml(String(value))}</span>`;
  }

  return escapeHtml(String(value));
}

// Format a count with a status badge (for grouped view)
function formatCount(count, statusKey) {
  if (count === 0) return '<span class="cell-null">0</span>';
  const variant = STATUS_BADGE_MAP[statusKey] || 'grey';
  return `<span class="badge badge--${variant}">${count}</span>`;
}


// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function renderTabs() {
  const tabsEl = $('#tabs');
  tabsEl.innerHTML = '';

  for (const tabKey of Object.keys(TAB_CONFIG)) {
    const item = document.createElement('rr-tab-bar-item');
    item.textContent = TAB_CONFIG[tabKey].label;
    if (tabKey === state.activeTab) {
      item.setAttribute('selected', '');
    }
    item.addEventListener('click', () => switchTab(tabKey));
    tabsEl.appendChild(item);
  }
}

function renderViewToggle() {
  const container = $('#view-toggle-container');
  container.innerHTML = '';

  if (state.activeTab !== 'jobs') return;

  const btn = document.createElement('rr-button');
  btn.setAttribute('variant', 'neutral-tinted');
  btn.setAttribute('size', 'md');
  btn.textContent = state.viewMode === 'grouped' ? 'Flat view' : 'Grouped view';
  btn.title = state.viewMode === 'grouped'
    ? 'Show individual jobs'
    : 'Group jobs by law';
  btn.addEventListener('click', () => {
    state.viewMode = state.viewMode === 'grouped' ? 'flat' : 'grouped';
    state.offset = 0;
    state.sort = state.viewMode === 'grouped' ? 'latest_created_at' : TAB_CONFIG.jobs.defaultSort;
    state.order = 'desc';
    state.expandedLawIds.clear();
    state.expandedJobsCache = {};
    state.filters = {};
    renderViewToggle();
    renderTableHead();
    loadData();
  });

  container.appendChild(btn);
}

function renderJobCreation() {
  const section = $('#job-creation');
  const body = $('#job-creation-body');
  const toggle = $('#job-creation-toggle');

  section.hidden = false;

  if (state.jobCreationOpen) {
    body.style.display = '';
    toggle.querySelector('rr-icon')?.setAttribute('name', 'chevron-up');
  } else {
    body.style.display = 'none';
    toggle.querySelector('rr-icon')?.setAttribute('name', 'chevron-down');
  }
}

function getActiveColumns() {
  if (state.activeTab === 'jobs' && state.viewMode === 'grouped') {
    return GROUPED_COLUMNS;
  }
  return TAB_CONFIG[state.activeTab].columns;
}

function renderTableHead() {
  const thead = $('#table-head');
  thead.innerHTML = '';

  const columns = getActiveColumns();

  // Label row
  const tr = document.createElement('tr');
  for (const col of columns) {
    const th = document.createElement('th');

    const labelSpan = document.createElement('span');
    labelSpan.className = 'th-label';
    labelSpan.textContent = col.label;

    if (col.sortable) {
      th.classList.add('sortable');
      if (state.sort === col.key) {
        th.classList.add('sort-active');
      }

      const indicator = document.createElement('span');
      indicator.className = 'sort-indicator';
      indicator.textContent = state.sort === col.key
        ? (state.order === 'asc' ? '\u25B2' : '\u25BC')
        : '\u25BC';
      labelSpan.appendChild(indicator);

      th.addEventListener('click', (e) => {
        // Don't sort when clicking on filter controls
        if (e.target.closest('.th-filter')) return;
        onSort(col.key);
      });
    }

    th.appendChild(labelSpan);

    // Column filter (only in flat view)
    if (col.filter && !(state.activeTab === 'jobs' && state.viewMode === 'grouped')) {
      const filterDiv = document.createElement('div');
      filterDiv.className = 'th-filter';

      if (col.filter.options) {
        const select = document.createElement('select');
        select.setAttribute('aria-label', `Filter ${col.label}`);

        const defaultOpt = document.createElement('option');
        defaultOpt.value = '';
        defaultOpt.textContent = 'All';
        select.appendChild(defaultOpt);

        for (const v of col.filter.options) {
          const opt = document.createElement('option');
          opt.value = v;
          opt.textContent = v;
          select.appendChild(opt);
        }

        if (state.filters[col.key]) {
          select.value = state.filters[col.key];
        }

        select.addEventListener('click', (e) => e.stopPropagation());
        select.addEventListener('change', () => {
          onFilterChange(col.key, select.value);
        });

        filterDiv.appendChild(select);
      } else if (col.filter.type === 'text') {
        const input = document.createElement('input');
        input.type = 'text';
        input.placeholder = 'Filter\u2026';
        input.setAttribute('aria-label', `Filter ${col.label}`);

        if (state.filters[col.key]) {
          input.value = state.filters[col.key];
        }

        input.addEventListener('click', (e) => e.stopPropagation());
        let debounceTimer;
        input.addEventListener('input', () => {
          clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            onFilterChange(col.key, input.value.trim());
          }, 300);
        });

        filterDiv.appendChild(input);
      }

      th.appendChild(filterDiv);
    }

    tr.appendChild(th);
  }

  // Add expand column header for grouped view
  if (state.activeTab === 'jobs' && state.viewMode === 'grouped') {
    const th = document.createElement('th');
    th.style.width = '40px';
    tr.appendChild(th);
  }

  thead.appendChild(tr);
}

function renderTableBody() {
  if (state.activeTab === 'jobs' && state.viewMode === 'grouped') {
    renderGroupedTableBody();
    return;
  }

  const tbody = $('#table-body');
  tbody.innerHTML = '';

  const columns = getActiveColumns();

  if (state.loading) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = columns.length;
    td.className = 'table-message';
    td.textContent = 'Loading\u2026';
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  if (state.error) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = columns.length;
    td.className = 'table-message table-message--error';
    td.textContent = `Failed to load data: ${state.error}`;
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  if (state.data.length === 0) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = columns.length;
    td.className = 'table-message';
    td.textContent = 'No data found';
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  for (const row of state.data) {
    const tr = document.createElement('tr');

    // Jobs rows are clickable to open the detail panel
    if (state.activeTab === 'jobs') {
      tr.classList.add('clickable-row');
      tr.addEventListener('click', () => openDetailPanel(row));
    }

    for (const col of columns) {
      const td = document.createElement('td');
      if (col.key === '_error' && state.activeTab === 'jobs') {
        const error = row.result && row.result.error;
        if (error) {
          const span = document.createElement('span');
          span.className = 'cell-error';
          span.title = error;
          span.textContent = error.length > 80 ? error.substring(0, 80) + '\u2026' : error;
          td.appendChild(span);
        } else {
          td.innerHTML = '<span class="cell-null">\u2014</span>';
        }
      } else if (col.key === '_actions' && state.activeTab === 'law_entries') {
        td.appendChild(renderRowActions(row));
      } else if (col.key === 'law_id' && state.activeTab === 'law_entries') {
        // Clickable law_id to view jobs for this law
        const link = document.createElement('a');
        link.className = 'cell-mono law-id-link';
        link.textContent = row.law_id;
        link.title = 'View jobs for this law';
        link.href = '#';
        link.addEventListener('click', (e) => {
          e.preventDefault();
          viewJobsForLaw(row.law_id);
        });
        td.appendChild(link);
      } else {
        td.innerHTML = formatCell(row[col.key], col.key);
      }
      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
}

function renderGroupedTableBody() {
  const tbody = $('#table-body');
  tbody.innerHTML = '';

  const columns = GROUPED_COLUMNS;
  const colCount = columns.length + 1; // +1 for expand column

  if (state.loading) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = colCount;
    td.className = 'table-message';
    td.textContent = 'Loading\u2026';
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  if (state.error) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = colCount;
    td.className = 'table-message table-message--error';
    td.textContent = `Failed to load data: ${state.error}`;
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  if (state.data.length === 0) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = colCount;
    td.className = 'table-message';
    td.textContent = 'No data found';
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  for (const group of state.data) {
    const isExpanded = state.expandedLawIds.has(group.law_id);

    // Group header row
    const tr = document.createElement('tr');
    tr.className = 'group-row' + (isExpanded ? ' group-row--expanded' : '');
    tr.addEventListener('click', () => toggleGroupExpansion(group.law_id));

    for (const col of columns) {
      const td = document.createElement('td');
      if (col.key === 'law_id') {
        td.innerHTML = `<span class="cell-mono">${escapeHtml(group.law_id)}</span>`;
      } else if (['pending', 'processing', 'completed', 'failed'].includes(col.key)) {
        td.innerHTML = formatCount(group[col.key], col.key);
      } else {
        td.innerHTML = formatCell(group[col.key], col.key);
      }
      tr.appendChild(td);
    }

    // Expand/collapse indicator
    const expandTd = document.createElement('td');
    expandTd.className = 'group-row__toggle';
    expandTd.textContent = isExpanded ? '\u25B2' : '\u25BC';
    tr.appendChild(expandTd);

    tbody.appendChild(tr);

    // Expanded child rows
    if (isExpanded) {
      const jobs = state.expandedJobsCache[group.law_id];
      if (!jobs) {
        // Loading state
        const loadTr = document.createElement('tr');
        loadTr.className = 'child-row';
        const loadTd = document.createElement('td');
        loadTd.colSpan = colCount;
        loadTd.className = 'table-message';
        loadTd.textContent = 'Loading jobs\u2026';
        loadTr.appendChild(loadTd);
        tbody.appendChild(loadTr);
      } else if (jobs.length === 0) {
        const emptyTr = document.createElement('tr');
        emptyTr.className = 'child-row';
        const emptyTd = document.createElement('td');
        emptyTd.colSpan = colCount;
        emptyTd.className = 'table-message';
        emptyTd.textContent = 'No jobs found';
        emptyTr.appendChild(emptyTd);
        tbody.appendChild(emptyTr);
      } else {
        // Child header row
        const childHeaderTr = document.createElement('tr');
        childHeaderTr.className = 'child-header';
        const childCols = TAB_CONFIG.jobs.columns;
        for (const col of childCols) {
          const th = document.createElement('td');
          th.className = 'child-header__cell';
          th.textContent = col.label;
          childHeaderTr.appendChild(th);
        }
        tbody.appendChild(childHeaderTr);

        for (const job of jobs) {
          const jobTr = document.createElement('tr');
          jobTr.className = 'child-row clickable-row';
          jobTr.addEventListener('click', (e) => {
            e.stopPropagation();
            openDetailPanel(job);
          });

          for (const col of childCols) {
            const td = document.createElement('td');
            if (col.key === '_error') {
              const error = job.result && job.result.error;
              if (error) {
                const span = document.createElement('span');
                span.className = 'cell-error';
                span.title = error;
                span.textContent = error.length > 80 ? error.substring(0, 80) + '\u2026' : error;
                td.appendChild(span);
              } else {
                td.innerHTML = '<span class="cell-null">\u2014</span>';
              }
            } else {
              td.innerHTML = formatCell(job[col.key], col.key);
            }
            jobTr.appendChild(td);
          }
          tbody.appendChild(jobTr);
        }
      }
    }
  }
}

function renderPagination() {
  const container = $('#pagination-container');
  container.innerHTML = '';

  const totalPages = Math.max(1, Math.ceil(state.totalCount / state.limit));
  const currentPage = Math.floor(state.offset / state.limit) + 1;

  const prevBtn = document.createElement('rr-button');
  prevBtn.setAttribute('variant', 'neutral-tinted');
  prevBtn.setAttribute('size', 'md');
  prevBtn.textContent = '\u2039';
  prevBtn.title = 'Previous page';
  if (currentPage <= 1) prevBtn.setAttribute('disabled', '');
  prevBtn.addEventListener('click', onPrevPage);

  const info = document.createElement('span');
  info.className = 'pagination-info';
  const unit = (state.activeTab === 'jobs' && state.viewMode === 'grouped') ? 'laws' : 'results';
  info.textContent = `${currentPage} / ${totalPages} (${state.totalCount} ${unit})`;

  const nextBtn = document.createElement('rr-button');
  nextBtn.setAttribute('variant', 'neutral-tinted');
  nextBtn.setAttribute('size', 'md');
  nextBtn.textContent = '\u203A';
  nextBtn.title = 'Next page';
  if (currentPage >= totalPages) nextBtn.setAttribute('disabled', '');
  nextBtn.addEventListener('click', onNextPage);

  container.appendChild(prevBtn);
  container.appendChild(info);
  container.appendChild(nextBtn);
}

function renderRowActions(row) {
  const container = document.createElement('span');
  container.className = 'action-btns';

  // Re-harvest: available for most statuses (not while actively processing)
  if (RE_HARVESTABLE_STATUSES.includes(row.status)) {
    const harvestBtn = document.createElement('rr-button');
    harvestBtn.setAttribute('variant', 'accent-outlined');
    harvestBtn.setAttribute('size', 'sm');
    harvestBtn.textContent = 'Harvest';
    harvestBtn.title = `Re-harvest ${row.law_id}`;
    harvestBtn.addEventListener('click', () => onRowHarvestClick(row.law_id, harvestBtn));
    container.appendChild(harvestBtn);
  }

  // Enrich: available after harvest completes
  if (ENRICHABLE_STATUSES.includes(row.status)) {
    const enrichBtn = document.createElement('rr-button');
    enrichBtn.setAttribute('variant', 'neutral-tinted');
    enrichBtn.setAttribute('size', 'sm');
    enrichBtn.textContent = 'Enrich';
    enrichBtn.title = `Trigger enrichment for ${row.law_id}`;
    enrichBtn.addEventListener('click', () => onEnrichClick(row.law_id, enrichBtn));
    container.appendChild(enrichBtn);
  }

  return container;
}

function renderAll() {
  renderViewToggle();
  renderJobCreation();
  renderTableHead();
  renderTableBody();
  renderPagination();
}


// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

async function checkAuth() {
  try {
    const response = await fetch('/auth/status');
    if (!response.ok) return { authenticated: false, oidc_configured: false };
    return await response.json();
  } catch {
    return { authenticated: false, oidc_configured: false };
  }
}

function setupLogout() {
  const nav = $('rr-top-navigation-bar');
  if (!nav) return;
  nav.addEventListener('account-click', (e) => {
    e.preventDefault();
    window.location.href = '/auth/logout';
  });
}

// ---------------------------------------------------------------------------
// Data fetching
// ---------------------------------------------------------------------------

async function loadData() {
  if (state.activeTab === 'jobs' && state.viewMode === 'grouped') {
    await fetchGroupedData();
  } else {
    await fetchData();
  }
}

async function fetchData() {
  const config = TAB_CONFIG[state.activeTab];

  const params = new URLSearchParams();
  params.set('sort', state.sort);
  params.set('order', state.order);
  params.set('limit', String(state.limit));
  params.set('offset', String(state.offset));

  for (const [key, value] of Object.entries(state.filters)) {
    if (value) {
      params.set(key, value);
    }
  }

  const url = `${config.endpoint}?${params.toString()}`;

  state.loading = true;
  state.error = null;
  renderTableBody();

  try {
    const response = await fetch(url);

    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }

    if (!response.ok) {
      const body = await response.text().catch(() => '');
      throw new Error(`HTTP ${response.status} from ${url}${body ? ': ' + body.substring(0, 200) : ''}`);
    }

    const json = await response.json();

    state.data = json.data || [];
    state.totalCount = json.total ?? state.data.length;
    state.error = null;
  } catch (err) {
    console.error('Failed to fetch data:', err);
    state.data = [];
    state.totalCount = 0;
    state.error = err.message;
  } finally {
    state.loading = false;
    renderTableBody();
    renderPagination();
  }
}

async function fetchGroupedData() {
  const params = new URLSearchParams();
  params.set('sort', state.sort);
  params.set('order', state.order);
  params.set('limit', String(state.limit));
  params.set('offset', String(state.offset));

  for (const [key, value] of Object.entries(state.filters)) {
    if (value) {
      params.set(key, value);
    }
  }

  const url = `api/jobs/summary?${params.toString()}`;

  state.loading = true;
  state.error = null;
  renderTableBody();

  try {
    const response = await fetch(url);

    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }

    if (!response.ok) {
      const body = await response.text().catch(() => '');
      throw new Error(`HTTP ${response.status} from ${url}${body ? ': ' + body.substring(0, 200) : ''}`);
    }

    const json = await response.json();

    state.data = json.data || [];
    state.totalCount = json.total ?? state.data.length;
    state.error = null;
  } catch (err) {
    console.error('Failed to fetch grouped data:', err);
    state.data = [];
    state.totalCount = 0;
    state.error = err.message;
  } finally {
    state.loading = false;
    renderTableBody();
    renderPagination();
  }

  // Re-fetch expanded groups
  const expandedIds = [...state.expandedLawIds];
  if (expandedIds.length > 0) {
    await Promise.all(expandedIds.map((lawId) => fetchJobsForLaw(lawId)));
    renderTableBody();
  }
}

async function fetchJobsForLaw(lawId) {
  try {
    const params = new URLSearchParams();
    params.set('law_id', lawId);
    params.set('sort', 'created_at');
    params.set('order', 'desc');
    params.set('limit', '50');

    // Pass through status/job_type filters
    if (state.filters.status) params.set('status', state.filters.status);
    if (state.filters.job_type) params.set('job_type', state.filters.job_type);

    const response = await fetch(`api/jobs?${params.toString()}`);

    if (!response.ok) {
      state.expandedJobsCache[lawId] = [];
      return;
    }

    const json = await response.json();
    state.expandedJobsCache[lawId] = json.data || [];
  } catch {
    state.expandedJobsCache[lawId] = [];
  }
}


// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

function switchTab(tabKey) {
  if (tabKey === state.activeTab) return;

  state.activeTab = tabKey;
  state.sort = tabKey === 'jobs' && state.viewMode === 'grouped'
    ? 'latest_created_at'
    : TAB_CONFIG[tabKey].defaultSort;
  state.order = 'desc';
  state.offset = 0;
  state.filters = {};
  state.data = [];
  state.totalCount = 0;
  state.error = null;
  state.expandedLawIds.clear();
  state.expandedJobsCache = {};

  renderTabs();
  renderAll();
  loadData();
}

// Web component .value may not always reflect the inner <input> state;
// fall back to shadow DOM as a workaround for rr-text-field quirks.
function getFieldValue(el) {
  if (el.value != null && el.value !== '') return el.value;
  const inner = el.shadowRoot?.querySelector('input');
  return inner?.value ?? '';
}

function setFieldValue(el, val) {
  el.value = val;
  const inner = el.shadowRoot?.querySelector('input');
  if (inner) inner.value = val;
}

async function onHarvestSubmit() {
  const input = $('#harvest-bwb-id');
  const btn = $('#harvest-btn');
  if (btn.hasAttribute('disabled')) return;
  const bwbId = getFieldValue(input).trim();
  if (!bwbId) return;
  if (!/^BWBR\d{7}$/.test(bwbId)) {
    alert('BWB ID format: BWBR followed by 7 digits (e.g. BWBR0018451)');
    return;
  }

  btn.setAttribute('disabled', '');
  btn.textContent = 'Submitting\u2026';

  try {
    const response = await fetch('api/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: bwbId }),
    });
    if (response.status === 401) {
      btn.removeAttribute('disabled');
      btn.textContent = 'Harvest';
      window.location.href = '/auth/login';
      return;
    }
    if (response.status === 409) {
      alert('A harvest job for this law is already pending or processing.');
      btn.removeAttribute('disabled');
      btn.textContent = 'Harvest';
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    await response.json();
    setFieldValue(input, '');
    btn.textContent = 'Queued \u2713';
    btn.removeAttribute('disabled');
    setTimeout(() => { btn.textContent = 'Harvest'; }, 2000);
    loadData();
  } catch (err) {
    alert('Harvest failed: ' + err.message);
    btn.removeAttribute('disabled');
    btn.textContent = 'Harvest';
  }
}

async function onRowHarvestClick(lawId, btn) {
  btn.setAttribute('disabled', '');
  btn.textContent = 'Submitting\u2026';

  try {
    const response = await fetch('api/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: lawId }),
    });
    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }
    if (response.status === 409) {
      alert('A harvest job for this law is already pending or processing.');
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    const result = await response.json();
    alert(`Created harvest job: ${result.job_id}`);
    loadData();
  } catch (err) {
    alert('Harvest failed: ' + err.message);
  } finally {
    btn.removeAttribute('disabled');
    btn.textContent = 'Harvest';
  }
}

async function onEnrichClick(lawId, btn) {
  btn.setAttribute('disabled', '');
  btn.textContent = 'Submitting\u2026';

  try {
    const response = await fetch('api/enrich-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ law_id: lawId }),
    });
    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }
    if (response.status === 409) {
      alert('Enrich jobs for this law are already pending or processing.');
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    const result = await response.json();
    alert(`Created ${result.job_ids.length} enrich job(s) for ${result.providers.join(', ')}`);
    loadData();
  } catch (err) {
    alert('Enrich failed: ' + err.message);
  } finally {
    btn.removeAttribute('disabled');
    btn.textContent = 'Enrich';
  }
}

function viewJobsForLaw(lawId) {
  state.activeTab = 'jobs';
  state.sort = state.viewMode === 'grouped' ? 'latest_created_at' : TAB_CONFIG.jobs.defaultSort;
  state.order = 'desc';
  state.offset = 0;
  state.data = [];
  state.totalCount = 0;
  state.error = null;

  if (state.viewMode === 'grouped') {
    // In grouped view, clear filters and auto-expand this law
    state.filters = {};
    state.expandedLawIds.clear();
    state.expandedLawIds.add(lawId);
    state.expandedJobsCache = {};
  } else {
    state.filters = { law_id: lawId };
  }

  renderTabs();
  renderAll();
  loadData();
}

async function toggleGroupExpansion(lawId) {
  if (state.expandedLawIds.has(lawId)) {
    state.expandedLawIds.delete(lawId);
    delete state.expandedJobsCache[lawId];
    renderTableBody();
  } else {
    state.expandedLawIds.add(lawId);
    renderTableBody(); // Show loading state
    await fetchJobsForLaw(lawId);
    renderTableBody(); // Show loaded jobs
  }
}

function onSort(key) {
  if (state.sort === key) {
    state.order = state.order === 'asc' ? 'desc' : 'asc';
  } else {
    state.sort = key;
    state.order = 'asc';
  }
  state.offset = 0;
  renderTableHead();
  loadData();
}

function onFilterChange(key, value) {
  if (value) {
    state.filters[key] = value;
  } else {
    delete state.filters[key];
  }
  state.offset = 0;
  loadData();
}

function onPrevPage() {
  if (state.offset > 0) {
    state.offset = Math.max(0, state.offset - state.limit);
    loadData();
  }
}

function onNextPage() {
  const totalPages = Math.ceil(state.totalCount / state.limit);
  const currentPage = Math.floor(state.offset / state.limit) + 1;
  if (currentPage < totalPages) {
    state.offset += state.limit;
    loadData();
  }
}


// ---------------------------------------------------------------------------
// Detail Panel
// ---------------------------------------------------------------------------

let _closePanelTransitionCleanup = null;
let _progressPollInterval = null;

const PHASE_LABELS = {
  mvt_research: 'MvT Research',
  generating: 'Generating',
  validating: 'Validating',
  reverse_validating: 'Reverse Validating',
};

function openDetailPanel(job) {
  const panel = $('#detail-panel');
  const backdrop = $('#detail-backdrop');
  const body = $('#detail-body');

  // Always cancel any in-flight progress poll from a previous panel.
  if (_progressPollInterval) {
    clearInterval(_progressPollInterval);
    _progressPollInterval = null;
  }

  // Cancel any pending close transition
  if (_closePanelTransitionCleanup) {
    _closePanelTransitionCleanup();
    _closePanelTransitionCleanup = null;
  }

  body.innerHTML = '';

  // --- Status & Info section ---
  const infoSection = document.createElement('div');
  infoSection.className = 'detail-section';
  infoSection.innerHTML = `<h3 class="detail-section__title">Info</h3>`;

  const infoGrid = document.createElement('dl');
  infoGrid.className = 'detail-grid';

  const fields = [
    ['ID', job.id],
    ['Type', job.job_type],
    ['Law ID', job.law_id],
    ['Status', job.status],
    ['Priority', job.priority],
    ['Attempts', `${job.attempts} / ${job.max_attempts}`],
    ['Created', job.created_at ? DATE_FORMATTER.format(new Date(job.created_at)) : null],
    ['Started', job.started_at ? DATE_FORMATTER.format(new Date(job.started_at)) : null],
    ['Completed', job.completed_at ? DATE_FORMATTER.format(new Date(job.completed_at)) : null],
  ];

  for (const [label, value] of fields) {
    if (value === null || value === undefined) continue;
    const dt = document.createElement('dt');
    dt.textContent = label;
    const dd = document.createElement('dd');
    if (label === 'Status') {
      const variant = STATUS_BADGE_MAP[value] || 'grey';
      dd.innerHTML = `<span class="badge badge--${variant}">${escapeHtml(value)}</span>`;
    } else {
      dd.textContent = value;
    }
    infoGrid.appendChild(dt);
    infoGrid.appendChild(dd);
  }

  infoSection.appendChild(infoGrid);
  body.appendChild(infoSection);

  // --- Progress section (for processing jobs) ---
  if (job.status === 'processing') {
    const progressContainer = document.createElement('div');
    progressContainer.id = 'detail-progress';
    renderProgressSection(progressContainer, job.progress);
    body.appendChild(progressContainer);

    // Start auto-refresh to poll progress updates
    _progressPollInterval = setInterval(async () => {
      try {
        const resp = await fetch(`api/jobs/${encodeURIComponent(job.id)}`);
        if (!resp.ok) return;
        const updated = await resp.json();
        const container = document.getElementById('detail-progress');
        if (container) renderProgressSection(container, updated.progress);
        // If job is no longer processing, stop polling and refresh detail
        if (updated.status !== 'processing') {
          clearInterval(_progressPollInterval);
          _progressPollInterval = null;
          openDetailPanel(updated);
        }
      } catch {
        // ignore fetch errors during polling
      }
    }, 10_000);
  }

  // --- Error section (only for failed jobs) ---
  if (job.status === 'failed' && job.result && job.result.error) {
    const errorSection = document.createElement('div');
    errorSection.className = 'detail-section';
    errorSection.innerHTML = `<h3 class="detail-section__title">Error</h3>`;

    const errorBlock = document.createElement('div');
    errorBlock.className = 'detail-error';
    errorBlock.textContent = job.result.error;

    errorSection.appendChild(errorBlock);
    body.appendChild(errorSection);
  }

  // --- Result section (for completed jobs) ---
  if (job.status === 'completed' && job.result) {
    const resultSection = document.createElement('div');
    resultSection.className = 'detail-section';
    resultSection.innerHTML = `<h3 class="detail-section__title">Result</h3>`;

    const resultBlock = document.createElement('div');
    resultBlock.className = 'detail-json';
    resultBlock.textContent = JSON.stringify(job.result, null, 2);

    resultSection.appendChild(resultBlock);
    body.appendChild(resultSection);
  }

  // --- Payload section ---
  if (job.payload) {
    const payloadSection = document.createElement('div');
    payloadSection.className = 'detail-section';
    payloadSection.innerHTML = `<h3 class="detail-section__title">Payload</h3>`;

    const payloadBlock = document.createElement('div');
    payloadBlock.className = 'detail-json';
    payloadBlock.textContent = JSON.stringify(job.payload, null, 2);

    payloadSection.appendChild(payloadBlock);
    body.appendChild(payloadSection);
  }

  // Show panel with animation
  panel.hidden = false;
  backdrop.hidden = false;
  // Force reflow before adding class for CSS transition
  panel.offsetHeight;
  panel.classList.add('is-open');
  backdrop.classList.add('is-open');
}

function renderProgressSection(container, progress) {
  container.innerHTML = '';

  const section = document.createElement('div');
  section.className = 'detail-section';
  section.innerHTML = '<h3 class="detail-section__title">Progress</h3>';

  if (!progress || !progress.phase) {
    const msg = document.createElement('div');
    msg.className = 'detail-phase';
    msg.innerHTML = '<span class="detail-phase__label">Processing\u2026</span>';
    section.appendChild(msg);
    container.appendChild(section);
    return;
  }

  const phaseEl = document.createElement('div');
  phaseEl.className = 'detail-phase';

  const totalSteps = progress.total_steps || 3;
  const currentStep = progress.step || 1;
  const phaseLabel = PHASE_LABELS[progress.phase] || progress.phase;

  // Step indicator dots
  const dotsEl = document.createElement('span');
  dotsEl.className = 'detail-phase__steps';
  for (let i = 1; i <= totalSteps; i++) {
    const dot = document.createElement('span');
    dot.className = 'detail-phase__dot' + (i <= currentStep ? ' detail-phase__dot--active' : '');
    dotsEl.appendChild(dot);
  }

  const labelEl = document.createElement('span');
  labelEl.className = 'detail-phase__label';
  labelEl.textContent = `Step ${currentStep} / ${totalSteps}: ${phaseLabel}`;

  phaseEl.appendChild(dotsEl);
  phaseEl.appendChild(labelEl);
  section.appendChild(phaseEl);

  // Extra details
  const details = [];
  if (progress.article_count) details.push(`${progress.article_count} articles`);
  if (progress.iteration) details.push(`iteration ${progress.iteration}`);
  if (details.length > 0) {
    const detailsEl = document.createElement('div');
    detailsEl.className = 'detail-phase__meta';
    detailsEl.textContent = details.join(' \u00B7 ');
    section.appendChild(detailsEl);
  }

  container.appendChild(section);
}

function closeDetailPanel() {
  if (_progressPollInterval) {
    clearInterval(_progressPollInterval);
    _progressPollInterval = null;
  }

  const panel = $('#detail-panel');
  const backdrop = $('#detail-backdrop');

  if (!panel.classList.contains('is-open')) return;

  panel.classList.remove('is-open');
  backdrop.classList.remove('is-open');

  // Hide after transition completes
  function hide() {
    panel.removeEventListener('transitionend', hide);
    _closePanelTransitionCleanup = null;
    panel.hidden = true;
    backdrop.hidden = true;
  }
  panel.addEventListener('transitionend', hide, { once: true });

  // Store cleanup so openDetailPanel can cancel a pending close
  _closePanelTransitionCleanup = () => {
    panel.removeEventListener('transitionend', hide);
  };
}


// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

async function fetchPlatformInfo() {
  try {
    const response = await fetch('/api/info');
    if (!response.ok) return null;
    return await response.json();
  } catch {
    return null;
  }
}

function showDeploymentBadge(info) {
  if (!info || !info.deployment_name || info.deployment_name === 'regelrecht') return;
  const nav = $('rr-top-navigation-bar');
  if (!nav) return;
  const badge = document.createElement('span');
  badge.className = 'env-badge';
  badge.textContent = info.deployment_name;
  nav.after(badge);
}

async function init() {
  const authStatus = await checkAuth();

  if (authStatus.oidc_configured && !authStatus.authenticated) {
    window.location.href = '/auth/login';
    return;
  }

  if (authStatus.authenticated && authStatus.person) {
    const nav = $('rr-top-navigation-bar');
    if (nav) {
      const label = authStatus.person.name || authStatus.person.email || 'Account';
      nav.setAttribute('utility-account-label', label);
    }
    setupLogout();
  }

  void fetchPlatformInfo().then(showDeploymentBadge);

  // Bind harvest button
  const harvestBtn = $('#harvest-btn');
  if (harvestBtn) {
    harvestBtn.addEventListener('click', onHarvestSubmit);
  }

  // BWB field: Enter submits harvest
  const harvestInput = $('#harvest-bwb-id');
  if (harvestInput) {
    harvestInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') onHarvestSubmit();
    });
  }

  // Job creation toggle
  const jobCreationToggle = $('#job-creation-toggle');
  if (jobCreationToggle) {
    jobCreationToggle.addEventListener('click', () => {
      state.jobCreationOpen = !state.jobCreationOpen;
      renderJobCreation();
    });
  }

  // Bind detail panel close
  $('#detail-close').addEventListener('click', closeDetailPanel);
  $('#detail-backdrop').addEventListener('click', closeDetailPanel);
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeDetailPanel();
  });

  // Initial render
  renderTabs();
  renderAll();
  loadData();

  // Auto-refresh data every 20 seconds
  setInterval(() => loadData(), 20_000);
}

// Start when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
