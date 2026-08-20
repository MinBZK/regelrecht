// De sectie "In dit traject" in het linkermenu.
//
// Die toont wat dit traject zelf heeft aangeraakt: de diff van de trajectbranch
// tegen zijn base. Bewerken en een wet hierheen halen gaan allebei via het
// writable-own schrijfpad, dus allebei staan ze erin. Wat de branch van zijn
// base erfde staat er bewust niet in: een traject op de centrale corpus zou
// anders duizenden wetten tonen die niemand hier heeft aangeraakt.
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
    // De catalogus waaruit `?ids=` zijn metadata haalt.
    sourceLaws: [],
    favorites: [],
    changed: [],
    // Een belofte die de changed-laws-aanroep laat hangen, zodat een test kan
    // zien wat het menu toont terwijl het antwoord van het nieuwe traject nog
    // onderweg is. `null` = meteen antwoorden.
    holdChanged: null,
  };
  // Metadata voor `?ids=`: de sidebar resolvet favorieten + aangeraakte wetten
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
    if (u.includes('/changed-laws')) return server.changed;
    // Favorieten zijn per plek: `/api/favorites` in Corpus juris,
    // `/api/trajects/{ref}/favorites` in een traject. Beide eindigen op
    // `/favorites`, dus dat is genoeg om ze hier te herkennen.
    if (u.endsWith('/favorites')) return server.favorites;
    if (u.includes('/corpus/laws')) return idsFrom(u).map(byId);
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
      if (server.holdChanged && String(url).includes('/changed-laws')) await server.holdChanged;
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

// De uitklapknop onder "In dit traject" ('Toon alle 21' / 'Toon minder').
function expander(wrapper) {
  return wrapper.find('nldd-button[data-testid="traject-laws-expander"]');
}

const TRAJECT_SECTION_TITLE = 'In dit traject';

// De koppen van de secties die echt gerenderd zijn. Bewust niet via
// `wrapper.html()`: die bevat ook de HTML-comments uit het template, dus een
// comment die de sectie bij naam noemt zou zo'n assertie laten slagen of falen
// om de verkeerde reden.
function sectionTitles(wrapper) {
  return wrapper.findAll('nldd-title h4').map((h) => h.text());
}

// De lege staat onder de kop: "nog niets, en dit kun je eraan doen".
function noLawsText(wrapper) {
  return wrapper.find('nldd-rich-text[data-testid="traject-no-laws"]');
}

beforeEach(() => {
  apiFetch.mockClear();
  apiFetchJson.mockClear();
  localStorage.clear();
  routeState.name = 'library-traject';
  routeState.params = { trajectRef: 'traject-abcd1234' };
  trajectScope.activeTrajectRef.value = 'traject-abcd1234';
  server.sourceLaws = SEVEN_LAWS;
  server.favorites = [];
  server.changed = SEVEN_LAWS.map((l) => l.law_id);
  server.holdChanged = null;
});

