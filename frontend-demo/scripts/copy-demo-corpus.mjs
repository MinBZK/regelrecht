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

// De Engelse woordenlijst voor veldnamen wordt een JS-module en geen asset:
// `format.js` leest hem synchroon en wordt zelf door zijn eigen tests
// geïmporteerd, dus een fetch erin zou die tests van een netwerkaanroep
// afhankelijk maken. Ontbreekt de lijst, dan is hij leeg en valt elk label
// terug op het Nederlands — dat is het gedrag vóór de woordenlijst bestond.
// De Engelse versie van demo-config.yaml en profiles.yaml, opgebouwd uit de
// overlay ernaast.
//
// De samenvoeging gebeurt hier en niet in de browser: een pad dat nergens heen
// wijst hoort de build te laten falen en niet tijdens een presentatie een lege
// dia op te leveren. Het Nederlands blijft de bron; de overlay zegt per pad wat
// de Engelse tekst is.
//
// Eén overlaybestand voedt twee documenten, en het eerste padsegment wijst aan
// welk: `profiles.` gaat naar profiles.yaml, de rest naar demo-config.yaml. Dat
// is een keuze vóór twee losse bestanden, omdat de portaalkop van een persona
// al in demo-config.yaml staat en zijn beschrijving in profiles.yaml: wie de
// ene vertaalt wil de andere ernaast zien staan, niet in een ander bestand.
const overlayFile = join(corpusDir, 'i18n', 'en.yaml');
if (existsSync(overlayFile)) {
  const overlay = yaml.load(readFileSync(overlayFile, 'utf8')) ?? {};
  const config = yaml.load(readFileSync(join(corpusDir, 'demo-config.yaml'), 'utf8'));
  const profilesNl = yaml.load(readFileSync(join(corpusDir, 'profiles.yaml'), 'utf8'));

  /** Zet `value` op `path` in een kopie van `node`, zonder het origineel te raken. */
  function setPath(node, path, value) {
    const parts = path.split('.');
    const copy = Array.isArray(node) ? [...node] : { ...node };
    let cur = copy;
    for (let i = 0; i < parts.length - 1; i += 1) {
      const key = Array.isArray(cur) ? Number(parts[i]) : parts[i];
      const child = cur[key];
      if (child === null || child === undefined) throw new Error(`en.yaml wijst naar een pad dat niet bestaat: ${path}`);
      cur[key] = Array.isArray(child) ? [...child] : { ...child };
      cur = cur[key];
    }
    const last = Array.isArray(cur) ? Number(parts.at(-1)) : parts.at(-1);
    if (cur[last] === undefined) throw new Error(`en.yaml wijst naar een pad dat niet bestaat: ${path}`);
    cur[last] = value;
    return copy;
  }

  // Welk document een pad bedoelt, blijkt uit welk document het pad heeft.
  //
  // Op het eerste segment routeren kan hier niet: béide documenten hebben een
  // `profiles:`-tak. In demo-config.yaml staat die op naam (`profiles.merijn.
  // portal_heading`), in profiles.yaml op BSN (`profiles.999100001.description`).
  // Routeren op de vorm van het tweede segment zou werken tot iemand een
  // persona `claudia` in profiles.yaml zet, en dan stil de verkeerde kant op
  // vallen. `hasPath` kijkt gewoon.
  //
  // Precies één document moet het pad hebben. Nul betekent een verwijzing die
  // nergens heen wijst, twee betekent dat de vertaling onbedoeld op twee
  // plekken landt; allebei falen de build in plaats van een halve vertaling op
  // te leveren.
  function hasPath(node, path) {
    let cur = node;
    for (const part of path.split('.')) {
      if (cur === null || cur === undefined) return false;
      cur = Array.isArray(cur) ? cur[Number(part)] : cur[part];
    }
    return cur !== undefined;
  }

  let english = config;
  let englishProfiles = profilesNl;
  for (const [path, value] of Object.entries(overlay)) {
    const inConfig = hasPath(config, path);
    const inProfiles = hasPath(profilesNl, path);
    if (inConfig && inProfiles) {
      throw new Error(`en.yaml wijst naar een pad dat in twee documenten bestaat: ${path}`);
    }
    if (inProfiles) englishProfiles = setPath(englishProfiles, path, value);
    else english = setPath(english, path, value);
  }
  writeFileSync(join(destDir, 'demo-config.en.yaml'), yaml.dump(english, { lineWidth: 120 }));
  writeFileSync(join(destDir, 'profiles.en.yaml'), yaml.dump(englishProfiles, { lineWidth: 120 }));
}

// De Engelse titels van de features en scenario's.
//
// De `.feature`-bestanden zelf blijven Nederlands: de Rust-runner en de
// browser-runner lezen dezelfde bestanden en `just bdd-demo` toetst erop, dus
// de vertaling staat ernaast en is gesleuteld op de Nederlandse titel. Een
// titel die ontbreekt blijft Nederlands; de stappen eronder zijn canoniek
// Engels en dragen de inhoud.
const scenarioTitlesFile = join(corpusDir, 'i18n', 'scenarios.en.yaml');
const scenarioTitles = existsSync(scenarioTitlesFile) ? yaml.load(readFileSync(scenarioTitlesFile, 'utf8')) ?? {} : {};
writeFileSync(
  resolve(appRoot, 'src', 'i18n', 'scenarioTitles.generated.js'),
  `// @generated from corpus/demo/i18n/scenarios.en.yaml by frontend-demo/scripts/copy-demo-corpus.mjs — do not edit.\nexport default ${JSON.stringify(scenarioTitles.titles ?? {}, null, 2)};\n`,
);

const glossaryFile = join(corpusDir, 'i18n', 'glossary.en.yaml');
const glossary = existsSync(glossaryFile) ? yaml.load(readFileSync(glossaryFile, 'utf8')) ?? {} : {};
const generated = resolve(appRoot, 'src', 'i18n', 'glossary.generated.js');
writeFileSync(
  generated,
  `// @generated from corpus/demo/i18n/glossary.en.yaml by frontend-demo/scripts/copy-demo-corpus.mjs — do not edit.\nexport default ${JSON.stringify(
    { laws: glossary.laws ?? {}, names: glossary.names ?? {}, words: glossary.words ?? {} },
    null,
    2,
  )};\n`,
);

laws.sort((a, b) => a.id.localeCompare(b.id) || a.valid_from.localeCompare(b.valid_from));
scenarios.sort((a, b) => a.path.localeCompare(b.path));
writeFileSync(join(destDir, 'index.json'), JSON.stringify({ laws, scenarios }, null, 2));
const wordCount = Object.keys(glossary.words ?? {}).length;
console.log(
  `demo corpus: ${laws.length} law files, ${scenarios.length} feature files → ${relative(appRoot, destDir)}` +
    ` (woordenlijst: ${wordCount} woorden, ${Object.keys(scenarioTitles.titles ?? {}).length} scenariotitels)`,
);
