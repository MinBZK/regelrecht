// De guard die de per-component imports bij de gebruikte tags houdt, leest de
// bron met twee reguliere expressies: de markup-vorm, en de naam tussen quotes
// voor componenten die in JS worden aangemaakt. Die tweede matcht in markdown
// ook inline-code-opmaak: `nldd-tab-bar` in een tabel die beschrijft wat de
// *frontend* rendert, is proza en geen gebruik. Alleen de backtick is die
// uitzondering — een enkele of dubbele quote telt ook in markdown gewoon mee,
// want rauwe HTML komt op de pagina terecht.
//
// Dat onderscheid is wat deze tests vastleggen. Zonder de uitzondering dwingt
// elke documentatiezin een dode import af; zonder de bestaanscontrole aan de
// andere kant blijft proza staan over een component dat het ontwerpsysteem
// niet meer levert.
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
  // Eén bestand per extensie, elk met een eigen component: valt er een
  // extensie uit EXTENSIONS, dan verdwijnt precies dat component uit de
  // uitkomst. Zonder deze test is `astro` eruit halen onzichtbaar, en dat
  // scheelt de docs-site 45 van de 45 imports terwijl de guard groen blijft.
  const tags = ['button', 'hero', 'card', 'list', 'divider', 'tag', 'sheet'];
  const files = Object.fromEntries(
    [...EXTENSIONS].map((ext, i) => [`page.${ext}`, `<nldd-${tags[i]}></nldd-${tags[i]}>`]),
  );
  assert.equal(EXTENSIONS.size, tags.length, 'geef elke nieuwe extensie een eigen component');

  const { rendered, mentioned } = usedTags(sourceTree(t, files), EXTENSIONS);
  assert.deepEqual([...rendered].sort(), [...tags].sort());
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

test('echte quotes in markdown tellen wél als gebruik', (t) => {
  // Rauwe HTML in markdown komt op de pagina terecht en draait daar ook, dus
  // dit is geen proza. Alleen de backtick is de uitzondering; zou de hele
  // markdown-regel op de extensie hangen, dan verdwijnt deze import.
  const dir = sourceTree(t, {
    'demo.md': "<script>document.createElement('nldd-sheet');</script>\n",
    'demo.mdx': '<Demo tag="nldd-hero" />\n',
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered].sort(), ['hero', 'sheet']);
  assert.deepEqual([...mentioned], []);
});

test('een naam tussen ongelijke aanhalingstekens telt niet mee', (t) => {
  const dir = sourceTree(t, {
    'raar.js': "const a = 'nldd-sheet`; const b = `nldd-menu';",
  });
  const { rendered, mentioned } = usedTags(dir, EXTENSIONS);
  assert.deepEqual([...rendered], []);
  assert.deepEqual([...mentioned], []);
});

test('mdx krijgt dezelfde backtick-uitzondering als md', (t) => {
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

test('proza levert geen import op, maar moet wel een bestaand component noemen', () => {
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

test('proza over een subcomponent lost op via zijn ouder', () => {
  // De keerzijde van de prefix-fallback, en de reden dat de belofte hierboven
  // alleen geldt voor namen op het hoogste niveau: `nldd-menu-item` blijft
  // stil oplossen zolang `menu` bestaat.
  const { needed, unknown } = resolveUsage(
    { rendered: new Set(), mentioned: new Set(['menu-item']) },
    new Set(['menu']),
  );
  assert.deepEqual(needed, [], 'een genoemde subcomponent hoort geen import af te dwingen');
  assert.deepEqual(unknown, []);
});

test('een onbekende naam uit markup landt in unknown, en niet dubbel', () => {
  const { needed, unknown } = resolveUsage(
    { rendered: new Set(['tab-bra']), mentioned: new Set(['tab-bra', 'nonsens']) },
    new Set(['tab-bar']),
  );
  assert.deepEqual(needed, []);
  assert.deepEqual(unknown, ['nonsens', 'tab-bra']);
});

test('een subcomponent valt onder het langste entry point dat hem bevat', () => {
  const entries = new Set(['button', 'button-bar']);
  const { needed, unresolved } = resolveEntries(new Set(['button-bar-divider']), entries);
  assert.deepEqual(needed, ['button-bar']);
  assert.deepEqual(unresolved, []);
});
