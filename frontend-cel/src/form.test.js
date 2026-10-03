import { describe, expect, it } from 'vitest';
import {
  external,
  fieldLabel,
  fieldText,
  getPath,
  formUnit,
  formValue,
  inEuros,
  inputKind,
  isEmpty,
  options,
  setPath,
  suppliedText,
  toForm,
  toLaw,
  withoutSupplied,
} from './form.js';

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

  it('gives an amount the unit of the law, also in a table column', () => {
    const fields = [
      { name: 'inkomen', type: 'amount', unit: 'eurocent' },
      { name: 'kosten', type: 'table', columns: [{ id: 'bedrag', type: 'amount', unit: 'eurocent' }, { id: 'post' }] },
    ];
    expect(external({ inkomen: 12.5, kosten: [{ bedrag: 1.25, post: 'huur' }] }, fields)).toEqual({
      inkomen: 1250,
      kosten: [{ bedrag: 125, post: 'huur' }],
    });
  });
});

describe('inputKind and formValues', () => {
  it('asks for an amount as a number', () => {
    expect(inputKind({ type: 'amount' })).toBe('number');
    expect(inputKind({ type: 'date' })).toBe('date');
  });

  it('turns values of the law into the units of the form', () => {
    const fields = [
      { name: 'inkomen', type: 'amount', unit: 'eurocent' },
      { name: 'kosten', type: 'table', columns: [{ id: 'bedrag', type: 'amount', unit: 'eurocent' }] },
    ];
    expect(formValue(fields[0], 1250)).toBe(12.5);
    expect(formValue(fields[1], [{ bedrag: 125 }])).toEqual([{ bedrag: 1.25 }]);
    expect(formValue(fields[1], null)).toBe(null);
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
    expect(formUnit(cent)).toBe('euro');
    expect(formUnit(bare)).toBe('');
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

describe('what the channel or a register supplies', () => {
  const fields = [
    { name: 'statutaire_naam', supplied: { value: 'Vereniging Voorbeeld', source: 'register', legal_basis: ['beleid#4 lid 1'] } },
    { name: 'dagtekening', supplied: { value: '2026-09-29', source: 'channel' } },
    { name: 'iban' },
  ];
  it('says where it came from', () => {
    expect(suppliedText(fields[0])).toBe('Automatisch ingevuld uit het register (beleid#4 lid 1)');
    expect(suppliedText(fields[1])).toBe('Automatisch ingevuld uit het inlogmiddel');
    expect(suppliedText(fields[2])).toBe('');
  });
  it('is not sent along', () => {
    expect(withoutSupplied({ statutaire_naam: 'x', dagtekening: 'y', iban: 'NL' }, fields)).toEqual({ iban: 'NL' });
  });
});
