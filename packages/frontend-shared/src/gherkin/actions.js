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
 * grammar `literals` appended. `table` is the step's dataTable rows (string[][])
 * or null. Whether row 0 is a header depends on the step: a data-source table
 * has one, a parameter table does not.
 */

import { VALUE_TYPING } from './grammar.generated.js';

/**
 * Parse a string value to a typed value, mirroring Rust value_conversion.rs.
 * - "true"/"false" → boolean
 * - "null" → null
 * - numeric → number (int or float)
 * - otherwise → string
 */
export function parseValue(str) {
  // Already typed (a form hands real booleans and numbers straight through).
  if (typeof str !== 'string') return str;
  if (str === 'true') return true;
  if (str === 'false') return false;
  if (str === 'null') return null;

  // A JSON array or object literal: the only way an array- or object-typed
  // register input fits in a table cell. Mirrors the Rust helper
  // (`convert_gherkin_value`); a cell that does not parse stays a string.
  if (str.startsWith('[') || str.startsWith('{')) {
    try {
      return JSON.parse(str);
    } catch {
      // not JSON after all: fall through to the scalar rules
    }
  }

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
 * Type a quoted capture. The three `value_typing` rules live in
 * bdd/grammar.yaml and reach both dispatchers through codegen, so neither
 * engine can hold its own opinion about what a quote means.
 */
export function quotedValue(raw) {
  switch (VALUE_TYPING.quoted) {
    case 'literal': return raw;
    case 'inferred': return parseValue(raw);
    default:
      throw new Error(`bdd/grammar.yaml: unknown value_typing.quoted '${VALUE_TYPING.quoted}'`);
  }
}

/** Type a bare (unquoted) capture; see [quotedValue] for where the rule lives. */
export function bareValue(raw) {
  if (VALUE_TYPING.bare !== 'number') {
    throw new Error(`bdd/grammar.yaml: unknown value_typing.bare '${VALUE_TYPING.bare}'`);
  }
  return Number(raw);
}

/** Type a data-table cell; see [quotedValue] for where the rule lives. */
export function tableCellValue(raw) {
  switch (VALUE_TYPING.table_cell) {
    case 'inferred': return parseValue(raw);
    case 'literal': return raw.trim();
    default:
      throw new Error(`bdd/grammar.yaml: unknown value_typing.table_cell '${VALUE_TYPING.table_cell}'`);
  }
}

/**
 * Parse a data table into record objects using the header row.
 *
 * A row that does not match the header row is rejected rather than filled with
 * undefined: today the Gherkin parser already rejects a varying cell count, but
 * that guarantee lives in a dependency we bump, and the Rust mirror
 * (`rows_to_records`) leans on a different parser. The explicit check keeps
 * both sides failing the same way if either parser ever loosens.
 */
export function tableToRecords(dataTable) {
  if (!dataTable || dataTable.length < 2) return [];
  const headers = dataTable[0];
  return dataTable.slice(1).map((row, rowIndex) => {
    if (row.length !== headers.length) {
      throw new Error(
        `data table row ${rowIndex + 1} has ${row.length} cells, header row has ${headers.length}`,
      );
    }
    const record = {};
    headers.forEach((h, i) => {
      record[h] = tableCellValue(row[i]);
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

/**
 * Structural equality between an engine output and an expected value,
 * mirroring the Rust harness (`values_equal_with_tolerance` on top of
 * `Value`'s `PartialEq` in law-model):
 * - numbers: equal within 1e-9 (the engine keeps exact decimals, the JS side
 *   sees f64, so literal-parsing noise must not fail a scenario);
 * - null only equals null (a missing output is `undefined`, not null);
 * - a number never equals its string form, a boolean never equals anything
 *   but the same boolean;
 * - arrays: same length, elements equal in order;
 * - objects: same key set, values equal per key (a BTreeMap on the Rust
 *   side, so key order is irrelevant).
 * The tolerance is applied at every depth. Rust applies it only at the top
 * level and compares nested Int/Decimal exactly, but that distinction does
 * not exist in JS (both are `number`), so a recursive tolerance is the
 * closest mapping and never rejects what Rust accepts.
 */
export function valuesEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null || a === undefined || b === undefined) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'number') return Math.abs(a - b) < 1e-9;
  if (typeof a !== 'object') return false;

  const aIsArray = Array.isArray(a);
  if (aIsArray !== Array.isArray(b)) return false;
  if (aIsArray) {
    if (a.length !== b.length) return false;
    return a.every((item, i) => valuesEqual(item, b[i]));
  }

  const aKeys = Object.keys(a);
  const bKeys = Object.keys(b);
  if (aKeys.length !== bKeys.length) return false;
  return aKeys.every((key) => Object.hasOwn(b, key) && valuesEqual(a[key], b[key]));
}

/** @deprecated Kept for callers of the old name; arrays and objects are compared too. */
export const primitiveEqual = valuesEqual;

function assertOutput(ctx, name, expected) {
  const actual = getOutput(ctx, name);
  if (!valuesEqual(actual, expected)) {
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
 * @param {string[][]|null} table - step dataTable rows, or null; row 0 is a
 *   header for a data-source table and a value row for a parameter table
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
      // Already typed by the grammar's `value_typing` rule in buildArgs.
      ctx.parameters[args[0]] = args[1];
      break;

    // No header row on a parameter table (mirror of Rust `rows_to_params`);
    // every row is a name/value pair.
    case 'set_parameters_table':
      for (const row of table || []) {
        if (row.length < 2) continue;
        ctx.parameters[row[0].trim()] = tableCellValue(row[1] || '');
      }
      break;

    case 'set_parameter_collection':
      // Header row plus one row per element, each element an object keyed by
      // the column names — the shape the iterating operations expect (RFC-016).
      ctx.parameters[args[0]] = tableToRecords(table);
      break;

    case 'set_data_source': {
      const sourceName = args[0];
      const keyField = args[1];
      const records = tableToRecords(table);
      engine.registerDataSource(sourceName, keyField, records);
      break;
    }

    // Bound to one law: consulted only while that law's inputs resolve, so
    // the rows can never shadow a same-named cross-law input elsewhere.
    case 'set_data_source_for_law': {
      const sourceName = args[0];
      const keyField = args[1];
      const lawId = args[2];
      const records = tableToRecords(table);
      engine.registerDataSourceForLaw(lawId, sourceName, keyField, records);
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
