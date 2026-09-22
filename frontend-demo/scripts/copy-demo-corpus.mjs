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
    ` (woordenlijst: ${wordCount} woorden)`,
);
