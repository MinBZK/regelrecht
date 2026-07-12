/**
 * Editor BDD semantics - the single JS dispatch file (mirror of Rust dispatch.rs).
 *
 * One `dispatch(ctx, engine, action, args, table, { loadDependency })` runs the
 * effect for a canonical grammar action. The generated grammar
 * (grammar.generated.js) supplies patterns/templates; this file supplies the
 * behavior. The editor's WASM engine implements only the `core` tier; actions
 * from the `notes`/`untranslatable`/`provenance` tiers throw, so a conformance
 * feature can never silently no-op in the editor.
 *
 * `args` is the ordered capture list (typed per the grammar's argTypes) with any
 * grammar `literals` appended. `table` is the step's dataTable rows (string[][],
 * header row first) or null.
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
 * Parse a data table into record objects using the header row.
 * Values are auto-typed via parseValue() so the engine receives correctly
 * typed numbers, booleans, and nulls.
 */
export function tableToRecords(dataTable) {
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

export function getOutput(ctx, name) {
  if (!ctx.result || !ctx.result.outputs) {
    throw new Error(`No outputs available (execution ${ctx.executed ? 'failed' : 'not performed'})`);
  }
  return ctx.result.outputs[name];
}

export function primitiveEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'number') return Math.abs(a - b) < 1e-9;
  return false;
}

function assertOutput(ctx, name, expected) {
  const actual = getOutput(ctx, name);
  if (!primitiveEqual(actual, expected)) {
    throw new Error(
      `Expected output "${name}" to equal ${JSON.stringify(expected)}, got: ${JSON.stringify(actual)}`,
    );
  }
}

function tierUnsupported(tier) {
  return new Error(`tier ${tier} not supported by the editor engine`);
}

/**
 * Run a canonical grammar action against the execution context + WASM engine.
 *
 * @param {object} ctx - ExecutionContext (calculationDate, parameters, result, error, executed)
 * @param {object} engine - WasmEngine instance
 * @param {string} action - canonical action id from the grammar
 * @param {Array} args - ordered typed captures + literals
 * @param {string[][]|null} table - step dataTable rows (header first) or null
 * @param {object} options
 * @param {(lawId: string) => Promise<void>} options.loadDependency
 */
export async function dispatch(ctx, engine, action, args, table, { loadDependency }) {
  switch (action) {
    // --- core: setup ---
    case 'set_calculation_date':
      ctx.calculationDate = args[0];
      break;

    case 'load_law': {
      const lawId = args[0];
      if (!engine.hasLaw(lawId)) {
        await loadDependency(lawId);
      }
      break;
    }

    case 'set_parameter':
      // String form keeps the raw string (preserves identifiers like BSNs);
      // numeric form arrives already typed as a Number from the grammar.
      ctx.parameters[args[0]] = args[1];
      break;

    case 'set_parameters_table':
      if (table) {
        for (const row of table.slice(1)) {
          ctx.parameters[row[0]] = parseValue(row[1] || '');
        }
      }
      break;

    case 'set_data_source': {
      const sourceName = args[0];
      const keyField = args[1];
      const records = tableToRecords(table);
      engine.registerDataSource(sourceName, keyField, records);
      break;
    }

    // --- core: execute ---
    case 'evaluate': {
      const outputName = args[0];
      const lawId = args[1];
      if (!ctx.calculationDate) {
        throw new Error('No calculation date set. Add: Given the calculation date is "YYYY-MM-DD"');
      }
      try {
        ctx.result = engine.execute(lawId, outputName, ctx.parameters, ctx.calculationDate);
        ctx.executed = true;
        ctx.error = null;
      } catch (e) {
        ctx.error = e;
        ctx.executed = true;
        ctx.result = null;
      }
      break;
    }

    // --- core: asserts ---
    case 'assert_succeeds':
      if (ctx.error) {
        throw new Error(`Expected execution to succeed, but got error: ${ctx.error}`);
      }
      if (!ctx.executed) {
        throw new Error('No execution was performed');
      }
      break;

    case 'assert_fails':
      if (!ctx.error) {
        throw new Error('Expected execution to fail, but it succeeded');
      }
      break;

    case 'assert_fails_with': {
      if (!ctx.error) {
        throw new Error('Expected execution to fail, but it succeeded');
      }
      const expected = args[0];
      const errorStr = String(ctx.error);
      if (!errorStr.toLowerCase().includes(expected.toLowerCase())) {
        throw new Error(`Expected error containing "${expected}", got: ${errorStr}`);
      }
      break;
    }

    case 'assert_boolean':
      // args = [output, literalBool]
      assertOutput(ctx, args[0], args[1]);
      break;

    case 'assert_equals':
      // Numeric form arrives as Number; string form as String.
      assertOutput(ctx, args[0], args[1]);
      break;

    case 'assert_null':
      assertOutput(ctx, args[0], null);
      break;

    case 'assert_contains': {
      const name = args[0];
      const substring = args[1];
      const actual = getOutput(ctx, name);
      if (typeof actual !== 'string' || !actual.toLowerCase().includes(substring.toLowerCase())) {
        throw new Error(
          `Expected output "${name}" to contain "${substring}", got: ${JSON.stringify(actual)}`,
        );
      }
      break;
    }

    // --- non-core tiers: not supported by the editor engine ---
    case 'evaluate_outputs':
    case 'assert_exact_outputs':
    case 'assert_provenance':
      throw tierUnsupported('provenance');

    case 'set_untranslatable_mode':
    case 'assert_tainted':
      throw tierUnsupported('untranslatable');

    case 'set_note_articles':
    case 'set_note_selector_exact':
    case 'set_note_selector_context':
    case 'set_note_hint_article':
    case 'set_note_hint_position':
    case 'resolve_note':
    case 'assert_note_resolves':
    case 'assert_note_exact_match':
    case 'assert_note_fuzzy_match':
    case 'assert_note_orphaned':
    case 'assert_note_ambiguous':
      throw tierUnsupported('notes');

    default:
      throw new Error(`unknown action '${action}' - grammar/dispatch out of sync`);
  }
}
