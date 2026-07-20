import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import SearchPopover from './SearchPopover.vue';

// SearchPopover queries the corpus server-side (`/corpus/laws?q=`) and orders
// the response into a flat option list (private repo first). We drive it via
// `wrapper.vm` and assert on the `sortedLaws` computed.
//
// useTrajects() calls vue-router's useRoute(); mock it so the component mounts
// without a router. Default: no active traject → global /api/corpus URL. The
// traject-scoped promote tests swap in a trajectRef per test (vi.hoisted so
// the hoisted mock factory can reference it).
const routeMock = vi.hoisted(() => ({ params: {} }));
vi.mock('vue-router', () => ({
  useRoute: () => routeMock,
}));

// Central corpus (priority 2) listed before the private traject repo
// (priority 0) to prove the sort order comes from source_priority, not
// response order. Source name/law id are anonymized fixtures, not real repos.
const LAWS = [
  {
    law_id: 'besluit_zorgverzekering',
    source_id: 'central',
    source_name: 'Centrale Regelrecht Corpus',
    source_priority: 2,
  },
  {
    law_id: 'besluit_zorgverzekering_example',
    source_id: 'traject-own',
    source_name: 'example-org/regelrecht-corpus-example',
    source_priority: 0,
  },
];

beforeEach(() => {
  routeMock.params = {};
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
  it('orders corpus matches private repo (priority 0) first, source per row', async () => {
    const wrapper = mount(SearchPopover);
    wrapper.vm.search = 'zorgverzekering';
    await nextTick();
    await settle();
    await nextTick();

    // Flat option list sorted by source_priority: the private traject repo
    // (priority 0) sorts above the central corpus (priority 2). Each row keeps
    // its own source_name (shown as supporting-text in the listbox).
    const laws = wrapper.vm.sortedLaws;
    expect(laws.map((l) => l.law_id)).toEqual([
      'besluit_zorgverzekering_example',
      'besluit_zorgverzekering',
    ]);
    expect(laws.map((l) => l.source_name)).toEqual([
      'example-org/regelrecht-corpus-example',
      'Centrale Regelrecht Corpus',
    ]);
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
    expect(wrapper.vm.sortedLaws).toEqual([]);
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
    expect(wrapper.vm.sortedLaws).toEqual([]);
    const bwbCalls = fetch.mock.calls
      .map((c) => String(c[0]))
      .filter((u) => u.includes('/harvest/search'));
    expect(bwbCalls).toHaveLength(0);
  });

  it('renders no promote buttons outside a traject (global corpus scope)', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    expect(wrapper.findAll('nldd-list-item[data-law-id]')).toHaveLength(2);
    expect(promoteButtons(wrapper)).toHaveLength(0);
  });
});

// Fictieve traject-ref (publiek testbestand, geen echte repo/traject-namen).
const TRAJECT_REF = 'voorbeeld-abcd1234';

async function searchFor(wrapper, term) {
  wrapper.vm.search = term;
  await nextTick();
  await settle();
  await nextTick();
}

function promoteButtons(wrapper) {
  return wrapper
    .findAll('nldd-button')
    .filter((b) => b.attributes('text') === 'Toevoegen aan traject');
}

describe('SearchPopover "Toevoegen aan traject" in zoekresultaten', () => {
  beforeEach(() => {
    routeMock.params = { trajectRef: TRAJECT_REF };
  });

  it('toont de knop alleen bij federatie-treffers die nog niet in de eigen repo staan', async () => {
    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    const rows = wrapper.findAll('nldd-list-item[data-law-id]');
    const central = rows.find((r) => r.attributes('data-law-id') === 'besluit_zorgverzekering');
    const own = rows.find(
      (r) => r.attributes('data-law-id') === 'besluit_zorgverzekering_example',
    );

    // Centrale-seed-treffer (priority 2): wel een promote-knop.
    expect(
      central.findAll('nldd-button').some((b) => b.attributes('text') === 'Toevoegen aan traject'),
    ).toBe(true);
    // Eigen-repo-treffer (priority 0): geen knop - er valt niets te promoten.
    expect(own.findAll('nldd-button')).toHaveLength(0);
    expect(own.get('nldd-text-cell').attributes('supporting-text')).toBe(
      'example-org/regelrecht-corpus-example',
    );
  });

  it('promoot via POST /promote en emit "promoted" (niet select-law) na sluiten', async () => {
    fetch.mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: true, json: async () => ({ law_id: 'besluit_zorgverzekering' }) };
      }
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return { ok: true, json: async () => LAWS };
      }
      return { ok: true, json: async () => [] };
    });

    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    await promoteButtons(wrapper)[0].trigger('click');
    await settle();

    const promoteCall = fetch.mock.calls.find(
      (c) => String(c[0]).includes('/promote') && c[1]?.method === 'POST',
    );
    expect(String(promoteCall[0])).toBe(
      `/api/trajects/${TRAJECT_REF}/corpus/laws/besluit_zorgverzekering/promote`,
    );

    // Zelfde deferral als select-law: de emit komt pas als de popover dicht
    // is, zodat de focus-op-de-wet van de parent wint van _returnFocus.
    expect(wrapper.emitted('promoted')).toBeUndefined();
    await wrapper.get('nldd-popover').trigger('close');
    expect(wrapper.emitted('promoted')).toEqual([['besluit_zorgverzekering']]);
    // click.stop: de knop mag de rij-navigatie (select-law) niet triggeren.
    expect(wrapper.emitted('select-law')).toBeUndefined();
  });

  it('een 409 markeert de wet als al-in-traject: knop weg, "Al in dit traject"', async () => {
    fetch.mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: false, status: 409, text: async () => 'al in traject', headers: { get: () => '' } };
      }
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return { ok: true, json: async () => LAWS };
      }
      return { ok: true, json: async () => [] };
    });

    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    await promoteButtons(wrapper)[0].trigger('click');
    await settle();
    await nextTick();

    expect(promoteButtons(wrapper)).toHaveLength(0);
    const central = wrapper.get('nldd-list-item[data-law-id="besluit_zorgverzekering"]');
    expect(central.get('nldd-text-cell').attributes('supporting-text')).toBe('Al in dit traject');
    await wrapper.get('nldd-popover').trigger('close');
    expect(wrapper.emitted('promoted')).toBeUndefined();
  });

  it('een niet-409-fout toont de foutbanner en laat de knop staan', async () => {
    fetch.mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: false, status: 500, text: async () => 'boem', headers: { get: () => '' } };
      }
      if (u.includes('/corpus/laws') && u.includes('q=')) {
        return { ok: true, json: async () => LAWS };
      }
      return { ok: true, json: async () => [] };
    });

    const wrapper = mount(SearchPopover);
    await searchFor(wrapper, 'zorgverzekering');

    await promoteButtons(wrapper)[0].trigger('click');
    await settle();
    await nextTick();

    expect(wrapper.vm.promoteError).toBeTruthy();
    // De banner-rij draagt de melding als attribute op de nldd-text-cell
    // (custom elements renderen tekst niet als DOM-tekst in happy-dom).
    const bannerTexts = wrapper
      .findAll('nldd-text-cell')
      .map((c) => c.attributes('text'));
    expect(bannerTexts).toContain(
      'Toevoegen aan het traject is mislukt. Probeer het opnieuw of neem contact op.',
    );
    expect(promoteButtons(wrapper)).toHaveLength(1);
    expect(wrapper.emitted('promoted')).toBeUndefined();
  });
});
