/**
 * useAnalyse: haalt de notitie-sidecars van een heel traject op en voedt
 * ze aan de pure aggregatie in `lib/notenanalyse.js` (bouwplan §3.2).
 *
 * Het netwerkwerk zit hier, het rekenwerk in de library. Die scheiding is niet
 * cosmetisch: de aggregatie is daardoor zonder stack te testen, en het
 * determinisme-eis uit §5 geldt over een pure functie in plaats van over een
 * component met fetch-races erin.
 *
 * ## Waarom een fan-out
 *
 * De annotatie-API is per wet (`.../laws/{law_id}/annotations`); een
 * corpusbrede variant bestaat niet. Voor een v0 is N verzoeken acceptabel (een
 * traject-corpus is tientallen wetten, niet duizenden), maar het is wel de
 * eerste plek die pijn gaat doen als een corpus groeit. Zodra dat speelt hoort
 * hier één backend-endpoint onder te komen, niet meer parallellisme.
 */
import { ref, computed } from 'vue';
import * as yaml from 'js-yaml';
import { useEngine, loadLawVersions } from './useEngine.js';
import { useCorpusLaws } from './useCorpusLaws.js';
import { useAmbiguityVocabulary } from './useAmbiguityVocabulary.js';
import { annotationsUrl, lawVersionsUrl } from './corpusUrls.js';
import { apiFetch, apiFetchJson } from '../lib/apiFetch.js';
import { aggregeer } from '../lib/notenanalyse.js';

// Hoeveel sidecar-fetches tegelijk. Hoog genoeg om niet sequentieel te
// kruipen, laag genoeg om de editor-API niet plat te leggen bij een groot
// corpus.
const GELIJKTIJDIG = 6;

/**
 * Vandaag als ISO-datum, de standaard-peildatum.
 *
 * Het rapport blijft deterministisch: gegeven een datum is de uitkomst altijd
 * dezelfde. Alleen de *keuze* van de datum begint bij vandaag, en die keuze
 * legt de gebruiker vast. Een momentopname draagt zijn peildatum expliciet mee,
 * dus daar komt `new Date()` niet aan te pas.
 */
function vandaag() {
  return new Date().toISOString().slice(0, 10);
}

/** Draai `taak` over `items`, maximaal `n` tegelijk, met behoud van volgorde. */
async function inBatches(items, n, taak) {
  const uit = new Array(items.length);
  let i = 0;
  const werkers = Array.from({ length: Math.min(n, items.length) }, async () => {
    while (i < items.length) {
      const eigen = i++;
      uit[eigen] = await taak(items[eigen], eigen);
    }
  });
  await Promise.all(werkers);
  return uit;
}

/**
 * @param {import('vue').Ref<string|null>} trajectRef
 */
