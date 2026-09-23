/**
 * Copy the demo corpus (corpus/demo) into public/data so the browser can fetch
 * it as static assets: every law YAML, every scenario feature, the bindings
 * sidecar and the persona profiles. Also writes public/data/index.json, the
 * catalogue the app reads first.
 *
 * Runs as the `predev` / `prebuild` hook; the repo root is two levels up.
 */
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';
import { DEFAULT_LOCALE, LOCALES } from '../src/i18n/index.js';

const here = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(here, '..');
const repoRoot = resolve(appRoot, '..');
const corpusDir = resolve(repoRoot, 'corpus', 'demo');
const lawsDir = resolve(corpusDir, 'regulation', 'nl');
const destDir = resolve(appRoot, 'public', 'data');

function walk(dir, predicate, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, predicate, out);
    else if (predicate(entry)) out.push(full);
  }
  return out;
}

rmSync(destDir, { recursive: true, force: true });
mkdirSync(destDir, { recursive: true });

// Which organisation executes which law. It lives in services.yaml and not in
// the law files: it drives logos, colours and grouping, and values like
// GEMEENTE_ROTTERDAM follow from no statute. Who is competent to decide is a
// different question, answered by `competent_authority` on the article.
// A missing map would give every law `service: null`, and the UI degrades
// quietly on that: no logo, the code as its own name. Refuse instead.
const serviceByLawId = yaml.load(readFileSync(join(corpusDir, 'services.yaml'), 'utf8')).laws;
if (!serviceByLawId || !Object.keys(serviceByLawId).length) {
  throw new Error('services.yaml heeft geen `laws:`-kaart; zie scripts/check-service-map.mjs');
}

const laws = [];
for (const file of walk(lawsDir, (n) => n.endsWith('.yaml'))) {
  const rel = relative(lawsDir, file);
  const text = readFileSync(file, 'utf8');
  const doc = yaml.load(text);
  const dest = join(destDir, 'laws', rel);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, text);
  const outputs = [];
  const inputs = [];
  for (const article of doc.articles ?? []) {
    const ex = article.machine_readable?.execution;
    if (!ex) continue;
    for (const o of ex.output ?? []) outputs.push(o.name);
    for (const i of ex.input ?? []) inputs.push(i.name);
  }
  laws.push({
    id: doc.$id,
    name: doc.name,
    service: serviceByLawId[doc.$id] ?? null,
    regulatory_layer: doc.regulatory_layer,
    valid_from: doc.valid_from ?? doc.publication_date,
    uuid: doc.uuid ?? null,
    path: `/data/laws/${rel.split('\\').join('/')}`,
    // The POC addressed a law by its directory path plus the executing service;
    // keep that as `law_path` so demo config written against it keeps working.
    law_path: rel.replace(/\/[^/]+\.yaml$/, ''),
    outputs,
    inputs,
  });
}

const scenarios = [];
for (const file of walk(lawsDir, (n) => n.endsWith('.feature'))) {
  const rel = relative(lawsDir, file);
  const dest = join(destDir, 'laws', rel);
  mkdirSync(dirname(dest), { recursive: true });
  cpSync(file, dest);
  const firstLine = readFileSync(file, 'utf8').split('\n').find((l) => l.trim().startsWith('Feature:'));
  scenarios.push({
    path: `/data/laws/${rel.split('\\').join('/')}`,
    law_path: rel.replace(/\/scenarios\/[^/]+\.feature$/, ''),
    title: firstLine ? firstLine.replace(/^\s*Feature:\s*/, '').trim() : rel,
  });
}

for (const name of ['bindings.yaml', 'profiles.yaml', 'demo-config.yaml', 'services.yaml']) {
  const src = join(corpusDir, name);
  if (existsSync(src)) cpSync(src, join(destDir, name));
}

// De vertaalde talen, in de volgorde van de talentabel. Het Nederlands zit er
// niet bij: dat is de bron, en het staat al in de bestanden hierboven.
const translatedLocales = LOCALES.filter((l) => l.code !== DEFAULT_LOCALE).map((l) => l.code);

/** Het YAML-bestand `corpus/demo/i18n/<name>`, of `{}` als het er niet is. */
function readCorpusI18n(name) {
  const file = join(corpusDir, 'i18n', name);
  return existsSync(file) ? (yaml.load(readFileSync(file, 'utf8')) ?? {}) : {};
}

