// @vitest-environment node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createSlide, indexCorpus, listDecks, lookupArticle, lookupDeckArticle, readAllLaws, readFeature, readLawVersions, pickVersion, readDeck, resolveDeckFile, writeDeckFile } from './presenterApi.js';

let tmp;
let decks;
beforeEach(() => {
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'presenter-'));
  decks = path.join(tmp, 'decks');
  fs.mkdirSync(path.join(decks, 'demo'), { recursive: true });
  fs.writeFileSync(path.join(decks, 'demo', '01-titel.md'), '# Titel\n');
  fs.writeFileSync(path.join(decks, 'demo', '02-verder.md'), '# Verder\n');
  fs.writeFileSync(path.join(tmp, 'geheim.md'), 'niet aankomen\n');
});
afterEach(() => fs.rmSync(tmp, { recursive: true, force: true }));

describe('resolveDeckFile', () => {
  it('accepts slide files and deck.yaml inside the deck', () => {
    expect(resolveDeckFile(decks, 'demo', '01-titel.md')).toBe(path.join(decks, 'demo', '01-titel.md'));
    expect(resolveDeckFile(decks, 'demo', 'deck.yaml')).toBe(path.join(decks, 'demo', 'deck.yaml'));
  });

  it.each([
    ['demo', '../../geheim.md'],
    ['demo', '../01-titel.md'],
    ['demo', '/etc/passwd.md'],
    ['demo', 'sub/x.md'],
    ['demo', 'script.js'],
    ['demo', 'other.yaml'],
    ['..', 'geheim.md'],
    ['.hidden', 'x.md'],
    ['a/b', 'x.md'],
  ])('refuses deck %s file %s', (deck, file) => {
    expect(() => resolveDeckFile(decks, deck, file)).toThrow(expect.objectContaining({ status: 400 }));
  });
});

describe('readDeck / writeDeckFile', () => {
  it('reads the slides in name order with their mtime', () => {
    const d = readDeck(decks, 'demo');
    expect(d.slides.map((s) => s.file)).toEqual(['01-titel.md', '02-verder.md']);
    expect(typeof d.slides[0].mtime).toBe('number');
    expect(d.metaMtime).toBeNull();
  });

  it('writes when the mtime matches and refuses with 409 when it does not', () => {
    const { mtime } = readDeck(decks, 'demo').slides[0];
    writeDeckFile(decks, 'demo', '01-titel.md', '# Nieuw\n', mtime);
    expect(fs.readFileSync(path.join(decks, 'demo', '01-titel.md'), 'utf8')).toBe('# Nieuw\n');
    expect(() => writeDeckFile(decks, 'demo', '01-titel.md', '# Oud\n', mtime - 1000)).toThrow(expect.objectContaining({ status: 409 }));
  });

  it('creates deck.yaml only when the client expected it not to exist', () => {
    writeDeckFile(decks, 'demo', 'deck.yaml', 'presenter: A\n', null);
    expect(() => writeDeckFile(decks, 'demo', 'deck.yaml', 'presenter: B\n', null)).toThrow(expect.objectContaining({ status: 409 }));
  });

  it('inserts a new slide right after the current one, also when inserting there again', () => {
    const { file: a } = createSlide(decks, 'demo', '01-titel.md');
    expect(readDeck(decks, 'demo').slides.map((s) => s.file)).toEqual(['01-titel.md', a, '02-verder.md']);
    const { file: b } = createSlide(decks, 'demo', '01-titel.md');
    expect(readDeck(decks, 'demo').slides.map((s) => s.file)).toEqual(['01-titel.md', b, a, '02-verder.md']);
    const { file: c } = createSlide(decks, 'demo', b);
    expect(readDeck(decks, 'demo').slides.map((s) => s.file)).toEqual(['01-titel.md', b, c, a, '02-verder.md']);
  });

  it('refuses to insert after something that is not a slide of the deck', () => {
    expect(() => createSlide(decks, 'demo', 'deck.yaml')).toThrow(expect.objectContaining({ status: 400 }));
    expect(() => createSlide(decks, 'demo', 'bestaatniet.md')).toThrow(expect.objectContaining({ status: 400 }));
  });

  it('still lists and opens a deck whose deck.yaml is broken', () => {
    fs.writeFileSync(path.join(decks, 'demo', 'deck.yaml'), 'title: [\n');
    expect(listDecks(decks)).toEqual([{ name: 'demo', title: 'demo', slides: 2 }]);
    expect(readDeck(decks, 'demo').meta).toEqual({});
  });
});

