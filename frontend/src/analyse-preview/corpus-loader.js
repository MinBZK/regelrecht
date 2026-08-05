/**
 * corpus-loader: draait de Analyse op het échte corpus van deze repo,
 * zonder backend, DB of auth.
 *
 * Dit is preview-gereedschap, maar het pad is geen imitatie: dezelfde
 * WASM-engine, dezelfde `resolveNotes()`, dezelfde sidecars en hetzelfde
 * vocabulaire als in de editor. Wat ontbreekt is uitsluitend de laag
 * eromheen: editor-api, traject-scope en de auth. De YAML komt via een eager
 * `?raw`-glob binnen (Vite lijnt de inhoud in bij het bouwen), zoals
 * `werkbank-preview/corpus-loader.js` het ook doet.
 *
 * Waarom dit de moeite waard is boven een verzonnen fixture: de ankerstatus
 * is het enige cijfer op de pagina dat niet uit een bestand komt maar uit een
 * berekening over de wettekst. Op verzonnen data bevestig je alleen je eigen
 * aanname; op het echte corpus meet je of de selectors van vier bestaande
 * noten hun tekst nog vinden.
 */
import * as yaml from 'js-yaml';
import { useEngine } from '../composables/useEngine.js';

const wettenRaw = import.meta.glob('../../../corpus/regulation/**/*.yaml', {
  query: '?raw',
  import: 'default',
  eager: true,
});

const sidecarsRaw = import.meta.glob('../../../corpus/annotations/**/annotations.yaml', {
  query: '?raw',
  import: 'default',
  eager: true,
});

const vocabulaireRaw = import.meta.glob('../../../corpus/annotations/_vocabulary/ambiguity.yaml', {
  query: '?raw',
  import: 'default',
  eager: true,
});

/** Het vocabulaire zoals CI het leest. */
export function laadVocabulaire() {
  const [raw] = Object.values(vocabulaireRaw);
  if (!raw) return [];
  const doc = yaml.load(raw);
  return Array.isArray(doc?.ambiguity) ? doc.ambiguity : [];
}

/**
 * Wetten per `$id`, met de nieuwste versie vooraan.
 *
 * Meerdere versies van dezelfde wet staan naast elkaar in het corpus. De
 * editor kiest date-aware; de preview neemt de laatste `valid_from`, wat
 * neerkomt op een peildatum "nu". Dat is een vereenvoudiging en geen
 * modellering. Als er ooit een toestandsdatum op deze pagina komt, hoort
 * die hier binnen te komen in plaats van hier bedacht te worden.
 */
function wettenPerId() {
  const perId = new Map();
  for (const [pad, raw] of Object.entries(wettenRaw)) {
    let doc;
    try {
      doc = yaml.load(raw);
    } catch (e) {
      console.warn(`Analyse-preview: ${pad} is geen leesbare YAML:`, e.message);
      continue;
    }
    const id = doc?.$id;
    if (!id) continue;
    const bestaand = perId.get(id);
    if (!bestaand || String(doc.valid_from ?? '') > String(bestaand.validFrom ?? '')) {
      perId.set(id, { lawId: id, raw, validFrom: doc.valid_from ?? null, pad });
    }
  }
  return perId;
}

/**
 * Laad élke wet in de engine en vraag het metriekenrapport op.
 *
 * Dit is het pad dat telt: de cijfers komen uit `corpusMetrics()` in de
 * WASM-engine, over precies het model dat de engine zelf resolvet. Geen tweede
 * parser, dus geen kans dat een tegel iets anders beweert dan wat er bij
 * uitvoering gebeurt.
 *
 * Een wet die de engine weigert is zelf een bevinding en wordt geteld in plaats
 * van stil overgeslagen; anders leest een corpus waarvan een deel niet laadt
 * als een compleet corpus.
 *
 * @param {string} peildatum ISO-datum die bepaalt welke versie van elke wet
 *   meetelt. De aanroeper kiest hem, net als in het product, zodat de preview
 *   hetzelfde gedrag toont en niet een vereenvoudiging ervan.
 * @returns {Promise<{rapport: object|null, geweigerd: Array<{lawId: string, pad: string, fout: string}>, fout: string|null}>}
 */
