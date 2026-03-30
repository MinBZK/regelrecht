/**
 * Bidirectional mapper between Gherkin AST and structured form state.
 *
 * Forward:  parseFeature() AST → mapFeatureToForm() → visual form state
 * Reverse:  visual form state → formStateToGherkin() → Gherkin text
 */

import { parseValue } from './steps.js';

// --- Extraction patterns (mirrors steps.js but extracts data instead of executing) ---

const PATTERNS = [
  {
    id: 'calculationDate',
    pattern: /^the calculation date is "([^"]+)"$/,
    extract: (match) => ({ type: 'calculationDate', value: match[1] }),
  },
  {
    id: 'parameterString',
    pattern: /^parameter "([^"]+)" is "([^"]*)"$/,
    extract: (match) => ({ type: 'parameter', name: match[1], value: match[2] }),
  },
  {
    id: 'parameterNumeric',
    pattern: /^parameter "([^"]+)" is (-?\d+(?:\.\d+)?)$/,
    extract: (match) => ({ type: 'parameter', name: match[1], value: parseValue(match[2]) }),
  },
  {
    id: 'parameterTable',
    pattern: /^the following parameters:$/,
    extract: (_match, step) => ({
      type: 'parameterTable',
      parameters: tableToParams(step.dataTable),
    }),
  },
  {
    id: 'dependency',
    pattern: /^law "([^"]+)" is loaded$/,
    extract: (match) => ({ type: 'dependency', lawId: match[1] }),
  },
  {
    id: 'dataSource',
    pattern: /^the following "([^"]+)" data with key "([^"]+)":$/,
    extract: (match, step) => ({
      type: 'dataSource',
      sourceName: match[1],
      keyField: match[2],
      headers: step.dataTable?.[0] || [],
      rows: step.dataTable?.slice(1) || [],
    }),
  },
  {
    id: 'dataSourceRust',
    pattern: /^the following (\w+) "([^"]+)" data:$/,
    extract: (match, step) => ({
      type: 'dataSource',
      sourceName: `${match[1]}_${match[2]}`,
      keyField: step.dataTable?.[0]?.[0] || 'id',
      headers: step.dataTable?.[0] || [],
      rows: step.dataTable?.slice(1) || [],
    }),
  },
  {
    id: 'evaluate',
    pattern: /^I evaluate "([^"]+)" of "([^"]+)"$/,
    extract: (match) => ({ type: 'execution', outputName: match[1], lawId: match[2] }),
  },
  {
    id: 'succeeds',
    pattern: /^the execution succeeds$/,
    extract: () => ({ type: 'assertion', assertionType: 'succeeds' }),
  },
  {
    id: 'fails',
    pattern: /^the execution fails$/,
    extract: () => ({ type: 'assertion', assertionType: 'fails' }),
  },
  {
    id: 'failsWith',
    pattern: /^the execution fails with "([^"]+)"$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'failsWith', value: match[1] }),
  },
  {
    id: 'outputTrue',
    pattern: /^output "([^"]+)" is true$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'boolean', outputName: match[1], value: true }),
  },
  {
    id: 'outputFalse',
    pattern: /^output "([^"]+)" is false$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'boolean', outputName: match[1], value: false }),
  },
  {
    id: 'outputEqualsNumeric',
    pattern: /^output "([^"]+)" equals (-?\d+(?:\.\d+)?)$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'equals', outputName: match[1], value: parseValue(match[2]) }),
  },
  {
    id: 'outputEqualsString',
    pattern: /^output "([^"]+)" equals "([^"]*)"$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'equalsString', outputName: match[1], value: match[2] }),
  },
  {
    id: 'outputNull',
    pattern: /^output "([^"]+)" is null$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'null', outputName: match[1] }),
  },
  {
    id: 'outputContains',
    pattern: /^output "([^"]+)" contains "([^"]+)"$/,
    extract: (match) => ({ type: 'assertion', assertionType: 'contains', outputName: match[1], value: match[2] }),
  },
];

