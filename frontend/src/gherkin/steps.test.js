import { describe, it, expect } from 'vitest';
import { parseValue, createStepDefinitions, SUPPORTED_TIERS } from './steps.js';
import { GRAMMAR } from './grammar.generated.js';

describe('parseValue', () => {
  it('parses booleans', () => {
    expect(parseValue('true')).toBe(true);
    expect(parseValue('false')).toBe(false);
  });

  it('parses null', () => {
    expect(parseValue('null')).toBe(null);
  });

  it('parses integers', () => {
    expect(parseValue('42')).toBe(42);
    expect(parseValue('-7')).toBe(-7);
    expect(parseValue('0')).toBe(0);
  });

  it('parses floats', () => {
    expect(parseValue('3.14')).toBeCloseTo(3.14);
    expect(parseValue('-0.5')).toBeCloseTo(-0.5);
  });

  it('returns strings for non-numeric values', () => {
    expect(parseValue('hello')).toBe('hello');
    expect(parseValue('Amsterdam')).toBe('Amsterdam');
  });

  it('parses large numeric strings as integers', () => {
    expect(parseValue('999993653')).toBe(999993653);
  });
});

describe('createStepDefinitions', () => {
  it('creates a step definition for every grammar entry', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    expect(defs).toHaveLength(GRAMMAR.length);
    for (const def of defs) {
      expect(def.pattern).toBeInstanceOf(RegExp);
      expect(typeof def.execute).toBe('function');
      expect(typeof def.tier).toBe('string');
    }
  });

  it('declares the core tier as the editor-supported tier set', () => {
    expect(SUPPORTED_TIERS).toEqual(['core']);
  });
});

// Each canonical example line is parsed by exactly one core grammar pattern,
// and that pattern carries the expected action. This is the proof that the
// generated patterns match their canonical phrasings.
describe('core grammar patterns match their canonical example lines', () => {
  const examples = [
    { line: 'the calculation date is "2025-01-01"', action: 'set_calculation_date' },
    { line: 'law "my_law" is loaded', action: 'load_law' },
    { line: 'parameter "bsn" is "999993653"', action: 'set_parameter' },
    { line: 'parameter "age" is 25', action: 'set_parameter' },
    { line: 'the following parameters:', action: 'set_parameters_table' },
    { line: 'the following "personal_data" data with key "bsn":', action: 'set_data_source' },
    { line: 'I evaluate "result" of "my_law"', action: 'evaluate' },
    { line: 'the execution succeeds', action: 'assert_succeeds' },
    { line: 'the execution fails', action: 'assert_fails' },
    { line: 'the execution fails with "some error"', action: 'assert_fails_with' },
    { line: 'output "x" is true', action: 'assert_boolean' },
    { line: 'output "x" is false', action: 'assert_boolean' },
    { line: 'output "x" equals 42', action: 'assert_equals' },
    { line: 'output "x" equals "hello"', action: 'assert_equals' },
    { line: 'output "x" is null', action: 'assert_null' },
    { line: 'output "x" contains "sub"', action: 'assert_contains' },
  ];

  const coreEntries = GRAMMAR.filter((e) => e.tier === 'core');

  for (const { line, action } of examples) {
    it(`matches: ${line}`, () => {
      const matching = coreEntries.filter((e) => e.pattern.test(line));
      expect(matching).toHaveLength(1);
      expect(matching[0].action).toBe(action);
    });
  }
});
