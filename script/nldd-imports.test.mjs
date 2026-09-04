// De guard die de per-component imports bij de gebruikte tags houdt, leest de
// bron met twee reguliere expressies. De tweede — de naam tussen quotes, voor
// componenten die in JS worden aangemaakt — matcht in markdown ook gewone
// opmaak: `nldd-tab-bar` in een tabel die beschrijft wat de *frontend* rendert,
// is proza en geen gebruik. Dat verschil is precies wat deze test vastlegt;
// zonder hem dwingt elke documentatiezin een dode import af, en zonder de
// bestaanscontrole aan de andere kant blijft een component dat het
// ontwerpsysteem heeft verwijderd ongemerkt in de tekst staan.
//
// Node's ingebouwde runner, geen dependency, en geen @nldd/design-system nodig:
// alles hieronder werkt op een bronmap en een meegegeven set entry points.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';

import { EXTENSIONS, usedTags, resolveUsage, resolveEntries } from './nldd-imports.mjs';

/** Een bronmap die na de test weer verdwijnt. Sleutels mogen submappen zijn. */
function sourceTree(t, files) {
  const dir = mkdtempSync(join(tmpdir(), 'nldd-imports-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [name, text] of Object.entries(files)) {
    const path = join(dir, name);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, text);
  }
  return dir;
}

test('markup telt als gebruik, in elke ondersteunde extensie', (t) => {
  const dir = sourceTree(t, {
    'App.vue': '<template><nldd-button></nldd-button></template>',
    'page.md': 'Een voorbeeld:\n\n<nldd-hero></nldd-hero>\n',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered].sort(), ['button', 'hero']);
  assert.deepEqual([...mentioned], []);
});

test('een tag tussen quotes buiten markdown telt als gebruik', (t) => {
  // De organisatiekiezer bouwt zijn rijen in JS; markup alleen zou die missen.
  const dir = sourceTree(t, {
    'picker.js': "document.createElement('nldd-list-item');",
  });
  const { rendered } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], ['list-item']);
});

test('een backtick buiten markdown is een template literal, dus gebruik', (t) => {
  // De uitzondering hangt aan markdown, niet aan het aanhalingsteken: in JS
  // levert een backtick gewoon een string op.
  const dir = sourceTree(t, {
    'chrome.js': 'document.querySelector(`nldd-toolbar`);',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], ['toolbar']);
  assert.deepEqual([...mentioned], []);
});

test('een naam tussen backticks in markdown is proza, geen gebruik', (t) => {
  const dir = sourceTree(t, {
    'frontend.md': '| **Navigatie** | `nldd-tab-bar`, `nldd-menu` |\n',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], [], 'proza mag geen import afdwingen');
  assert.deepEqual([...mentioned].sort(), ['menu', 'tab-bar']);
});

test('mdx krijgt dezelfde proza-uitzondering als md', (t) => {
  // Een documentatiepagina die één component nodig heeft wordt .mdx; zonder
  // deze regel zou die hernoeming de dode imports terugbrengen.
  const dir = sourceTree(t, {
    'guide.mdx': 'De frontend gebruikt `nldd-sheet` hiervoor.\n',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], []);
  assert.deepEqual([...mentioned], ['sheet']);
});

test('markup wint van proza in hetzelfde markdown-bestand', (t) => {
  // Een pagina die het component ook echt rendert, heeft de import wél nodig.
  const dir = sourceTree(t, {
    'demo.md': 'Zo ziet `nldd-hero` eruit:\n\n<nldd-hero></nldd-hero>\n',
  });
  const { rendered } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], ['hero']);
});

test('node_modules en verborgen mappen blijven buiten de scan', (t) => {
  // Een geïnstalleerde @nldd/design-system staat vol met nldd-markup; zou die
  // meetellen, dan eist de guard prompt alle ~110 entry points.
  const dir = sourceTree(t, {
    'src/App.vue': '<nldd-button></nldd-button>',
    'node_modules/@nldd/design-system/sheet.js': '<nldd-sheet>',
    '.cache/oud.vue': '<nldd-hero>',
  });
  const { rendered } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], ['button']);
});

test('proza levert geen import op, maar moet wel een bestaand component noemen', (t) => {
  const entries = new Set(['button', 'tab-bar']);

  const alleenGenoemd = resolveUsage(
    { rendered: new Set(['button']), mentioned: new Set(['tab-bar']) },
    entries,
  );
  assert.deepEqual(alleenGenoemd.needed, ['button'], 'een genoemd component hoort niet in de lijst');
  assert.deepEqual(alleenGenoemd.unknown, []);

  const verdwenen = resolveUsage(
    { rendered: new Set(['button']), mentioned: new Set(['step-indicator']) },
    entries,
  );
  assert.deepEqual(verdwenen.needed, ['button']);
  assert.deepEqual(
    verdwenen.unknown,
    ['step-indicator'],
    'proza over een verdwenen component hoort de build te breken',
  );
});

test('een onbekende naam uit markup landt in unknown, en niet dubbel', (t) => {
  const { needed, unknown } = resolveUsage(
    { rendered: new Set(['tab-bra']), mentioned: new Set(['tab-bra', 'nonsens']) },
    new Set(['tab-bar']),
  );
  assert.deepEqual(needed, []);
  assert.deepEqual(unknown, ['nonsens', 'tab-bra']);
});

test('een subcomponent valt onder het langste entry point dat hem bevat', (t) => {
  const entries = new Set(['button', 'button-bar']);
  const { needed, unresolved } = resolveEntries(new Set(['button-bar-divider']), entries);
  assert.deepEqual(needed, ['button-bar']);
  assert.deepEqual(unresolved, []);
});
