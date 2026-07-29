// Guards the fix for the 2026-07-29 bug report "taakmenu crasht na ophalen
// wetgeving in private repo": when the traject-scoped corpus listing fails
// (502 because the traject's writable-own repo could not be scanned),
// LibraryView's top-level index-error pane used to take over the WHOLE route -
// including /taken and /instellingen/details, which do not read the corpus
// index at all. After the fix the takeover is gated to library mode; the
// non-library modes keep their panes and get a non-blocking warning banner,
// and the backend's own explanation is surfaced instead of a generic fallback.
//
// Harness copied from LibraryView.conversionJobs.test.js: shallow-mount with
// every composable stubbed, deliberately narrow to this one surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref, nextTick } from 'vue';

const routeState = {
  name: 'taken-traject',
  params: { trajectRef: 'traject-abcd1234', categorie: 'alle' },
  query: {},
  fullPath: '/trajecten/traject-abcd1234/taken',
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

vi.mock('./composables/useTrajects.js', () => ({
  useTrajects: () => ({
    activeTrajectRef: ref('traject-abcd1234'),
    activeTraject: ref({ name: 'Afgeschermde test Kandidaatstellingsprocedure Kieswet' }),
  }),
  refreshTrajects: vi.fn(),
}));

// The 502 body below is the backend's own text for a traject whose
// writable-own (private) repo could not be scanned - see
// `require_traject_index` in corpus_handlers.rs.
//
// `vi.mock` factories are hoisted above module-level consts, so the shared
// stubs have to be hoisted too.
const { ApiErrorStub, apiFetch, apiFetchJson } = vi.hoisted(() => {
  class ApiErrorStub extends Error {
    constructor(message, { status, body = '' } = {}) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
      this.body = body;
    }
  }
  const BODY =
    'De bibliotheek van dit traject is niet beschikbaar: de traject-repo kon niet ' +
    'worden gescand (Trees API for org/afgeschermd-traject@main: 409)';
  const fail = () => {
    throw new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: BODY });
  };
  // Every traject-scoped corpus call fails; favorites/changed-laws resolve
  // (they swallow their own errors anyway, so failing them would prove nothing
  // about the takeover).
  return {
    ApiErrorStub,
    apiFetch: vi.fn(async (url) => {
      if (String(url).includes('/corpus/laws')) fail();
      return { ok: true, status: 200, json: async () => [] };
    }),
    apiFetchJson: vi.fn(async (url) => {
      if (String(url).includes('/corpus/laws')) fail();
      return [];
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

async function mountAfterFailedIndex() {
  const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
  await nextTick();
  await nextTick();
  await nextTick();
  return wrapper;
}

const defaultApiFetch = apiFetch.getMockImplementation();

beforeEach(() => {
  apiFetch.mockClear().mockImplementation(defaultApiFetch);
  apiFetchJson.mockClear();
  routeState.name = 'taken-traject';
  routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  routeState.query = {};
});

// The warning banner LibraryView now raises over the non-library modes.
const BANNER_TEXT = 'Wetten en regels van dit traject zijn niet geladen';
// The fullscreen takeover that still owns the library routes themselves.
const FULLSCREEN_TEXT = 'Wetten en regels zijn niet geladen';

describe('LibraryView index-error scoping', () => {
  it('probes the traject corpus even with nothing curated (fresh private repo)', async () => {
    await mountAfterFailedIndex();
    // No favorites, no changed laws => the ids set is empty, but LibraryView
    // still probes `?limit=1` so a broken traject repo is not shown as a
    // normal-looking empty library.
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/trajects/traject-abcd1234/corpus/laws?limit=1',
      expect.anything(),
    );
  });

  it('keeps the TAKEN route standing and shows a non-blocking warning banner', async () => {
    const wrapper = await mountAfterFailedIndex();
    // The task panes survive the unrelated 502 - the user can still work.
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
    expect(wrapper.findComponent({ name: 'TasksListPane' }).exists()).toBe(true);
    const html = wrapper.html();
    // ...with a warning banner instead of the fullscreen takeover.
    expect(html).toContain(BANNER_TEXT);
    const banner = wrapper.find('nldd-banner.corpus-warning');
    expect(banner.exists()).toBe(true);
    expect(banner.attributes('variant')).toBe('warning');
  });

  it('keeps the taken panes even when the corpus call fails mid-flight (no flash)', async () => {
    let rejectCorpus;
    apiFetch.mockImplementation(async (url) => {
      if (String(url).includes('/corpus/laws')) {
        return new Promise((_, rej) => {
          rejectCorpus = () =>
            rej(new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: 'kapot' }));
        });
      }
      return { ok: true, status: 200, json: async () => [] };
    });
    const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
    await nextTick();
    await nextTick();
    // The task list is on screen while the corpus probe is still in flight...
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
    rejectCorpus();
    for (let i = 0; i < 5; i++) await nextTick();
    // ...and it stays put once the unrelated corpus call fails - only the
    // warning banner appears on top.
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(true);
    expect(wrapper.html()).toContain(BANNER_TEXT);
  });

  it('keeps the INSTELLINGEN route standing under a corpus 502', async () => {
    routeState.name = 'instellingen-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', tab: 'details' };
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.findComponent({ name: 'TrajectDetailsPane' }).exists()).toBe(true);
    expect(wrapper.html()).toContain(BANNER_TEXT);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  it('still shows the fullscreen index-error page on the library route itself', async () => {
    routeState.name = 'library-traject';
    routeState.params = { trajectRef: 'traject-abcd1234' };
    const wrapper = await mountAfterFailedIndex();
    const html = wrapper.html();
    // The library route DOES depend on the index, so it keeps the fullscreen
    // takeover (unchanged) - and does not fall back to the warning banner.
    expect(html).toContain(FULLSCREEN_TEXT);
    expect(wrapper.find('nldd-banner.corpus-warning').exists()).toBe(false);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  it('surfaces the backend explanation while it fits the cap', async () => {
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.html()).toContain('traject-repo kon niet worden gescand');
  });

  it('still surfaces the backend reason once GitHub\'s JSON body pushes it long', async () => {
    // What a real 409 "Git Repository is empty" looks like once the Trees
    // error body is interpolated into the 502 message. The old 300-char cap
    // dropped this whole sentence to the useless generic fallback (the bug
    // report's screenshots); it must now come through, clamped.
    const githubBody =
      '{"message":"Git Repository is empty.","documentation_url":' +
      '"https://docs.github.com/rest/git/trees/trees#get-a-tree","status":"409"}';
    const long =
      'De bibliotheek van dit traject is niet beschikbaar: de traject-repo kon niet ' +
      `worden gescand (GitHub API error 409: Trees API for example-org/regelrecht-corpus-example@main: ${githubBody})`;
    expect(long.length).toBeGreaterThan(300);
    apiFetch.mockImplementation(async (url) => {
      if (String(url).includes('/corpus/laws')) {
        throw new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: long });
      }
      return { ok: true, status: 200, json: async () => [] };
    });
    const wrapper = await mountAfterFailedIndex();
    const html = wrapper.html();
    // The reason reaches the user instead of "De gegevens konden niet worden opgehaald."
    expect(html).toContain('traject-repo kon niet worden gescand');
    expect(html).not.toContain('De gegevens konden niet worden opgehaald');
  });
});
