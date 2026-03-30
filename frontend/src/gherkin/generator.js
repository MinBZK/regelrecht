/**
 * Gherkin generator — creates Gherkin text from form state for preview.
 *
 * This is purely for display in the right panel of the scenario builder.
 * The actual execution goes directly via engine.execute(), not via Gherkin parsing.
 */

/**
 * Generate Gherkin feature text from form state.
 *
 * @param {object} formState
 * @param {string} formState.lawId - Law ID
 * @param {string} formState.lawName - Human-readable law name
 * @param {string} formState.calculationDate - YYYY-MM-DD date
 * @param {object} formState.parameters - { paramName: value }
 * @param {Array} formState.dataSources - [{ sourceName, keyField, rows: [{field: value}] }]
 * @param {Array} formState.selectedOutputs - [{ name, expectedValue }]
 * @returns {string} Gherkin feature text
 */
export function generateGherkin(formState) {
  const lines = [];

  lines.push(`Feature: ${formState.lawName || formState.lawId}`);
  lines.push(`  Scenario: Test ${formState.lawId}`);

  // Calculation date
  if (formState.calculationDate) {
    lines.push(`    Given the calculation date is "${formState.calculationDate}"`);
  }

  // Parameters
  for (const [name, value] of Object.entries(formState.parameters || {})) {
    if (value !== '' && value !== null && value !== undefined) {
      if (typeof value === 'number' || /^-?\d+(\.\d+)?$/.test(value)) {
        lines.push(`    And parameter "${name}" is ${value}`);
      } else {
        lines.push(`    And parameter "${name}" is "${value}"`);
      }
    }
  }

  // Data source tables
  for (const ds of formState.dataSources || []) {
    if (!ds.rows || ds.rows.length === 0) continue;

    lines.push(`    And the following "${ds.sourceName}" data with key "${ds.keyField}":`);

    // Collect all column names from all rows (skip internal _id)
    const columns = [];
    const seen = new Set();
    for (const row of ds.rows) {
      for (const key of Object.keys(row)) {
        if (key === '_id') continue;
        if (!seen.has(key)) {
          seen.add(key);
          columns.push(key);
        }
      }
    }

    if (columns.length === 0) continue;

    // Header row
    lines.push(`      | ${columns.join(' | ')} |`);

    // Data rows
    for (const row of ds.rows) {
      const cells = columns.map((col) => formatCell(row[col]));
      lines.push(`      | ${cells.join(' | ')} |`);
    }
  }

  // When step
  const outputNames = (formState.selectedOutputs || []).map((o) => o.name);
  if (outputNames.length > 0) {
    lines.push(
      `    When I evaluate "${outputNames[0]}" of "${formState.lawId}"`,
    );
  }

  // Then steps
  for (const output of formState.selectedOutputs || []) {
    if (output.expectedValue === null || output.expectedValue === undefined || output.expectedValue === '') {
      continue;
    }
    const expected = output.expectedValue;
    if (expected === true || expected === 'true') {
      lines.push(`    Then output "${output.name}" is true`);
    } else if (expected === false || expected === 'false') {
      lines.push(`    Then output "${output.name}" is false`);
    } else if (typeof expected === 'number' || /^-?\d+(\.\d+)?$/.test(expected)) {
      lines.push(`    Then output "${output.name}" equals ${expected}`);
    } else {
      lines.push(`    Then output "${output.name}" equals "${expected}"`);
    }
  }

  return lines.join('\n') + '\n';
}

function formatCell(value) {
  if (value === null || value === undefined || value === '') return 'null';
  return String(value).replace(/\\/g, '\\\\').replace(/\|/g, '\\|');
}
