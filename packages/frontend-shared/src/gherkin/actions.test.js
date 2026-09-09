import { describe, it, expect } from 'vitest';
import { valuesEqual, primitiveEqual, quotedValue, tableCellValue, dispatch } from './actions.js';

// Mirrors packages/engine/tests/bdd/helpers/value_conversion.rs
// (`values_equal_with_tolerance`) plus law-model's `PartialEq for Value`:
// the two runners must accept and reject the same pairs.
describe('valuesEqual', () => {
  it('compares scalars like before', () => {
    expect(valuesEqual(1, 1)).toBe(true);
    expect(valuesEqual('a', 'a')).toBe(true);
    expect(valuesEqual(true, true)).toBe(true);
    expect(valuesEqual(null, null)).toBe(true);
    expect(valuesEqual(true, false)).toBe(false);
    expect(valuesEqual('a', 'b')).toBe(false);
  });

  it('applies the 1e-9 numeric tolerance', () => {
    expect(valuesEqual(1.0, 1.0000000001)).toBe(true);
    expect(valuesEqual(100, 100.0)).toBe(true);
    expect(valuesEqual(100, 101)).toBe(false);
    expect(valuesEqual(0.1 + 0.2, 0.3)).toBe(true);
  });

  it('never equates a number with its numeric string', () => {
    expect(valuesEqual(1, '1')).toBe(false);
    expect(valuesEqual('1', 1)).toBe(false);
    expect(valuesEqual(true, 'true')).toBe(false);
    expect(valuesEqual(1, true)).toBe(false);
  });

  it('treats null as equal to null only', () => {
    expect(valuesEqual(null, [])).toBe(false);
    expect(valuesEqual([], null)).toBe(false);
    expect(valuesEqual(null, {})).toBe(false);
    expect(valuesEqual(null, 0)).toBe(false);
    expect(valuesEqual(null, '')).toBe(false);
    expect(valuesEqual(undefined, null)).toBe(false);
    expect(valuesEqual(undefined, [])).toBe(false);
  });

  it('compares arrays element-wise in order', () => {
    expect(valuesEqual([], [])).toBe(true);
    expect(valuesEqual([1, 'a', null, true], [1, 'a', null, true])).toBe(true);
    expect(valuesEqual([1, 2], [2, 1])).toBe(false);
    expect(valuesEqual([1, 2], [1, 2, 3])).toBe(false);
    expect(valuesEqual([1, 2, 3], [1, 2])).toBe(false);
    expect(valuesEqual([1], ['1'])).toBe(false);
  });

  it('applies the numeric tolerance inside arrays', () => {
    expect(valuesEqual([1.0], [1.0000000001])).toBe(true);
    expect(valuesEqual([[1.5, 2]], [[1.5000000001, 2]])).toBe(true);
  });

  it('compares nested objects by key set and values, regardless of key order', () => {
    const a = { postcode: '1234AB', huisnummer: 10, kinderen: [{ bsn: '1', leeftijd: 4 }] };
    const b = { kinderen: [{ leeftijd: 4, bsn: '1' }], huisnummer: 10, postcode: '1234AB' };
    expect(valuesEqual(a, b)).toBe(true);
    expect(valuesEqual({ a: 1 }, { a: 1, b: 2 })).toBe(false);
    expect(valuesEqual({ a: 1, b: 2 }, { a: 1 })).toBe(false);
    expect(valuesEqual({ a: 1 }, { b: 1 })).toBe(false);
    expect(valuesEqual({ a: { x: 1 } }, { a: { x: 2 } })).toBe(false);
    expect(valuesEqual({ a: null }, { a: [] })).toBe(false);
    expect(valuesEqual({ a: undefined }, { b: undefined })).toBe(false);
  });

  it('does not confuse arrays with objects', () => {
    expect(valuesEqual([], {})).toBe(false);
    expect(valuesEqual({}, [])).toBe(false);
    expect(valuesEqual({ 0: 'a', length: 1 }, ['a'])).toBe(false);
  });

  it('keeps the old name as an alias', () => {
    expect(primitiveEqual).toBe(valuesEqual);
  });
});

describe('quoted and table-cell values', () => {
  it('parses a quoted JSON array or object like the Rust convert_gherkin_value', () => {
    expect(quotedValue('[]')).toEqual([]);
    expect(quotedValue('[5, 7]')).toEqual([5, 7]);
    expect(quotedValue('{"postcode": "1234AB", "huisnummer": "10"}')).toEqual({
      postcode: '1234AB',
      huisnummer: '10',
    });
    expect(tableCellValue('[]')).toEqual([]);
  });

  it('keeps a cell that only starts like JSON as a string', () => {
    expect(quotedValue('[not json')).toBe('[not json');
  });

  it('keeps the scalar rules', () => {
    expect(quotedValue('42')).toBe(42);
    expect(quotedValue('true')).toBe(true);
    expect(quotedValue('null')).toBe(null);
    expect(quotedValue('GM0384')).toBe('GM0384');
  });
});

describe('assert_equals through dispatch', () => {
  const ctxWith = (outputs) => ({ result: { outputs }, executed: true, error: null });
  const run = (ctx, expected) =>
    dispatch(ctx, null, 'assert_equals', ['kinderen', expected], null, { loadDependency: async () => {} });

  it('accepts an empty array output against a quoted "[]"', async () => {
    await expect(run(ctxWith({ kinderen: [] }), quotedValue('[]'))).resolves.toBeUndefined();
  });

  it('accepts an object output against a quoted object literal', async () => {
    await expect(
      run(ctxWith({ kinderen: { a: 1, b: [1, 2] } }), quotedValue('{"b":[1,2],"a":1}')),
    ).resolves.toBeUndefined();
  });

  it('rejects a null output against "[]" and renders both sides as JSON', async () => {
    await expect(run(ctxWith({ kinderen: null }), quotedValue('[]'))).rejects.toThrow(
      'Expected output "kinderen" to equal [], got: null',
    );
  });

  it('renders unequal arrays with JSON.stringify in the message', async () => {
    await expect(run(ctxWith({ kinderen: [1, 2] }), quotedValue('[2,1]'))).rejects.toThrow(
      'Expected output "kinderen" to equal [2,1], got: [1,2]',
    );
  });

  it('rejects a numeric string against a number', async () => {
    await expect(run(ctxWith({ kinderen: '3' }), 3)).rejects.toThrow(
      'Expected output "kinderen" to equal 3, got: "3"',
    );
  });
});