/** Zet `value` op `path` in een kopie van `node`, zonder het origineel te raken. */
function setPath(node, path, value, source) {
  const parts = path.split('.');
  const copy = Array.isArray(node) ? [...node] : { ...node };
  let cur = copy;
  for (let i = 0; i < parts.length - 1; i += 1) {
    const key = Array.isArray(cur) ? Number(parts[i]) : parts[i];
    const child = cur[key];
    if (child === null || child === undefined) throw new Error(`${source} wijst naar een pad dat niet bestaat: ${path}`);
    cur[key] = Array.isArray(child) ? [...child] : { ...child };
    cur = cur[key];
  }
  const last = Array.isArray(cur) ? Number(parts.at(-1)) : parts.at(-1);
  if (cur[last] === undefined) throw new Error(`${source} wijst naar een pad dat niet bestaat: ${path}`);
  cur[last] = value;
  return copy;
}

// De vertaalde versies van demo-config.yaml, opgebouwd uit de overlay ernaast.
//
// De samenvoeging gebeurt hier en niet in de browser: een pad dat nergens heen
// wijst hoort de build te laten falen en niet tijdens een presentatie een lege
// dia op te leveren. Het Nederlands blijft de bron; de overlay zegt per pad wat
// de vertaalde tekst is.
const baseConfig = yaml.load(readFileSync(join(corpusDir, 'demo-config.yaml'), 'utf8'));
for (const code of translatedLocales) {
  const source = `${code}.yaml`;
  const overlay = readCorpusI18n(source);
  if (!Object.keys(overlay).length) continue;
  let translated = baseConfig;
  for (const [path, value] of Object.entries(overlay)) translated = setPath(translated, path, value, source);
  writeFileSync(join(destDir, `demo-config.${code}.yaml`), yaml.dump(translated, { lineWidth: 120 }));
}

// De vertaalde titels van de features en scenario's, en de woordenlijst voor
// veldnamen. Allebei een JS-module en geen asset: `format.js` en `gherkinNl.js`
// lezen ze synchroon en worden zelf door hun eigen tests geïmporteerd, dus een
// fetch erin zou die tests van een netwerkaanroep afhankelijk maken.
//
// De taal is de buitenste sleutel, zodat de consument op `currentLocale()`
// indexeert in plaats van op een taalcode die in de code staat. Een taal zonder
// bestand levert een lege ingang op: elk label en elke titel valt dan terug op
// het Nederlands, wat het gedrag is van vóór de vertaling bestond.
//
// De `.feature`-bestanden zelf blijven Nederlands: de Rust-runner en de
// browser-runner lezen dezelfde bestanden en `just bdd-demo` toetst erop, dus
// de vertaling staat ernaast en is gesleuteld op de Nederlandse titel.
const scenarioTitles = {};
const glossaries = {};
for (const code of translatedLocales) {
  scenarioTitles[code] = readCorpusI18n(`scenarios.${code}.yaml`).titles ?? {};
  const g = readCorpusI18n(`glossary.${code}.yaml`);
  glossaries[code] = { laws: g.laws ?? {}, names: g.names ?? {}, words: g.words ?? {} };
}

/** Een gegenereerde module onder `src/i18n/`, met taal als buitenste sleutel. */
function writeGeneratedModule(name, from, value) {
  writeFileSync(
    resolve(appRoot, 'src', 'i18n', name),
    `// @generated from ${from} by frontend-demo/scripts/copy-demo-corpus.mjs — do not edit.\n` +
      `export default ${JSON.stringify(value, null, 2)};\n`,
  );
}

writeGeneratedModule('scenarioTitles.generated.js', 'corpus/demo/i18n/scenarios.<taal>.yaml', scenarioTitles);
writeGeneratedModule('glossary.generated.js', 'corpus/demo/i18n/glossary.<taal>.yaml', glossaries);

laws.sort((a, b) => a.id.localeCompare(b.id) || a.valid_from.localeCompare(b.valid_from));
scenarios.sort((a, b) => a.path.localeCompare(b.path));
writeFileSync(join(destDir, 'index.json'), JSON.stringify({ laws, scenarios }, null, 2));
// Per taal geteld: een lege ingang valt zo op, en dat is precies het geval dat
// stil terugvalt op het Nederlands in plaats van te falen.
const perLocale = translatedLocales
  .map((code) => {
    const words = Object.keys(glossaries[code].words).length;
    return `${code}: ${words} woorden, ${Object.keys(scenarioTitles[code]).length} titels`;
  })
  .join('; ');
console.log(
  `demo corpus: ${laws.length} law files, ${scenarios.length} feature files → ${relative(appRoot, destDir)}` +
    (perLocale ? ` (${perLocale})` : ''),
);
