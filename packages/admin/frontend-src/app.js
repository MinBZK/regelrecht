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
    endpoint: 'api/law_entries',
    columns: [
      { key: 'law_id', label: 'Law ID', sortable: true },
      { key: 'law_name', label: 'Name', sortable: true },
      { key: 'status', label: 'Status', sortable: true },
      { key: 'coverage_score', label: 'Coverage', sortable: true },
      { key: 'updated_at', label: 'Updated', sortable: true },
      { key: '_actions', label: 'Actions', sortable: false },
    ],
    defaultSort: 'updated_at',
    filters: [
      { key: 'status', label: 'Status', options: LAW_STATUSES },
    ],
  },
  jobs: {
    endpoint: 'api/jobs',
    columns: [
      { key: 'id', label: 'ID', sortable: true },
      { key: 'job_type', label: 'Type', sortable: true },
      { key: 'law_id', label: 'Law ID', sortable: true },
      { key: 'status', label: 'Status', sortable: true },
      { key: 'priority', label: 'Priority', sortable: true },
      { key: 'attempts', label: 'Attempts', sortable: true },
      { key: 'created_at', label: 'Created', sortable: true },
    ],
    defaultSort: 'created_at',
    filters: [
      { key: 'status', label: 'Status', options: JOB_STATUSES },
      { key: 'job_type', label: 'Type', options: JOB_TYPES },
      { key: 'law_id', label: 'Law ID', type: 'text' },
    ],
  },
};

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
};


// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------

function $(selector, parent = document) {
  return parent.querySelector(selector);
}

function $$(selector, parent = document) {
  return parent.querySelectorAll(selector);
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


// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function renderTabs() {
  const tabsEl = $('#tabs');
  tabsEl.innerHTML = '';

  for (const tabKey of Object.keys(TAB_CONFIG)) {
    const btn = document.createElement('button');
    btn.className = 'tab' + (tabKey === state.activeTab ? ' tab--active' : '');
    btn.textContent = tabKey === 'law_entries' ? 'Law Entries' : 'Jobs';
    btn.dataset.tab = tabKey;
    btn.addEventListener('click', () => switchTab(tabKey));
    tabsEl.appendChild(btn);
  }
}

function renderFilters() {
  const filtersEl = $('#filters');
  filtersEl.innerHTML = '';

  const config = TAB_CONFIG[state.activeTab];

  for (const filter of config.filters) {
    const label = document.createElement('label');
    label.className = 'toolbar__filter-label';
    label.textContent = filter.label + ':';
    label.setAttribute('for', `filter-${filter.key}`);

    filtersEl.appendChild(label);

    if (filter.type === 'text') {
      const input = document.createElement('input');
      input.type = 'text';
      input.className = 'toolbar__input';
      input.id = `filter-${filter.key}`;
      input.placeholder = filter.label;
      input.value = state.filters[filter.key] || '';
      input.addEventListener('change', (e) => {
        onFilterChange(filter.key, e.target.value.trim());
      });
      filtersEl.appendChild(input);
    } else {
      const select = document.createElement('select');
      select.className = 'toolbar__select';
      select.id = `filter-${filter.key}`;
      select.dataset.filterKey = filter.key;

      const allOption = document.createElement('option');
      allOption.value = '';
      allOption.textContent = `All`;
      select.appendChild(allOption);

      for (const optionValue of filter.options) {
        const option = document.createElement('option');
        option.value = optionValue;
        option.textContent = optionValue;
        if (state.filters[filter.key] === optionValue) {
          option.selected = true;
        }
        select.appendChild(option);
      }

      select.addEventListener('change', (e) => {
        onFilterChange(filter.key, e.target.value);
      });

      filtersEl.appendChild(select);
    }
  }
}

function renderTableHead() {
  const thead = $('#table-head');
  thead.innerHTML = '';

  const config = TAB_CONFIG[state.activeTab];
  const tr = document.createElement('tr');

  for (const col of config.columns) {
    const th = document.createElement('th');
    th.textContent = col.label;

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
      th.appendChild(indicator);

      th.addEventListener('click', () => onSort(col.key));
    }

    tr.appendChild(th);
  }

  thead.appendChild(tr);
}

