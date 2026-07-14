// Targeted tests for the document-review orchestration in LibraryView.vue
// (rejectDocReview / onDocSaved / clearDocReviewQuery / the proposal-seeding
// watch, ~lines 240-380). LibraryView.vue itself has no other test coverage
// (it's a 1500-line multi-concern view with no existing test harness), so
// this file mounts it `shallow` with every composable it touches replaced by
// a controllable stub - deliberately narrow: it exercises only the
// review-flow surface, not the rest of the view (laws browsing, search,
// traject panes, ...), which stay in their default/empty state.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref } from 'vue';

// --- vue-router: no real router install. `onBeforeRouteUpdate`/
// `onBeforeRouteLeave` are just named exports LibraryView calls once at
// setup to register a callback - stubbing them as no-ops absorbs that
// registration without needing real guard machinery (same technique
// TasksSheet.test.js / MobileTrajectSheet.test.js use for useRoute/useRouter).
const routeState = {
  name: 'werkdocumenten-traject',
  params: { trajectRef: 'traject-abcd1234', docPath: '' },
  query: {},
  fullPath: '/traject/traject-abcd1234/werkdocumenten',
};
const replaceMock = vi.fn();
const pushMock = vi.fn();
vi.mock('vue-router', () => ({
  useRoute: () => routeState,
  useRouter: () => ({ replace: replaceMock, push: pushMock, resolve: () => ({ href: '#' }) }),
  onBeforeRouteUpdate: vi.fn(),
  onBeforeRouteLeave: vi.fn(),
}));

vi.mock('./composables/useAuth.js', () => ({
  useAuth: () => ({ authenticated: ref(true), login: vi.fn() }),
}));

