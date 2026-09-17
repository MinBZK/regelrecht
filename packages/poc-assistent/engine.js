/**
 * WASM-engine en corpus in Node, voor de beleidsassistent.
 *
 * Laadt dezelfde regelrecht-engine als de browser (uit app/public/wasm/pkg)
 * en de corpus-YAML van schijf. Overlays (bewerkte YAML per document) komen
 * bovenop de basisversies.
 */
import { readFileSync, readdirSync, statSync, existsSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import * as yaml from 'js-yaml';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Waar de casus-inhoud en de engine staan. In de monorepo is dat
// corpus-poc/<casus>/ en de wasm-build; in het image zet de Dockerfile ze
// neer. Eén assistent bedient elke casus, dus dit kan niet uit __dirname
// worden afgeleid — het hangt af van welke casus hij draait.
const CASUS = process.env.POC_CASUS ?? 'terugbetaalregimes';
export const projectRoot =
  process.env.POC_CASUS_DIR ?? resolve(__dirname, '..', '..', 'corpus-poc', CASUS);
// De simulatie- en patch-modules die de assistent deelt met de browser. Ze
// horen bij de app van de casus, niet bij de assistent: elke poc heeft zijn
// eigen sim/ en lib/. In de monorepo staat dat in frontend-poc-<casus>/src;
// het image zet die map onder /app/assistent/app.
export const appSrc =
  process.env.POC_APP_SRC ?? resolve(__dirname, '..', '..', `frontend-poc-${CASUS}`, 'src');
const pkgDir =
  process.env.POC_WASM_DIR ?? resolve(__dirname, '..', '..', 'frontend', 'public', 'wasm', 'pkg');
const corpusDir = resolve(projectRoot, 'corpus');

let wasmModule = null;

async function initWasm() {
  if (!wasmModule) {
    const wasm = await import(resolve(pkgDir, 'regelrecht_engine.js'));
    await wasm.default({
      module_or_path: readFileSync(resolve(pkgDir, 'regelrecht_engine_bg.wasm')),
    });
    wasmModule = wasm;
  }
  return wasmModule;
}

function findYaml(dir) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    if (statSync(full).isDirectory()) results.push(...findYaml(full));
    else if (entry.endsWith('.yaml')) results.push(full);
  }
  return results;
}

/** Basiscorpus van schijf: [{key, id, valid_from, path, yaml}]. */
export function loadBaseCorpus() {
  return findYaml(corpusDir).map((path) => {
    const text = readFileSync(path, 'utf-8');
    const doc = yaml.load(text);
    return {
      key: `${doc.$id}@${doc.valid_from ?? ''}`,
      id: doc.$id,
      valid_from: doc.valid_from ?? null,
      name: typeof doc.name === 'string' ? doc.name.trim().replace(/\s+/g, ' ') : doc.$id,
      path,
      yaml: text,
    };
  });
}

/**
 * Nieuwe engine met het basiscorpus plus overlays (map van key → yaml-tekst).
 */
export async function createEngine(overlays = new Map()) {
  const wasm = await initWasm();
  const engine = new wasm.WasmEngine();
  for (const doc of corpusMetOverlays(overlays)) engine.loadLaw(doc.yaml);
  return engine;
}

/**
 * Basiscorpus met overlays erop, plus overlays voor sleutels die niet in de
 * basis zitten (bijvoorbeeld een nieuw versiebestand van een variant die als
 * werkversie is meegestuurd). Zo rekent de assistent op precies de werkversie.
 */
export function corpusMetOverlays(overlays = new Map()) {
  const corpus = loadBaseCorpus().map((doc) => (overlays.has(doc.key) ? { ...doc, yaml: overlays.get(doc.key), gewijzigd: true } : doc));
  const bekend = new Set(corpus.map((d) => d.key));
  for (const [key, text] of overlays) {
    if (bekend.has(key)) continue;
    let doc = {};
    try { doc = yaml.load(text) ?? {}; } catch { continue; }
    corpus.push({
      key,
      id: doc.$id ?? key.split('@')[0],
      valid_from: doc.valid_from ?? key.split('@')[1] ?? null,
      name: typeof doc.name === 'string' ? doc.name.trim().replace(/\s+/g, ' ') : (doc.$id ?? key),
      path: null,
      yaml: text,
      gewijzigd: true,
    });
  }
  return corpus;
}

/** Valideer één YAML-tekst door hem in een verse engine te laden. */
export async function validateYaml(overlays, key, nieuweYaml) {
  const wasm = await initWasm();
  const engine = new wasm.WasmEngine();
  try {
    let gezien = false;
    for (const doc of corpusMetOverlays(overlays)) {
      const eigen = doc.key === key;
      gezien = gezien || eigen;
      engine.loadLaw(eigen ? nieuweYaml : doc.yaml);
    }
    if (!gezien) engine.loadLaw(nieuweYaml);
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e?.message ?? e) };
  } finally {
    engine.free?.();
  }
}

export function loadPersonas() {
  const file = resolve(projectRoot, 'data', 'personas.yaml');
  return yaml.load(readFileSync(file, 'utf-8')).personas;
}

export function loadDistributions() {
  const file = resolve(projectRoot, 'data', 'distributions.yaml');
  if (!existsSync(file)) return null;
  return yaml.load(readFileSync(file, 'utf-8'));
}

/** Pad naar het uitvoeringslastmodel van deze casus. */
export function handelingenPath() {
  return resolve(projectRoot, 'data', 'handelingen.yaml');
}

/** Het uitvoeringslastmodel van schijf, of null als deze casus er geen heeft. */
export function loadHandelingen() {
  const file = handelingenPath();
  if (!existsSync(file)) return null;
  return yaml.load(readFileSync(file, 'utf-8'));
}

/** De ruwe yaml-tekst van het uitvoeringslastmodel, of null. */
export function readHandelingenYaml() {
  const file = handelingenPath();
  if (!existsSync(file)) return null;
  return readFileSync(file, 'utf-8');
}
