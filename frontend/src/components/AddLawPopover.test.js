import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import AddLawPopover from './AddLawPopover.vue';

// De popover is traject-gebonden: useTrajects() leest de trajectRef uit de
// route. Fictieve ref, geen echte repo/traject-namen (publiek testbestand).
const TRAJECT_REF = 'voorbeeld-abcd1234';

vi.mock('vue-router', () => ({
  useRoute: () => ({ params: { trajectRef: TRAJECT_REF } }),
}));

// Traject-scoped zoekrespons: het traject federeert de centrale seed, dus de
// lijst mengt eigen-repo-treffers (source_priority 0, al in het traject) met
// centrale-seed-treffers (hogere priority, promoteerbaar). Fictieve namen.
const SEARCH_LAWS = [
  {
    law_id: 'wet_voorbeeld',
    name: 'Wet voorbeeld',
    source_id: 'central',
    source_name: 'Centrale Regelrecht Corpus',
    source_priority: 2,
  },
  {
    law_id: 'wet_al_binnen',
    name: 'Wet al binnen',
    source_id: 'traject-own',
    source_name: 'example-org/traject-corpus-example',
    source_priority: 0,
  },
];

const PROMOTABLE = SEARCH_LAWS[0];

function stubFetch({ searchLaws = SEARCH_LAWS, bwbResults = [] } = {}) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.includes('/harvest/search')) return { ok: true, json: async () => bwbResults };
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: true, json: async () => ({ law_id: 'wet_voorbeeld', copied_files: 3 }) };
      }
      if (u.includes('/corpus/harvest') && init.method === 'POST') {
        return { ok: true, status: 202, json: async () => ({ job_id: 'j1' }) };
      }
      if (u.startsWith(`/api/trajects/${TRAJECT_REF}/corpus/laws`)) {
        return { ok: true, json: async () => searchLaws };
      }
      return { ok: true, json: async () => [] };
    }),
  );
}

beforeEach(() => {
  stubFetch();
});

// Voorbij de 200ms-debounce plus de fetches.
const settle = () => new Promise((r) => setTimeout(r, 300));

async function searchFor(wrapper, term) {
  wrapper.vm.search = term;
  await nextTick();
  await settle();
  await nextTick();
}

