import { describe, expect, it } from 'vitest';
import { external, fieldLabel, fieldText, getPath, inEuros, isEmpty, options, setPath, toForm, toLaw } from './form.js';

describe('isEmpty', () => {
  it('treats null, blank text and an empty list as empty', () => {
    expect(isEmpty(undefined)).toBe(true);
    expect(isEmpty(null)).toBe(true);
    expect(isEmpty('  ')).toBe(true);
    expect(isEmpty([])).toBe(true);
  });

  it('treats 0 and false as filled in', () => {
    expect(isEmpty(0)).toBe(false);
    expect(isEmpty(false)).toBe(false);
    expect(isEmpty('x')).toBe(false);
  });
});

describe('setPath and getPath', () => {
  it('nest a dotted name', () => {
    const target = {};
    setPath(target, 'a.b.c', 3);
    expect(target).toEqual({ a: { b: { c: 3 } } });
    expect(getPath(target, 'a.b.c')).toBe(3);
  });

  it('returns null for a missing path', () => {
    expect(getPath({ a: {} }, 'a.b.c')).toBeNull();
  });
});

describe('external', () => {
  it('leaves out empty fields and nests dotted names', () => {
    expect(external({ naam: 'x', leeg: '', 'adres.plaats': 'Utrecht', nul: 0 })).toEqual({
      naam: 'x',
      adres: { plaats: 'Utrecht' },
      nul: 0,
    });
  });

  it('removes empty cells and empty rows from a table', () => {
    expect(external({ rijen: [{ a: 1, b: '' }, { a: '', b: null }] })).toEqual({
      rijen: [{ a: 1 }],
    });
  });

  it('leaves out a table with only empty rows', () => {
    expect(external({ rijen: [{ a: '' }] })).toEqual({});
  });
});

describe('options', () => {
  it('turns strings and {value, label} objects into {value, label}', () => {
    expect(options(['a', { value: 2, label: 'twee' }, { value: 3 }])).toEqual([
      { value: 'a', label: 'a' },
      { value: 2, label: 'twee' },
      { value: 3, label: '3' },
    ]);
  });

  it('is empty without a list', () => {
    expect(options(undefined)).toEqual([]);
  });
});

describe('fieldText', () => {
  it('reads detail.value of an nldd field, otherwise target.value', () => {
    expect(fieldText({ detail: { value: 'a' }, target: { value: 'b' } })).toBe('a');
    expect(fieldText({ target: { value: 'b' } })).toBe('b');
    expect(fieldText({})).toBe('');
  });
});

describe('an amount in the unit of the regulation', () => {
  const cent = { name: 'bedrag', label: 'Bedrag', type: 'amount', unit: 'eurocent' };
  const euro = { ...cent, unit: 'euro' };
  const bare = { ...cent, unit: undefined };

  it('asks for eurocents and euros in euros', () => {
    expect(inEuros(cent)).toBe(true);
    expect(inEuros(euro)).toBe(true);
    expect(inEuros(bare)).toBe(false);
    expect(fieldLabel(cent)).toBe('Bedrag (euro)');
    expect(fieldLabel({ ...cent, unit: 'punten' })).toBe('Bedrag (punten)');
    expect(fieldLabel(bare)).toBe('Bedrag');
  });

  it('converts only eurocents', () => {
    expect(toLaw(cent, 12.34)).toBe(1234);
    expect(toForm(cent, 1234)).toBe(12.34);
    expect(toLaw(euro, 12.34)).toBe(12.34);
    expect(toLaw(bare, 12)).toBe(12);
    expect(toLaw(cent, null)).toBe(null);
    expect(toLaw({ name: 'n', type: 'number', unit: 'eurocent' }, 5)).toBe(5);
  });
});
