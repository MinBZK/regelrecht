import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import SearchPopover from './SearchPopover.vue';

// SearchPopover queries the corpus server-side (`/corpus/laws?q=`) and groups
// the response by source. We drive it via `wrapper.vm` and assert on the
// `groupedLaws` computed.
//
// useTrajects() calls vue-router's useRoute(); mock it so the component mounts
// without a router (no active traject → global /api/corpus URL).
vi.mock('vue-router', () => ({
  useRoute: () => ({ params: {} }),
}));

// Central corpus (priority 2) listed before the BES repo (priority 0) to prove
// the grouping order comes from source_priority, not response order.
const LAWS = [
  {
    law_id: 'besluit_zorgverzekering',
    source_id: 'central',
    source_name: 'Centrale Regelrecht Corpus',
    source_priority: 2,
  },
  {
    law_id: 'besluit_zorgverzekering_bes',
    source_id: 'traject-own',
    source_name: 'MinBZK/regelrecht-corpus-BES',
    source_priority: 0,
  },
];

beforeEach(() => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return { ok: true, json: async () => LAWS };
      }
      return { ok: true, json: async () => [] };
    }),
  );
});

// Wait past the 200ms input debounce plus the awaited fetch.
const settle = () => new Promise((r) => setTimeout(r, 300));

describe('SearchPopover server-side search', () => {
  it('groups corpus matches by source, private repo (priority 0) first', async () => {
    const wrapper = mount(SearchPopover);
    wrapper.vm.search = 'zorgverzekering';
    await nextTick();
    await settle();
    await nextTick();

    const groups = wrapper.vm.groupedLaws;
    expect(groups.map((g) => g.source_name)).toEqual([
      'MinBZK/regelrecht-corpus-BES',
      'Centrale Regelrecht Corpus',
    ]);
    expect(groups[0].laws.map((l) => l.law_id)).toEqual(['besluit_zorgverzekering_bes']);
  });

  it('queries the backend with the q parameter (not a client-side filter)', async () => {
    const wrapper = mount(SearchPopover);
    wrapper.vm.search = 'kieswet';
    await nextTick();
    await settle();

    const urls = fetch.mock.calls.map((c) => String(c[0]));
    expect(urls.some((u) => u.includes('/corpus/laws') && u.includes('q=kieswet'))).toBe(true);
  });

  it('debounces: a single query fires one corpus request', async () => {
    const wrapper = mount(SearchPopover);
    wrapper.vm.search = 'z';
    await nextTick();
    wrapper.vm.search = 'zo';
    await nextTick();
    wrapper.vm.search = 'zorg';
    await nextTick();
    await settle();

    const corpusCalls = fetch.mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/corpus/laws') && u.includes('q='));
    expect(corpusCalls).toHaveLength(1);
    expect(corpusCalls[0]).toContain('q=zorg');
  });

  it('clearing the query discards an in-flight corpus fetch (no stale results)', async () => {
    // Make the corpus fetch hang so we can clear the input mid-flight.
    let resolveCorpus;
    fetch.mockImplementation((url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return Promise.resolve({ ok: false, json: async () => ({}) });
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return new Promise((r) => {
          resolveCorpus = () => r({ ok: true, json: async () => LAWS });
        });
      }
      return Promise.resolve({ ok: true, json: async () => [] });
    });

    const wrapper = mount(SearchPopover);
    wrapper.vm.search = 'zorg';
    await nextTick();
    await settle(); // debounce fires; the corpus fetch is now pending

    // Clear before the fetch resolves, then let the stale fetch settle.
    wrapper.vm.search = '';
    await nextTick();
    resolveCorpus();
    await settle();
    await nextTick();

    // The cleared term's response must not repopulate the list, and no
    // wetten.overheid.nl search should have been fired for it.
    expect(wrapper.vm.groupedLaws).toEqual([]);
    const bwbCalls = fetch.mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/harvest/search'));
    expect(bwbCalls).toHaveLength(0);
  });

  it('a backend error surfaces as failed, not a cascade to the external fallback', async () => {
    fetch.mockImplementation((url) => {
      const u = String(url);
      if (u.includes('/auth/status')) return Promise.resolve({ ok: false, json: async () => ({}) });
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return Promise.resolve({ ok: false, status: 500, json: async () => [] });
      }
      return Promise.resolve({ ok: true, json: async () => [] });
    });

    const wrapper = mount(SearchPopover);
    wrapper.vm.search = 'zorg';
    await nextTick();
    await settle();
    await nextTick();

    expect(wrapper.vm.searchFailed).toBe(true);
    expect(wrapper.vm.groupedLaws).toEqual([]);
    const bwbCalls = fetch.mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/harvest/search'));
    expect(bwbCalls).toHaveLength(0);
  });
});
