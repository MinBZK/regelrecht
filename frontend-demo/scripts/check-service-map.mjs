/**
 * Every demo law has an executing organisation in services.yaml.
 *
 * The organisation used to sit in the law file itself, which meant a new law
 * carried its own answer. Now it lives in `services.yaml` under `laws:`, and
 * a law that is missing there silently gets `service: null`: no logo, no
 * colour, no group heading, and no error. This check is what replaces the
 * guarantee the old field gave for free.
 *
 * It also holds the map to real organisations, and refuses an entry for a law
 * that no longer exists, so the map cannot rot in either direction.
 */
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';

const here = dirname(fileURLToPath(import.meta.url));
const corpusDir = resolve(here, '..', '..', 'corpus', 'demo');
const lawsDir = join(corpusDir, 'regulation', 'nl');

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (entry.endsWith('.yaml')) out.push(full);
  }
  return out;
}

const registry = yaml.load(readFileSync(join(corpusDir, 'services.yaml'), 'utf8'));
const organisations = registry.services ?? {};
const map = registry.laws ?? {};

const ids = new Set();
const carriesField = [];
for (const file of walk(lawsDir)) {
  const doc = yaml.load(readFileSync(file, 'utf8'));
  if (!doc?.$id) continue;
  ids.add(doc.$id);
  if (doc.service !== undefined) carriesField.push(file);
}

const problems = [];
for (const id of [...ids].sort()) {
  if (!(id in map)) problems.push(`wet zonder organisatie in services.yaml: ${id}`);
}
for (const [id, code] of Object.entries(map)) {
  if (!ids.has(id)) problems.push(`services.yaml noemt een wet die niet bestaat: ${id}`);
  if (!(code in organisations)) problems.push(`${id} verwijst naar onbekende organisatie ${code}`);
}
for (const file of carriesField) {
  problems.push(`wetsbestand draagt nog een service-veld: ${file}`);
}

if (problems.length) {
  console.error('services.yaml en het democorpus lopen uiteen:\n');
  for (const p of problems) console.error('  ' + p);
  console.error(`\n${problems.length} probleem(en).`);
  process.exit(1);
}
console.log(`services.yaml dekt alle ${ids.size} democwetten, met ${Object.keys(organisations).length} organisaties.`);
