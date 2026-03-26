import { describe, it, expect } from 'vitest';
import { parseFeature } from './parser.js';

describe('parseFeature', () => {
  it('parses a simple feature', () => {
    const text = `
Feature: Simple test

  Scenario: Hello world
    Given the calculation date is "2025-01-01"
    When I evaluate "output" of "law_id"
    Then the execution succeeds
`;

    const result = parseFeature(text);
    expect(result.feature).toBe('Simple test');
    expect(result.scenarios).toHaveLength(1);
    expect(result.scenarios[0].name).toBe('Hello world');
    expect(result.scenarios[0].steps).toHaveLength(3);
    expect(result.scenarios[0].steps[0].keyword).toBe('Given');
    expect(result.scenarios[0].steps[0].text).toBe('the calculation date is "2025-01-01"');
  });

  it('parses background steps', () => {
    const text = `
Feature: With background

  Background:
    Given the calculation date is "2025-01-01"

  Scenario: Test
    When I evaluate "x" of "y"
`;

    const result = parseFeature(text);
    expect(result.background).toHaveLength(1);
    expect(result.background[0].keyword).toBe('Given');
    expect(result.scenarios).toHaveLength(1);
  });

  it('parses data tables', () => {
    const text = `
Feature: Data tables

  Scenario: With table
    Given the following "data" data with key "bsn":
      | bsn | name  |
      | 123 | Alice |
      | 456 | Bob   |
`;

    const result = parseFeature(text);
    const step = result.scenarios[0].steps[0];
    expect(step.dataTable).toBeDefined();
    expect(step.dataTable).toHaveLength(3);
    expect(step.dataTable[0]).toEqual(['bsn', 'name']);
    expect(step.dataTable[1]).toEqual(['123', 'Alice']);
  });

  it('returns empty result for empty feature', () => {
    const text = 'Feature: Empty';
    const result = parseFeature(text);
    expect(result.feature).toBe('Empty');
    expect(result.scenarios).toHaveLength(0);
  });
});
