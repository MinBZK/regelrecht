import { shallowMount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import { ref, reactive } from 'vue';
import MarkingsView from './MarkingsView.vue';
import DataTable from '../components/DataTable.vue';
import MarkingDetailPanel from '../components/MarkingDetailPanel.vue';
import MarkingClusters from '../components/MarkingClusters.vue';
import { MARKING_COLUMNS, MARKING_SORT_OPTIONS } from '../constants.js';

const setSort = vi.fn();
const setFilter = vi.fn();
const goToPage = vi.fn();
const filterParams = vi.fn(() => new URLSearchParams());

vi.mock('../composables/useMarkings.js', () => ({
  useMarkings: () => ({
    data: ref([
      {
        id: '1',
        law_id: 'test_law',
        article: '5',
        about: 'de eerstvolgende werkdag',
        resolution: 'operation',
        resolved_by: 'een WORKING_DAY-bewerking',
        target: ['datum_van_betaling'],
        accepted: false,
      },
    ]),
    loading: ref(false),
    error: ref(null),
    sort: ref('created_at'),
    order: ref('desc'),
    filters: reactive({}),
    currentPage: ref(1),
    totalPages: ref(1),
    setSort,
    setFilter,
    goToPage,
    filterParams,
  }),
}));

vi.mock('../composables/useMarkingClusters.js', () => ({
  useMarkingClusters: () => ({
    data: ref([
      {
        resolution: 'operation',
        resolved_by: 'een WORKING_DAY-bewerking',
        markings: 3,
        laws: 2,
        articles: 3,
        providers: ['opencode'],
        all_accepted: false,
      },
    ]),
    loading: ref(false),
    error: ref(null),
  }),
}));

describe('MarkingsView', () => {
  it('wires the DataTable with the marking columns and clickable rows', () => {
    const w = shallowMount(MarkingsView);
    const table = w.findComponent(DataTable);
    expect(table.props('columns')).toBe(MARKING_COLUMNS);
    expect(table.props('sortOptions')).toBe(MARKING_SORT_OPTIONS);
    expect(table.props('clickableRows')).toBe(true);
  });

  it('forwards sort and filter events to the composable', () => {
    const w = shallowMount(MarkingsView);
    const table = w.findComponent(DataTable);
    table.vm.$emit('sort', 'resolved_by', 'asc');
    table.vm.$emit('filter-change', 'provider', 'claude');
    expect(setSort).toHaveBeenCalledWith('resolved_by', 'asc');
    expect(setFilter).toHaveBeenCalledWith('provider', 'claude');
  });

  it('opens the detail panel with the clicked row', async () => {
    const w = shallowMount(MarkingsView);
    const panel = w.findComponent(MarkingDetailPanel);
    expect(panel.props('isOpen')).toBe(false);

    const row = { id: '1', about: 'werkdag' };
    w.findComponent(DataTable).vm.$emit('row-click', row);
    await w.vm.$nextTick();

    expect(panel.props('isOpen')).toBe(true);
    expect(panel.props('row')).toEqual(row);
  });

  it('shows the backlog above the list', () => {
    const w = shallowMount(MarkingsView);
    const clusters = w.findComponent(MarkingClusters);
    expect(clusters.exists()).toBe(true);
    expect(clusters.props('clusters')).toHaveLength(1);
  });

  // Getting from "this change is wanted in four places" to the four articles
  // is the whole point of showing the backlog beside the list.
  it('filters the list when a cluster is picked', () => {
    const w = shallowMount(MarkingsView);
    w.findComponent(MarkingClusters).vm.$emit('select', {
      resolution: 'operation',
      resolved_by: 'een WORKING_DAY-bewerking',
    });
    expect(setFilter).toHaveBeenCalledWith('resolution', 'operation');
  });
});
