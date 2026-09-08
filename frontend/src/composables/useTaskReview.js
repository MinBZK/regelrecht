/**
 * useTaskReview - review-modus voor een job_review-taak in de editor.
 *
 * Een verrijking levert één taak per gewijzigd artikel. Je oordeelt per
 * artikel, maar de verrijking is de eenheid die geschreven wordt: pas als
 * elk onderdeel een oordeel heeft, gaat alles in één keer naar de backend,
 * die er één commit van maakt. Tot dat moment verandert er niets aan de wet.
 *
 * Bij ?task=<id>: haal de taakdetail op, vind het law-YAML-resultaat en lever
 * (a) de voorgestelde content, (b) de andere onderdelen van dezelfde
 * verrijking, en (c) de acties om een oordeel vast te leggen en de verrijking
 * te verwerken.
 *
 * Er is geen staleness-vlag meer. Die vergeleek `payload.source_etag` met de
 * actuele ETag en sloeg daardoor aan zodra een zusje van dezelfde verrijking
 * was goedgekeurd - een waarschuwing over je eigen werk. Met één schrijfmoment
 * hoort er ook één controle te zijn, en die zit nu op het `If-Match` van het
 * verwerken.
 */
import { ref, computed } from 'vue';
import { useTaskActions } from './useTasks.js';

const STORAGE_KEY = 'regelrecht.enrich-verdicts';

/**
 * Vastgelegde oordelen per verrijking: `{ [jobId]: { [taskId]: { action, content } } }`.
 *
 * Bewust module-level en niet per component-mount: je beoordeelt artikel voor
 * artikel en navigeert daar tussendoor, dus de tussenstand moet die navigatie
 * overleven. sessionStorage houdt hem ook een herlaadbeurt vol; is die niet
 * beschikbaar (quota vol, privacy-modus), dan blijft het geheugen over en is de
 * tussenstand na een herlading weg - dan beoordeel je opnieuw, er is nog niets
 * geschreven.
 */
const verdicts = ref(loadVerdicts());

function loadVerdicts() {
  try {
    return JSON.parse(window.sessionStorage.getItem(STORAGE_KEY) ?? '{}') ?? {};
  } catch {
    return {};
  }
}

function persistVerdicts() {
  try {
    window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(verdicts.value));
  } catch {
    // Geheugen is de bron; opslag is alleen de herlaadbestendigheid.
  }
}

export function verdictsForJob(jobId) {
  return (jobId && verdicts.value[jobId]) || {};
}

export function recordVerdict(jobId, taskId, action, content) {
  if (!jobId || !taskId) return;
  verdicts.value = {
    ...verdicts.value,
    [jobId]: { ...verdictsForJob(jobId), [taskId]: { action, content } },
  };
  persistVerdicts();
}

export function clearVerdicts(jobId) {
  if (!jobId || !verdicts.value[jobId]) return;
  const next = { ...verdicts.value };
  delete next[jobId];
  verdicts.value = next;
  persistVerdicts();
}