describe('corpus lookup', () => {
  beforeEach(() => {
    const dir = path.join(tmp, 'corpus', 'nl', 'wet', 'voorbeeldwet');
    fs.mkdirSync(dir, { recursive: true });
    for (const [date, text] of [['2024-01-01', 'oud'], ['2025-01-01', 'nieuw']]) {
      fs.writeFileSync(
        path.join(dir, `${date}.yaml`),
        `$id: voorbeeldwet\nname: Voorbeeldwet\nvalid_from: '${date}'\narticles:\n  - number: '2'\n    text: ${text}\n`,
      );
    }
  });

  it('picks the newest version on or before the date', () => {
    expect(pickVersion(['2024-01-01', '2025-01-01'], '2024-06-01')).toBe('2024-01-01');
    expect(pickVersion(['2024-01-01', '2025-01-01'], '2025-01-01')).toBe('2025-01-01');
    expect(pickVersion(['2024-01-01', '2025-01-01'], '2023-01-01')).toBe('2024-01-01');
  });

  it('finds an article by law id and date', () => {
    const index = indexCorpus([path.join(tmp, 'corpus')]);
    expect(lookupArticle(index, { law: 'voorbeeldwet', article: '2', date: '2024-03-01' }).article.text).toBe('oud');
    expect(lookupArticle(index, { law: 'voorbeeldwet', article: '2', date: '2026-01-01' })).toMatchObject({
      law: { name: 'Voorbeeldwet', valid_from: '2025-01-01' },
      article: { text: 'nieuw' },
    });
  });

  it('reports a missing law or article as 404', () => {
    const index = indexCorpus([path.join(tmp, 'corpus')]);
    expect(() => lookupArticle(index, { law: 'bestaatniet', article: '1' })).toThrow(expect.objectContaining({ status: 404 }));
    expect(() => lookupArticle(index, { law: 'voorbeeldwet', article: '99' })).toThrow(expect.objectContaining({ status: 404 }));
  });

  it('finds the real zorgtoeslag law in the repository corpus', () => {
    const index = indexCorpus([path.resolve(import.meta.dirname, '../../corpus/regulation')]);
    const { article } = lookupArticle(index, { law: 'wet_op_de_zorgtoeslag', article: '2', date: '2025-01-01' });
    expect(article.machine_readable.execution.input.length).toBeGreaterThan(0);
  });
});

describe('law YAML in the deck folder', () => {
  beforeEach(() => {
    fs.writeFileSync(path.join(decks, 'demo', 'variant.yaml'), "$id: variant\nname: Variantwet\nvalid_from: '2027-01-01'\narticles:\n  - number: '1'\n    text: nieuw\n");
    fs.writeFileSync(path.join(decks, 'demo', 'kapot.yaml'), 'articles: [\n');
    fs.writeFileSync(path.join(decks, 'demo', 'deck.yaml'), 'title: T\n');
  });

  it('reads an article from a law file next to the slides', () => {
    expect(lookupDeckArticle(decks, { deck: 'demo', file: 'variant.yaml', article: '1' })).toMatchObject({
      law: { id: 'variant', name: 'Variantwet', valid_from: '2027-01-01' },
      article: { text: 'nieuw' },
    });
  });

  it.each([['../geheim.yaml'], ['sub/x.yaml'], ['deck.yaml'], ['01-titel.md'], ['/etc/x.yaml']])('refuses %s', (file) => {
    expect(() => lookupDeckArticle(decks, { deck: 'demo', file, article: '1' })).toThrow(expect.objectContaining({ status: 400 }));
  });

  it('says so when the file is missing or broken', () => {
    expect(() => lookupDeckArticle(decks, { deck: 'demo', file: 'weg.yaml', article: '1' })).toThrow(expect.objectContaining({ status: 404 }));
    expect(() => lookupDeckArticle(decks, { deck: 'demo', file: 'kapot.yaml', article: '1' })).toThrow(expect.objectContaining({ status: 422 }));
  });

  it('does not list a law file as a slide', () => {
    expect(readDeck(decks, 'demo').slides.map((s) => s.file)).toEqual(['01-titel.md', '02-verder.md']);
  });
});

