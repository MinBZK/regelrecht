/**
 * De stand van de wereld in de browser: één beeld, en de bediening eromheen.
 *
 * De app houdt geen eigen administratie van de wereld bij. Elke wijziging gaat
 * naar de server en wat terugkomt is het nieuwe beeld; alles wat de pagina toont
 * komt daaruit. Wat er nieuw is, is het verschil met het vorige beeld — daarom
 * bewaart deze store per kroniek hoeveel grammen er vóór de wijziging lagen en
 * hoeveel journaalregels er stonden, en niets meer.
 *
 * `createWorld` neemt zijn API als argument, zodat een test hem kan vervangen.
 * `useWorld` is dezelfde store met de echte routes, één keer per pagina.
 */
import { computed, ref, shallowRef } from 'vue';
import * as worldApi from '../api/worldApi.js';
import { snapshotFrom } from '../api/worldApi.js';
import { journalEntries } from './journal.js';
import { gramCounts, newGrams } from './snapshot.js';

export function createWorld(api = worldApi) {
  /** Het laatste beeld dat de server gaf. */
  const snapshot = shallowRef(null);
  /** Het aantal grammen per kroniek vóór de laatste wijziging; `null` = niets nieuw. */
  const previousCounts = ref(null);
  /**
   * Het aantal journaalregels vóór de laatste wijziging; `null` = niets nieuw.
   *
   * Dezelfde telling als bij de grammen, en om dezelfde reden: het journaal
   * groeit achteraan en wijzigt nooit, dus elke regel vanaf het oude aantal is
   * erbij gekomen.
   */
  const previousJournalLength = ref(null);
  /** Een eerste keer laden, of opnieuw. */
  const loading = ref(false);
  /** Een wijziging onderweg. */
  const busy = ref(false);
  /** De laatste fout, in de woorden van de server. */
  const error = ref(null);
  /**
   * Dezelfde fout, maar dan van een actie: `{ action, message }`.
   *
   * Een formulier dat geweigerd wordt hoort dat bij zichzelf te laten zien en
   * niet alleen bovenaan de pagina — wie een veld verkeerd invult, kijkt naar
   * dat veld. Daarom staat hier ook bij wélke actie het was; zonder dat zou de
   * melding onder elke kaart tegelijk staan.
   */
  const actionError = ref(null);
  /** Wat de laatste wijziging opleverde. */
  const result = ref(null);

  const ready = computed(() => snapshot.value !== null);
  const clock = computed(() => snapshot.value?.clock ?? null);

  /**
   * Eén plek waar een fout binnenkomt, en daarmee de plek die `actionError`
   * leegmaakt. Zonder dat blijft de melding van een mislukte actie staan terwijl
   * er allang iets anders misging, en omdat de banner bovenaan zwijgt zolang er
   * een actiefout is, zou die nieuwe fout nergens meer te zien zijn. Wie hem
   * daarna wél wil tonen, zet hem ná deze oproep (zie `act`).
   */
  function fail(cause) {
    error.value = cause?.message || 'De server gaf geen leesbare fout.';
    actionError.value = null;
    return null;
  }

  /** Het beeld ophalen. Wist wat er nieuw was: dit is de stand, niet een stap. */
  async function load() {
    loading.value = true;
    error.value = null;
    actionError.value = null;
    try {
      snapshot.value = await api.fetchWorld();
      previousCounts.value = null;
      previousJournalLength.value = null;
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
    actionError.value = null;
    result.value = null;
    const before = snapshot.value ? gramCounts(snapshot.value) : null;
    const journalBefore = snapshot.value ? journalEntries(snapshot.value).length : null;
    try {
      // Het beeld uit het antwoord, of een verse ophaling als het antwoord er
      // geen draagt: de stand komt van de server, nooit uit een eigen boekhouding.
      const next = snapshotFrom(await run()) ?? (await api.fetchWorld());
      previousCounts.value = before;
      previousJournalLength.value = journalBefore;
      snapshot.value = next;
      result.value = { label, grams: newGrams(next, before) };
      return next;
    } catch (cause) {
      return fail(cause);
    } finally {
      busy.value = false;
    }
  }

  /**
   * Eén actie uitvoeren. Gaat het mis, dan wordt de melding óók aan de actie
   * gehangen, zodat haar eigen kaart hem kan tonen.
   */
  async function act(action, values) {
    const next = await step(action.label ?? action.id, () => api.runAction(action.id, values));
    if (next === null) actionError.value = { action: action.id, message: error.value };
    return next;
  }

  const advance = (until) => step(`Vooruitgespoeld tot ${until}`, () => api.advanceTo(until));

  const saveSettings = (changes) => step('Instellingen gewijzigd', () => api.updateSettings(changes));

  /** Terugzetten naar de startstand; daarna is er niets nieuw. */
  async function reset() {
    const next = await step('Wereld teruggezet', () => api.resetWorld());
    previousCounts.value = null;
    previousJournalLength.value = null;
    result.value = next ? { label: 'Wereld teruggezet', grams: [] } : result.value;
    return next;
  }

  /**
   * Een cel naar een lexostatus vragen, op een moment. Verandert niets, dus
   * geen nieuw beeld: het antwoord is van de cel en de wereld blijft staan.
   */
  async function askLexostatus(cell, name, params, opMoment) {
    error.value = null;
    actionError.value = null;
    try {
      return await api.askLexostatus(cell, name, params, opMoment);
    } catch (cause) {
      return fail(cause);
    }
  }

  function dismissResult() {
    result.value = null;
  }

  function dismissError() {
    error.value = null;
    actionError.value = null;
  }

  return {
    snapshot,
    previousCounts,
    previousJournalLength,
    loading,
    busy,
    error,
    actionError,
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