vi.mock('./composables/useTrajects.js', () => ({
  useTrajects: () => ({
    activeTrajectRef: ref('traject-abcd1234'),
    activeTraject: ref({ name: 'Testtraject' }),
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

// The manager stub deliberately keeps `currentPath`/`currentBody`/`docLoading`/
// `docError` as module-level refs so tests can drive the exact state the
// review-flow watch reacts to, and assert on `dropDraft`/`open` (aliased
// `openDoc` in LibraryView) being called (or not).
const openDoc = vi.fn().mockResolvedValue(undefined);
const dropDraft = vi.fn();
const currentPath = ref('report.md');
const currentBody = ref('');
const docLoading = ref(false);
const docError = ref(null);
vi.mock('./composables/useDocumentsManager.js', () => ({
  useDocumentsManager: () => ({
    documents: ref([]),
    listLoading: ref(false),
    listError: ref(null),
    currentPath,
    currentBody,
    hasChanges: ref(false),
    docLoading,
    docError,
    saving: ref(false),
    open: openDoc,
    startNew: vi.fn(),
    close: vi.fn(),
    uploadDocument: vi.fn(),
    displayTitle: (p) => p,
    dropDraft,
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

// The composable under test's caller-side orchestration: kept as
// module-level refs (identity-equal to what LibraryView destructures) so
// tests can set `reviewTask.value` / `proposedContent.value` directly and
// assert on `loadError.value` after a failed resolve.
const reviewTask = ref(null);
const proposedContent = ref(null);
const loadError = ref(null);
const loadReview = vi.fn();
const approveAfterSave = vi.fn();
const rejectTask = vi.fn();
vi.mock('./composables/useDocumentTaskReview.js', () => ({
  useDocumentTaskReview: () => ({
    reviewTask,
    proposedContent,
    loadError,
    loadReview,
    approveAfterSave,
    reject: rejectTask,
  }),
}));

import LibraryView from './LibraryView.vue';

function mountLibrary() {
  return shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
}

beforeEach(() => {
  reviewTask.value = null;
  proposedContent.value = null;
  loadError.value = null;
  loadReview.mockReset().mockResolvedValue(undefined);
  approveAfterSave.mockReset();
  rejectTask.mockReset();
  openDoc.mockReset().mockResolvedValue(undefined);
  dropDraft.mockReset();
  replaceMock.mockReset().mockReturnValue(Promise.resolve());
  pushMock.mockReset();
  currentPath.value = 'report.md';
  currentBody.value = '';
  docLoading.value = false;
  docError.value = null;
  routeState.name = 'werkdocumenten-traject';
  routeState.params = { trajectRef: 'traject-abcd1234', docPath: '' };
  routeState.query = {};
});

function openReview(overrides = {}) {
  reviewTask.value = {
    id: 't1',
    payload: { target_path: 'report.md', traject_ref: 'traject-abcd1234', ...overrides },
  };
  routeState.query = { task: 't1' };
}

describe('LibraryView document-review flow', () => {
  // Fix 1: "Verwerpen" must throw away the seeded draft before reopening the
  // document - otherwise the debounced localStorage persistence in
  // useTrajectDocuments resurrects the rejected proposal as a 'draft-present'
  // notice (or leaves an orphan draft for a document that never existed).
  it('rejectDocReview drops the draft before reopening the document, then clears the task query', async () => {
    rejectTask.mockResolvedValue(undefined);
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    const order = [];
    dropDraft.mockImplementation(() => order.push('dropDraft'));
    openDoc.mockImplementation(() => {
      order.push('openDoc');
      return Promise.resolve();
    });

    await wrapper.vm.rejectDocReview();

    expect(rejectTask).toHaveBeenCalled();
    expect(order).toEqual(['dropDraft', 'openDoc']);
    expect(openDoc).toHaveBeenCalledWith('report.md');
    expect(replaceMock).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'werkdocumenten-traject' }),
    );
  });

  // Fix 6: a failed reject-resolve must not be treated as a successful
  // reject - the draft/document state stays untouched and the failure is
  // surfaced, not silently swallowed.
  it('rejectDocReview leaves the draft and task open when the resolve call fails', async () => {
    rejectTask.mockRejectedValue(new Error('network'));
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await wrapper.vm.rejectDocReview();

    expect(dropDraft).not.toHaveBeenCalled();
    expect(openDoc).not.toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
    expect(loadError.value).toMatch(/mislukt/i);
  });

  // Fix 2 + Fix 4: onDocSaved only approves when the saved path AND traject
  // match the task under review.
  it('onDocSaved approves and clears the task query when the saved path matches', async () => {
    approveAfterSave.mockResolvedValue(undefined);
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await wrapper.vm.onDocSaved('report.md');

    expect(approveAfterSave).toHaveBeenCalled();
    expect(replaceMock).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'werkdocumenten-traject' }),
    );
  });

  it('onDocSaved ignores a save of a different path (e.g. renamed away from target_path)', async () => {
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await wrapper.vm.onDocSaved('other.md');

    expect(approveAfterSave).not.toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  it('onDocSaved ignores a review task that belongs to a different traject', async () => {
    openReview({ traject_ref: 'ander-traject-9999zzzz' });
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await wrapper.vm.onDocSaved('report.md');

    expect(approveAfterSave).not.toHaveBeenCalled();
  });

  // Fix 6: a failed approve-resolve must not crash the (already-succeeded)
  // save flow; the task simply stays open.
  it('onDocSaved leaves the task open without crashing when the resolve call fails', async () => {
    approveAfterSave.mockRejectedValue(new Error('network'));
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    await expect(wrapper.vm.onDocSaved('report.md')).resolves.toBeUndefined();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  // Fix 3: a save-and-leave already lets the user's chosen navigation
  // through before the approve-resolve settles; clearDocReviewQuery must not
  // drag them back once the route has moved off the werkdocumenten/task URL.
  it('clearDocReviewQuery does not stomp a navigation the user already made', async () => {
    approveAfterSave.mockResolvedValue(undefined);
    openReview();
    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();

    routeState.name = 'library-traject';
    await wrapper.vm.onDocSaved('report.md');

    expect(approveAfterSave).toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  // Fix 2: the seeding `.then()` callback must re-validate against the
  // task's own payload before writing into the editor body - a stale
  // response arriving after the user has moved to another document must not
  // seed the wrong one.
  it('seeds the proposal into the document body when the task matches the open document', async () => {
    openReview();
    loadReview.mockImplementation(() => {
      proposedContent.value = '# Voorstel';
      return Promise.resolve();
    });

    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();

    expect(loadReview).toHaveBeenCalledWith('t1');
    expect(currentBody.value).toBe('# Voorstel');
  });

  it('skips seeding when the open document changed while the task fetch was in flight', async () => {
    openReview();
    let resolveLoad;
    loadReview.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveLoad = resolve;
        }),
    );

    const wrapper = mountLibrary();
    await wrapper.vm.$nextTick();
    expect(loadReview).toHaveBeenCalledWith('t1');

    // The user navigated to a different document before the fetch resolved.
    currentPath.value = 'other.md';
    proposedContent.value = '# Voorstel';
    resolveLoad();
    await wrapper.vm.$nextTick();
    await wrapper.vm.$nextTick();

    expect(currentBody.value).toBe('');
  });
});
