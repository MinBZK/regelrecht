import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useTaskReview } from './useTaskReview.js';

const fetchTask = vi.fn();
const resolveTask = vi.fn();
vi.mock('./useTasks.js', () => ({
  useTaskActions: () => ({ fetchTask: (...a) => fetchTask(...a), resolveTask: (...a) => resolveTask(...a) }),
}));

function openTask(overrides = {}) {
  return {
    id: 't1',
    task_type: 'job_review',
    status: 'open',
    payload: { law_id: 'test_wet', source_etag: 'etag-1' },
    results: [{ path: 'corpus/regulation/nl/wet/test_wet/2025-01-01.yaml', content: 'proposed: yaml' }],
    ...overrides,
  };
}

describe('useTaskReview', () => {
  beforeEach(() => {
    fetchTask.mockReset();
    resolveTask.mockReset();
  });

  it('laadt het voorstel en zet stale wanneer de etag afwijkt', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, stale, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-2');
    expect(fetchTask).toHaveBeenCalledWith('t1');
    expect(reviewTask.value?.id).toBe('t1');
    expect(proposedContent.value).toBe('proposed: yaml');
    expect(stale.value).toBe(true);
    expect(loadError.value).toBeNull();
  });

  it('zet stale niet wanneer de etag overeenkomt', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { stale, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(stale.value).toBe(false);
  });

  it('weigert een taak die al afgehandeld is', async () => {
    fetchTask.mockResolvedValue(openTask({ status: 'resolved' }));
    const { reviewTask, proposedContent, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');
  });

  it('weigert een taak van het verkeerde type', async () => {
    fetchTask.mockResolvedValue(openTask({ task_type: 'job_failed' }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');
  });

  it('zet loadError wanneer er geen law-resultaat bij de taak zit', async () => {
    fetchTask.mockResolvedValue(openTask({ results: [{ path: '.enrichment.yaml', content: 'x' }] }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('zet loadError wanneer de taak geen law_id in de payload heeft', async () => {
    fetchTask.mockResolvedValue(openTask({ payload: { source_etag: 'etag-1' } }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('kiest het result-bestand op payload.yaml_path, niet de eerste dot-loze file', async () => {
    fetchTask.mockResolvedValue(
      openTask({
        payload: { law_id: 'test_wet', source_etag: 'etag-1', yaml_path: 'laws/w/law.yaml' },
        results: [
          { path: 'features/x.feature', content: 'Feature: ...' },
          { path: 'laws/w/law.yaml', content: 'wet: ja' },
        ],
      })
    );
    const { proposedContent, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(proposedContent.value).toBe('wet: ja');
  });

  it('zet loadError wanneer de taak geen payload heeft', async () => {
    fetchTask.mockResolvedValue(openTask({ payload: undefined }));
    const { reviewTask, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(reviewTask.value).toBeNull();
    expect(loadError.value).toBe('Geen resultaat gevonden bij deze taak.');
  });

  it('zet loadError wanneer fetchTask faalt', async () => {
    fetchTask.mockRejectedValue(new Error('netwerk'));
    const { loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(loadError.value).toBe('Taak laden mislukt.');
  });

  it('approveAfterSave resolvet als approved en reset de review-state', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, loadReview, approveAfterSave } = useTaskReview();
    await loadReview('t1', 'etag-1');
    await approveAfterSave();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'approved');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
  });

  it('reject resolvet als rejected en reset de review-state', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, loadReview, reject } = useTaskReview();
    await loadReview('t1', 'etag-1');
    await reject();
    expect(resolveTask).toHaveBeenCalledWith('t1', 'rejected');
    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
  });

  it('reset ruimt ook stale en loadError op, niet alleen de taak', async () => {
    fetchTask.mockResolvedValue(openTask());
    const { reviewTask, proposedContent, stale, loadError, loadReview, reset } = useTaskReview();
    await loadReview('t1', 'etag-2');
    expect(stale.value).toBe(true);

    reset();

    expect(reviewTask.value).toBeNull();
    expect(proposedContent.value).toBeNull();
    expect(stale.value).toBe(false);
    expect(loadError.value).toBeNull();
    // Puur lokaal: de taak zelf blijft open, reset is geen afhandeling.
    expect(resolveTask).not.toHaveBeenCalled();
  });

  // Vanuit de editor kun je via de takenlijst rechtstreeks van de ene taak naar
  // de andere - `?task=` gaat dan van A naar B zonder tussenstop op "geen taak",
  // dus zonder dat er iets `reset()` aanroept. Een nieuwe load hoort daarom zelf
  // schoon te beginnen; anders staat de foutmelding van A boven het voorstel
  // van B (en kleurt de balk kritiek terwijl er niets mis is).
  it('begint schoon bij een volgende load, ook zonder reset ertussen', async () => {
    fetchTask.mockResolvedValueOnce(openTask({ status: 'resolved' }));
    const { reviewTask, proposedContent, stale, loadError, loadReview } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(loadError.value).toBe('Deze taak is al afgehandeld.');

    fetchTask.mockResolvedValueOnce(openTask({ id: 't2' }));
    await loadReview('t2', 'etag-1');

    expect(reviewTask.value?.id).toBe('t2');
    expect(proposedContent.value).toBe('proposed: yaml');
    expect(loadError.value).toBeNull();
    expect(stale.value).toBe(false);
  });

  it('reset laat een foutmelding van een mislukte load niet blijven staan', async () => {
    fetchTask.mockRejectedValue(new Error('netwerk'));
    const { loadError, loadReview, reset } = useTaskReview();
    await loadReview('t1', 'etag-1');
    expect(loadError.value).toBe('Taak laden mislukt.');

    reset();

    expect(loadError.value).toBeNull();
  });

  it('approveAfterSave/reject zijn een no-op zonder actieve review', async () => {
    const { approveAfterSave, reject } = useTaskReview();
    await approveAfterSave();
    await reject();
    expect(resolveTask).not.toHaveBeenCalled();
  });
});
