import { describe, expect, it } from 'vitest';
import { external, inEuro, leeg, leesPad, naarFormulier, naarWet, opties, veldLabel, veldTekst, zetPad } from './formulier.js';

describe('leeg', () => {
  it('ziet null, lege tekst en lege lijst als leeg', () => {
    expect(leeg(undefined)).toBe(true);
    expect(leeg(null)).toBe(true);
    expect(leeg('  ')).toBe(true);
    expect(leeg([])).toBe(true);
  });

  it('ziet 0 en false als ingevuld', () => {
    expect(leeg(0)).toBe(false);
    expect(leeg(false)).toBe(false);
    expect(leeg('x')).toBe(false);
  });
});

describe('zetPad en leesPad', () => {
  it('nesten een naam met punten', () => {
    const doel = {};
    zetPad(doel, 'a.b.c', 3);
    expect(doel).toEqual({ a: { b: { c: 3 } } });
    expect(leesPad(doel, 'a.b.c')).toBe(3);
  });

  it('geeft null voor een ontbrekend pad', () => {
    expect(leesPad({ a: {} }, 'a.b.c')).toBeNull();
  });
});

describe('external', () => {
  it('laat lege velden weg en nest namen met punten', () => {
    expect(external({ naam: 'x', leeg: '', 'adres.plaats': 'Utrecht', nul: 0 })).toEqual({
      naam: 'x',
      adres: { plaats: 'Utrecht' },
      nul: 0,
    });
  });

  it('haalt lege cellen en lege regels uit een tabel', () => {
    expect(external({ rijen: [{ a: 1, b: '' }, { a: '', b: null }] })).toEqual({
      rijen: [{ a: 1 }],
    });
  });

  it('laat een tabel met alleen lege regels weg', () => {
    expect(external({ rijen: [{ a: '' }] })).toEqual({});
  });
});

describe('opties', () => {
  it('maakt van tekst en objecten {waarde, label}', () => {
    expect(opties(['a', { waarde: 2, label: 'twee' }, { value: 3 }])).toEqual([
      { waarde: 'a', label: 'a' },
      { waarde: 2, label: 'twee' },
      { waarde: 3, label: '3' },
    ]);
  });

  it('is leeg zonder lijst', () => {
    expect(opties(undefined)).toEqual([]);
  });
});

describe('veldTekst', () => {
  it('leest detail.value van een nldd-veld, anders target.value', () => {
    expect(veldTekst({ detail: { value: 'a' }, target: { value: 'b' } })).toBe('a');
    expect(veldTekst({ target: { value: 'b' } })).toBe('b');
    expect(veldTekst({})).toBe('');
  });
});

describe('een bedrag in de eenheid van de regeling', () => {
  const cent = { naam: 'bedrag', label: 'Bedrag', type: 'bedrag', eenheid: 'eurocent' };
  const euro = { ...cent, eenheid: 'euro' };
  const kaal = { ...cent, eenheid: undefined };

  it('vraagt eurocent en euro in euro', () => {
    expect(inEuro(cent)).toBe(true);
    expect(inEuro(euro)).toBe(true);
    expect(inEuro(kaal)).toBe(false);
    expect(veldLabel(cent)).toBe('Bedrag (euro)');
    expect(veldLabel({ ...cent, eenheid: 'punten' })).toBe('Bedrag (punten)');
    expect(veldLabel(kaal)).toBe('Bedrag');
  });

  it('rekent alleen eurocent om', () => {
    expect(naarWet(cent, 12.34)).toBe(1234);
    expect(naarFormulier(cent, 1234)).toBe(12.34);
    expect(naarWet(euro, 12.34)).toBe(12.34);
    expect(naarWet(kaal, 12)).toBe(12);
    expect(naarWet(cent, null)).toBe(null);
    expect(naarWet({ naam: 'n', type: 'getal', eenheid: 'eurocent' }, 5)).toBe(5);
  });
});
