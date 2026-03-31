/**
 * useExecution — shared execution state between form and result panels.
 *
 * Manages the WASM engine execution lifecycle: registering data sources,
 * executing laws, and storing results.
 */
import { ref } from 'vue';
import { parseValue } from '../gherkin/steps.js';

export function useExecution() {
  const result = ref(null);
  const trace = ref(null);
  const traceText = ref(null);
  const running = ref(false);
  const error = ref(null);
  const expectations = ref({});

  /**
   * Execute a law.
   *
   * @param {object} engine - WasmEngine instance
   * @param {object} payload - Execution payload from the form
   */
  async function execute(engine, payload) {
    if (!engine) return;

    running.value = true;
    result.value = null;
    trace.value = null;
    traceText.value = null;
    error.value = null;
    expectations.value = payload.expectations || {};

    try {
      // Clear previous data sources
      engine.clearDataSources();

      // Register data source tables
      for (const ds of payload.dataSources || []) {
        if (ds.rows.length === 0) continue;

        const typedRows = ds.rows.map((row) => {
          const typed = {};
          for (const [k, v] of Object.entries(row)) {
            if (k === '_id') continue;
            typed[k] = typeof v === 'string' ? parseValue(v) : v;
          }
          return typed;
        });
        engine.registerDataSource(ds.sourceName, ds.keyField, typedRows);
      }

      // Build parameters
      const params = {};
      for (const [k, v] of Object.entries(payload.parameters || {})) {
        if (v !== '' && v !== null && v !== undefined) {
          params[k] = typeof v === 'string' ? parseValue(v) : v;
        }
      }

      // Use executeWithTrace for full execution trace tree
      const execResult = engine.executeWithTrace(
        payload.lawId,
        payload.outputName,
        params,
        payload.calculationDate,
      );

      result.value = execResult;
      trace.value = execResult.trace || null;
      traceText.value = execResult.trace_text || null;
    } catch (e) {
      // TracedError returns a JS object with { error, trace, trace_text }
      if (e && typeof e === 'object' && e.error) {
        error.value = e.error;
        trace.value = e.trace || null;
        traceText.value = e.trace_text || null;
      } else {
        error.value = typeof e === 'string' ? e : (e.message || String(e));
      }
    } finally {
      running.value = false;
    }
  }

  function reset() {
    result.value = null;
    trace.value = null;
    traceText.value = null;
    error.value = null;
    expectations.value = {};
  }

  return {
    result,
    trace,
    traceText,
    running,
    error,
    expectations,
    execute,
    reset,
  };
}
