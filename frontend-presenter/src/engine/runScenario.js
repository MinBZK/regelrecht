/**
 * Run one scenario from a .feature file against a WasmEngine, the way the
 * Rust BDD runner does: background first, then the scenario's steps, stopping
 * at the first failure (the steps after it are reported as not run).
 *
 * Two departures from the plain `dispatch` loop, both for the slide:
 * - An evaluate step runs with a trace (`executeMultipleWithTrace`), so the
 *   slide can show how the outcome came about. Same interception as the
 *   demo's scenario view (frontend-demo/src/views/ScenariosView.vue).
 * - A law is loaded just before it is evaluated when the engine does not have
 *   it yet. A scenario next to a law lists its dependencies in the background
 *   but not the law under test; the Rust runner loads that one from the
 *   folder, and here it comes from the same corpus lookup.
 *
 * The engine and the law loader are passed in, so this runs against a fake
 * engine in a test and against the real WASM in the browser.
 */
import { ExecutionContext, dispatch, matchStep, parseFeature, traceRoot, typedArgs } from '@regelrecht/frontend-shared/gherkin';

/** The scenario by exact title; without a title, the only one. */
export function pickScenario(parsed, name) {
  const all = parsed.scenarios ?? [];
  if (name) {
    const hit = all.find((s) => s.name === name);
    if (hit) return hit;
    throw new Error(`Scenario "${name}" staat niet in dit bestand. Wel: ${all.map((s) => `"${s.name}"`).join(', ')}`);
  }
  if (all.length === 1) return all[0];
  throw new Error(`Dit bestand heeft ${all.length} scenario's; kies er een met naam:. Keuze: ${all.map((s) => `"${s.name}"`).join(', ')}`);
}

/**
 * @returns {Promise<{
 *   scenario: {name: string, tags: string[]},
 *   steps: {keyword: string, kind: string, text: string, status: 'passed'|'failed'|'skipped'|'unsupported', message?: string}[],
 *   run: {law: string, outputs: object, trace: object|null, traceText: string, engineVersion?: string}|null,
 *   error: string|null,
 *   failed: boolean,
 * }>}
 */
export async function runScenario({ engine, text, name, loadLaw }) {
  const parsed = parseFeature(text);
  const scenario = pickScenario(parsed, name);

  // Two runs of the same scenario must give the same answer, whatever ran before.
  engine.clearDataSources();
  const ctx = new ExecutionContext();
  const ensureLaw = async (lawId) => {
    if (!engine.hasLaw(lawId)) await loadLaw(lawId);
  };

  const steps = [];
  let run = null;
  let error = null;
  let failed = false;

  let kind = 'Given';
  for (const step of [...(parsed.background ?? []), ...scenario.steps]) {
    const keyword = step.keyword.trim();
    // `And`/`But`/`*` continue the step kind before them: an `And` after a
    // `Then` is an expectation too.
    if (!['And', 'But', '*'].includes(keyword)) kind = keyword;
    const record = { keyword, kind, text: step.text, status: 'skipped' };
    steps.push(record);
    if (failed) continue;

    const match = matchStep(step.text);
    if (!match) {
      failed = true;
      record.status = 'failed';
      record.message = 'Deze formulering staat niet in de grammatica (bdd/grammar.yaml).';
      continue;
    }
    const { entry, args } = match;
    // The shared dispatch knows only the core tier; the Rust runner in CI also
    // checks provenance, notes and untranslatables. Such a step is marked as
    // not checked here instead of failing a scenario that is green in CI.
    // evaluate_outputs is the exception: it is a run, and the block does that itself.
    if (entry.tier !== 'core' && entry.action !== 'evaluate_outputs') {
      record.status = 'unsupported';
      record.message = `Wordt in CI gecontroleerd, niet in de presentatie (${entry.tier}).`;
      continue;
    }
    const typed = typedArgs(entry, args);
    try {
      if (entry.action === 'evaluate' || entry.action === 'evaluate_outputs') {
        const outputs = entry.action === 'evaluate' ? [typed[0]] : String(typed[0]).split(',').map((s) => s.trim());
        const lawId = typed[1];
        await ensureLaw(lawId);
        run = evaluateWithTrace(ctx, engine, lawId, outputs);
        if (ctx.error) error = errorText(ctx.error);
      } else {
        await dispatch(ctx, engine, entry.action, [...typed, ...entry.literals], step.dataTable ?? null, { loadDependency: loadLaw });
      }
      record.status = 'passed';
    } catch (e) {
      failed = true;
      record.status = 'failed';
      record.message = errorText(e);
    }
  }

  engine.clearDataSources();
  return { scenario: { name: scenario.name, tags: [...featureTags(text), ...(scenario.tags ?? [])] }, steps, run, error, failed };
}

function evaluateWithTrace(ctx, engine, lawId, outputs) {
  if (!ctx.calculationDate) throw new Error('Geen rekendatum. Voeg toe: Given the calculation date is "JJJJ-MM-DD"');
  try {
    const raw = engine.executeMultipleWithTrace(lawId, outputs, ctx.parameters, ctx.calculationDate);
    const result = { ...raw, outputs: nullForUndefined(raw.outputs) };
    Object.assign(ctx, { result, executed: true, error: null });
    return {
      law: lawId,
      asked: outputs,
      outputs: result.outputs ?? {},
      trace: traceRoot(result.trace),
      traceText: result.trace_text ?? '',
      engineVersion: result.engine_version,
    };
  } catch (e) {
    // The engine throws a string or {error, trace}; the assertions after it
    // report the failure, as in the Rust runner.
    // As text: the shared assertions compare String(ctx.error), and an
    // {error, trace} object would read as "[object Object]".
    Object.assign(ctx, { result: null, executed: true, error: errorText(e) });
    return { law: lawId, asked: outputs, outputs: {}, trace: traceRoot(e?.trace), traceText: e?.trace_text ?? '' };
  }
}

/**
 * Tags on the Feature itself (the shared parser returns only scenario tags):
 * a `@wip` there applies to every scenario, as in the Rust runner.
 */
export function featureTags(text) {
  const head = String(text).split(/^\s*Feature:/m)[0];
  return head.match(/@[\w-]+/g) ?? [];
}

/**
 * The WASM binding serialises a Rust null (an output that is absent, RFC-036)
 * as `undefined`: serde_wasm_bindgen does that unless `serialize_missing_as_null`
 * is set, and packages/engine/src/wasm.rs does not set it. The key is still
 * there. The Rust BDD runner sees null, so without this `output "x" is absent`
 * fails on a slide while it passes in CI. A key with `undefined` can only be
 * such a null, so it is turned back. (To be fixed in the binding itself.)
 */
export function nullForUndefined(outputs) {
  return Object.fromEntries(Object.entries(outputs ?? {}).map(([k, v]) => [k, v === undefined ? null : v]));
}

export function errorText(e) {
  return String(typeof e === 'string' ? e : e?.error ?? e?.message ?? JSON.stringify(e));
}