export function useAnalyse(trajectRef) {
  const { initEngine, loadDependency } = useEngine();
  const { laws, displayName } = useCorpusLaws(trajectRef);
  const { items: vocabulaire } = useAmbiguityVocabulary();

  // Het metriekenrapport uit de engine (de afgeleide cijfers). Staat los van
  // `perWet` hieronder, dat de notitielaag draagt: die twee komen uit
  // verschillende bronnen en het scherm hoort te laten zien welke welke is.
  const metrieken = ref(null);
  // Wetten die de engine weigert. Zelf een bevinding: zonder deze lijst leest
  // een corpus waarvan een deel niet laadt als een compleet corpus.
  const geweigerd = ref([]);

  // De peildatum bepaalt welke versie van elke wet meetelt. Wijzigen vraagt
  // geen nieuwe fetch: de wetten staan al in de engine, dus het rapport wordt
  // alleen opnieuw berekend.
  const peildatum = ref(vandaag());

  const perWet = ref([]);
  // Het aantal wetten waarover de fan-out liep. Los van `perWet`, want dat
  // bevat alleen wetten mét een sidecar: zonder dit getal leest "noten over 3
  // wetten" als volledige dekking terwijl het over 3 van de 24 kan gaan.
  const wettenInCorpus = ref(0);
  const loading = ref(false);
  const error = ref(null);
  // Wetten waarvan de sidecar niet gelezen kon worden. Apart van `error`: één
  // kapotte wet mag het hele rapport niet wegnemen, maar hij moet ook niet
  // stil verdwijnen; anders leest een half rapport als een heel rapport.
  const overgeslagen = ref([]);

  const rapport = computed(() =>
    aggregeer(perWet.value, vocabulaire.value, { wettenInCorpus: wettenInCorpus.value }),
  );

  /**
   * Laad elke versie van elke wet in de engine en vraag het rapport op.
   *
   * Alle versies, niet alleen de nieuwste: het rapport telt regelingen en
   * versies apart, en dat onderscheid verdwijnt als de frontend er hier al een
   * uitkiest. Een wet die niet laadt wordt geteld in plaats van overgeslagen.
   */
  async function meet(ids) {
    const tr = trajectRef?.value ?? null;
    geweigerd.value = [];
    let engine;
    try {
      engine = await initEngine();
    } catch (e) {
      metrieken.value = null;
      geweigerd.value = [{ lawId: '(engine)', fout: e.message }];
      return;
    }

    await inBatches(ids, GELIJKTIJDIG, async (lawId) => {
      try {
        const versies = await apiFetchJson(lawVersionsUrl(tr, lawId), {
          errorMessage: (status) => `HTTP ${status}`,
        });
        if (!loadLawVersions(engine, versies, lawId)) {
          geweigerd.value.push({ lawId, fout: 'geen enkele versie kon geladen worden' });
        }
      } catch (e) {
        geweigerd.value.push({ lawId, fout: String(e.message ?? e) });
      }
    });

    try {
      metrieken.value = engine.corpusMetrics(peildatum.value);
    } catch (e) {
      metrieken.value = null;
      geweigerd.value.push({ lawId: '(rapport)', fout: String(e.message ?? e) });
    }
  }

  /**
   * Lees één wet: sidecar ophalen, en als die er is de engine-resolver
   * erover halen voor de ankerstatus.
   *
   * @returns {{ invoer: object|null, fout: string|null }}
   */
  async function leesWet(lawId) {
    const tr = trajectRef?.value ?? null;

    const res = await apiFetch(annotationsUrl(tr, lawId), {
      allowStatuses: [404],
      errorMessage: (status) => `HTTP ${status}`,
    });
    // Geen sidecar is de normale toestand voor de meeste wetten, geen fout.
    if (res.status === 404) return { invoer: null, fout: null };

    const yamlText = await res.text();

    // Pad 1: de engine kan de wet laden, dus notes én ankers komen uit de
    // resolver, dus ze zijn uitgelijnd en afgebakend tot deze wet.
    try {
      const engine = await initEngine();
      await loadDependency(lawId, tr);
      const resolved = engine.resolveNotes(lawId, yamlText);
      if (Array.isArray(resolved)) {
        return {
          invoer: {
            lawId,
            notes: resolved.map((r) => r.note),
            ankers: resolved.map((r) => r.match ?? null),
          },
          fout: null,
        };
      }
    } catch (e) {
      // Pad 2: de wet laadt niet (niet in dit traject, kapotte YAML, engine
      // faalt). De tellingen die niet van de engine afhangen zijn nog steeds
      // waar, dus die leveren we, met `ankers: null` zodat het rapport deze
      // wet als ongemeten toont in plaats van als gezond.
      console.warn(`Analyse: ankers niet gemeten voor ${lawId}:`, e.message);
    }

    const doc = yaml.load(yamlText);
    const notes = Array.isArray(doc?.annotations) ? doc.annotations : [];
    return { invoer: { lawId, notes, ankers: null }, fout: null };
  }

  /**
   * Herbereken op een nieuwe peildatum, zonder iets opnieuw op te halen.
   *
   * De wetten zitten al in de engine; alleen de versiekeuze verandert. Daardoor
   * is de datumkiezer direct en niet een verkapte herlaadknop.
   */
  async function zetPeildatum(datum) {
    peildatum.value = datum;
    if (!metrieken.value) return;
    try {
      const engine = await initEngine();
      metrieken.value = engine.corpusMetrics(datum);
    } catch (e) {
      geweigerd.value = [...geweigerd.value, { lawId: '(rapport)', fout: String(e.message ?? e) }];
    }
  }

  async function laad() {
    loading.value = true;
    error.value = null;
    overgeslagen.value = [];
    try {
      const ids = laws.value.map((l) => l.law_id);
      wettenInCorpus.value = ids.length;
      // Meten en noten lezen lopen los van elkaar: een kapotte sidecar mag de
      // cijfers niet wegnemen, en andersom.
      await meet(ids);
      const uitkomsten = await inBatches(ids, GELIJKTIJDIG, async (lawId) => {
        try {
          return await leesWet(lawId);
        } catch (e) {
          return { invoer: null, fout: `${lawId}: ${e.message}` };
        }
      });
      perWet.value = uitkomsten.map((u) => u.invoer).filter(Boolean);
      overgeslagen.value = uitkomsten.map((u) => u.fout).filter(Boolean);
    } catch (e) {
      error.value = e;
      perWet.value = [];
      wettenInCorpus.value = 0;
      metrieken.value = null;
    } finally {
      loading.value = false;
    }
  }

  return {
    rapport,
    metrieken,
    geweigerd,
    peildatum,
    zetPeildatum,
    loading,
    error,
    overgeslagen,
    laad,
    displayName,
  };
}
