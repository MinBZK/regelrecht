/**
 * Generic Gherkin step definitions for the RegelRecht engine.
 *
 * Each step definition is { pattern: RegExp, execute: async (ctx, engine, match, step) => void }.
 * Steps receive the ExecutionContext, the WasmEngine instance, the regex match,
 * and the full step object (for data tables).
 */

/**
 * Parse a string value to a typed value, mirroring Rust value_conversion.rs.
 * - "true"/"false" → boolean
 * - "null" → null
 * - numeric → number (int or float)
 * - otherwise → string
 */
export function parseValue(str) {
  if (str === 'true') return true;
  if (str === 'false') return false;
  if (str === 'null') return null;

  // Try integer first, then float
  if (/^-?\d+$/.test(str)) {
    const n = parseInt(str, 10);
    if (Number.isSafeInteger(n)) return n;
  }
  if (/^-?\d+\.\d+$/.test(str)) {
    const f = parseFloat(str);
    if (Number.isFinite(f)) return f;
  }

  return str;
}

/**
 * Parse a data table row into a record object using the header row.
 * Values are auto-typed via parseValue().
 */
function tableToRecords(dataTable) {
  if (!dataTable || dataTable.length < 2) return [];
  const headers = dataTable[0];
  return dataTable.slice(1).map((row) => {
    const record = {};
    headers.forEach((h, i) => {
      record[h] = parseValue(row[i] || '');
    });
    return record;
  });
}

/**
 * Create the step definitions registry.
 *
 * @param {object} options
 * @param {(lawId: string) => Promise<void>} options.loadDependency - Callback to fetch and load a dependent law
 * @returns {Array<{pattern: RegExp, execute: Function}>}
 */
export function createStepDefinitions({ loadDependency }) {
  return [
    // --- Given: Setup steps ---
    {
      pattern: /^the calculation date is "([^"]+)"$/,
      execute: async (ctx, _engine, match) => {
        ctx.calculationDate = match[1];
      },
    },
    {
      pattern: /^parameter "([^"]+)" is "([^"]*)"$/,
      execute: async (ctx, _engine, match) => {
        ctx.parameters[match[1]] = parseValue(match[2]);
      },
    },
    {
      pattern: /^parameter "([^"]+)" is (-?\d+(?:\.\d+)?)$/,
      execute: async (ctx, _engine, match) => {
        ctx.parameters[match[1]] = parseValue(match[2]);
      },
    },
    {
      pattern: /^the following parameters:$/,
      execute: async (ctx, _engine, _match, step) => {
        if (!step.dataTable) return;
        for (const row of step.dataTable.slice(1)) {
          ctx.parameters[row[0]] = parseValue(row[1] || '');
        }
      },
    },
    {
      pattern: /^law "([^"]+)" is loaded$/,
      execute: async (_ctx, engine, match) => {
        const lawId = match[1];
        if (!engine.hasLaw(lawId)) {
          await loadDependency(lawId);
        }
      },
    },
    {
      pattern: /^the following "([^"]+)" data with key "([^"]+)":$/,
      execute: async (_ctx, engine, match, step) => {
        const sourceName = match[1];
        const keyField = match[2];
        const records = tableToRecords(step.dataTable);
        engine.registerDataSource(sourceName, keyField, records);
      },
    },

    // --- When: Execution steps ---
    {
      pattern: /^I evaluate "([^"]+)" of "([^"]+)"$/,
      execute: async (ctx, engine, match) => {
        const outputName = match[1];
        const lawId = match[2];
        if (!ctx.calculationDate) {
          throw new Error('No calculation date set. Add: Given the calculation date is "YYYY-MM-DD"');
        }
        const date = ctx.calculationDate;

        try {
          ctx.result = engine.execute(lawId, outputName, ctx.parameters, date);
          ctx.executed = true;
          ctx.error = null;
        } catch (e) {
          ctx.error = e;
          ctx.executed = true;
          ctx.result = null;
        }
      },
    },

    // --- Then: Assertion steps ---
    {
      pattern: /^the execution succeeds$/,
      execute: async (ctx) => {
        if (ctx.error) {
          throw new Error(`Expected execution to succeed, but got error: ${ctx.error}`);
        }
        if (!ctx.executed) {
          throw new Error('No execution was performed');
        }
      },
    },
    {
      pattern: /^the execution fails$/,
      execute: async (ctx) => {
        if (!ctx.error) {
          throw new Error('Expected execution to fail, but it succeeded');
        }
      },
    },
    {
      pattern: /^the execution fails with "([^"]+)"$/,
      execute: async (ctx, _engine, match) => {
        if (!ctx.error) {
          throw new Error('Expected execution to fail, but it succeeded');
        }
        const expected = match[1];
        const errorStr = String(ctx.error);
        if (!errorStr.includes(expected)) {
          throw new Error(
            `Expected error containing "${expected}", got: ${errorStr}`,
          );
        }
      },
    },
    {
      pattern: /^output "([^"]+)" is true$/,
      execute: async (ctx, _engine, match) => {
        assertOutput(ctx, match[1], true);
      },
    },
    {
      pattern: /^output "([^"]+)" is false$/,
      execute: async (ctx, _engine, match) => {
        assertOutput(ctx, match[1], false);
      },
    },
    {
      pattern: /^output "([^"]+)" equals (-?\d+(?:\.\d+)?)$/,
      execute: async (ctx, _engine, match) => {
        assertOutput(ctx, match[1], parseValue(match[2]));
      },
    },
    {
      pattern: /^output "([^"]+)" equals "([^"]*)"$/,
      execute: async (ctx, _engine, match) => {
        assertOutput(ctx, match[1], match[2]);
      },
    },
    {
      pattern: /^output "([^"]+)" is null$/,
      execute: async (ctx, _engine, match) => {
        assertOutput(ctx, match[1], null);
      },
    },
    {
      pattern: /^output "([^"]+)" contains "([^"]+)"$/,
      execute: async (ctx, _engine, match) => {
        const name = match[1];
        const substring = match[2];
        const actual = getOutput(ctx, name);
        if (typeof actual !== 'string' || !actual.includes(substring)) {
          throw new Error(
            `Expected output "${name}" to contain "${substring}", got: ${JSON.stringify(actual)}`,
          );
        }
      },
    },
  ];
}

function getOutput(ctx, name) {
  if (!ctx.result || !ctx.result.outputs) {
    throw new Error(`No outputs available (execution ${ctx.executed ? 'failed' : 'not performed'})`);
  }
  return ctx.result.outputs[name];
}

function assertOutput(ctx, name, expected) {
  const actual = getOutput(ctx, name);
  if (!primitiveEqual(actual, expected)) {
    throw new Error(
      `Expected output "${name}" to equal ${JSON.stringify(expected)}, got: ${JSON.stringify(actual)}`,
    );
  }
}

function primitiveEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'number') return Math.abs(a - b) < 1e-9;
  return false;
}
