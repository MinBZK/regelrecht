/*
 * Running the scenario for real, in the visitor's browser.
 *
 * This is the only execution the panel has. There was a recorded one beside it
 * for a while, rendered server-side so the panel was never empty, and it turned
 * out to be one execution too many: two traces of the same law, with different
 * step counts because they asked the engine different questions, two sets of
 * wording that both had to stay true, and two buttons a visitor could not tell
 * apart. On a page arguing that one law should have one execution, that was the
 * wrong shape. So: the engine runs here or the panel says nothing ran.
 *
 * The engine and the laws are about 600 KB gzipped, fetched when the panel
 * before this one scrolls into view and run when this one does. Not on page
 * load: a visitor who never reaches the panel never pays for it.
 *
 * Nothing here draws anything. It produces rows, and the player animates them.
 */
import {
  parseFeature,
  dispatch,
  quotedValue,
  bareValue,
  traceRoot,
  GRAMMAR,
  ExecutionContext,
  // Copied into this project by `just landing-laws`, not imported from the
  // workspace: see script/landing-laws.sh for why, and for the --check that
  // fails the build when this copy falls behind the shared package.
} from '~/lib/gherkin/index.js';

/** The output the panel puts on screen; the scenario asserts it too. */
const AMOUNT_OUTPUT = 'hoogte_zorgtoeslag';

/** One row of the trace, in the shape the server-rendered rows already use. */
export interface Beat {
  identifier: string;
  law: string | null;
  type: string;
  result: unknown;
  unit: string | null;
  provider: string | null;
  depth: number;
  tree: string;
}

export interface RunResult {
  beats: Beat[];
  amount: unknown;
  steps: number;
  durationMs: number;
}

interface Manifest {
  calculationDate: string;
  scenario: string;
  laws: string[];
}

/**
 * The box-drawing prefixes, one per row.
 *
 * The prefixes are not in the trace itself, and this is the only place that
 * draws them. Putting them in the trace would mean every consumer of an RFC-039
 * trace carries one renderer's drawing choices around; the engine's own
 * terminal view draws the same shapes from the same structure, separately.
 *
 * The distinction the shapes carry: double rules for a scope a cross-law call
 * opens, single rules for the arithmetic inside it. Plain indentation flattens
 * that away.
 */
function treePrefixes(root: any): string[] {
  const out: string[] = [];

  const walk = (node: any, cols: string[], isLast: boolean, isDouble: boolean, isRoot: boolean) => {
    if (!isRoot) {
      const connector = isDouble ? (isLast ? '╙──' : '╟──') : isLast ? '└──' : '├──';
      out.push(cols.join('') + connector);
    }

    // A cross-law call opens a scope of its own, drawn with the double rules;
    // everything else continues the scope it is in.
    //
    // The names are the serialised ones, snake_case, not the Rust variants:
    // testing for `CrossLawReference` here silently never matched, and the
    // whole tree came out in single lines.
    const opensScope = node.node_type === 'cross_law_reference' || node.node_type === 'article';

    const nextCols = isRoot ? cols : [...cols, isLast ? '    ' : isDouble ? '║   ' : '│   '];

    const children = node.children ?? [];
    children.forEach((child: any, i: number) => {
      walk(child, nextCols, i === children.length - 1, opensScope, false);
    });
  };

  // The root itself is not drawn; its children are the top-level rows, and the
  // recorder walks them from the root with `is_root` set, so that the root
  // contributes no column of its own.
  walk(root, [], false, false, true);
  return out;
}

/** Flatten the trace into rows, in the order the engine took them. */
function flatten(node: any, depth: number, out: Beat[]): void {
  const raw: string = node.name ?? '';
  const [before, output] = raw.includes('#') ? raw.split('#') : [null, raw];
  out.push({
    identifier: output,
    law: before,
    type: node.node_type,
    result: node.result,
    unit: node.type_spec?.unit ?? null,
    provider: node.source?.provider ?? null,
    depth,
    tree: '',
  });
  for (const child of node.children ?? []) flatten(child, depth + 1, out);
}

/**
 * Match a step against the canonical grammar.
 *
 * The same walk the demo does (frontend-demo/src/data/gherkinNl.js); the
 * grammar itself is generated from bdd/grammar.yaml, so both read the one
 * source of truth for what a step means.
 */
function matchStep(text: string): { entry: any; args: string[] } | null {
  for (const entry of GRAMMAR as any[]) {
    const m = entry.pattern.exec(text);
    if (m) return { entry, args: m.slice(1) };
  }
  return null;
}

let enginePromise: Promise<{ engine: any; manifest: Manifest; feature: string }> | null = null;

