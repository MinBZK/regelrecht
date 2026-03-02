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

const TAB_CONFIG = {
  law_entries: {
    endpoint: 'api/law_entries',
    columns: [
      { key: 'law_id', label: 'Law ID', sortable: true },
      { key: 'law_name', label: 'Name', sortable: true },
      { key: 'status', label: 'Status', sortable: true },
      { key: 'quality_score', label: 'Quality', sortable: true },
      { key: 'updated_at', label: 'Updated', sortable: true },
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
  if (key === 'quality_score') {
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

    const select = document.createElement('select');
    select.className = 'toolbar__select';
    select.id = `filter-${filter.key}`;
    select.dataset.filterKey = filter.key;

    // "All" option
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

    filtersEl.appendChild(label);
    filtersEl.appendChild(select);
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
    for (const col of config.columns) {
      const td = document.createElement('td');
      td.innerHTML = formatCell(row[col.key], col.key);
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

  // Initial render
  renderTabs();
  renderAll();
  fetchData();
}

// Start when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
