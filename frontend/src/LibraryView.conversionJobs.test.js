// Targeted tests for the conversion-jobs watcher in LibraryView.vue: what
// happens to a job the user is viewing when it leaves the polled list.
//
// The endpoint only returns pending/processing jobs once tasks.job_review is on
// (failures become a job_failed task instead - see list_traject_document_jobs'
// include_failed), so "gone from the list" means completed OR failed. Only a
// written .md tells them apart. Same harness technique as
// LibraryView.docReview.test.js: shallow-mount with every composable stubbed,
// deliberately narrow to this one surface.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { shallowMount } from '@vue/test-utils';
import { ref, nextTick } from 'vue';

const routeState = {
  name: 'werkdocumenten-traject',
  params: { trajectRef: 'traject-abcd1234', docPath: '' },
  query: {},
  fullPath: '/traject/traject-abcd1234/werkdocumenten',
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

// Module-level so each test can drive the exact state the watcher reacts to.
const documents = ref([]);
const openDoc = vi.fn().mockResolvedValue(undefined);
const closeDoc = vi.fn();
const refreshList = vi.fn().mockResolvedValue(undefined);
vi.mock('./composables/useDocumentsManager.js', () => ({
  useDocumentsManager: () => ({
    documents,
    listLoading: ref(false),
    listError: ref(null),
    currentPath: ref(null),
    currentBody: ref(''),
    hasChanges: ref(false),
    docLoading: ref(false),
    docError: ref(null),
    saving: ref(false),
    open: openDoc,
    startNew: vi.fn(),
    close: closeDoc,
    uploadDocument: vi.fn(),
    displayTitle: (p) => String(p ?? '').replace(/\.md$/, ''),
    dropDraft: vi.fn(),
    refreshList,
  }),
}));

const jobs = ref([]);
const cancelJob = vi.fn();
vi.mock('./composables/useTrajectDocumentJobs.js', () => ({
  useTrajectDocumentJobs: () => ({
    jobs,
    cancelJob,
    refresh: vi.fn(),
    startPolling: vi.fn(),
    stopPolling: vi.fn(),
  }),
}));

// Capture the onUploaded callback LibraryView hands in, so a test can fire it
// with an upload result the way a real successful upload would.
let onUploaded = null;
vi.mock('./composables/useDocumentUpload.js', () => ({
  useDocumentUpload: (_uploadFn, uploadedCb) => {
    onUploaded = uploadedCb;
    return {
      fileInput: ref(null),
      uploadError: ref(null),
      uploadRetryable: ref(false),
      onUpload: vi.fn(),
      onFileChange: vi.fn(),
    };
  },
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

const JOB_PATH = 'upload.md';

// Mount already viewing JOB_PATH's job: the URL addresses it and the jobs
// arrive a tick later, which is the watcher's own deep-link path into
// viewingJobPath.
async function mountViewingJob() {
  routeState.params = { trajectRef: 'traject-abcd1234', docPath: JOB_PATH };
  const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
  await nextTick();
  jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'processing' }];
  await nextTick();
  await nextTick();
  openDoc.mockClear();
  return wrapper;
}

// Drop the job from the polled list and let the (async) watcher settle.
async function jobLeavesList() {
  jobs.value = [];
  await nextTick();
  await nextTick();
  await nextTick();
}

beforeEach(() => {
  documents.value = [];
  jobs.value = [];
  openDoc.mockReset().mockResolvedValue(undefined);
  closeDoc.mockReset();
  refreshList.mockReset().mockResolvedValue(undefined);
  cancelJob.mockReset();
  routeState.name = 'werkdocumenten-traject';
  routeState.params = { trajectRef: 'traject-abcd1234', docPath: '' };
  routeState.query = {};
});

describe('LibraryView conversion-jobs watcher', () => {
  it('opens the converted document once the job completed and its .md exists', async () => {
    await mountViewingJob();
    // A completed conversion leaves the list AND writes its document.
    documents.value = [{ path: JOB_PATH }];
    await jobLeavesList();
    expect(openDoc).toHaveBeenCalledWith(JOB_PATH);
  });

  it('does not open a document for a job that left the list without writing one', async () => {
    await mountViewingJob();
    // A failed conversion leaves the list too (tasks.job_review on excludes
    // failed rows) but never wrote its .md. Opening it would 404 into a
    // generic load error instead of reporting the failure.
    await jobLeavesList();
    expect(openDoc).not.toHaveBeenCalled();
  });

  it('reports the failure in the job view when the job vanished without its .md', async () => {
    const wrapper = await mountViewingJob();
    await jobLeavesList();
    // Match the dialog's own text, not a bare 'mislukt' - "Uploaden mislukt"
    // also renders in this view, which makes a loose assertion pass regardless.
    expect(wrapper.html()).toContain('Conversie mislukt');
  });

  it('selects a fresh upload so the main pane shows its conversion', async () => {
    const wrapper = shallowMount(LibraryView, { global: { stubs: { teleport: true } } });
    await nextTick();
    // Only the upload response knows where the conversion will land.
    onUploaded({ ok: true, targetPath: JOB_PATH });
    jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'processing' }];
    await nextTick();
    await nextTick();
    expect(wrapper.html()).toContain('Aan het converteren');
    expect(openDoc).not.toHaveBeenCalled(); // no .md exists yet
  });

  it('names the document in the toolbar, not in the dialog', async () => {
    const wrapper = await mountViewingJob();
    // The title belongs where a document's title lives, so the dialog can stay
    // about the conversion. It is there from the start (the path is known) and
    // survives the job resolving either way.
    expect(wrapper.html()).toContain('<nldd-toolbar-title slot="center" align="center" text="upload"');
    await jobLeavesList();
    expect(wrapper.html()).toContain('<nldd-toolbar-title slot="center" align="center" text="upload"');
  });

  it('offers a way back out of the job view', async () => {
    // On a stacked (small) viewport the job view is the whole screen, with no
    // document list beside it to click back to. Whether the item is *visible*
    // is the pane's call via --context-back-button-display; that it exists at
    // all is this view's.
    const wrapper = await mountViewingJob();
    expect(wrapper.html()).toContain('Terug naar werkdocumenten');
  });

  it('keeps the job view while the job is still listed', async () => {
    await mountViewingJob();
    jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'failed' }];
    await nextTick();
    await nextTick();
    expect(openDoc).not.toHaveBeenCalled();
  });
});
