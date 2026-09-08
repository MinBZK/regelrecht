/**
 * Bidirectional mapper between Gherkin AST and structured form state.
 *
 * Forward:  parseFeature() AST → mapFeatureToForm() → visual form state
 * Reverse:  visual form state → formStateToGherkin() → Gherkin text
 */

import { bareValue, quotedValue, tableCellValue } from './actions.js';
import { GRAMMAR } from './grammar.generated.js';

// --- Generated grammar lookups (single source of truth for phrasing) ---

// Emit-templates keyed by grammar entry id, and a capitalized keyword prefix
// (Given/When/Then) per entry id - both derived from GRAMMAR so phrasing and
// keyword stay single-sourced.
const TPL = Object.fromEntries(GRAMMAR.map((e) => [e.id, e.template]));
const KW = Object.fromEntries(
  GRAMMAR.map((e) => [e.id, e.keyword.charAt(0).toUpperCase() + e.keyword.slice(1)]),
);

// --- Extraction: classify a step line into a form-state fragment ---
//
// We reuse the GENERATED patterns (the drift surface) and keep only the
// extraction → form-state mapping hand-written, keyed on the grammar `action`.
// The editor consumes the `core` tier only.
const CORE_ENTRIES = GRAMMAR.filter((e) => e.tier === 'core');

/**
 * Extract a form-state fragment from a matched core grammar entry.
 * Disambiguates the two parameter forms by entry id (string vs number).
 */
function extractFragment(entry, match, step) {
  switch (entry.action) {
    case 'set_calculation_date':
      return { type: 'calculationDate', value: match[1] };
    case 'load_law':
      return { type: 'dependency', lawId: match[1] };
    case 'set_parameter':
      return {
        type: 'parameter',
        name: match[1],
        value: entry.id === 'set_parameter_number' ? bareValue(match[2]) : quotedValue(match[2]),
      };
    case 'set_parameters_table':
      return { type: 'parameterTable', parameters: tableToParams(step.dataTable) };
    case 'set_parameter_collection': {
      // A collection-valued parameter (RFC-016): header row plus one row per
      // element. The columns are kept next to the records so a collection
      // without elements still serializes with its header.
      const { columns, records } = tableToCollection(step.dataTable);
      return { type: 'parameter', name: match[1], value: records, columns };
    }
    case 'set_data_source':
      return {
        type: 'dataSource',
        sourceName: match[1],
        keyField: match[2],
        headers: step.dataTable?.[0] || [],
        rows: step.dataTable?.slice(1) || [],
      };
    case 'set_data_source_for_law':
      // Same shape plus the law the source is bound to; the builder shows it
      // as a data source and writes the scoped step back out.
      return {
        type: 'dataSource',
        sourceName: match[1],
        keyField: match[2],
        lawId: match[3],
        headers: step.dataTable?.[0] || [],
        rows: step.dataTable?.slice(1) || [],
      };
    case 'evaluate':
      return { type: 'execution', outputName: match[1], lawId: match[2] };
    case 'assert_succeeds':
      return { type: 'assertion', assertionType: 'succeeds' };
    case 'assert_fails':
      return { type: 'assertion', assertionType: 'fails' };
    case 'assert_fails_with':
      return { type: 'assertion', assertionType: 'failsWith', value: match[1] };
    case 'assert_boolean':
      return {
        type: 'assertion',
        assertionType: 'boolean',
        outputName: match[1],
        value: entry.id === 'assert_boolean_true',
      };
    case 'assert_equals':
      return entry.id === 'assert_equals_number'
        ? { type: 'assertion', assertionType: 'equals', outputName: match[1], value: bareValue(match[2]) }
        : { type: 'assertion', assertionType: 'equalsString', outputName: match[1], value: match[2] };
    case 'assert_null':
      return { type: 'assertion', assertionType: 'null', outputName: match[1] };
    case 'assert_contains':
      return { type: 'assertion', assertionType: 'contains', outputName: match[1], value: match[2] };
    default:
      return null;
  }
}

// A `Given the following parameters:` table is two columns of name/value with
// no header row (mirror of Rust `rows_to_params`), so every row is data.
function tableToParams(dataTable) {
  if (!dataTable) return [];
  return dataTable
    .filter((row) => row.length >= 2)
    .map((row) => ({
      name: row[0].trim(),
      value: tableCellValue(row[1] || ''),
    }));
}