function renderTableBody() {
  const tbody = $('#table-body');
  tbody.innerHTML = '';

  const config = TAB_CONFIG[state.activeTab];

  if (state.loading) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = config.columns.length;
    td.className = 'table-message';
    td.textContent = 'Loading\u2026';
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  if (state.error) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = config.columns.length;
    td.className = 'table-message table-message--error';
    td.textContent = `Failed to load data: ${state.error}`;
    tr.appendChild(td);
    tbody.appendChild(tr);
    return;
  }

  if (state.data.length === 0) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = config.columns.length;
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

    for (const col of config.columns) {
      const td = document.createElement('td');
      if (col.key === '_actions' && state.activeTab === 'law_entries') {
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

function renderPagination() {
  const totalPages = Math.max(1, Math.ceil(state.totalCount / state.limit));
  const currentPage = Math.floor(state.offset / state.limit) + 1;

  const infoEl = $('#pagination-info');
  infoEl.textContent = `${currentPage} / ${totalPages} (${state.totalCount} results)`;

  const prevBtn = $('#pagination-prev');
  const nextBtn = $('#pagination-next');

  prevBtn.disabled = currentPage <= 1;
  nextBtn.disabled = currentPage >= totalPages;
}

function renderRowActions(row) {
  const container = document.createElement('span');
  container.className = 'action-btns';

  // Re-harvest: available for most statuses (not while actively processing)
  if (RE_HARVESTABLE_STATUSES.includes(row.status)) {
    const harvestBtn = document.createElement('button');
    harvestBtn.className = 'action-btn action-btn--harvest';
    harvestBtn.textContent = 'Harvest';
    harvestBtn.title = `Re-harvest ${row.law_id}`;
    harvestBtn.addEventListener('click', () => onRowHarvestClick(row.law_id, harvestBtn));
    container.appendChild(harvestBtn);
  }

  // Enrich: available after harvest completes
  if (ENRICHABLE_STATUSES.includes(row.status)) {
    const enrichBtn = document.createElement('button');
    enrichBtn.className = 'action-btn action-btn--enrich';
    enrichBtn.textContent = 'Enrich';
    enrichBtn.title = `Trigger enrichment for ${row.law_id}`;
    enrichBtn.addEventListener('click', () => onEnrichClick(row.law_id, enrichBtn));
    container.appendChild(enrichBtn);
  }

  return container;
}

function renderAll() {
  renderFilters();
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


// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

function switchTab(tabKey) {
  if (tabKey === state.activeTab) return;

  state.activeTab = tabKey;
  state.sort = TAB_CONFIG[tabKey].defaultSort;
  state.order = 'desc';
  state.offset = 0;
  state.filters = {};
  state.data = [];
  state.totalCount = 0;
  state.error = null;

  // Show harvest form on both law_entries and jobs tabs
  const harvestForm = $('#harvest-form');
  if (harvestForm) {
    harvestForm.style.display = '';
  }

  renderTabs();
  renderAll();
  fetchData();
}

async function onResetJobs() {
  if (!confirm('Are you sure? This will delete all non-processing jobs from the database.')) return;

  const btn = $('#reset-jobs-btn');
  btn.disabled = true;
  btn.textContent = 'Deleting\u2026';

  try {
    const response = await fetch('api/jobs', { method: 'DELETE' });
    if (response.status === 401) {
      window.location.href = '/auth/login';
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new Error(text || `HTTP ${response.status}`);
    }
    const result = await response.json();
    btn.textContent = `Deleted ${result.deleted} jobs`;
    setTimeout(() => { btn.textContent = 'Reset Jobs'; }, 2000);
    fetchData();
  } catch (err) {
    alert('Reset failed: ' + err.message);
    btn.textContent = 'Reset Jobs';
  } finally {
    btn.disabled = false;
  }
}

async function onHarvestSubmit(e) {
  e.preventDefault();
  const input = $('#harvest-bwb-id');
  const btn = $('#harvest-form .harvest-form__btn');
  const bwbId = input.value.trim();
  if (!bwbId) return;

  btn.disabled = true;
  btn.textContent = 'Submitting\u2026';

  try {
    const response = await fetch('api/harvest-jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bwb_id: bwbId }),
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
    await response.json();
    input.value = '';
    btn.textContent = 'Queued \u2713';
    btn.disabled = false;
    setTimeout(() => { btn.textContent = 'Harvest'; }, 2000);
    fetchData();
  } catch (err) {
    alert('Harvest failed: ' + err.message);
    btn.disabled = false;
    btn.textContent = 'Harvest';
  }
}

async function onRowHarvestClick(lawId, btn) {
  btn.disabled = true;
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
    fetchData();
  } catch (err) {
    alert('Harvest failed: ' + err.message);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Harvest';
  }
}

async function onEnrichClick(lawId, btn) {
  btn.disabled = true;
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
    fetchData();
  } catch (err) {
    alert('Enrich failed: ' + err.message);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Enrich';
  }
}

function viewJobsForLaw(lawId) {
  state.activeTab = 'jobs';
  state.sort = TAB_CONFIG.jobs.defaultSort;
  state.order = 'desc';
  state.offset = 0;
  state.filters = { law_id: lawId };
  state.data = [];
  state.totalCount = 0;
  state.error = null;

  const harvestForm = $('#harvest-form');
  if (harvestForm) {
    harvestForm.style.display = '';
  }

  renderTabs();
  renderAll();
  fetchData();
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
  fetchData();
}

function onFilterChange(key, value) {
  if (value) {
    state.filters[key] = value;
  } else {
    delete state.filters[key];
  }
  state.offset = 0;
  fetchData();
}

function onPrevPage() {
  if (state.offset > 0) {
    state.offset = Math.max(0, state.offset - state.limit);
    fetchData();
  }
}

function onNextPage() {
  const totalPages = Math.ceil(state.totalCount / state.limit);
  const currentPage = Math.floor(state.offset / state.limit) + 1;
  if (currentPage < totalPages) {
    state.offset += state.limit;
    fetchData();
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

  // Bind pagination buttons
  $('#pagination-prev').addEventListener('click', onPrevPage);
  $('#pagination-next').addEventListener('click', onNextPage);

  // Bind harvest form
  const harvestForm = $('#harvest-form');
  if (harvestForm) {
    harvestForm.addEventListener('submit', onHarvestSubmit);
  }

  // Bind reset jobs button
  const resetBtn = $('#reset-jobs-btn');
  if (resetBtn) {
    resetBtn.addEventListener('click', onResetJobs);
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
  fetchData();

  // Auto-refresh data every 20 seconds
  setInterval(() => fetchData(), 20_000);
}

// Start when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
