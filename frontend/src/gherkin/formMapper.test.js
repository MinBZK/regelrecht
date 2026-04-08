import { describe, it, expect } from 'vitest';
import { parseFeature } from './parser.js';
import { mapFeatureToForm, getEffectiveSetup, formStateToGherkin, syncEditedValues } from './formMapper.js';

describe('mapFeatureToForm', () => {
  it('maps a simple scenario with date, evaluate, and assertions', () => {
    const parsed = parseFeature(`
Feature: Simple test

  Scenario: Basic check
    Given the calculation date is "2025-01-01"
    When I evaluate "result" of "my_law"
    Then the execution succeeds
    Then output "result" is true
`);

    const form = mapFeatureToForm(parsed);
    expect(form.featureName).toBe('Simple test');
    expect(form.background).toBeNull();
    expect(form.scenarios).toHaveLength(1);

    const s = form.scenarios[0];
    expect(s.name).toBe('Basic check');
    expect(s.setup.calculationDate).toBe('2025-01-01');
    expect(s.execution).toEqual({ outputName: 'result', lawId: 'my_law' });
    expect(s.assertions).toHaveLength(2);
    expect(s.assertions[0]).toEqual({ assertionType: 'succeeds', outputName: null, value: null });
    expect(s.assertions[1]).toEqual({ assertionType: 'boolean', outputName: 'result', value: true });
    expect(s.unmatchedSteps).toHaveLength(0);
  });

  it('maps background with dependencies', () => {
    const parsed = parseFeature(`
Feature: With background

  Background:
    Given the calculation date is "2025-01-01"
    Given law "law_a" is loaded
    Given law "law_b" is loaded

  Scenario: Test
    Given parameter "bsn" is "999993653"
    When I evaluate "output" of "law_a"
    Then output "output" is false
`);

    const form = mapFeatureToForm(parsed);
    expect(form.background).not.toBeNull();
    expect(form.background.calculationDate).toBe('2025-01-01');
    expect(form.background.dependencies).toEqual(['law_a', 'law_b']);

    const s = form.scenarios[0];
    expect(s.setup.parameters).toEqual([{ name: 'bsn', value: '999993653' }]);
    expect(s.assertions[0]).toEqual({ assertionType: 'boolean', outputName: 'output', value: false });
  });

  it('maps data sources with headers and rows', () => {
    const parsed = parseFeature(`
Feature: Data sources

  Scenario: With data
    Given the calculation date is "2025-01-01"
    Given the following "personal_data" data with key "bsn":
      | bsn       | name  | age |
      | 999993653 | Alice | 30  |
      | 999993654 | Bob   | 25  |
    When I evaluate "result" of "law_x"
`);

    const form = mapFeatureToForm(parsed);
    const ds = form.scenarios[0].setup.dataSources[0];
    expect(ds.sourceName).toBe('personal_data');
    expect(ds.keyField).toBe('bsn');
    expect(ds.headers).toEqual(['bsn', 'name', 'age']);
    expect(ds.rows).toHaveLength(2);
    expect(ds.rows[0]).toEqual(['999993653', 'Alice', '30']);
  });

  it('maps all assertion types', () => {
    const parsed = parseFeature(`
Feature: Assertions

  Scenario: All types
    Given the calculation date is "2025-01-01"
    When I evaluate "x" of "law"
    Then the execution succeeds
    Then the execution fails
    Then the execution fails with "some error"
    Then output "a" is true
    Then output "b" is false
    Then output "c" equals 42
    Then output "d" equals "hello"
    Then output "e" is null
    Then output "f" contains "sub"
`);

    const form = mapFeatureToForm(parsed);
    const a = form.scenarios[0].assertions;
    expect(a).toHaveLength(9);
    expect(a[0].assertionType).toBe('succeeds');
    expect(a[1].assertionType).toBe('fails');
    expect(a[2]).toEqual({ assertionType: 'failsWith', outputName: null, value: 'some error' });
    expect(a[3]).toEqual({ assertionType: 'boolean', outputName: 'a', value: true });
    expect(a[4]).toEqual({ assertionType: 'boolean', outputName: 'b', value: false });
    expect(a[5]).toEqual({ assertionType: 'equals', outputName: 'c', value: 42 });
    expect(a[6]).toEqual({ assertionType: 'equalsString', outputName: 'd', value: 'hello' });
    expect(a[7]).toEqual({ assertionType: 'null', outputName: 'e', value: null });
    expect(a[8]).toEqual({ assertionType: 'contains', outputName: 'f', value: 'sub' });
  });

  it('maps numeric and string parameters', () => {
    const parsed = parseFeature(`
Feature: Params

  Scenario: Mixed params
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    Given parameter "amount" is 1500
    Given parameter "rate" is 3.14
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const params = form.scenarios[0].setup.parameters;
    expect(params).toEqual([
      { name: 'bsn', value: '999993653' },
      { name: 'amount', value: 1500 },
      { name: 'rate', value: 3.14 },
    ]);
  });

  it('maps parameter table', () => {
    const parsed = parseFeature(`
Feature: Param table

  Scenario: Bulk params
    Given the calculation date is "2025-01-01"
    Given the following parameters:
      | name   | value |
      | bsn    | 123   |
      | amount | 500   |
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const params = form.scenarios[0].setup.parameters;
    expect(params).toEqual([
      { name: 'bsn', value: 123 },
      { name: 'amount', value: 500 },
    ]);
  });

  it('puts unrecognized steps in unmatchedSteps', () => {
    const parsed = parseFeature(`
Feature: Custom steps

  Scenario: With custom
    Given the calculation date is "2025-01-01"
    When the healthcare allowance law is executed
    Then the execution succeeds
`);

    const form = mapFeatureToForm(parsed);
    const s = form.scenarios[0];
    expect(s.execution).toBeNull();
    expect(s.unmatchedSteps).toHaveLength(1);
    expect(s.unmatchedSteps[0].keyword).toBe('When');
    expect(s.unmatchedSteps[0].text).toBe('the healthcare allowance law is executed');
  });

  it('handles multiple scenarios', () => {
    const parsed = parseFeature(`
Feature: Multi scenario

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: First
    When I evaluate "a" of "law"
    Then output "a" is true

  Scenario: Second
    When I evaluate "b" of "law"
    Then output "b" is false
`);

    const form = mapFeatureToForm(parsed);
    expect(form.scenarios).toHaveLength(2);
    expect(form.scenarios[0].name).toBe('First');
    expect(form.scenarios[1].name).toBe('Second');
  });
});

describe('getEffectiveSetup', () => {
  it('merges background and scenario setup', () => {
    const parsed = parseFeature(`
Feature: Merge test

  Background:
    Given the calculation date is "2025-01-01"
    Given law "dep_a" is loaded

  Scenario: Test
    Given law "dep_b" is loaded
    Given parameter "bsn" is "123"
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const effective = getEffectiveSetup(form, 0);

    expect(effective.calculationDate).toBe('2025-01-01');
    expect(effective.dependencies).toEqual(['dep_a', 'dep_b']);
    expect(effective.parameters).toEqual([{ name: 'bsn', value: '123' }]);
  });

  it('scenario date overrides background date', () => {
    const parsed = parseFeature(`
Feature: Date override

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Override
    Given the calculation date is "2026-06-15"
    When I evaluate "x" of "law"
`);

    const form = mapFeatureToForm(parsed);
    const effective = getEffectiveSetup(form, 0);
    expect(effective.calculationDate).toBe('2026-06-15');
  });
});

describe('formStateToGherkin round-trip', () => {
  it('round-trips a simple scenario', () => {
    const original = `Feature: Round trip

  Scenario: Test
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    When I evaluate "result" of "my_law"
    Then the execution succeeds
    Then output "result" is true
`;

    const parsed = parseFeature(original);
    const form = mapFeatureToForm(parsed);
    const regenerated = formStateToGherkin(form);
    const reparsed = parseFeature(regenerated);
    const reformatted = mapFeatureToForm(reparsed);

    expect(reformatted.featureName).toBe(form.featureName);
    expect(reformatted.scenarios[0].setup.calculationDate).toBe(form.scenarios[0].setup.calculationDate);
    expect(reformatted.scenarios[0].setup.parameters).toEqual(form.scenarios[0].setup.parameters);
    expect(reformatted.scenarios[0].execution).toEqual(form.scenarios[0].execution);
    expect(reformatted.scenarios[0].assertions).toEqual(form.scenarios[0].assertions);
  });

  it('round-trips a feature with background and data sources', () => {
    const original = `Feature: Complex round trip

  Background:
    Given the calculation date is "2025-01-01"
    Given law "dep_a" is loaded
    Given law "dep_b" is loaded

  Scenario: With data
    Given the following "personal_data" data with key "bsn":
      | bsn | name | age |
      | 123 | Alice | 30 |
    Given parameter "bsn" is "123"
    When I evaluate "eligible" of "my_law"
    Then output "eligible" is true
`;

    const parsed = parseFeature(original);
    const form = mapFeatureToForm(parsed);
    const regenerated = formStateToGherkin(form);
    const reparsed = parseFeature(regenerated);
    const reformatted = mapFeatureToForm(reparsed);

    expect(reformatted.background.dependencies).toEqual(form.background.dependencies);
    expect(reformatted.scenarios[0].setup.dataSources).toHaveLength(1);
    expect(reformatted.scenarios[0].setup.dataSources[0].sourceName).toBe('personal_data');
    expect(reformatted.scenarios[0].setup.dataSources[0].keyField).toBe('bsn');
  });
});

describe('syncEditedValues', () => {
  it('updates scenario-level parameter values', () => {
    const parsed = parseFeature(`
Feature: Sync test

  Scenario: Test
    Given parameter "loon" is 30000
    When I evaluate "result" of "law"
    Then output "result" is true
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { loon: '50000' },
      calculationDate: null,
    });

    expect(form.scenarios[0].setup.parameters[0].value).toBe(50000);

    const gherkin = formStateToGherkin(form);
    expect(gherkin).toContain('parameter "loon" is 50000');
    expect(gherkin).not.toContain('30000');
  });

  it('adds scenario override when background parameter is changed', () => {
    const parsed = parseFeature(`
Feature: Background override

  Background:
    Given parameter "bsn" is "999993653"
    Given parameter "loon" is 30000

  Scenario: First
    When I evaluate "result" of "law"

  Scenario: Second
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);

    // Change loon only in the first scenario
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653', loon: '50000' },
      calculationDate: null,
    });

    // First scenario should have an override
    expect(form.scenarios[0].setup.parameters).toEqual([
      { name: 'loon', value: 50000 },
    ]);

    // Background should be unchanged
    expect(form.background.parameters[1].value).toBe(30000);

    // Second scenario should still have no overrides
    expect(form.scenarios[1].setup.parameters).toHaveLength(0);
  });

  it('does not add override when value matches background', () => {
    const parsed = parseFeature(`
Feature: No-op sync

  Background:
    Given parameter "bsn" is "999993653"

  Scenario: Test
    When I evaluate "result" of "law"
`);

    const form = mapFeatureToForm(parsed);
    syncEditedValues(form, 0, {
      parameterValues: { bsn: '999993653' },
      calculationDate: null,
    });

    // No scenario-level override should be added
    expect(form.scenarios[0].setup.parameters).toHaveLength(0);
  });
});