describe('scenarios and law versions for the reken block', () => {
  let index;
  beforeEach(() => {
    const dir = path.join(tmp, 'corpus', 'nl', 'wet', 'voorbeeldwet');
    fs.mkdirSync(path.join(dir, 'scenarios'), { recursive: true });
    fs.writeFileSync(path.join(dir, '2024-01-01.yaml'), "$id: voorbeeldwet\nvalid_from: '2024-01-01'\n");
    fs.writeFileSync(path.join(dir, '2025-01-01.yaml'), "$id: voorbeeldwet\nvalid_from: '2025-01-01'\n");
    fs.writeFileSync(path.join(dir, 'scenarios', 'recht.feature'), 'Feature: Recht\n');
    fs.writeFileSync(path.join(decks, 'demo', 'eigen.feature'), 'Feature: Eigen casus\n');
    fs.writeFileSync(path.join(decks, 'demo', 'variant.yaml'), "$id: voorbeeldwet\nvalid_from: '2025-01-01'\n# variant\n");
    fs.writeFileSync(path.join(decks, 'demo', 'andere.yaml'), '$id: iets_anders\n');
    index = indexCorpus([path.join(tmp, 'corpus')]);
  });

  it('reads a scenario next to a law, or one in the deck folder', () => {
    expect(readFeature(index, decks, { feature: 'voorbeeldwet/recht.feature' }).text).toBe('Feature: Recht\n');
    expect(readFeature(index, decks, { feature: 'eigen.feature', deck: 'demo' }).text).toBe('Feature: Eigen casus\n');
  });

  it.each([
    ['voorbeeldwet/../../geheim.md'],
    ['voorbeeldwet/recht.yaml'],
    ['a/b/c.feature'],
    ['../eigen.feature'],
    ['eigen.md'],
  ])('refuses scenario %s', (feature) => {
    expect(() => readFeature(index, decks, { feature, deck: 'demo' })).toThrow(expect.objectContaining({ status: 400 }));
  });

  it('reports a missing scenario or law as 404', () => {
    expect(() => readFeature(index, decks, { feature: 'bestaatniet/x.feature' })).toThrow(expect.objectContaining({ status: 404 }));
    expect(() => readFeature(index, decks, { feature: 'voorbeeldwet/weg.feature' })).toThrow(expect.objectContaining({ status: 404 }));
  });

  it('gives every corpus version, then the deck files with the same $id', () => {
    const { versions } = readLawVersions(index, decks, { law: 'voorbeeldwet', deck: 'demo' });
    expect(versions.map((v) => v.source)).toEqual(['corpus 2024-01-01', 'corpus 2025-01-01', 'variant.yaml']);
    expect(readLawVersions(index, decks, { law: 'voorbeeldwet' }).versions).toHaveLength(2);
  });

  it('finds a law that only exists in the deck folder, and 404s one that exists nowhere', () => {
    expect(readLawVersions(index, decks, { law: 'iets_anders', deck: 'demo' }).versions.map((v) => v.source)).toEqual(['andere.yaml']);
    expect(() => readLawVersions(index, decks, { law: 'nergens', deck: 'demo' })).toThrow(expect.objectContaining({ status: 404 }));
  });
});

describe('corpus index by $id', () => {
  beforeEach(() => {
    const gm = path.join(tmp, 'corpus', 'nl', 'gemeentelijke_verordening', 'amsterdam', 'apv_erfgrens');
    fs.mkdirSync(gm, { recursive: true });
    fs.writeFileSync(path.join(gm, '2025-01-01.yaml'), "---\n$id: apv_erfgrens_amsterdam\nvalid_from: '2025-01-01'\n");
    const wet = path.join(tmp, 'corpus', 'nl', 'wet', 'hoofdwet');
    fs.mkdirSync(wet, { recursive: true });
    fs.writeFileSync(path.join(wet, '2025-01-01.yaml'), "$id: hoofdwet\nvalid_from: '2025-01-01'\n");
    const poc = path.join(tmp, 'poc', 'hoofdwet');
    fs.mkdirSync(poc, { recursive: true });
    fs.writeFileSync(path.join(poc, '2025-01-01.yaml'), "$id: hoofdwet\n# poc-variant\n");
    fs.writeFileSync(path.join(decks, 'demo', 'eigen.yaml'), "$id: hoofdwet\nvalid_from: '2025-01-01'\n# deck\n");
  });

  it('finds a law by the $id in its file, and by its folder name as an alias', () => {
    const index = indexCorpus([path.join(tmp, 'corpus')]);
    expect(index.get('apv_erfgrens_amsterdam').id).toBe('apv_erfgrens_amsterdam');
    expect(index.get('apv_erfgrens').id).toBe('apv_erfgrens_amsterdam');
  });

  it('lets the first root win when two roots have the same $id', () => {
    const index = indexCorpus([path.join(tmp, 'corpus'), path.join(tmp, 'poc')]);
    expect(index.get('hoofdwet').dir).toBe(path.join(tmp, 'corpus', 'nl', 'wet', 'hoofdwet'));
  });

  it('gives the reken engine every law of the main root, then the deck laws last', () => {
    const root = path.join(tmp, 'corpus');
    const index = indexCorpus([root, path.join(tmp, 'poc')]);
    const all = readAllLaws(index, decks, { deck: 'demo', root }).versions;
    expect(all.map((v) => `${v.law} ${v.source}`).sort()).toEqual(
      ['apv_erfgrens_amsterdam corpus 2025-01-01', 'hoofdwet corpus 2025-01-01', 'hoofdwet eigen.yaml'].sort(),
    );
    expect(all.at(-1)).toMatchObject({ law: 'hoofdwet', source: 'eigen.yaml' });
    expect(all.some((v) => v.text.includes('poc-variant'))).toBe(false);
  });
});
