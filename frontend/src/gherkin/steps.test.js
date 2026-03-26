import { describe, it, expect } from 'vitest';
import { parseValue, createStepDefinitions } from './steps.js';

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

  it('returns strings for non-numeric', () => {
    expect(parseValue('hello')).toBe('hello');
    expect(parseValue('999993653')).toBe(999993653);
    expect(parseValue('Amsterdam')).toBe('Amsterdam');
  });
});

describe('createStepDefinitions', () => {
  it('creates step definitions with all patterns', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    expect(defs.length).toBeGreaterThan(10);
  });

  it('matches calculation date step', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    const dateStep = defs.find((d) => d.pattern.test('the calculation date is "2025-01-01"'));
    expect(dateStep).toBeDefined();
  });

  it('matches parameter steps', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    const strParam = defs.find((d) => d.pattern.test('parameter "bsn" is "999993653"'));
    expect(strParam).toBeDefined();
    const numParam = defs.find((d) => d.pattern.test('parameter "age" is 25'));
    expect(numParam).toBeDefined();
  });

  it('matches evaluate step', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    const evalStep = defs.find((d) => d.pattern.test('I evaluate "output" of "law_id"'));
    expect(evalStep).toBeDefined();
  });

  it('matches assertion steps', () => {
    const defs = createStepDefinitions({ loadDependency: async () => {} });
    expect(defs.find((d) => d.pattern.test('the execution succeeds'))).toBeDefined();
    expect(defs.find((d) => d.pattern.test('the execution fails'))).toBeDefined();
    expect(defs.find((d) => d.pattern.test('output "x" is true'))).toBeDefined();
    expect(defs.find((d) => d.pattern.test('output "x" is false'))).toBeDefined();
    expect(defs.find((d) => d.pattern.test('output "x" equals 42'))).toBeDefined();
    expect(defs.find((d) => d.pattern.test('output "x" equals "hello"'))).toBeDefined();
    expect(defs.find((d) => d.pattern.test('output "x" is null'))).toBeDefined();
  });
});
