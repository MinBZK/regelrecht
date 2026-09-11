/**
 * De stand van de wereld in de browser: één beeld, en de bediening eromheen.
 *
 * De app houdt geen eigen administratie van de wereld bij. Elke wijziging gaat
 * naar de server en wat terugkomt is het nieuwe beeld; alles wat de pagina toont
 * komt daaruit. Wat er nieuw is, is het verschil met het vorige beeld — daarom
 * bewaart deze store per kroniek hoeveel grammen er vóór de wijziging lagen, en
 * niets meer.
 *
 * `createWorld` neemt zijn API als argument, zodat een test hem kan vervangen.
 * `useWorld` is dezelfde store met de echte routes, één keer per pagina.
 */
import { computed, ref, shallowRef } from 'vue';
import * as worldApi from '../api/worldApi.js';
import { snapshotFrom } from '../api/worldApi.js';
import { gramCounts, newGrams } from './snapshot.js';

export function createWorld(api = worldApi) {
  /** Het laatste beeld dat de server gaf. */
  const snapshot = shallowRef(null);
  /** Het aantal grammen per kroniek vóór de laatste wijziging; `null` = niets nieuw. */
  const previousCounts = ref(null);
  /** Een eerste keer laden, of opnieuw. */
  const loading = ref(false);
  /** Een wijziging onderweg. */
  const busy = ref(false);
  /** De laatste fout, in de woorden van de server. */
  const error = ref(null);
  /** Wat de laatste wijziging opleverde. */
  const result = ref(null);

  const ready = computed(() => snapshot.value !== null);
  const clock = computed(() => snapshot.value?.clock ?? null);

  function fail(cause) {
    error.value = cause?.message || 'De server gaf geen leesbare fout.';
    return null;
  }

  /** Het beeld ophalen. Wist wat er nieuw was: dit is de stand, niet een stap. */
  async function load() {
    loading.value = true;
    error.value = null;
    try {
      snapshot.value = await api.fetchWorld();
      previousCounts.value = null;
      return snapshot.value;
    } catch (cause) {
      return fail(cause);
    } finally {
      loading.value = false;
    }
  }

  /**
   * Eén wijziging: uitvoeren, het nieuwe beeld overnemen, en verslag doen van wat
   * erbij kwam. `label` is wat de bezoeker deed, in zijn eigen woorden.
   */
  async function step(label, run) {
    busy.value = true;
    error.value = null;
    result.value = null;
    const before = snapshot.value ? gramCounts(snapshot.value) : null;
    try {
      // Het beeld uit het antwoord, of een verse ophaling als het antwoord er
      // geen draagt: de stand komt van de server, nooit uit een eigen boekhouding.
      const next = snapshotFrom(await run()) ?? (await api.fetchWorld());
      previousCounts.value = before;
      snapshot.value = next;
      result.value = { label, grams: newGrams(next, before) };
      return next;
    } catch (cause) {
      return fail(cause);
    } finally {
      busy.value = false;
    }
  }

  const act = (action, values) => step(action.label ?? action.id, () => api.runAction(action.id, values));

  const advance = (until) => step(`Vooruitgespoeld tot ${until}`, () => api.advanceTo(until));

  const saveSettings = (changes) => step('Instellingen gewijzigd', () => api.updateSettings(changes));

  /** Terugzetten naar de startstand; daarna is er niets nieuw. */
  async function reset() {
    const next = await step('Wereld teruggezet', () => api.resetWorld());
    previousCounts.value = null;
    result.value = next ? { label: 'Wereld teruggezet', grams: [] } : result.value;
    return next;
  }

  /** Een cel naar een lexostatus vragen. Verandert niets, dus geen nieuw beeld. */
  async function askLexostatus(cell, name, params) {
    try {
      return await api.askLexostatus(cell, name, params);
    } catch (cause) {
      error.value = cause?.message || 'De server gaf geen leesbare fout.';
      return null;
    }
  }

  function dismissResult() {
    result.value = null;
  }

  function dismissError() {
    error.value = null;
  }

  return {
    snapshot,
    previousCounts,
    loading,
    busy,
    error,
    result,
    ready,
    clock,
    load,
    act,
    advance,
    saveSettings,
    reset,
    askLexostatus,
    dismissResult,
    dismissError,
  };
}

let shared = null;

/** De store van deze pagina. */
export function useWorld() {
  if (!shared) shared = createWorld();
  return shared;
}
