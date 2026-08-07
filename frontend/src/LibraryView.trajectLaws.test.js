// De sectie "Traject" in het linkermenu.
//
// Zonder deze sectie is het linkermenu leeg zodra je een nieuw traject
// aanmaakt of dat van iemand anders opent: "Bewerkt" is de branch-diff (leeg
// bij een vers traject) en Favorieten/Recent bekeken zijn persoonlijk. Deze
// tests leggen vast wat er dan wél staat, en wat er gebeurt als de eigen bron
// te groot, leeg, of niet scanbaar is.
//
// Harnas overgenomen van LibraryView.indexErrorTakeover.repro.test.js:
// shallow-mount met elke composable gestubd, bewust smal op dit ene oppervlak.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref, nextTick } from 'vue';

const routeState = {
  name: 'library-traject',
  params: { trajectRef: 'traject-abcd1234' },
  query: {},
  fullPath: '/trajecten/traject-abcd1234',
};
vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({
    replace: vi.fn().mockResolvedValue(undefined),
    push: vi.fn(),
    resolve: () => ({ href: '#' }),
  }),
  onBeforeRouteUpdate: vi.fn(),
  onBeforeRouteLeave: vi.fn(),
}));

vi.mock('./composables/useAuth.js', () => ({
  useAuth: () => ({ authenticated: ref(true), login: vi.fn() }),
}));

// In een hoisted box zodat een test in-place van traject kan wisselen: de
// traject-switcher laat LibraryView gemonteerd en verandert alleen de
// route-param, waar `watch(activeTrajectRef)` op reageert.
const { trajectScope } = vi.hoisted(() => ({ trajectScope: {} }));

vi.mock('./composables/useTrajects.js', () => {
  trajectScope.activeTrajectRef = ref('traject-abcd1234');
  return {
    useTrajects: () => ({
      activeTrajectRef: trajectScope.activeTrajectRef,
      activeTraject: ref({ name: 'Testtraject' }),
    }),
    refreshTrajects: vi.fn(),
  };
});

// Het antwoordboek dat elke test naar smaak omzet: de bronnen van het traject,
// de wetten van de eigen bron, de favorieten en de bewerkte wetten.
const { server, ApiErrorStub, apiFetch, apiFetchJson } = vi.hoisted(() => {
  class ApiErrorStub extends Error {
    constructor(message, { status, body = '' } = {}) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
      this.body = body;
    }
  }
  const server = {
    sources: [],
    sourceLaws: [],
    favorites: [],
    changed: [],
    sourcesFails: false,
    // Een belofte die de bronnen-aanroep laat hangen, zodat een test kan zien
    // wat het menu toont terwijl het antwoord van het nieuwe traject nog
    // onderweg is. `null` = meteen antwoorden.
    holdSources: null,
  };
  // Metadata voor `?ids=`: de sidebar resolvet favorieten + bewerkte wetten
  // via die route, dus die moeten uit dezelfde catalogus komen.
  const byId = (id) =>
    server.sourceLaws.find((l) => l.law_id === id) || {
      law_id: id,
      display_name: id,
      source_name: 'Onbekend',
      source_priority: 0,
    };
  const idsFrom = (url) => {
    const match = /[?&]ids=([^&]*)/.exec(String(url));
    return match ? decodeURIComponent(match[1]).split(',').filter(Boolean) : [];
  };
  const answer = (url) => {
    const u = String(url);
    if (u.endsWith('/sources')) {
      if (server.sourcesFails) {
        throw new ApiErrorStub('Failed to load sources: 502', { status: 502 });
      }
      return server.sources;
    }
    if (u.includes('/changed-laws')) return server.changed;
    if (u.includes('/api/favorites')) return server.favorites;
    if (u.includes('/corpus/laws')) {
      if (u.includes('source=')) return server.sourceLaws;
      return idsFrom(u).map(byId);
    }
    return [];
  };
  return {
    server,
    ApiErrorStub,
    apiFetch: vi.fn(async (url) => ({
      ok: true,
      status: 200,
      json: async () => answer(url),
    })),
    apiFetchJson: vi.fn(async (url) => {
      if (server.holdSources && String(url).endsWith('/sources')) await server.holdSources;
      return answer(url);
    }),
  };
});

vi.mock('./lib/apiFetch.js', () => ({
  apiFetch: (...a) => apiFetch(...a),
  apiFetchJson: (...a) => apiFetchJson(...a),
  ApiError: ApiErrorStub,
}));

