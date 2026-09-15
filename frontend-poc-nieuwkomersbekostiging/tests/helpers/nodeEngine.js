/**
 * Test helper: WasmEngine in Node (vitest), met wetten van schijf.
 *
 * Importeert de wasm-glue rechtstreeks uit public/wasm/pkg/ (in Node mag
 * dat wel) en initialiseert met de bytes van de .wasm zelf, zodat er geen
 * fetch nodig is.
 */
import { readFileSync, readdirSync, statSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(__dirname, '..', '..');
const pkgDir = resolve(appRoot, 'public', 'wasm', 'pkg');
const lawsDir = resolve(appRoot, 'public', 'laws');

let enginePromise = null;

async function initWasm() {
  const wasm = await import(resolve(pkgDir, 'regelrecht_engine.js'));
  const bytes = readFileSync(resolve(pkgDir, 'regelrecht_engine_bg.wasm'));
  await wasm.default({ module_or_path: bytes });
  return wasm;
}

function findFiles(dir, ext) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = resolve(dir, entry);
    // varianten/ bevat de export van variant/*-branches: dezelfde wetten met
    // andere inhoud. Die horen niet in de basis-engine van de tests.
    if (entry === 'varianten') continue;
    if (statSync(full).isDirectory()) results.push(...findFiles(full, ext));
    else if (entry.endsWith(ext)) results.push(full);
  }
  return results;
}

/** Nieuwe engine met alle corpus-wetten geladen. */
export async function createEngineWithLaws() {
  const wasm = await initWasm();
  const engine = new wasm.WasmEngine();
  for (const file of findFiles(lawsDir, '.yaml')) {
    engine.loadLaw(readFileSync(file, 'utf-8'));
  }
  return engine;
}

/** Gedeelde engine-instantie voor tests die niet muteren. */
export function sharedEngine() {
  if (!enginePromise) enginePromise = createEngineWithLaws();
  return enginePromise;
}

export { lawsDir };