describe('AddLawPopover', () => {
  it('zoekt via de traject-scoped corpus-API en sorteert eigen repo eerst', async () => {
    const wrapper = mount(AddLawPopover);
    await searchFor(wrapper, 'wet');

    const urls = fetch.mock.calls.map((c) => String(c[0]));
    expect(
      urls.some(
        (u) => u.startsWith(`/api/trajects/${TRAJECT_REF}/corpus/laws?`) && u.includes('q=wet'),
      ),
    ).toBe(true);
    // Geen tweede zoekpad: de globale corpus-URL wordt niet aangeroepen.
    expect(urls.some((u) => u.startsWith('/api/corpus/laws'))).toBe(false);
    expect(wrapper.vm.sortedLaws.map((l) => l.law_id)).toEqual([
      'wet_al_binnen',
      'wet_voorbeeld',
    ]);
  });

  it('markeert een wet die al in de traject-repo staat (priority 0) als niet-promoteerbaar', async () => {
    const wrapper = mount(AddLawPopover);
    await searchFor(wrapper, 'wet');

    const rows = wrapper.findAll('nldd-list-item[data-law-id]');
    const binnen = rows.find((r) => r.attributes('data-law-id') === 'wet_al_binnen');
    expect(binnen.get('nldd-text-cell').attributes('supporting-text')).toBe('Al in dit traject');
    expect(binnen.get('nldd-button').attributes('disabled')).toBeDefined();

    const buiten = rows.find((r) => r.attributes('data-law-id') === 'wet_voorbeeld');
    expect(buiten.get('nldd-button').attributes('disabled')).toBeUndefined();
  });

  it('promoot via POST /promote en emit "promoted"', async () => {
    const wrapper = mount(AddLawPopover);
    await searchFor(wrapper, 'wet');

    await wrapper.vm.promote(PROMOTABLE);

    const promoteCall = fetch.mock.calls.find(
      (c) => String(c[0]).includes('/promote') && c[1]?.method === 'POST',
    );
    expect(String(promoteCall[0])).toBe(
      `/api/trajects/${TRAJECT_REF}/corpus/laws/wet_voorbeeld/promote`,
    );
    expect(wrapper.emitted('promoted')).toEqual([['wet_voorbeeld']]);
  });

  it('een 409 op promote markeert de wet alsnog als al-in-traject', async () => {
    stubFetch();
    fetch.mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/promote') && init.method === 'POST') {
        return { ok: false, status: 409, text: async () => 'al in traject', headers: { get: () => '' } };
      }
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      if (u.startsWith(`/api/trajects/${TRAJECT_REF}/corpus/laws`)) {
        return { ok: true, json: async () => [] };
      }
      return { ok: true, json: async () => SEARCH_LAWS };
    });

    const wrapper = mount(AddLawPopover);
    await searchFor(wrapper, 'wet');
    await wrapper.vm.promote(PROMOTABLE);
    await nextTick();

    expect(wrapper.emitted('promoted')).toBeUndefined();
    expect(wrapper.vm.isInTraject(PROMOTABLE)).toBe(true);
  });

  it('valt bij nul corpus-treffers terug op de wetten.overheid.nl-zoeker', async () => {
    stubFetch({
      searchLaws: [],
      bwbResults: [{ bwb_id: 'BWBR0002399', title: 'Voorbeeldwet extern', type: 'wet' }],
    });
    const wrapper = mount(AddLawPopover);
    await searchFor(wrapper, 'voorbeeldwet');
    // Debounce van useBwbSearch (400ms) na de corpus-respons.
    await new Promise((r) => setTimeout(r, 450));
    await nextTick();

    const urls = fetch.mock.calls.map((c) => String(c[0]));
    expect(urls.some((u) => u.includes('/harvest/search'))).toBe(true);
    const row = wrapper.get('nldd-list-item[data-bwb-id="BWBR0002399"]');
    expect(row.get('nldd-text-cell').attributes('text')).toBe('Voorbeeldwet extern');
  });

  it('start een traject-scoped harvest voor een BWB-resultaat en emit "harvest-requested"', async () => {
    const wrapper = mount(AddLawPopover);
    await wrapper.vm.requestTrajectHarvest('BWBR0002399', 'Voorbeeldwet extern');

    const call = fetch.mock.calls.find(
      (c) => String(c[0]).includes('/corpus/harvest') && c[1]?.method === 'POST',
    );
    expect(String(call[0])).toBe(`/api/trajects/${TRAJECT_REF}/corpus/harvest`);
    expect(JSON.parse(call[1].body)).toEqual({
      bwb_id: 'BWBR0002399',
      law_name: 'Voorbeeldwet extern',
    });
    expect(wrapper.emitted('harvest-requested')).toEqual([['BWBR0002399']]);
    expect(wrapper.vm.harvestState.BWBR0002399).toBe('requested');
  });

  it('een 409 op de harvest-aanvraag toont "er loopt al een aanvraag"', async () => {
    fetch.mockImplementation(async (url, init = {}) => {
      const u = String(url);
      if (u.includes('/corpus/harvest') && init.method === 'POST') {
        return { ok: false, status: 409, text: async () => 'loopt al', headers: { get: () => '' } };
      }
      if (u.includes('/auth/status')) return { ok: false, json: async () => ({}) };
      return { ok: true, json: async () => [] };
    });
    const wrapper = mount(AddLawPopover);
    await wrapper.vm.requestTrajectHarvest('BWBR0002399');
    expect(wrapper.vm.harvestState.BWBR0002399).toBe('conflict');
    expect(wrapper.emitted('harvest-requested')).toBeUndefined();
  });

  it('herkent een direct getypt BWB-id (ook lowercase) als harvest-kandidaat', async () => {
    stubFetch({ searchLaws: [] });
    const wrapper = mount(AddLawPopover);
    await searchFor(wrapper, 'bwbr0002399');
    expect(wrapper.vm.bwbIdQuery).toBe('BWBR0002399');

    await searchFor(wrapper, 'wet op het voortgezet onderwijs');
    expect(wrapper.vm.bwbIdQuery).toBe(null);
  });
});
