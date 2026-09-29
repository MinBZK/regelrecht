import { afterEach, describe, expect, it } from 'vitest';
import { displayCell, emphasiseArguments, hasHeaderRow, isExpectation, keywordColumnWidth } from './scenarioSteps.js';
import { stepKeywords } from './gherkinNl.js';
import { adoptLocale } from '../i18n/index.js';

afterEach(() => adoptLocale('nl'));

describe('displayCell', () => {
  it('reads a stated absence as "geen", never as the literal token', () => {
    expect(displayCell('null')).toEqual({ text: 'geen', quiet: true });
    expect(displayCell(' [] ')).toEqual({ text: 'geen', quiet: true });
  });

  it('keeps "not stated" apart from "none" (RFC-036)', () => {
    expect(displayCell('   ')).toEqual({ text: 'niet opgegeven', quiet: true });
  });

  it('leaves every other value as written, without converting it', () => {
    expect(displayCell(' 209692 ')).toEqual({ text: '209692', quiet: false });
    expect(displayCell('2008-01-01')).toEqual({ text: '2008-01-01', quiet: false });
    expect(displayCell('ONBEPAALDE_TIJD')).toEqual({ text: 'ONBEPAALDE_TIJD', quiet: false });
    expect(displayCell('0')).toEqual({ text: '0', quiet: false });
  });

  it('follows the language', () => {
    adoptLocale('en');
    expect(displayCell('null').text).toBe('none');
    expect(displayCell('').text).toBe('not stated');
  });
});

describe('emphasiseArguments', () => {
  it('sets quoted arguments in bold', () => {
    expect(emphasiseArguments('is "hoogte_toeslag" gelijk aan 165412')).toBe('is **hoogte_toeslag** gelijk aan 165412');
  });

  it('keeps an empty argument visible', () => {
    expect(emphasiseArguments('parameter "x" is ""')).toBe('parameter **x** is ""');
  });
});

describe('keywordColumnWidth', () => {
  it('fits the longest keyword of the active language, in ch', () => {
    expect(keywordColumnWidth(stepKeywords())).toBe('10ch'); // Gegeven
    adoptLocale('en');
    expect(keywordColumnWidth(stepKeywords())).toBe('7ch'); // Given
    adoptLocale('fy');
    // Frisian has no keywords of its own and shows the canonical English ones.
    expect(stepKeywords()).toContain('Given');
    expect(keywordColumnWidth(stepKeywords())).toBe('7ch');
  });
});

describe('hasHeaderRow', () => {
  it('is false only for the parameters table', () => {
    expect(hasHeaderRow({ text: 'the following parameters:' })).toBe(false);
    expect(hasHeaderRow({ text: 'the following "RvIG" data with key "bsn" for law "wet_brp":' })).toBe(true);
    expect(hasHeaderRow({ text: 'parameter "kinderen" is the collection:' })).toBe(true);
  });
});

describe('isExpectation', () => {
  it('knows a Then step by the grammar, not by its keyword', () => {
    expect(isExpectation({ keyword: 'And', text: 'output "hoogte_toeslag" equals 1' })).toBe(true);
    expect(isExpectation({ keyword: 'Then', text: 'the execution succeeds' })).toBe(true);
    expect(isExpectation({ keyword: 'And', text: 'parameter "bsn" is "1"' })).toBe(false);
    expect(isExpectation({ keyword: 'Then', text: 'not a step' })).toBe(false);
  });
});
