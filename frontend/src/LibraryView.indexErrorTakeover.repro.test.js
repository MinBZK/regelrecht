// REPRO (bug report 2026-07-29 "taakmenu crasht na ophalen wetgeving in
// private repo"): when the traject-scoped corpus listing fails (502 because
// the traject's writable-own repo could not be scanned), LibraryView's
// top-level `v-if="indexError"` pane takes over the WHOLE route - including
// /taken and /instellingen/details, which do not read the corpus index at all.
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

describe('LibraryView index-error takeover (repro)', () => {
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

  it('replaces the TAKEN route with the corpus-error page', async () => {
    const wrapper = await mountAfterFailedIndex();
    const html = wrapper.html();
    // What the user sees instead of their task list:
    expect(html).toContain('Wetten en regels zijn niet geladen');
    // ...and the taken panes are gone entirely.
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(false);
    expect(wrapper.findComponent({ name: 'TasksListPane' }).exists()).toBe(false);
  });

  it('renders the taken panes first and only then loses them (the reported flash)', async () => {
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
    // ...and is replaced the moment the unrelated corpus call fails.
    expect(wrapper.findComponent({ name: 'TasksCategoriesPane' }).exists()).toBe(false);
    expect(wrapper.html()).toContain('Wetten en regels zijn niet geladen');
  });

  it('replaces the INSTELLINGEN route with the corpus-error page', async () => {
    routeState.name = 'instellingen-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', tab: 'details' };
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.html()).toContain('Wetten en regels zijn niet geladen');
    expect(wrapper.findComponent({ name: 'TrajectDetailsPane' }).exists()).toBe(false);
    routeState.name = 'taken-traject';
    routeState.params = { trajectRef: 'traject-abcd1234', categorie: 'alle' };
  });

  it('surfaces the backend explanation while it fits the 300-char cap', async () => {
    const wrapper = await mountAfterFailedIndex();
    expect(wrapper.html()).toContain('traject-repo kon niet worden gescand');
  });

  it('falls back to the generic text once GitHub\'s JSON body pushes it past 300 chars', async () => {
    // What a real 409 "Git Repository is empty" looks like once the Trees
    // error body is interpolated into the 502 message — which is the generic
    // "De gegevens konden niet worden opgehaald." the bug report shows.
    const githubBody =
      '{"message":"Git Repository is empty.","documentation_url":' +
      '"https://docs.github.com/rest/git/trees/trees#get-a-tree","status":"409"}';
    const long =
      'De bibliotheek van dit traject is niet beschikbaar: de traject-repo kon niet ' +
      `worden gescand (GitHub API error 409: Trees API for rijksoverheid-org/afgeschermde-test-kandidaatstellingsprocedure-kieswet@main: ${githubBody})`;
    expect(long.length).toBeGreaterThan(300);
    apiFetch.mockImplementation(async (url) => {
      if (String(url).includes('/corpus/laws')) {
        throw new ApiErrorStub('Failed to load corpus: 502', { status: 502, body: long });
      }
      return { ok: true, status: 200, json: async () => [] };
    });
    const wrapper = await mountAfterFailedIndex();
    const html = wrapper.html();
    expect(html).toContain('De gegevens konden niet worden opgehaald');
    expect(html).not.toContain('traject-repo kon niet worden gescand');
  });
});