vi.mock('./composables/useFeatureFlags.js', () => ({
  useFeatureFlags: () => ({ isEnabled: () => true }),
}));

vi.mock('./composables/useDocumentsManager.js', () => ({
  useDocumentsManager: () => ({
    documents: ref([]),
    listLoading: ref(false),
    listError: ref(null),
    currentPath: ref(null),
    currentBody: ref(''),
    hasChanges: ref(false),
    docLoading: ref(false),
    docError: ref(null),
    saving: ref(false),
    open: vi.fn(),
    startNew: vi.fn(),
    close: vi.fn(),
    uploadDocument: vi.fn(),
    displayTitle: (p) => String(p ?? '').replace(/\.md$/, ''),
    dropDraft: vi.fn(),
    refreshList: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock('./composables/useTrajectDocumentJobs.js', () => ({
  useTrajectDocumentJobs: () => ({
    jobs: ref([]),
    cancelJob: vi.fn(),
    refresh: vi.fn(),
    startPolling: vi.fn(),
    stopPolling: vi.fn(),
  }),
}));

vi.mock('./composables/useDocumentUpload.js', () => ({
  useDocumentUpload: () => ({
    fileInput: ref(null),
    uploadError: ref(null),
    uploadRetryable: ref(false),
    onUpload: vi.fn(),
    onFileChange: vi.fn(),
  }),
}));

vi.mock('./composables/useDocumentTaskReview.js', () => ({
  useDocumentTaskReview: () => ({
    reviewTask: ref(null),
    proposedContent: ref(null),
    loadError: ref(null),
    loadReview: vi.fn().mockResolvedValue(undefined),
    approveAfterSave: vi.fn(),
    reject: vi.fn(),
  }),
}));

import LibraryView from './LibraryView.vue';

const OWN_SOURCE = 'traject-eigen';

/// Zeven wetten, met weergavenamen die bewust NIET in law_id-volgorde staan:
/// de backend sorteert op law_id, de gebruiker leest de weergavenaam.
const SEVEN_LAWS = [
  ['wet_a', 'Zorgverzekeringswet'],
  ['wet_b', 'Algemene wet bestuursrecht'],
  ['wet_c', 'Wet langdurige zorg'],
  ['wet_d', 'Besluit langdurige zorg'],
  ['wet_e', 'Regeling langdurige zorg'],
  ['wet_f', 'Kieswet'],
  ['wet_g', 'Participatiewet'],
].map(([law_id, display_name]) => ({
  law_id,
  display_name,
  source_id: OWN_SOURCE,
  source_name: 'Eigen traject-repo',
  source_priority: 0,
}));

/// `n` wetten met oplopende weergavenamen (`Wet 001` … `Wet 0nn`), zodat de
/// alfabetische volgorde gelijkloopt met de nummering en een test aan de eerste
/// en laatste regel kan zien hoeveel er zijn uitgeklapt.
function manyLaws(n) {
  return Array.from({ length: n }, (_, i) => ({
    law_id: `wet_${String(i + 1).padStart(3, '0')}`,
    display_name: `Wet ${String(i + 1).padStart(3, '0')}`,
    source_id: OWN_SOURCE,
    source_name: 'Eigen traject-repo',
    source_priority: 0,
  }));
}

function ownSource(overrides = {}) {
  return [
    {
      id: OWN_SOURCE,
      name: 'Eigen traject-repo',
      priority: 0,
      law_count: SEVEN_LAWS.length,
      index_error: null,
      ...overrides,
    },
    // Een gefedereerde centrale bron staat er altijd naast; alleen de eigen
    // bron (priority 0) voedt de sectie.
    {
      id: 'centraal',
      name: 'Corpus juris',
      priority: 1,
      law_count: 4321,
      index_error: null,
    },
  ];
}

async function mountLibrary() {
  const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
  for (let i = 0; i < 6; i++) await nextTick();
  return wrapper;
}

// De weergavenamen van één sidebar-sectie, in renderorde.
function sectionLaws(wrapper, key) {
  return wrapper
    .findAll(`nldd-list-item[data-section="${key}"]`)
    .map((item) => item.find('nldd-text-cell').attributes('text'));
}

// De sleutels van de secties met wetten, in renderorde, ontdubbeld.
function sectionOrder(wrapper) {
  const seen = [];
  for (const item of wrapper.findAll('nldd-list-item[data-section]')) {
    const key = item.attributes('data-section');
    if (!seen.includes(key)) seen.push(key);
  }
  return seen;
}

// De uitklapknop onder de traject-sectie ('Toon alle 21' / 'Toon minder').
function expander(wrapper) {
  return wrapper.find('nldd-button[data-testid="traject-laws-expander"]');
}

const TRAJECT_SECTION_TITLE = 'Traject';

beforeEach(() => {
  apiFetch.mockClear();
  apiFetchJson.mockClear();
  localStorage.clear();
  routeState.name = 'library-traject';
  routeState.params = { trajectRef: 'traject-abcd1234' };
  trajectScope.activeTrajectRef.value = 'traject-abcd1234';
  server.sources = ownSource();
  server.sourceLaws = SEVEN_LAWS;
  server.favorites = [];
  server.changed = [];
  server.sourcesFails = false;
  server.holdSources = null;
});

describe('LibraryView - wetten van het traject in het linkermenu', () => {
  it('toont alle wetten van de eigen bron, alfabetisch op weergavenaam', async () => {
    const wrapper = await mountLibrary();

    expect(wrapper.html()).toContain(TRAJECT_SECTION_TITLE);
    expect(sectionLaws(wrapper, 'traject')).toEqual([
      'Algemene wet bestuursrecht',
      'Besluit langdurige zorg',
      'Kieswet',
      'Participatiewet',
      'Regeling langdurige zorg',
      'Wet langdurige zorg',
      'Zorgverzekeringswet',
    ]);

    // Precies de afgesproken aanroep: alleen de eigen bron, met de bovengrens
    // als limiet.
    expect(apiFetchJson).toHaveBeenCalledWith(
      `/api/trajects/traject-abcd1234/corpus/laws?source=${OWN_SOURCE}&limit=200`,
    );
  });

  it('zet de sectie onderaan, na Recent bekeken', async () => {
    // Recent bekeken wordt per traject bewaard; de platte sleutel is legacy en
    // wordt bij het laden juist opgeruimd.
    localStorage.setItem(
      'regelrecht-recent-laws:traject-abcd1234',
      JSON.stringify([{ law_id: 'wet_a', name: 'Zorgverzekeringswet' }]),
    );
    server.favorites = ['wet_f'];
    server.changed = ['wet_c'];
    const wrapper = await mountLibrary();

    expect(sectionOrder(wrapper)).toEqual(['changed', 'favorites', 'recent', 'traject']);
  });

  it('laat een bewerkte wet in Bewerkt én in de traject-lijst staan', async () => {
    // Geen ontdubbeling: de traject-lijst is "alles wat in dit traject zit",
    // niet "de rest". Zou hij tegen Bewerkt filteren, dan springt een wet
    // eruit op het moment dat je hem bewerkt.
    server.changed = ['wet_c']; // Wet langdurige zorg
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'changed')).toEqual(['Wet langdurige zorg']);
    const traject = sectionLaws(wrapper, 'traject');
    expect(traject).toContain('Wet langdurige zorg');
    expect(traject).toHaveLength(SEVEN_LAWS.length);
  });

  it('laat een favoriet in beide secties staan (geen ontdubbeling tegen Favorieten)', async () => {
    // Zou de sectie ook favorieten wegfilteren, dan springt een wet eruit op
    // het moment dat je hem markeert - precies wat we niet willen.
    server.favorites = ['wet_f']; // Kieswet
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'traject')).toContain('Kieswet');
  });

  it('toont bij precies 20 wetten alles, zonder uitklapknop', async () => {
    server.sources = ownSource({ law_count: 20 });
    server.sourceLaws = manyLaws(20);
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
    expect(expander(wrapper).exists()).toBe(false);
  });

  it('klapt boven de 20 in, en de knop klapt heen en weer', async () => {
    server.sources = ownSource({ law_count: 21 });
    server.sourceLaws = manyLaws(21);
    const wrapper = await mountLibrary();

    let shown = sectionLaws(wrapper, 'traject');
    expect(shown).toHaveLength(20);
    expect(shown.at(-1)).toBe('Wet 020');
    expect(expander(wrapper).attributes('text')).toBe('Toon alle 21');

    await expander(wrapper).trigger('click');
    await nextTick();
    shown = sectionLaws(wrapper, 'traject');
    expect(shown).toHaveLength(21);
    expect(shown.at(-1)).toBe('Wet 021');
    expect(expander(wrapper).attributes('text')).toBe('Toon minder');

    await expander(wrapper).trigger('click');
    await nextTick();
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
    expect(expander(wrapper).attributes('text')).toBe('Toon alle 21');
  });

  it('vervangt de lijst boven de bovengrens door een hint met zoekknop', async () => {
    // Uitklappen naar duizenden regels is geen lijst maar een muur tekst;
    // daar verwijzen we naar het zoekvenster.
    server.sources = ownSource({ law_count: 201 });
    const wrapper = await mountLibrary();

    const html = wrapper.html();
    expect(html).not.toContain(TRAJECT_SECTION_TITLE);
    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(expander(wrapper).exists()).toBe(false);
    expect(html).toContain('Dit traject bevat 201 wetten.');
    expect(html).toContain('Zoek een wet');
    // Boven de bovengrens wordt de wettenlijst niet eens opgehaald.
    expect(
      apiFetchJson.mock.calls.some(([url]) => String(url).includes('source=')),
    ).toBe(false);
  });

  it('schrijft het aantal in de hint als Nederlands getal', async () => {
    server.sources = ownSource({ law_count: 1243 });
    const wrapper = await mountLibrary();
    expect(wrapper.html()).toContain('Dit traject bevat 1.243 wetten.');
  });

  it('toont de hint in de sidebar in plaats van de paginabrede lege staat', async () => {
    // Zonder favorieten, recent bekeken of bewerkte wetten zijn er nul
    // secties. Zou `isEmptyLibrary` de hint niet meetellen, dan verving de
    // lege staat de split-view en zag de gebruiker de hint nooit - precies
    // het scenario dat dit repareert.
    server.sources = ownSource({ law_count: 201 });
    server.favorites = [];
    server.changed = [];
    const wrapper = await mountLibrary();

    expect(wrapper.find('nldd-navigation-split-view').exists()).toBe(true);
    expect(wrapper.html()).toContain('Dit traject bevat 201 wetten.');
  });

  it('toont niets bij een lege eigen bron', async () => {
    server.sources = ownSource({ law_count: 0 });
    server.sourceLaws = [];
    const wrapper = await mountLibrary();

    const html = wrapper.html();
    expect(html).not.toContain(TRAJECT_SECTION_TITLE);
    expect(html).not.toContain('Dit traject bevat');
  });

  it('toont niets als de eigen bron niet gescand kon worden', async () => {
    // De bestaande index-error-surface doet daar het werk; een sectie of
    // hint erbovenop zou de storing juist verhullen.
    server.sources = ownSource({ law_count: 0, index_error: 'Trees API 409' });
    const wrapper = await mountLibrary();

    const html = wrapper.html();
    expect(html).not.toContain(TRAJECT_SECTION_TITLE);
    expect(html).not.toContain('Dit traject bevat');
  });

  it('houdt de sectie weg als de bronnen-aanroep faalt, zonder fout in het menu', async () => {
    server.sourcesFails = true;
    const wrapper = await mountLibrary();

    const html = wrapper.html();
    expect(html).not.toContain(TRAJECT_SECTION_TITLE);
    expect(html).not.toContain('Dit traject bevat');
    // Geen foutmelding: dezelfde stille stance als `fetchChangedLawIds`.
    expect(html).not.toContain('Wetten en regels zijn niet geladen');
    expect(wrapper.find('nldd-banner.corpus-warning').exists()).toBe(false);
  });

  it('laat na een traject-wissel geen lijst van het vorige traject staan', async () => {
    // Een favoriet houdt de split-view overeind terwijl het nieuwe traject
    // laadt, zodat de assertie hieronder over een zichtbaar menu gaat en niet
    // over een paginabrede lege staat.
    server.favorites = ['wet_f'];
    const wrapper = await mountLibrary();
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(7);

    // Het volgende traject heeft een lege eigen bron, en laat zijn
    // bronnen-aanroep hangen. Het venster tussen de wissel en het antwoord is
    // precies waar de reset voor is: zonder die reset staan de zeven wetten
    // van het vorige traject hier nog in het menu van het nieuwe.
    let releaseSources;
    server.holdSources = new Promise((resolve) => {
      releaseSources = resolve;
    });
    server.sources = ownSource({ law_count: 0 });
    server.sourceLaws = [];
    routeState.params = { trajectRef: 'traject-99998888' };
    trajectScope.activeTrajectRef.value = 'traject-99998888';
    for (let i = 0; i < 4; i++) await nextTick();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(wrapper.html()).not.toContain(TRAJECT_SECTION_TITLE);

    // En na het antwoord blijft het weg - de nieuwe bron is leeg.
    releaseSources();
    for (let i = 0; i < 8; i++) await nextTick();

    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(wrapper.html()).not.toContain(TRAJECT_SECTION_TITLE);
  });
});
