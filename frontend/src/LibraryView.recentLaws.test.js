// "Recent bekeken" is stored per traject: two trajects hold different laws, so
// a law opened in the one must not turn up under the other, where that id may
// not even resolve. Mounts LibraryView `shallow` with the same composable stubs
// LibraryView.docReview.test.js uses - deliberately narrow, this file only
// exercises the recent-laws surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref } from 'vue';

const routeState = {
  name: 'library-traject',
  params: { trajectRef: 'traject-aaaa1111', docPath: '' },
  query: {},
  fullPath: '/traject/traject-aaaa1111/bibliotheek',
  hash: '',
};
vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({ replace: vi.fn(), push: vi.fn(), resolve: () => ({ href: '#' }) }),
  onBeforeRouteUpdate: vi.fn(),
  onBeforeRouteLeave: vi.fn(),
}));

vi.mock('./composables/useAuth.js', () => ({
  useAuth: () => ({ authenticated: ref(true), login: vi.fn() }),
}));

// The ref the view watches: switching traject keeps the view mounted, so the
// test drives the switch by writing to this exact ref.
const activeTrajectRef = ref('traject-aaaa1111');
vi.mock('./composables/useTrajects.js', () => ({
  useTrajects: () => ({
    activeTrajectRef,
    activeTraject: ref({ name: 'Traject A' }),
  }),
  refreshTrajects: vi.fn(),
}));

vi.mock('./lib/apiFetch.js', () => ({
  apiFetch: vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => [] }),
  apiFetchJson: vi.fn().mockResolvedValue([]),
  ApiError: class ApiError extends Error {},
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
    displayTitle: (p) => p,
    dropDraft: vi.fn(),
  }),
}));

vi.mock('./composables/useTrajectDocumentJobs.js', () => ({
  useTrajectDocumentJobs: () => ({
    jobs: ref([]),
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
    loadReview: vi.fn(),
    approveAfterSave: vi.fn(),
    reject: vi.fn(),
  }),
}));

import LibraryView from './LibraryView.vue';

const KEY = 'regelrecht-recent-laws';
const mountLibrary = () => shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
const stored = (trajectRef) => JSON.parse(localStorage.getItem(`${KEY}:${trajectRef}`) || '[]');

beforeEach(() => {
  localStorage.clear();
  activeTrajectRef.value = 'traject-aaaa1111';
  routeState.params = { trajectRef: 'traject-aaaa1111', docPath: '' };
});

describe('LibraryView recent bekeken', () => {
  it('bewaart een bekeken wet onder de sleutel van het actieve traject', async () => {
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('toont de lijst van het andere traject na een wissel, niet die van het eerste', async () => {
    localStorage.setItem(`${KEY}:traject-bbbb2222`, JSON.stringify([{ law_id: 'wet_b', name: 'Wet B' }]));
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();

    activeTrajectRef.value = 'traject-bbbb2222';
    await wrapper.vm.$nextTick();

    expect(wrapper.vm.recentLaws.map(r => r.law_id)).toEqual(['wet_b']);
    // En het eerste traject houdt zijn eigen lijst.
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);
  });

  it('houdt de globale corpus apart van een traject', async () => {
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();

    activeTrajectRef.value = null;
    await wrapper.vm.$nextTick();
    expect(wrapper.vm.recentLaws).toEqual([]);

    wrapper.vm.recordRecentLaw('wet_c', 'Wet C');
    await wrapper.vm.$nextTick();
    expect(stored('corpus').map(r => r.law_id)).toEqual(['wet_c']);
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);
  });

  it('wist bij "wis recent" alleen het actieve traject', async () => {
    localStorage.setItem(`${KEY}:traject-bbbb2222`, JSON.stringify([{ law_id: 'wet_b', name: 'Wet B' }]));
    const wrapper = mountLibrary();
    wrapper.vm.recordRecentLaw('wet_a', 'Wet A');
    await wrapper.vm.$nextTick();

    wrapper.vm.clearRecent();
    await wrapper.vm.$nextTick();

    expect(wrapper.vm.recentLaws).toEqual([]);
    expect(localStorage.getItem(`${KEY}:traject-aaaa1111`)).toBeNull();
    expect(stored('traject-bbbb2222').map(r => r.law_id)).toEqual(['wet_b']);
  });

  // De wet blijft bij een wissel gewoon openstaan, en de naam wordt opnieuw
  // opgelost, dus de watch die de lijst bijhoudt vuurt nog een keer. Dat is een
  // wissel en geen bezoek.
  it('schrijft de openstaande wet niet in de lijst van het traject waar je heen wisselt', async () => {
    const wrapper = mountLibrary();
    wrapper.vm.laws = [{ law_id: 'wet_a', name: 'Wet A in traject' }];
    wrapper.vm.selectedLawId = 'wet_a';
    await wrapper.vm.$nextTick();
    expect(stored('traject-aaaa1111').map(r => r.law_id)).toEqual(['wet_a']);

    // Wat de wissel in het echt doet: de corpus laadt opnieuw, dus de naam van
    // de nog openstaande wet lost opnieuw op en de watch vuurt.
    activeTrajectRef.value = null;
    wrapper.vm.laws = [{ law_id: 'wet_a', name: 'Wet A in de corpus' }];
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();

    expect(stored('corpus')).toEqual([]);
    expect(wrapper.vm.recentLaws).toEqual([]);
  });

  it('gooit de oude ongescopete lijst weg', async () => {
    localStorage.setItem(KEY, JSON.stringify([{ law_id: 'wet_oud', name: 'Oud' }]));
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    expect(localStorage.getItem(KEY)).toBeNull();
    expect(wrapper.vm.recentLaws).toEqual([]);
  });
});
