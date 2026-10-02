// The two vocabulary rules of check-prose-style.mjs, run against small files.
//
// These rules gate CI over the whole docs set, so both directions matter: a
// term that slips through is a rule that guards nothing, and an ordinary
// sentence that trips it is a rule people learn to work around.
//
//   node --test .claude/skills/docs-writing/check-prose-style.test.mjs

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const SCRIPT = fileURLToPath(new URL('./check-prose-style.mjs', import.meta.url));
const ONLY = '--only=rulework-compound,instance-term';

/** Run the linter on one file with the given text; return its rule ids, in order. */
function findings(text, { name = 'page.md', dir = '' } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'prose-style-'));
  const folder = join(root, dir);
  mkdirSync(folder, { recursive: true });
  const file = join(folder, name);
  writeFileSync(file, text);
  let out = '';
  try {
    out = execFileSync('node', [SCRIPT, ONLY, file], { encoding: 'utf8', stdio: 'pipe' });
  } catch (err) {
    out = `${err.stdout ?? ''}${err.stderr ?? ''}`;
  }
  return [...out.matchAll(/^\s+\d+: \[([\w-]+)\]/gm)].map((m) => m[1]);
}

test('a compound of rulework with schema, format or language is rejected', () => {
  for (const text of [
    'Het regelwerkschema bewaakt de structuur.',
    'Het regelwerk-schema bewaakt de structuur.',
    'De regelwerkformaten lopen uiteen.',
    'Een regelwerktaal bestaat niet.',
    'The rulework schema guards the structure.',
    'Several rulework formats exist.',
    'The ruleworks schema is versioned.',
    'A rulework-language would be a mistake.',
    'This describes the rulework\nschema in full.',
  ]) {
    assert.deepEqual(findings(text), ['rulework-compound'], text);
  }
});

test('an ordinary sentence that puts the two words side by side passes', () => {
  for (const text of [
    'We controleren of het regelwerk schema-valide is.',
    'Elk regelwerk schema v0.7.1 volgt, wordt gemigreerd.',
    'Check whether the rulework schema-valid is, in the Dutch word order.',
    '- regelwerk\n- schema\n',
    '- rulework\n- schema\n',
    '## Het regelwerk\n\nTaal en teken horen bij het schema.\n',
    'A rulework conforms to the schema, and is written in the law format.',
  ]) {
    assert.deepEqual(findings(text), [], text);
  }
});

test('the older words for a rulework are rejected', () => {
  for (const text of [
    'Het wetsbestand staat in de map.',
    'Twee wetbestanden zijn nagekeken.',
    'A law file holds one version.',
    'All law YAML files conform to the schema.',
    'The engine loads the law\nfiles in order.',
  ]) {
    assert.deepEqual(findings(text), ['instance-term'], text);
  }
});

test('words that only look like the older ones pass', () => {
  for (const text of [
    'The court keeps its case-law files for ten years.',
    'The court keeps its case law files for ten years.',
    'De rechtsbestanden zijn hier niet bedoeld.',
    'A rulework has versions; each version is one YAML file.',
    'See [The rulework](/reference/schema#law-file) for the keys.',
  ]) {
    assert.deepEqual(findings(text), [], text);
  }
});

test('code is not prose', () => {
  assert.deepEqual(findings('Run `cat law file` first.\n\n```\nthe law file\n```\n'), []);
});

test('an RFC is a dated document and keeps the words it was written in', () => {
  const text = 'A law file on schema v0.5.x still carries the field.';
  assert.deepEqual(findings(text, { dir: 'rfcs', name: 'rfc-012.md' }), []);
  assert.deepEqual(findings(text, { dir: 'docs', name: 'page.md' }), ['instance-term']);
});

test('a compound is rejected in an RFC too', () => {
  assert.deepEqual(findings('The rulework schema is new.', { dir: 'rfcs', name: 'rfc-099.md' }), [
    'rulework-compound',
  ]);
});

test('an unknown rule id is an error, not a silent pass', () => {
  assert.throws(() => execFileSync('node', [SCRIPT, '--only=no-such-rule'], { stdio: 'pipe' }));
});
