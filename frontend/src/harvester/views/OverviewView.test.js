import { shallowMount } from '@vue/test-utils';
import { describe, it, expect, afterEach, vi } from 'vitest';
import { ref } from 'vue';
import OverviewView from './OverviewView.vue';
import DataTable from '../components/DataTable.vue';

const STATS = {
  jobs: {
    total: 1204,
    by_type: { harvest: 842, enrich: 362 },
    by_status: { pending: 12, processing: 3, completed: 1180, failed: 9 },
    by_type_status: {},
  },
  executed: {
    today: { total: 47, harvest: 30, enrich: 17 },
    last_7d: { total: 210, harvest: 140, enrich: 70 },
  },
  open_untranslatables: 23,
  recent_failures: [
    {
      id: 'job-1',
      law_id: 'BWBR0001',
      job_type: 'enrich',
      failed_at: '2026-07-03T10:00:00Z',
      error: 'job timed out after 300s',
    },
  ],
};

const openDetail = vi.fn();
const closeDetail = vi.fn();

vi.mock('../composables/useDashboardStats.js', () => ({
  useDashboardStats: () => ({
    stats: ref(STATS),
    loading: ref(false),
    error: ref(null),
  }),
}));

vi.mock('../composables/useJobDetail.js', () => ({
  useJobDetail: () => ({
    job: ref(null),
    isOpen: ref(false),
    open: openDetail,
    close: closeDetail,
  }),
}));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('OverviewView', () => {
  it('renders the KPI figures (nl-NL formatted)', () => {
    const w = shallowMount(OverviewView);
    const text = w.text();
    expect(text).toContain('1.204'); // jobs total
    expect(text).toContain('842'); // harvest
    expect(text).toContain('362'); // enrich
    expect(text).toContain('23'); // open untranslatables
    // Executed windows
    expect(text).toContain('Vandaag');
    expect(text).toContain('Afgelopen 7 dagen');
  });

  it('wires the failures DataTable with the recent_failures data', () => {
    const w = shallowMount(OverviewView);
    const table = w.findComponent(DataTable);
    expect(table.props('data')).toEqual(STATS.recent_failures);
    expect(table.props('clickableRows')).toBe(true);
    // No sortOptions passed → the table toolbar stays empty.
    expect(table.props('sortOptions')).toBeNull();
  });

  it('fetches the full job and opens the detail panel on row click', async () => {
    const fullJob = { id: 'job-1', status: 'failed', result: { error: 'job timed out after 300s' } };
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve(
          new Response(JSON.stringify(fullJob), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          }),
        ),
      ),
    );

    const w = shallowMount(OverviewView);
    w.findComponent(DataTable).vm.$emit('row-click', STATS.recent_failures[0]);

    await vi.waitFor(() => expect(openDetail).toHaveBeenCalled());
    expect(openDetail).toHaveBeenCalledWith(fullJob);
  });
});