describe('LibraryView - wat dit traject heeft aangeraakt, in het linkermenu', () => {
  it('toont de aangeraakte wetten, alfabetisch op weergavenaam', async () => {
    const wrapper = await mountLibrary();

    expect(sectionTitles(wrapper)).toContain(TRAJECT_SECTION_TITLE);
    expect(sectionLaws(wrapper, 'traject')).toEqual([
      'Algemene wet bestuursrecht',
      'Besluit langdurige zorg',
      'Kieswet',
      'Participatiewet',
      'Regeling langdurige zorg',
      'Wet langdurige zorg',
      'Zorgverzekeringswet',
    ]);
  });

  it('laat wat de branch erfde maar niet aanraakte buiten het menu', async () => {
    // De catalogus houdt zeven wetten; de branch raakte er twee aan. De andere
    // vijf horen bij wat het traject van zijn base erfde en zijn niet zijn werk.
    server.changed = ['wet_c', 'wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual(['Kieswet', 'Wet langdurige zorg']);
  });

  it('zet de sectie onder Favorieten en boven Recent bekeken', async () => {
    // Recent bekeken wordt per traject bewaard; de platte sleutel is legacy en
    // wordt bij het laden juist opgeruimd.
    localStorage.setItem(
      'regelrecht-recent-laws:traject-abcd1234',
      JSON.stringify([{ law_id: 'wet_a', name: 'Zorgverzekeringswet' }]),
    );
    server.favorites = ['wet_f'];
    server.changed = ['wet_c'];
    const wrapper = await mountLibrary();

    expect(sectionOrder(wrapper)).toEqual(['favorites', 'traject', 'recent']);
  });

  it('toont de kop met een lege staat als er nog niets is aangeraakt', async () => {
    // De lege staat is de sectie in wording: dezelfde kop, met eronder wat je
    // moet doen om hem te vullen. Geen rijen dus, wel de kop.
    server.changed = [];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(sectionTitles(wrapper)).toContain(TRAJECT_SECTION_TITLE);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });

  it('houdt het trajectmenu overeind als er nog niets is aangeraakt', async () => {
    // De paginabrede lege staat vervangt de hele split-view. Sloeg die aan in
    // een traject, dan verdween met de wetten ook Instellingen, Werkdocumenten
    // en Taken, en bleef er een lege pagina over.
    server.changed = [];
    server.favorites = [];
    const wrapper = await mountLibrary();

    expect(wrapper.find('nldd-navigation-split-view').exists()).toBe(true);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });

  it('toont de lege staat ook naast een favoriet', async () => {
    // De kop hoort bij deze sectie, niet bij "het menu is leeg": dat je iets
    // bewaard of bekeken hebt zegt niets over wat dit traject heeft gedaan.
    server.changed = [];
    server.favorites = ['wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });

  it('laat een aangeraakte favoriet in beide secties staan', async () => {
    // Geen ontdubbeling: een favoriet die je hier ook bewerkte hoort in allebei
    // thuis, dezelfde houding die Recent bekeken al aanneemt.
    server.favorites = ['wet_f'];
    server.changed = ['wet_f'];
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
  });

  it('toont bij precies 20 wetten alles, zonder uitklapknop', async () => {
    server.sourceLaws = manyLaws(20);
    server.changed = server.sourceLaws.map((l) => l.law_id);
    const wrapper = await mountLibrary();

    expect(sectionLaws(wrapper, 'traject')).toHaveLength(20);
    expect(expander(wrapper).exists()).toBe(false);
  });

  it('klapt boven de 20 in, en de knop klapt heen en weer', async () => {
    server.sourceLaws = manyLaws(21);
    server.changed = server.sourceLaws.map((l) => l.law_id);
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

  it('laat na een traject-wissel geen lijst van het vorige traject staan', async () => {
    // Een favoriet houdt de split-view overeind terwijl het nieuwe traject
    // laadt, zodat de assertie hieronder over een zichtbaar menu gaat en niet
    // over een paginabrede lege staat.
    server.favorites = ['wet_f'];
    const wrapper = await mountLibrary();
    expect(sectionLaws(wrapper, 'traject')).toHaveLength(7);

    // Het volgende traject heeft nog niets aangeraakt, en laat zijn
    // changed-laws-aanroep hangen. Het venster tussen de wissel en het antwoord
    // is precies waar de reset voor is: zonder die reset staan de zeven wetten
    // van het vorige traject hier nog in het menu van het nieuwe.
    let releaseChanged;
    server.holdChanged = new Promise((resolve) => {
      releaseChanged = resolve;
    });
    server.changed = [];
    routeState.params = { trajectRef: 'traject-99998888' };
    trajectScope.activeTrajectRef.value = 'traject-99998888';
    for (let i = 0; i < 4; i++) await nextTick();

    expect(sectionLaws(wrapper, 'favorites')).toEqual(['Kieswet']);
    expect(sectionLaws(wrapper, 'traject')).toEqual([]);

    // En na het antwoord staat er de lege staat: het nieuwe traject raakte niets
    // aan. De favoriet ernaast doet daar niets aan af, want die zegt niets over
    // wat dit traject heeft gedaan.
    releaseChanged();
    for (let i = 0; i < 8; i++) await nextTick();

    expect(sectionLaws(wrapper, 'traject')).toEqual([]);
    expect(noLawsText(wrapper).exists()).toBe(true);
  });
});
