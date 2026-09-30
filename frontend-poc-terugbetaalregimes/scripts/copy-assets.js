/**
 * Copy build-time assets into app/public/:
 *
 *  - corpus/regulation YAML + scenario .feature files → public/laws/ + index.json
 *  - data/*.yaml (personas, distributions)           → public/data/
 *  - WASM engine pkg from the regelrecht checkout    → public/wasm/pkg/
 *
 * The index lists every version file per law: the engine's date-aware
 * resolution needs all versions loaded, not just the newest.
 */
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'fs';
import { execFileSync } from 'child_process';
import { resolve, dirname, relative } from 'path';
import { fileURLToPath } from 'url';
import * as yaml from 'js-yaml';

const __dirname = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(__dirname, '..');
// In de monorepo is regelrecht geen checkout ernaast maar de repo zelf, en
// staat het casus-corpus onder corpus-poc/<casus>/. Dat scheelt de hele
// REGELRECHT_PATH-dans: `just wasm-build` levert de engine op een vaste plek.
const repoRoot = resolve(appRoot, '..');
const regelrechtRoot = repoRoot;
const projectRoot = resolve(repoRoot, 'corpus-poc', 'terugbetaalregimes');

function findFiles(dir, ext) {
  if (!existsSync(dir)) return [];
  const results = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = resolve(dir, entry.name);
    if (entry.isDirectory()) results.push(...findFiles(full, ext));
    else if (entry.name.endsWith(ext)) results.push(full);
  }
  return results;
}

// --- laws ---
const corpusDir = resolve(projectRoot, 'corpus');
const lawsDest = resolve(appRoot, 'public', 'laws');
mkdirSync(lawsDest, { recursive: true });

const index = [];
for (const filePath of findFiles(corpusDir, '.yaml')) {
  const relPath = relative(corpusDir, filePath);
  const content = readFileSync(filePath, 'utf-8');
  const dest = resolve(lawsDest, relPath);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, content);

  let doc = {};
  try {
    doc = yaml.load(content) || {};
  } catch {
    console.warn(`  ! ${relPath}: YAML parse failed, indexed without metadata`);
  }
  index.push({
    id: doc.$id || relPath,
    name: typeof doc.name === 'string' ? doc.name.trim().replace(/\s+/g, ' ') : doc.$id,
    regulatory_layer: doc.regulatory_layer || 'unknown',
    valid_from: doc.valid_from || null,
    // Relatief, zonder leidende slash: de app zet er met basePad.js de
    // montagebasis voor (`/` los, `/terugbetaalregimes/` achter het portaal).
    path: `laws/${relPath}`,
  });
}

const features = findFiles(corpusDir, '.feature');
for (const filePath of features) {
  const relPath = relative(corpusDir, filePath);
  const dest = resolve(lawsDest, relPath);
  mkdirSync(dirname(dest), { recursive: true });
  cpSync(filePath, dest);
}

index.sort((a, b) => a.id.localeCompare(b.id) || (a.valid_from || '').localeCompare(b.valid_from || ''));
writeFileSync(resolve(lawsDest, 'index.json'), JSON.stringify(index, null, 2));
console.log(`laws: ${index.length} YAML file(s), ${features.length} scenario file(s)`);

// --- beleidsvarianten ---
// Elke beleidsvariant is een set wetsbestanden die de basisbestanden vervangen:
// een wetswijziging als reviewbare diff. Ze stonden als git-branches in de
// oorspronkelijke PoC-repo; in de monorepo zijn het gewone bestanden onder
// corpus-poc/<casus>/varianten/<id>/, beschreven in varianten.yaml.
//
// Waarom niet meer uit git: een CI-build heeft die branches niet, en een
// variant hoort reviewbaar te zijn in een pull request in plaats van onzichtbaar
// in een branch die niemand uitcheckt.
//
// `base` houdt zijn vorm: lawStore.js gebruikt dat veld als sleutel in docs[],
// niet als URL. Is er geen basisdocument (een variant die een nieuwe
// versiedatum toevoegt), dan registreert de app het bestand als extra document.
const variantenDir = resolve(projectRoot, 'varianten');
const variants = [];
const variantenIndex = resolve(variantenDir, 'varianten.yaml');
if (existsSync(variantenIndex)) {
  const beschreven = (yaml.load(readFileSync(variantenIndex, 'utf8')) || {}).varianten ?? [];
  for (const variant of beschreven) {
    const { id, titel } = variant;
    const files = [];
    const data = [];
    for (const bestand of variant.bestanden ?? []) {
      const bron = resolve(variantenDir, id, bestand);
      if (!existsSync(bron)) {
        // Hard, niet een waarschuwing: een ontbekend bestand haalt stil een
        // vergelijkingskolom uit de beleidsview weg, en dat valt pas op als
        // iemand de demo geeft.
        console.error(`variant ${id}: ${bestand} ontbreekt in ${variantenDir}`);
        process.exit(1);
      }
      const content = readFileSync(bron, 'utf8');
      if (bestand.startsWith('data/')) {
        const relPath = relative('data', bestand);
        const dest = resolve(appRoot, 'public', 'data', 'varianten', id, relPath);
        mkdirSync(dirname(dest), { recursive: true });
        writeFileSync(dest, content);
        data.push({ path: `data/varianten/${id}/${relPath}`, base: `data/${relPath}` });
        continue;
      }
      const relPath = relative('corpus', bestand);
      const dest = resolve(lawsDest, 'varianten', id, relPath);
      mkdirSync(dirname(dest), { recursive: true });
      writeFileSync(dest, content);
      files.push({
        path: `laws/varianten/${id}/${relPath}`,
        base: `laws/${relPath}`,
      });
    }
    if (files.length > 0 || data.length > 0) {
      // `title` en niet `titel`: lawStore.js leest dat veld (variantLabel),
      // en dit manifest is zijn contract, geen nieuw formaat.
      variants.push({ id, title: titel, toelichting: variant.toelichting ?? '', files, data });
    }
  }
}
writeFileSync(resolve(lawsDest, 'variants.json'), JSON.stringify(variants, null, 2));
console.log(`varianten: ${variants.length} branch(es) geëxporteerd`);

// --- data (personas, distributions) ---
const dataSrc = resolve(projectRoot, 'data');
const dataDest = resolve(appRoot, 'public', 'data');
mkdirSync(dataDest, { recursive: true });
let dataFiles = 0;
for (const filePath of findFiles(dataSrc, '.yaml')) {
  cpSync(filePath, resolve(dataDest, relative(dataSrc, filePath)));
  dataFiles++;
}
console.log(`data: ${dataFiles} file(s)`);

// --- wasm ---
const wasmSrc = resolve(regelrechtRoot, 'frontend', 'public', 'wasm', 'pkg');
const wasmDest = resolve(appRoot, 'public', 'wasm', 'pkg');
if (!existsSync(resolve(wasmSrc, 'regelrecht_engine_bg.wasm'))) {
  console.error(`wasm pkg niet gevonden in ${wasmSrc} — draai 'just wasm' (regelrecht wasm-build) eerst.`);
  process.exit(1);
}
mkdirSync(wasmDest, { recursive: true });
cpSync(wasmSrc, wasmDest, { recursive: true });
console.log(`wasm: pkg gekopieerd uit ${wasmSrc}`);