export async function laadMetrieken(peildatum) {
  const geweigerd = [];
  let engine;
  try {
    const { initEngine } = useEngine();
    engine = await initEngine();
  } catch (e) {
    return { rapport: null, geweigerd, fout: `engine niet geladen: ${e.message}` };
  }

  // Alle versies laden, niet alleen de nieuwste: het rapport telt regelingen en
  // versies apart, en dat onderscheid verdwijnt als de preview er hier al één
  // uitkiest.
  for (const [pad, raw] of Object.entries(wettenRaw).sort(([a], [b]) => a.localeCompare(b, 'nl'))) {
    try {
      engine.loadLaw(raw);
    } catch (e) {
      const id = pad.split('/').slice(-2, -1)[0] ?? pad;
      geweigerd.push({ lawId: id, pad, fout: String(e.message ?? e) });
    }
  }

  try {
    return { rapport: engine.corpusMetrics(peildatum), geweigerd, fout: null };
  } catch (e) {
    return { rapport: null, geweigerd, fout: String(e.message ?? e) };
  }
}

/**
 * Herbereken op een andere peildatum zonder opnieuw te laden.
 *
 * De wetten zitten al in de engine, dus dit is dezelfde directe herberekening
 * als in het product. Zonder deze functie zou de preview bij elke datumwissel
 * het hele corpus opnieuw inlezen en anders aanvoelen dan de echte pagina.
 */
export async function hermeet(peildatum) {
  const { initEngine } = useEngine();
  const engine = await initEngine();
  return engine.corpusMetrics(peildatum);
}

/** Sidecars per lawId: de mapnaam onder `corpus/annotations/` ís het id. */
function sidecarsPerId() {
  const perId = new Map();
  for (const [pad, raw] of Object.entries(sidecarsRaw)) {
    const m = pad.match(/annotations\/([^/]+)\/annotations\.yaml$/);
    if (m && m[1] !== '_vocabulary') perId.set(m[1], raw);
  }
  return perId;
}

/**
 * Bouw de `perWet`-invoer voor `aggregeer()` uit het repo-corpus.
 *
 * @returns {Promise<{perWet: object[], wettenInCorpus: number, diagnostiek: object}>}
 */
export async function laadEchtCorpus() {
  const wetten = wettenPerId();
  const sidecars = sidecarsPerId();

  const diagnostiek = {
    wettenInCorpus: wetten.size,
    sidecars: sidecars.size,
    geladenInEngine: 0,
    engineFouten: [],
    zonderWet: [],
  };

  let engine = null;
  try {
    const { initEngine } = useEngine();
    engine = await initEngine();
  } catch (e) {
    // Zonder engine blijven de tellingen die niet van hem afhangen geldig;
    // de ankers worden dan `null` en het rapport meldt ze als ongemeten.
    diagnostiek.engineFouten.push(`engine niet geladen: ${e.message}`);
  }

  const perWet = [];
  for (const [lawId, raw] of [...sidecars.entries()].sort(([a], [b]) => a.localeCompare(b, 'nl'))) {
    const wet = wetten.get(lawId);
    if (!wet) {
      // Een sidecar voor een wet die niet in het corpus staat is zelf een
      // bevinding; hem stil overslaan zou het rapport laten kloppen terwijl
      // er noten verdwijnen.
      diagnostiek.zonderWet.push(lawId);
      const doc = yaml.load(raw);
      perWet.push({ lawId, notes: Array.isArray(doc?.annotations) ? doc.annotations : [], ankers: null });
      continue;
    }

    if (engine) {
      try {
        // Direct `loadLaw` in plaats van `loadDependency`: die laatste haalt
        // de YAML via editor-api op, en die is er hier niet. De engine en de
        // resolver zijn wél de echte.
        if (!engine.hasLaw(lawId)) engine.loadLaw(wet.raw);
        diagnostiek.geladenInEngine += 1;
        const resolved = engine.resolveNotes(lawId, raw);
        if (Array.isArray(resolved)) {
          perWet.push({
            lawId,
            notes: resolved.map((r) => r.note),
            ankers: resolved.map((r) => r.match ?? null),
          });
          continue;
        }
      } catch (e) {
        diagnostiek.engineFouten.push(`${lawId}: ${e.message}`);
      }
    }

    const doc = yaml.load(raw);
    perWet.push({ lawId, notes: Array.isArray(doc?.annotations) ? doc.annotations : [], ankers: null });
  }

  return { perWet, wettenInCorpus: wetten.size, diagnostiek };
}
