/**
 * Load the demo corpus from the static assets copy-demo-corpus.mjs produced:
 * the law catalogue, every law's YAML (text and parsed), the bindings sidecar,
 * the persona profiles, the organisation registry and the demo configuration.
 *
 * Everything is fetched once and kept in a module-level promise, so any view
 * that needs the corpus awaits the same load.
 */
import * as yaml from 'js-yaml';
import { DEFAULT_LOCALE, LOCALES } from '../i18n/index.js';

let corpusPromise = null;

async function fetchText(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Kon ${url} niet laden (${res.status})`);
  return res.text();
}

async function fetchYaml(url) {
  return yaml.load(await fetchText(url));
}

/**
 * @typedef {object} LawEntry
 * @property {string} id
 * @property {string} name
 * @property {string|null} service
 * @property {string} valid_from
 * @property {string} path
 * @property {string} law_path
 * @property {string[]} outputs
 * @property {string[]} inputs
 * @property {string} text        raw YAML
 * @property {object} doc         parsed YAML
 */

export function loadCorpus() {
  if (corpusPromise) return corpusPromise;
  corpusPromise = (async () => {
    const index = await (await fetch('/data/index.json')).json();
    // De vertaalde talen, in de volgorde van de talentabel. Het Nederlands zit
    // er niet bij: dat is de bron en staat in `demo-config.yaml` zelf.
    const translated = LOCALES.filter((l) => l.code !== DEFAULT_LOCALE);
    // Elke taal van de configuratie wordt opgehaald, niet één op basis van de
    // taal die op dat moment aanstaat: een taalwissel halverwege een
    // presentatie mag geen laadmoment opleveren, en het corpus wordt maar één
    // keer geladen. Het verschil is per taal een bestand van twintig kilobyte.
    const [bindings, profiles, services, config, ...overlays] = await Promise.all([
      fetchYaml('/data/bindings.yaml'),
      fetchYaml('/data/profiles.yaml'),
      fetchYaml('/data/services.yaml'),
      fetchYaml('/data/demo-config.yaml'),
      // `null` bij een ontbrekende overlay: een taal zonder vertaalde
      // configuratie of persona's valt terug op het Nederlands, en dat mag het
      // opstarten niet breken. Config en profiles per taal, in die volgorde,
      // zodat de index hieronder klopt.
      ...translated.flatMap((l) => [
        fetchYaml(`/data/demo-config.${l.code}.yaml`).catch(() => null),
        fetchYaml(`/data/profiles.${l.code}.yaml`).catch(() => null),
      ]),
    ]);
    const laws = await Promise.all(
      index.laws.map(async (entry) => {
        const text = await fetchText(entry.path);
        return { ...entry, text, doc: yaml.load(text) };
      }),
    );
    // Latest version per law id, for the UI (the engine keeps every version and
    // picks by calculation date itself).
    const latestById = new Map();
    for (const law of laws) {
      const cur = latestById.get(law.id);
      if (!cur || law.valid_from > cur.valid_from) latestById.set(law.id, law);
    }
    return {
      laws,
      latestById,
      lawById: (id) => latestById.get(id) ?? null,
      /** Resolve the POC-style (law_path, service) address to the latest law entry. */
      lawByPath: (lawPath, service) => {
        const candidates = laws.filter(
          (l) => l.law_path === lawPath && (!service || l.service === service),
        );
        candidates.sort((a, b) => b.valid_from.localeCompare(a.valid_from));
        return candidates[0] ?? null;
      },
      scenarios: index.scenarios,
      bindings,
      profiles,
      services: services.services ?? {},
      config,
      /**
       * Dezelfde configuratie per taal, met de vertaalde teksten erin.
       *
       * Alle talen staan naast elkaar in het corpus; welke een scherm ziet,
       * bepaalt de store (`useDemo().corpus`), want dat is de plek die
       * reactief is. Een taal zonder overlay staat er niet in en valt daar
       * terug op het Nederlands, wat beter is dan lege dia's.
       */
      configByLocale: {
        [DEFAULT_LOCALE]: config,
        ...Object.fromEntries(
          translated.map((l, i) => [l.code, overlays[i * 2]]).filter(([, doc]) => doc),
        ),
      },
      /**
       * Dezelfde persona's met hun vertaalde beschrijving, per taal.
       *
       * Alleen `description` verschilt: namen blijven namen, en de
       * geregistreerde gegevens eronder zijn de invoer van de wet en horen in
       * geen enkele taal vertaald te worden. Een taal zonder overlay staat er
       * niet in en valt daar terug op het Nederlands.
       */
      profilesByLocale: {
        [DEFAULT_LOCALE]: profiles,
        ...Object.fromEntries(
          translated.map((l, i) => [l.code, overlays[i * 2 + 1]]).filter(([, doc]) => doc),
        ),
      },
    };
  })();
  return corpusPromise;
}

/** Organisation display data for a service code. */
export function serviceInfo(corpus, code) {
  const entry = corpus.services[code] ?? {};
  return {
    code,
    name: entry.name ?? code,
    logo: entry.logo ? `/logos/${entry.logo}.png` : null,
  };
}
