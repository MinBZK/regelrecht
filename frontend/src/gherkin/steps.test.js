import { describe, it, expect } from 'vitest';
import { parseValue, createStepDefinitions, SUPPORTED_TIERS } from './steps.js';
import { GRAMMAR, VALUE_TYPING } from './grammar.generated.js';

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

// Mirror of Rust `rows_to_params` in packages/engine/tests/bdd/dispatch.rs:
// a parameter table carries no header row.
describe('set_parameters_table dispatch', () => {
  async function runTable(dataTable) {
    const ctx = { calculationDate: null, parameters: {}, result: null, error: null, executed: false };
    const def = createStepDefinitions({ loadDependency: async () => {} }).find(
      (d) => d.pattern.test('the following parameters:'),
    );
    await def.execute(ctx, null, 'the following parameters:'.match(def.pattern), { dataTable });
    return ctx.parameters;
  }

  it('reads every row of a corpus parameter table', async () => {
    expect(
      await runTable([
        ['gemeente_code', 'GM0363'],
        ['type_beplanting', 'boom'],
        ['postcode', '1012'],
      ]),
    ).toEqual({ gemeente_code: 'GM0363', type_beplanting: 'boom', postcode: 1012 });
  });

  it('reads a single-row parameter table', async () => {
    expect(await runTable([['indieningsdatum', '2025-01-01']])).toEqual({
      indieningsdatum: '2025-01-01',
    });
  });
});

// The canonical typing rule lives in bdd/grammar.yaml (`value_typing`) and is
// carried into both dispatchers by codegen. These assertions are the editor
// half of bdd/conformance/value_typing.feature: the same three lines, the same
// three types. Drift here is the failure mode of issue #1160 — a scenario that
// runs green in the editor and red in CI.
describe('value typing follows bdd/grammar.yaml', () => {
  const defs = createStepDefinitions({ loadDependency: async () => {} });

  async function runStep(line, table = null) {
    const ctx = { parameters: {} };
    const def = defs.find((d) => d.pattern.test(line));
    expect(def, `no step matches "${line}"`).toBeTruthy();
    await def.execute(ctx, null, line.match(def.pattern), table ? { dataTable: table } : null);
    return ctx.parameters;
  }

  it('declares the three rules', () => {
    expect(VALUE_TYPING).toEqual({ quoted: 'inferred', bare: 'number', table_cell: 'inferred' });
  });

  it('reads a quoted value by its content, not by its quotes', async () => {
    expect(await runStep('parameter "waarde" is "42"')).toEqual({ waarde: 42 });
    expect(await runStep('parameter "verzekerd" is "true"')).toEqual({ verzekerd: true });
    expect(await runStep('parameter "gemeente" is "GM0384"')).toEqual({ gemeente: 'GM0384' });
  });

  it('reads a bare value as a number', async () => {
    expect(await runStep('parameter "waarde" is 42')).toEqual({ waarde: 42 });
  });

  it('lets the content decide in a data table cell', async () => {
    expect(await runStep('the following parameters:', [['waarde', '42']])).toEqual({ waarde: 42 });
  });
});