// A `Given parameter "x" is the collection:` table has a header row and one
// row per element (mirror of Rust `rows_to_records`): every element becomes
// an object keyed by the column names, cells typed by content. Lenient on a
// short row - a missing cell reads as null - so one ragged row does not
// take the whole feature down in the editor.
function tableToCollection(dataTable) {
  if (!dataTable || dataTable.length === 0) return { columns: [], records: [] };
  const columns = dataTable[0].map((h) => h.trim());
  const records = dataTable.slice(1).map((row) => {
    const record = {};
    columns.forEach((h, i) => {
      record[h] = i < row.length ? tableCellValue(row[i]) : null;
    });
    return record;
  });
  return { columns, records };
}

/** True when a parameter value is a collection (array of records). */
export function isCollectionValue(value) {
  return Array.isArray(value);
}

/**
 * Type one cell of a collection as edited in the form: an empty or absent
 * cell is null (what formatCell writes and the runner reads back), a string
 * is typed by content, anything else is already typed.
 */
export function collectionCell(v) {
  if (v === undefined || v === null || v === '') return null;
  return typeof v === 'string' ? tableCellValue(v) : v;
}

/** Column names of a collection parameter: the recorded header, or the keys of the first element. */
export function collectionColumns(param) {
  if (param.columns?.length) return param.columns;
  const first = (param.value || [])[0];
  return first ? Object.keys(first) : [];
}