export function useTaskReview() {
  // useTaskActions(), not useTasks(): useTaskReview() is itself called
  // unconditionally in EditorView's setup(), so joining the polled
  // useTasks() here would start the 30s poll for every editor visitor
  // (including anonymous ones).
  const { fetchTask, resolveTask, fetchJobTasks, applyEnrichment } = useTaskActions();
  const reviewTask = ref(null);
  const proposedContent = ref(null);
  const loadError = ref(null);
  // De onderdelen van deze verrijking: `[{ id, article, status, title }]`.
  // Eén element voor een taak zonder job (of wanneer het ophalen mislukt) -
  // dan is deze taak zelf de hele verrijking.
  const jobParts = ref([]);

  const jobId = computed(() => reviewTask.value?.job_id ?? null);
  const openParts = computed(() => jobParts.value.filter((p) => p.status === 'open'));
  /** Onderdelen die nog op een oordeel wachten. */
  const undecidedParts = computed(() => {
    const decided = verdictsForJob(jobId.value);
    return openParts.value.filter((p) => !decided[p.id]);
  });
  /** Hoeveelste onderdeel je nu beoordeelt, 1-based (0 als het er niet bij zit). */
  const partIndex = computed(() => {
    const idx = openParts.value.findIndex((p) => p.id === reviewTask.value?.id);
    return idx < 0 ? 0 : idx + 1;
  });
  const partCount = computed(() => openParts.value.length);

  async function loadReview(taskId) {
    try {
      const detail = await fetchTask(taskId);
      if (detail.task_type !== 'job_review' || detail.status !== 'open') {
        loadError.value = 'Deze taak is al afgehandeld.';
        return;
      }
      const lawId = detail.payload?.law_id;
      // Het law-YAML-resultaat is het bestand met de wet zelf. De worker
      // staged ook `features/*.feature`-bestanden naast `laws/...`, en die
      // sorteren er soms vóór - dus eerst het exacte pad uit de payload
      // proberen, en pas als dat niets oplevert terugvallen op de eerste
      // niet-dot-prefixed file (sidecars als .enrichment.yaml sluiten we
      // in v1 nog steeds uit).
      const results = detail.results || [];
      const lawFile =
        results.find((f) => f.path === detail.payload?.yaml_path) ||
        results.find((f) => !f.path.split('/').pop().startsWith('.'));
      if (!lawFile || !lawId) {
        loadError.value = 'Geen resultaat gevonden bij deze taak.';
        return;
      }
      reviewTask.value = detail;
      proposedContent.value = lawFile.content;
      loadError.value = null;
      jobParts.value = await loadJobParts(detail);
    } catch (e) {
      loadError.value = 'Taak laden mislukt.';
    }
  }

  /**
   * De andere onderdelen van dezelfde verrijking. Mislukt dat (of hangt de
   * taak aan geen job), dan is deze taak in zijn eentje de verrijking: dan kun
   * je hem nog steeds beoordelen en verwerken.
   */
  async function loadJobParts(detail) {
    const fallback = [
      {
        id: detail.id,
        article: detail.payload?.article == null ? null : String(detail.payload.article),
        status: 'open',
        title: detail.title,
      },
    ];
    if (!detail.job_id) return fallback;
    try {
      const json = await fetchJobTasks(detail.job_id);
      const parts = Array.isArray(json?.tasks) ? json.tasks : [];
      return parts.length > 0 ? parts : fallback;
    } catch {
      return fallback;
    }
  }

  /**
   * Leg het oordeel over het onderdeel dat nu open staat vast en zeg wat er
   * daarna moet gebeuren: het volgende onbeoordeelde onderdeel, of - als dit
   * de laatste was - dat de verrijking verwerkt kan worden.
   *
   * `content` is de inhoud die de gebruiker accordeert (het artikel zoals het
   * in de editor staat, of de hele wet bij een onderdeel zonder artikelnummer);
   * bij "niet overnemen" blijft hij weg.
   */
  function decide(action, content) {
    const task = reviewTask.value;
    if (!task) return { done: false, next: null };
    recordVerdict(jobId.value ?? task.id, task.id, action, content ?? null);
    const next = undecidedParts.value.find((p) => p.id !== task.id) ?? null;
    return { done: !next, next };
  }

  /**
   * Verwerk de verrijking: de vastgelegde oordelen plus, wanneer
   * `rejectRemaining` waar is, "niet overnemen" voor alles waar nog geen
   * oordeel over is geveld. Dat laatste is de weg om een verrijking af te
   * ronden zonder alles één voor één langs te lopen.
   */
  async function processEnrichment(etag, { rejectRemaining = false } = {}) {
    const task = reviewTask.value;
    if (!task) return null;
    const key = jobId.value ?? task.id;
    const decided = verdictsForJob(key);
    const decisions = openParts.value
      .map((part) => {
        const verdict = decided[part.id];
        if (verdict) {
          return verdict.action === 'approved'
            ? { task_id: part.id, action: 'approved', content: verdict.content ?? undefined }
            : { task_id: part.id, action: 'rejected' };
        }
        return rejectRemaining ? { task_id: part.id, action: 'rejected' } : null;
      })
      .filter(Boolean);
    const result = await applyEnrichment(jobId.value ?? task.id, decisions, etag);
    clearVerdicts(key);
    reviewTask.value = null;
    proposedContent.value = null;
    jobParts.value = [];
    return result;
  }

  /**
   * Los afhandelen van één taak, zonder de wet te schrijven. Alleen nog voor
   * het `law_create`-pad: daar bestaat de wet nog niet, dus die gaat via het
   * aanmaakpad (POST) en niet via het verwerken van een verrijking.
   */
  async function approveAfterSave() {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'approved');
    resetReview();
  }

  async function reject() {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'rejected');
    resetReview();
  }

  function resetReview() {
    reviewTask.value = null;
    proposedContent.value = null;
    jobParts.value = [];
  }

  return {
    reviewTask,
    proposedContent,
    loadError,
    jobParts,
    openParts,
    undecidedParts,
    partIndex,
    partCount,
    loadReview,
    decide,
    processEnrichment,
    approveAfterSave,
    reject,
  };
}
