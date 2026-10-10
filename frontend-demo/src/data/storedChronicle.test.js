import { describe, expect, it } from 'vitest';
import { fieldsByArticle, onlyCase, provenanceText, rootOf, storedChronicle, streamEventOf } from './storedChronicle.js';

const cell = {
  id: 'c',
  events: [
    { name: 'a', establishes: 'w#1', chronicle: 'k', stream: 's1', streamFile: 'streams/s1.yaml' },
    { name: 'b', establishes: 'w#2', stage: 'EEN', chronicle: 'k', stream: 's2', streamFile: 'streams/s2.yaml' },
  ],
};
const gram = (id, name, recorded_at, refers_to, chronicle = 'k') => ({ id, name, chronicle, recorded_at, refers_to });

describe('de kroniek vanaf de achterkant', () => {
  const grams = [
    gram('b1', 'b', '2025-01-03T10:00:00+01:00', { op: 'a1' }),
    gram('a1', 'a', '2025-01-02T10:00:00+01:00'),
    gram('x', 'a', '2025-01-01T10:00:00+01:00', undefined, 'andere'),
    gram('c1', 'b', '2025-01-03T10:00:00+01:00', { op: 'b1' }),
    gram('a2', 'a', '2025-01-04T09:00:00+01:00'),
  ];

  it('zet de grammen van de cel in de volgorde van vastleggen, bij gelijke tijd zoals de cel ze geeft', () => {
    const entries = storedChronicle(cell, grams);
    expect(entries.map((e) => e.gram.id)).toEqual(['a1', 'b1', 'c1', 'a2']);
    expect(entries.map((e) => e.position)).toEqual([1, 2, 3, 4]);
  });

  it('laat een gram uit een kroniek van een andere cel weg', () => {
    expect(storedChronicle(cell, grams).some((e) => e.gram.id === 'x')).toBe(false);
  });

  it('koppelt elke gram aan de gebeurtenis in de stroom die hem liet ontstaan', () => {
    const entries = storedChronicle(cell, grams);
    expect(entries[1].event).toMatchObject({ stream: 's2', stage: 'EEN', establishes: 'w#2' });
    expect(streamEventOf(cell, gram('z', 'onbekend', ''))).toBeNull();
  });

  it('wijst een verwijzing naar de plaats van de gram waarnaar zij wijst', () => {
    const entries = storedChronicle(cell, [...grams, gram('d', 'b', '2025-01-05T00:00:00+01:00', { op: 'weg' })]);
    expect(entries[2].references).toEqual([{ role: 'op', id: 'b1', position: 2 }]);
    expect(entries.at(-1).references).toEqual([{ role: 'op', id: 'weg', position: null }]);
  });

  it('vindt via de verwijzingen de gram waar de zaak mee begon', () => {
    const entries = storedChronicle(cell, grams);
    expect(entries.map((e) => e.root)).toEqual(['a1', 'a1', 'a1', 'a2']);
    expect(onlyCase(entries, 'a1').map((e) => e.gram.id)).toEqual(['a1', 'b1', 'c1']);
    expect(onlyCase(entries, null)).toHaveLength(4);
  });

  it('loopt niet rond bij een kring van verwijzingen', () => {
    const p = gram('p', 'b', '', { op: 'q' });
    const q = gram('q', 'b', '', { op: 'p' });
    expect(['p', 'q']).toContain(rootOf(p, new Map([['p', p], ['q', q]])));
  });

  it('volgt bij twee verwijzingen de alfabetisch eerste, zoals de cel (Chronicle::root_of)', () => {
    const r1 = gram('r1', 'a', '');
    const r2 = gram('r2', 'a', '');
    const byId = new Map([['r1', r1], ['r2', r2]]);
    // In de volgorde van het object staat `zaak` eerst; de cel kiest `besluit`.
    expect(rootOf(gram('x', 'b', '', { zaak: 'r1', besluit: 'r2' }), byId)).toBe('r2');
    // Wijst die eerste naar een gram die er niet is, dan stopt de weg: de
    // tweede verwijzing telt niet.
    expect(rootOf(gram('y', 'b', '', { besluit: 'weg', zaak: 'r1' }), byId)).toBe('y');
  });
});

describe('de zaak binnen één kroniek', () => {
  it('volgt een verwijzing niet naar een andere kroniek van dezelfde cel, zoals de cel', () => {
    const twee = {
      id: 'c',
      events: [
        { name: 'a', chronicle: 'k' },
        { name: 'b', chronicle: 'l' },
      ],
    };
    const a1 = gram('a1', 'a', '2025-01-01T10:00:00+01:00');
    const b1 = gram('b1', 'b', '2025-01-02T10:00:00+01:00', { op: 'a1' }, 'l');
    const entries = storedChronicle(twee, [a1, b1]);
    expect(entries.map((e) => [e.gram.id, e.root])).toEqual([
      ['a1', 'a1'],
      ['b1', 'b1'],
    ]);
  });
});

describe('wat de wet over de velden zegt', () => {
  it('groepeert de velden per artikel dat erom vraagt, in volgorde van voorkomen', () => {
    const fields = [
      { name: 'x', declared_by: 'w#1' },
      { name: 'y', declared_by: 'awb#4:2' },
      { name: 'z', declared_by: 'w#1' },
    ];
    expect(fieldsByArticle(fields)).toEqual([
      { article: 'w#1', names: ['x', 'z'], fields: [{ name: 'x', basis: [] }, { name: 'z', basis: [] }] },
      { article: 'awb#4:2', names: ['y'], fields: [{ name: 'y', basis: [] }] },
    ]);
    // Elk veld met de grondslag die de gram ervoor vastlegt; een veld zonder grondslag heeft er geen.
    const groups = fieldsByArticle(fields, { x: ['w#1 lid 2'], y: ['awb#4:2 lid 1', 'beleid#3'] });
    expect(groups[0].fields).toEqual([{ name: 'x', basis: ['w#1 lid 2'] }, { name: 'z', basis: [] }]);
    expect(groups[1].fields).toEqual([{ name: 'y', basis: ['awb#4:2 lid 1', 'beleid#3'] }]);
  });
});

describe('de herkomst van een invoer', () => {
  it('leest elk onderdeel als sleutel en waarde', () => {
    expect(provenanceText({ source: 'decision', stage: 'EEN' })).toBe('source: decision, stage: EEN');
    expect(provenanceText(null)).toBe('');
  });
});
