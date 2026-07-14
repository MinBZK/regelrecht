/**
 * useDocumentTaskReview - review-modus voor een job_review-taak met
 * `payload.kind === 'document'` (documentconversie) in de werkdocumenten-
 * sectie.
 *
 * Zelfde vorm als useTaskReview.js (de wet-tegenhanger): haal de taakdetail
 * op, valideer 'm, en lever (a) de voorgestelde markdown, (b) approve/reject-
 * acties. Puur fetch/resolve - het INZETTEN van de voorgestelde markdown als
 * niet-opgeslagen wijziging in de documenten-manager (en het weer terugdraaien
 * bij Verwerpen) is orkestratie die, net als bij de wet-review in
 * EditorView.vue, in de aanroeper (LibraryView.vue) blijft: deze composable
 * kent de documenten-manager niet.
 */
import { ref } from 'vue';
import { useTaskActions } from './useTasks.js';

export function useDocumentTaskReview() {
  // useTaskActions(), not useTasks(): zie useTaskReview.js - voorkomt dat elke
  // (ook anonieme) bezoeker van de werkdocumenten-sectie de 30s-poll van de
  // gedeelde takenlijst start.
  const { fetchTask, resolveTask } = useTaskActions();
  const reviewTask = ref(null);
  const proposedContent = ref(null);
  const loadError = ref(null);

  async function loadReview(taskId) {
    try {
      const detail = await fetchTask(taskId);
      if (detail.task_type !== 'job_review' || detail.status !== 'open') {
        loadError.value = 'Deze taak is al afgehandeld.';
        return;
      }
      const payload = detail.payload || {};
      if (payload.kind !== 'document' || !payload.traject_ref || !payload.target_path) {
        loadError.value = 'Geen documentvoorstel gevonden bij deze taak.';
        return;
      }
      const results = detail.results || [];
      const doc = results.find((r) => r.path === payload.target_path) || results[0];
      if (!doc) {
        loadError.value = 'Geen resultaat gevonden bij deze taak.';
        return;
      }
      reviewTask.value = detail;
      proposedContent.value = doc.content;
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

  return { reviewTask, proposedContent, loadError, loadReview, approveAfterSave, reject };
}