function classifyStep(step) {
  const text = step.text;
  for (const entry of CORE_ENTRIES) {
    const match = text.match(entry.pattern);
    if (match) {
      return extractFragment(entry, match, step);
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
        setup.parameters.push(
          classified.columns
            ? { name: classified.name, value: classified.value, columns: classified.columns }
            : { name: classified.name, value: classified.value },
        );
        break;
      case 'parameterTable':
        setup.parameters.push(...classified.parameters);
        break;
      case 'dataSource':
        setup.dataSources.push({
          sourceName: classified.sourceName,
          keyField: classified.keyField,
          ...(classified.lawId ? { lawId: classified.lawId } : {}),
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

/**
 * Convert a DataSourceTable form-format data source to the formState format.
 *
 * Form format: `{ sourceName, keyField, fields: [{name, type}], rows: [{_id, [h]: v}] }`
 * State format: `{ sourceName, keyField, headers: string[], rows: string[][] }`
 */
function formDataSourceToState(ds) {
  const headers = [ds.keyField, ...(ds.fields || []).map((f) => f.name)];
  const rows = (ds.rows || []).map((row) =>
    headers.map((h) => {
      const v = row[h];
      return v === undefined || v === null ? '' : String(v);
    }),
  );
  return { sourceName: ds.sourceName, keyField: ds.keyField, headers, rows };
}

/**
 * Convert a collection as edited in the form to the formState parameter
 * value: the column names plus one record per row, `_id` dropped and cells
 * typed by content (the same rule the runner applies to the saved table).
 *
 * Form format:  `{ name, columns: [{name, type}], rows: [{_id, [col]: v}] }`
 * State format: `{ columns: string[], records: object[] }`
 */
export function formCollectionToState(coll) {
  const columns = (coll.columns || []).map((c) => c.name);
  const records = (coll.rows || []).map((row) => {
    const record = {};
    for (const c of columns) {
      record[c] = collectionCell(row[c]);
    }
    return record;
  });
  return { columns, records };
}

/**
 * Equality for two parameter values, scalar or collection. A collection is
 * compared record by record; `String()` on an array would flatten every
 * collection to "[object Object]" and call them all equal.
 */
function parameterValuesEqual(a, b) {
  // Key order counts: an override that lists the same columns in another
  // order never collapses into the background. That keeps a step, it never
  // loses one, so it is left as is.
  if (isCollectionValue(a) || isCollectionValue(b)) {
    return isCollectionValue(a) && isCollectionValue(b) && JSON.stringify(a) === JSON.stringify(b);
  }
  return String(a) === String(b);
}

/** Deep equality check for two state-format data sources. */
function dataSourcesEqual(a, b) {
  if (!a || !b) return false;
  if (a.sourceName !== b.sourceName || a.keyField !== b.keyField || (a.lawId ?? null) !== (b.lawId ?? null)) return false;
  if ((a.headers || []).length !== (b.headers || []).length) return false;
  for (let i = 0; i < a.headers.length; i++) {
    if (a.headers[i] !== b.headers[i]) return false;
  }
  if ((a.rows || []).length !== (b.rows || []).length) return false;
  for (let i = 0; i < a.rows.length; i++) {
    const ra = a.rows[i];
    const rb = b.rows[i];
    if (ra.length !== rb.length) return false;
    for (let j = 0; j < ra.length; j++) {
      if (String(ra[j]) !== String(rb[j])) return false;
    }
  }
  return true;
}

/**
 * Sync edited form values back into formState for a given scenario.
 *
 * Parameters that exist in the scenario's own setup are updated in-place.
 * Background-only parameters that were changed get added as scenario-level
 * overrides so other scenarios are not affected. Scenario-level overrides
 * that end up matching the background value are removed so the Gherkin
 * round-trip does not accumulate redundant `Given parameter ...` steps.
 *
 * Data sources follow the same rule: scenario-level overrides exist only
 * when they differ from the background source with the same name.
 *
 * @param {object} formState - Mutable form state
 * @param {number} scenarioIndex - Which scenario was edited
 * @param {object} values - { parameterValues, calculationDate, dataSources }
 */
export function syncEditedValues(formState, scenarioIndex, values) {
  const scenario = formState.scenarios[scenarioIndex];
  if (!scenario) return;

  const { parameterValues, calculationDate, dataSources, collections } = values;

  // --- Parameters ---
  const scenarioParamMap = new Map(
    scenario.setup.parameters.map((p, i) => [p.name, i]),
  );
  const bgParams = formState.background?.parameters || [];
  const bgParamMap = new Map(bgParams.map((p) => [p.name, p]));

  // Scalar parameters and collections share the override rule: a value that
  // lives in the scenario is updated in place, a background value that was
  // changed becomes a scenario-level override. `columns` only exists on a
  // collection.
  const applyParameter = (name, value, columns) => {
    const entry = columns ? { name, value, columns } : { name, value };
    if (scenarioParamMap.has(name)) {
      Object.assign(scenario.setup.parameters[scenarioParamMap.get(name)], entry);
    } else if (bgParamMap.has(name)) {
      if (!parameterValuesEqual(bgParamMap.get(name).value, value)) {
        scenario.setup.parameters.push(entry);
      }
    }
  };

  for (const [name, rawValue] of Object.entries(parameterValues)) {
    // Same rule as reading a step: the content decides. An input control hands
    // back a raw string, and leaving it at that would write `is "50000"` where
    // the scenario said `is 50000`.
    applyParameter(name, quotedValue(rawValue));
  }

  for (const coll of collections || []) {
    const { columns, records } = formCollectionToState(coll);
    applyParameter(coll.name, records, columns);
  }

  // Drop scenario-level overrides that now match the background - otherwise
  // a save/edit/save cycle accumulates redundant `Given parameter ...` steps.
  scenario.setup.parameters = scenario.setup.parameters.filter((p) => {
    if (!bgParamMap.has(p.name)) return true;
    return !parameterValuesEqual(bgParamMap.get(p.name).value, p.value);
  });

  // --- Data sources ---
  if (Array.isArray(dataSources)) {
    const bgDataSources = formState.background?.dataSources || [];
    const bgDsMap = new Map(bgDataSources.map((ds) => [ds.sourceName, ds]));
    const scenarioDsMap = new Map(
      scenario.setup.dataSources.map((ds, i) => [ds.sourceName, i]),
    );

    for (const formDs of dataSources) {
      const stateDs = formDataSourceToState(formDs);

      if (scenarioDsMap.has(stateDs.sourceName)) {
        // Update existing scenario-level data source in place
        scenario.setup.dataSources[scenarioDsMap.get(stateDs.sourceName)] = stateDs;
      } else if (bgDsMap.has(stateDs.sourceName)) {
        // Background data source - add scenario override only if it differs
        if (!dataSourcesEqual(bgDsMap.get(stateDs.sourceName), stateDs)) {
          scenario.setup.dataSources.push(stateDs);
        }
      } else {
        // Wholly new data source - add at scenario level
        scenario.setup.dataSources.push(stateDs);
      }
    }

    // Drop scenario-level data sources that now match the background.
    scenario.setup.dataSources = scenario.setup.dataSources.filter((ds) => {
      const bg = bgDsMap.get(ds.sourceName);
      return !bg || !dataSourcesEqual(bg, ds);
    });
  }

  // Sync calculation date (scenario-level override).
  // Treat `null`, `undefined` and `""` as "user cleared the field" so that
  // a clear is not silently dropped - otherwise the next save would write
  // the stale date back to the .feature file.
  if (calculationDate !== undefined) {
    const cleared = calculationDate === null || calculationDate === '';
    const bgDate = formState.background?.calculationDate || null;
    if (cleared || calculationDate === bgDate) {
      // Either the user cleared the field, or it now matches the background:
      // in both cases the scenario-level override should disappear.
      scenario.setup.calculationDate = null;
    } else {
      scenario.setup.calculationDate = calculationDate;
    }
  }
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
    writeSetupSteps(lines, formState.background, '    ');
    writeUnmatchedSteps(lines, formState.background.unmatchedSteps, '    ');
  }

  // Scenarios
  for (const scenario of formState.scenarios) {
    lines.push('');
    if (scenario.tags?.length > 0) {
      lines.push(`  ${scenario.tags.join(' ')}`);
    }
    lines.push(`  Scenario: ${scenario.name}`);
    writeSetupSteps(lines, scenario.setup, '    ');

    // Execution
    if (scenario.execution) {
      const line = TPL.evaluate([scenario.execution.outputName, scenario.execution.lawId]);
      lines.push(`    ${KW.evaluate} ${line}`);
    }

    // Assertions
    for (const assertion of scenario.assertions) {
      const [id, line] = formatAssertion(assertion);
      lines.push(`    ${KW[id]} ${line}`);
    }

    // Unmatched steps
    writeUnmatchedSteps(lines, scenario.unmatchedSteps, '    ');
  }

  return lines.join('\n') + '\n';
}

function writeSetupSteps(lines, setup, indent) {
  // All setup phrasings + keywords come from the generated grammar by entry id.
  if (setup.calculationDate) {
    lines.push(`${indent}${KW.set_calculation_date} ${TPL.set_calculation_date([setup.calculationDate])}`);
  }

  for (const dep of setup.dependencies || []) {
    lines.push(`${indent}${KW.load_law} ${TPL.load_law([dep])}`);
  }

  for (const param of setup.parameters || []) {
    if (isCollectionValue(param.value)) {
      lines.push(`${indent}${KW.set_parameter_collection} ${TPL.set_parameter_collection([param.name])}`);
      const columns = collectionColumns(param);
      lines.push(`${indent}  | ${columns.map(formatCell).join(' | ')} |`);
      for (const record of param.value) {
        const cells = columns.map((c) => formatCell(record[c]));
        lines.push(`${indent}  | ${cells.join(' | ')} |`);
      }
    } else if (typeof param.value === 'number') {
      lines.push(`${indent}${KW.set_parameter_number} ${TPL.set_parameter_number([param.name, formatValue(param.value)])}`);
    } else {
      lines.push(`${indent}${KW.set_parameter_string} ${TPL.set_parameter_string([param.name, formatValue(param.value)])}`);
    }
  }

  for (const ds of setup.dataSources || []) {
    if (ds.headers.length === 0) continue;
    lines.push(
      ds.lawId
        ? `${indent}${KW.set_data_source_for_law} ${TPL.set_data_source_for_law([ds.sourceName, ds.keyField, ds.lawId])}`
        : `${indent}${KW.set_data_source} ${TPL.set_data_source([ds.sourceName, ds.keyField])}`,
    );

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

/**
 * Render an assertion to its canonical line via the generated grammar templates.
 * Returns [grammarEntryId, line] so the caller can prefix the right keyword.
 */
function formatAssertion(assertion) {
  switch (assertion.assertionType) {
    case 'succeeds':
      return ['assert_succeeds', TPL.assert_succeeds([])];
    case 'fails':
      return ['assert_fails', TPL.assert_fails([])];
    case 'failsWith':
      return ['assert_fails_with', TPL.assert_fails_with([assertion.value])];
    case 'boolean':
      return assertion.value
        ? ['assert_boolean_true', TPL.assert_boolean_true([assertion.outputName])]
        : ['assert_boolean_false', TPL.assert_boolean_false([assertion.outputName])];
    case 'equals':
      return ['assert_equals_number', TPL.assert_equals_number([assertion.outputName, assertion.value])];
    case 'equalsString':
      return ['assert_equals_string', TPL.assert_equals_string([assertion.outputName, assertion.value])];
    case 'null':
      return ['assert_null', TPL.assert_null([assertion.outputName])];
    case 'contains':
      return ['assert_contains', TPL.assert_contains([assertion.outputName, assertion.value])];
    default:
      throw new Error(`formatAssertion: unhandled assertionType '${assertion.assertionType}' - classifier/serializer out of sync`);
  }
}
