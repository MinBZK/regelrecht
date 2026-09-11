import { describe, expect, it } from 'vitest';
import { formatMissing, formatMoment, formatValue, humanize } from './format.js';

describe('momenten', () => {
  it('schrijft een ISO-dag in de Nederlandse notatie', () => {
    expect(formatMoment('2027-04-01')).toBe('01-04-2027');
  });

  it('laat staan wat geen dag is', () => {
    expect(formatMoment('later')).toBe('later');
    expect(formatMoment(null)).toBe('');
  });
});

describe('waarden', () => {
  it('laat een getal staan zoals de cel het vastlegde, zonder eenheid erbij', () => {
    expect(formatValue(197178.01)).toBe('197178.01');
    expect(formatValue(0)).toBe('0');
  });

  it('zegt ja en nee', () => {
    expect(formatValue(true)).toBe('ja');
    expect(formatValue(false)).toBe('nee');
  });

  it('houdt een gestelde afwezigheid en een onbekend feit uit elkaar', () => {
    expect(formatValue(null)).toBe('geen');
    expect(formatValue({ __unknown: true, missing: [{ name: 'toetsingsinkomen' }] })).toBe('nog niet bekend');
  });

  it('noemt bij een onbekend feit wat er ontbreekt', () => {
    expect(formatMissing({ __unknown: true, missing: [{ name: 'toetsingsinkomen' }] })).toBe(
      'ontbreekt: toetsingsinkomen',
    );
    expect(formatMissing('gewoon een waarde')).toBe('');
  });

  it('vat een lijst samen op zijn aantal', () => {
    expect(formatValue([1, 2, 3])).toBe('3 items');
    expect(formatValue([1])).toBe('1 item');
  });
});

describe('namen', () => {
  it('maakt woorden van een veldnaam', () => {
    expect(humanize('verwacht_inkomen')).toBe('Verwacht inkomen');
    expect(humanize('')).toBe('');
  });
});
