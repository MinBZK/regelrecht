import { describe, expect, it } from 'vitest';
import { external, leeg, leesPad, opties, veldTekst, zetPad } from './formulier.js';

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
