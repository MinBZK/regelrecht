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
    // also renders in this view and makes a loose assertion pass regardless.
    // Match the dialog's own text, not a bare 'mislukt' - "Uploaden mislukt"
    // also renders in this view, which makes a loose assertion pass regardless.
    expect(wrapper.html()).toContain("Conversie van 'upload' mislukt");
  });

  it('keeps the job view while the job is still listed', async () => {
    await mountViewingJob();
    jobs.value = [{ id: 'job-1', target_path: JOB_PATH, status: 'failed' }];
    await nextTick();
    await nextTick();
    expect(openDoc).not.toHaveBeenCalled();
  });
});
