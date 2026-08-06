/**
 * useTrajectIntegrity - haalt het integriteitsrapport van een traject op
 * (`GET /api/trajects/:trajectRef/integrity`).
 *
 * Het rapport is een momentopname van de traject-branch: elke aanroep laat de
 * backend de repo opnieuw doorlopen (die cachet zelf op blob-sha's, dus een
 * herhaalde aanroep zonder nieuwe commits is goedkoop). De pagina laadt daarom
 * bij openen en bij een expliciete verversing - er wordt niet gepolld.
 *
 * Zelfde vorm als `useTrajectDetail`: verse state per aanroep, `loading` en
 * `error` als refs, en de state wordt vóór de await gewist zodat een tweede
 * aanroep niet even het vorige rapport laat staan.
 */
import { computed, ref } from 'vue';
import { apiFetchJson } from '../lib/apiFetch.js';

/**
 * Volgorde waarin bevindingen op de pagina staan: fouten (breken nu iets)
 * boven waarschuwingen (waarschijnlijk een vergissing, werkt nog wel).
 */
export const SEVERITY_ORDER = ['error', 'warning'];

/** Kop en icoon per severity-groep. */
export const SEVERITY_LABELS = {
  error: { title: 'Fouten', icon: 'error' },
  warning: { title: 'Waarschuwingen', icon: 'warning' },
};

/**
 * Groepeer de bevindingen van een rapport op severity, in de vaste volgorde
 * van `SEVERITY_ORDER`. Lege groepen komen niet terug, zodat de pagina alleen
 * koppen toont die ook bevindingen hebben. Een onbekende severity (nieuwe
 * backend, oude frontend) belandt achteraan in plaats van te verdwijnen.
 *
 * @param {{findings?: Array}|null} report
 * @returns {Array<{severity: string, title: string, icon: string, findings: Array}>}
 */
export function groupBySeverity(report) {
  const findings = report?.findings ?? [];
  const bySeverity = new Map();
  for (const finding of findings) {
    const key = finding?.severity ?? 'error';
    if (!bySeverity.has(key)) bySeverity.set(key, []);
    bySeverity.get(key).push(finding);
  }
  const known = SEVERITY_ORDER.filter((s) => bySeverity.has(s));
  const unknown = [...bySeverity.keys()].filter((s) => !SEVERITY_ORDER.includes(s));
  return [...known, ...unknown].map((severity) => ({
    severity,
    title: SEVERITY_LABELS[severity]?.title ?? severity,
    icon: SEVERITY_LABELS[severity]?.icon ?? 'info',
    findings: bySeverity.get(severity),
  }));
}

export function useTrajectIntegrity() {
  const report = ref(null);
  const loading = ref(false);
  const error = ref(null);

  async function load(trajectRef) {
    if (!trajectRef) return;
    // Wissen vóór de await: een tweede aanroep (verversen, ander traject) mag
    // het oude rapport niet laten staan terwijl het nieuwe onderweg is.
    loading.value = true;
    error.value = null;
    report.value = null;
    try {
      report.value = await apiFetchJson(
        `/api/trajects/${encodeURIComponent(trajectRef)}/integrity`,
        {
          // De backend schrijft zijn eigen Nederlandse uitleg in de body
          // (bijv. "GitHub is nu niet bereikbaar..."); die is nuttiger dan
          // een statuscode, dus die tonen we als hij er is.
          errorMessage: (status, body) =>
            body?.trim() || `Integriteitscontrole mislukt: ${status}`,
        },
      );
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  const groups = computed(() => groupBySeverity(report.value));
  const hasFindings = computed(() => (report.value?.findings?.length ?? 0) > 0);

  return { report, groups, hasFindings, loading, error, load };
}