function tableToParams(dataTable) {
  if (!dataTable || dataTable.length < 2) return [];
  return dataTable.slice(1).map((row) => ({
    name: row[0],
    value: parseValue(row[1] || ''),
  }));
}

function classifyStep(step) {
  const text = step.text;
  for (const def of PATTERNS) {
    const match = text.match(def.pattern);
    if (match) {
      return def.extract(match, step);
    }
  }
  return null;
}

function classifySteps(steps) {
  const setup = {
    calculationDate: null,
    dependencies: [],
    parameters: [],
    dataSources: [],
  };
  const executions = [];
  const assertions = [];
  const unmatchedSteps = [];

  for (const step of steps) {
    const classified = classifyStep(step);
    if (!classified) {
      unmatchedSteps.push(step);
      continue;
    }

    switch (classified.type) {
      case 'calculationDate':
        setup.calculationDate = classified.value;
        break;
      case 'dependency':
        setup.dependencies.push(classified.lawId);
        break;
      case 'parameter':
        setup.parameters.push({ name: classified.name, value: classified.value });
        break;
      case 'parameterTable':
        setup.parameters.push(...classified.parameters);
        break;
      case 'dataSource':
        setup.dataSources.push({
          sourceName: classified.sourceName,
          keyField: classified.keyField,
          headers: classified.headers,
          rows: classified.rows,
        });
        break;
      case 'execution':
        executions.push({ outputName: classified.outputName, lawId: classified.lawId });
        break;
      case 'assertion':
        assertions.push({
          assertionType: classified.assertionType,
          outputName: classified.outputName || null,
          value: classified.value !== undefined ? classified.value : null,
        });
        break;
    }
  }

  return { setup, executions, assertions, unmatchedSteps };
}

/**
 * Map a parsed Gherkin feature AST to structured form state.
 *
 * @param {object} parsed - Output of parseFeature()
 * @returns {object} Structured form state
 */
export function mapFeatureToForm(parsed) {
  const backgroundResult = parsed.background
    ? classifySteps(parsed.background)
    : null;

  const scenarios = (parsed.scenarios || []).map((scenario) => {
    const result = classifySteps(scenario.steps);

    return {
      name: scenario.name,
      tags: scenario.tags || [],
      setup: {
        calculationDate: result.setup.calculationDate,
        dependencies: result.setup.dependencies,
        parameters: result.setup.parameters,
        dataSources: result.setup.dataSources,
      },
      execution: result.executions[0] || null,
      assertions: result.assertions,
      unmatchedSteps: result.unmatchedSteps,
    };
  });

  return {
    featureName: parsed.feature || '',
    background: backgroundResult
      ? {
          calculationDate: backgroundResult.setup.calculationDate,
          dependencies: backgroundResult.setup.dependencies,
          parameters: backgroundResult.setup.parameters,
          dataSources: backgroundResult.setup.dataSources,
          unmatchedSteps: backgroundResult.unmatchedSteps,
        }
      : null,
    scenarios,
  };
}

/**
 * Get the effective setup for a scenario by merging background + scenario setup.
 *
 * @param {object} formState - Output of mapFeatureToForm()
 * @param {number} scenarioIndex - Which scenario
 * @returns {object} Merged setup
 */
export function getEffectiveSetup(formState, scenarioIndex) {
  const scenario = formState.scenarios[scenarioIndex];
  if (!scenario) return null;

  const bg = formState.background || {};
  return {
    calculationDate: scenario.setup.calculationDate || bg.calculationDate || null,
    dependencies: [...(bg.dependencies || []), ...scenario.setup.dependencies],
    parameters: [...(bg.parameters || []), ...scenario.setup.parameters],
    dataSources: [...(bg.dataSources || []), ...scenario.setup.dataSources],
  };
}

// --- Reverse: Form State → Gherkin text ---

function formatCell(value) {
  if (value === null || value === undefined || value === '') return 'null';
  return String(value).replace(/\\/g, '\\\\').replace(/\|/g, '\\|');
}