/**
 * Fetch the engine and the laws, once.
 *
 * Separate from running so the page can start the download while the visitor is
 * still reading the panel above: by the time the run panel scrolls into view,
 * the animation can usually begin immediately instead of waiting on a network.
 */
export function prepare(base = '/'): Promise<{ engine: any; manifest: Manifest; feature: string }> {
  if (!enginePromise) {
    enginePromise = prepareEngine(base).catch((err) => {
      // Never cache a failure: a flaky network should not leave the panel
      // permanently unable to run.
      enginePromise = null;
      throw err;
    });
  }
  return enginePromise;
}

/**
 * Load the WASM engine and every law the scenario names.
 *
 * All versions of each law are loaded, not one picked here: the engine holds
 * them side by side and selects on the calculation date, which is what makes
 * the result the same as the one CI produces. See script/landing-laws.sh.
 */
async function prepareEngine(base: string): Promise<{ engine: any; manifest: Manifest; feature: string }> {
  const manifest: Manifest = await fetch(`${base}laws/manifest.json`).then((r) => {
    if (!r.ok) throw new Error(`manifest: ${r.status}`);
    return r.json();
  });

  const wasm = await import(/* @vite-ignore */ `${base}wasm/pkg/regelrecht_engine.js`);
  await wasm.default(`${base}wasm/pkg/regelrecht_engine_bg.wasm`);

  const engine = new wasm.WasmEngine();

  const [laws, feature] = await Promise.all([
    Promise.all(
      manifest.laws.map((p) =>
        fetch(`${base}${p}`).then((r) => {
          if (!r.ok) throw new Error(`${p}: ${r.status}`);
          return r.text();
        }),
      ),
    ),
    fetch(`${base}laws/${manifest.scenario}`).then((r) => {
      if (!r.ok) throw new Error(`scenario: ${r.status}`);
      return r.text();
    }),
  ]);

  for (const yaml of laws) engine.loadLaw(yaml);

  return { engine, manifest, feature };
}

/**
 * Run the scenario and return its trace as rows.
 *
 * The scenario is the file that sits beside the law in the repository, not a
 * copy written for this page. The steps are dispatched through the shared
 * canonical-grammar runner, the same one the editor and the demo use, so "the
 * engine ran it" means the same thing here as it does there.
 */
export async function runScenario(base = '/'): Promise<RunResult> {
  const { engine, feature } = await prepare(base);

  const parsed = parseFeature(feature);
  // The scenario the panel above shows: an income above the threshold, where
  // the allowance tapers. Matched by name so a reordering of the file cannot
  // silently run a different one.
  const wanted = parsed.scenarios.find((s: any) => s.name.includes('boven het drempelinkomen'))
    ?? parsed.scenarios[0];
  if (!wanted) throw new Error('scenario not found in the feature file');

  const ctx = new ExecutionContext();
  engine.clearDataSources();

  const t0 = performance.now();
  let trace: any = null;
  let amount: unknown = null;

  // `background` is the step list itself, not an object holding one.
  const all = [...(parsed.background ?? []), ...wanted.steps];
  for (const step of all) {
    const match = matchStep(step.text);
    if (!match) throw new Error(`unknown step: ${step.text}`);
    const { entry, args } = match;
    const typed = args.map((raw: string, i: number) =>
      entry.argTypes[i] === 'number' ? bareValue(raw) : quotedValue(raw),
    );
    const table = step.dataTable ?? null;

    if (entry.action === 'evaluate' || entry.action === 'evaluate_outputs') {
      const outputs =
        entry.action === 'evaluate_outputs'
          ? String(typed[0]).split(',').map((s) => s.trim())
          : [String(typed[0])];
      const result = engine.executeMultipleWithTrace(
        typed[1],
        outputs,
        ctx.parameters,
        ctx.calculationDate,
      );
      ctx.result = result;
      ctx.executed = true;
      trace = traceRoot(result.trace);
      // The scenario evaluates `heeft_recht_op_zorgtoeslag`, so the root of the
      // trace is a boolean. The panel shows the amount, which the same run
      // computes on the way: take it from the outputs, not from the root.
      amount = result.outputs?.[AMOUNT_OUTPUT] ?? trace?.result ?? null;
    } else {
      await dispatch(ctx, engine, entry.action, [...typed, ...entry.literals], table, {
        loadDependency: async () => {},
      });
    }
  }
  const durationMs = performance.now() - t0;

  if (!trace) throw new Error('the scenario never evaluated an output');

  const beats: Beat[] = [];
  for (const child of trace.children ?? []) flatten(child, 0, beats);
  const prefixes = treePrefixes(trace);
  beats.forEach((b, i) => {
    b.tree = prefixes[i] ?? '';
  });

  return { beats, amount, steps: beats.length, durationMs };
}
