/**
 * useTaskReview - review-modus voor een job_review-taak in de editor.
 *
 * Bij ?task=<id>: haal de taakdetail op, vind het law-YAML-resultaat en
 * lever (a) de voorgestelde content, (b) een staleness-vlag (payload.
 * source_etag ≠ actuele law-ETag), en (c) approve/reject-acties.
 * Approve-volgorde (spec §5.3): éérst opslaan via het normale save-pad
 * (doet de caller), dán resolve(approved).
 */
import { ref } from 'vue';
import { useTasks } from './useTasks.js';

export function useTaskReview() {
  const { fetchTask, resolveTask } = useTasks();
  const reviewTask = ref(null);
  const proposedContent = ref(null);
  const stale = ref(false);
  const loadError = ref(null);

  async function loadReview(taskId, currentLawEtag) {
    try {
      const detail = await fetchTask(taskId);
      if (detail.task_type !== 'job_review' || detail.status !== 'open') {
        loadError.value = 'Deze taak is al afgehandeld.';
        return;
      }
      const lawId = detail.payload?.law_id;
      // Het law-YAML-resultaat is het bestand met de wet zelf; sidecars
      // (.enrichment.yaml e.d.) committen we in v1 niet mee.
      const lawFile = (detail.results || []).find(
        (f) => !f.path.split('/').pop().startsWith('.')
      );
      if (!lawFile || !lawId) {
        loadError.value = 'Geen resultaat gevonden bij deze taak.';
        return;
      }
      reviewTask.value = detail;
      proposedContent.value = lawFile.content;
      stale.value = Boolean(
        detail.payload?.source_etag &&
          currentLawEtag &&
          detail.payload.source_etag !== currentLawEtag
      );
    } catch (e) {
      loadError.value = 'Taak laden mislukt.';
    }
  }

  async function approveAfterSave() {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'approved');
    reviewTask.value = null;
    proposedContent.value = null;
  }

  async function reject() {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'rejected');
    reviewTask.value = null;
    proposedContent.value = null;
  }

  return { reviewTask, proposedContent, stale, loadError, loadReview, approveAfterSave, reject };
}
