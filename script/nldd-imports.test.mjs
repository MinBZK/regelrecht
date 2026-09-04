// De guard die de per-component imports bij de gebruikte tags houdt, leest de
// bron met twee reguliere expressies. De tweede — de naam tussen quotes, voor
// componenten die in JS worden aangemaakt — matcht in markdown ook gewone
// opmaak: `nldd-tab-bar` in een tabel die beschrijft wat de *frontend*
// gebruikt, is proza en geen gebruik. Dat verschil is precies wat deze test
// vastlegt; zonder hem dwingt elke documentatiezin een dode import af, en
// zonder de bestaanscontrole aan de andere kant blijft een tikfout of een door
// het ontwerpsysteem verwijderd component ongemerkt in de tekst staan.
//
// Node's ingebouwde runner, geen dependency, en geen @nldd/design-system nodig:
// beide functies hieronder werken op een bronmap en een set entry points.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { usedTags, resolveEntries } from './nldd-imports.mjs';

const EXTENSIONS = new Set(['vue', 'js', 'ts', 'astro', 'mdx', 'md', 'html']);

/** Een wegwerp-bronmap met de opgegeven bestanden. */
function sourceTree(files) {
  const dir = mkdtempSync(join(tmpdir(), 'nldd-imports-'));
  for (const [name, text] of Object.entries(files)) writeFileSync(join(dir, name), text);
  return dir;
}

test('markup telt als gebruik, in elke ondersteunde extensie', () => {
  const dir = sourceTree({
    'App.vue': '<template><nldd-button></nldd-button></template>',
    'page.md': 'Een voorbeeld:\n\n<nldd-hero></nldd-hero>\n',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered].sort(), ['button', 'hero']);
  assert.deepEqual([...mentioned], []);
});

test('een tag tussen quotes buiten markdown telt als gebruik', () => {
  // De organisatiekiezer bouwt zijn rijen in JS; markup alleen zou die missen.
  const dir = sourceTree({
    'picker.js': "document.createElement('nldd-list-item');",
  });
  const { rendered } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], ['list-item']);
});

test('een naam tussen backticks in markdown is proza, geen gebruik', () => {
  const dir = sourceTree({
    'frontend.md': '| **Navigatie** | `nldd-tab-bar`, `nldd-menu` |\n',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], [], 'proza mag geen import afdwingen');
  assert.deepEqual([...mentioned].sort(), ['menu', 'tab-bar']);
});

test('markup wint van proza in hetzelfde markdown-bestand', () => {
  // Een pagina die het component ook echt rendert, heeft de import wél nodig.
  const dir = sourceTree({
    'demo.md': 'Zo ziet `nldd-hero` eruit:\n\n<nldd-hero></nldd-hero>\n',
  });
  const { rendered } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], ['hero']);
});

test('een genoemde naam die geen entry point heeft blijft onopgelost', () => {
  // Dit is wat de bestaanscontrole waard maakt: de tekst noemt iets dat het
  // ontwerpsysteem niet (meer) levert.
  const { needed, unresolved } = resolveEntries(new Set(['tab-bra']), new Set(['tab-bar']));
  assert.deepEqual(needed, []);
  assert.deepEqual(unresolved, ['tab-bra']);
});

test('een subcomponent valt onder het langste entry point dat hem bevat', () => {
  const entries = new Set(['button', 'button-bar']);
  const { needed, unresolved } = resolveEntries(new Set(['button-bar-divider']), entries);
  assert.deepEqual(needed, ['button-bar']);
  assert.deepEqual(unresolved, []);
});