function formatValue(value) {
  if (value === true) return 'true';
  if (value === false) return 'false';
  if (value === null) return 'null';
  if (typeof value === 'number') return String(value);
  return value;
}

/**
 * Serialize structured form state back to Gherkin feature text.
 *
 * @param {object} formState - Output of mapFeatureToForm()
 * @returns {string} Gherkin text
 */
export function formStateToGherkin(formState) {
  const lines = [];
  lines.push(`Feature: ${formState.featureName}`);

  // Background
  if (formState.background) {
    lines.push('');
    lines.push('  Background:');
    writeSetupSteps(lines, formState.background, '    ', true);
    writeUnmatchedSteps(lines, formState.background.unmatchedSteps, '    ');
  }

  // Scenarios
  for (const scenario of formState.scenarios) {
    lines.push('');
    if (scenario.tags?.length > 0) {
      lines.push(`  ${scenario.tags.join(' ')}`);
    }
    lines.push(`  Scenario: ${scenario.name}`);
    writeSetupSteps(lines, scenario.setup, '    ', true);

    // Execution
    if (scenario.execution) {
      lines.push(`    When I evaluate "${scenario.execution.outputName}" of "${scenario.execution.lawId}"`);
    }

    // Assertions
    for (const assertion of scenario.assertions) {
      lines.push(`    Then ${formatAssertion(assertion)}`);
    }

    // Unmatched steps
    writeUnmatchedSteps(lines, scenario.unmatchedSteps, '    ');
  }

  return lines.join('\n') + '\n';
}

function writeSetupSteps(lines, setup, indent, useGiven) {
  const keyword = useGiven ? 'Given' : 'And';

  if (setup.calculationDate) {
    lines.push(`${indent}${keyword} the calculation date is "${setup.calculationDate}"`);
  }

  for (const dep of setup.dependencies || []) {
    lines.push(`${indent}Given law "${dep}" is loaded`);
  }

  for (const param of setup.parameters || []) {
    const v = formatValue(param.value);
    if (typeof param.value === 'number') {
      lines.push(`${indent}Given parameter "${param.name}" is ${v}`);
    } else {
      lines.push(`${indent}Given parameter "${param.name}" is "${v}"`);
    }
  }

  for (const ds of setup.dataSources || []) {
    if (ds.headers.length === 0) continue;
    lines.push(`${indent}Given the following "${ds.sourceName}" data with key "${ds.keyField}":`);

    // Header
    lines.push(`${indent}  | ${ds.headers.join(' | ')} |`);

    // Data rows
    for (const row of ds.rows) {
      const cells = row.map((cell) => formatCell(cell));
      lines.push(`${indent}  | ${cells.join(' | ')} |`);
    }
  }
}

function writeUnmatchedSteps(lines, unmatchedSteps, indent) {
  for (const step of unmatchedSteps || []) {
    lines.push(`${indent}${step.keyword} ${step.text}`);
    if (step.dataTable) {
      for (const row of step.dataTable) {
        lines.push(`${indent}  | ${row.join(' | ')} |`);
      }
    }
    if (step.docString) {
      lines.push(`${indent}  """`);
      lines.push(step.docString);
      lines.push(`${indent}  """`);
    }
  }
}

function formatAssertion(assertion) {
  switch (assertion.assertionType) {
    case 'succeeds':
      return 'the execution succeeds';
    case 'fails':
      return 'the execution fails';
    case 'failsWith':
      return `the execution fails with "${assertion.value}"`;
    case 'boolean':
      return `output "${assertion.outputName}" is ${assertion.value ? 'true' : 'false'}`;
    case 'equals':
      return `output "${assertion.outputName}" equals ${assertion.value}`;
    case 'equalsString':
      return `output "${assertion.outputName}" equals "${assertion.value}"`;
    case 'null':
      return `output "${assertion.outputName}" is null`;
    case 'contains':
      return `output "${assertion.outputName}" contains "${assertion.value}"`;
    default:
      return `unknown assertion: ${assertion.assertionType}`;
  }
}
