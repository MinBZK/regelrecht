/**
 * Load the demo corpus from the static assets copy-demo-corpus.mjs produced:
 * the law catalogue, every law's YAML (text and parsed), the bindings sidecar,
 * the persona profiles, the organisation registry and the demo configuration.
 *
 * Everything is fetched once and kept in a module-level promise, so any view
 * that needs the corpus awaits the same load.
 */
import * as yaml from 'js-yaml';

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
 * @property {string|null} discoverable
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
    const [bindings, profiles, services, config] = await Promise.all([
      fetchYaml('/data/bindings.yaml'),
      fetchYaml('/data/profiles.yaml'),
      fetchYaml('/data/services.yaml'),
      fetchYaml('/data/demo-config.yaml'),
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
