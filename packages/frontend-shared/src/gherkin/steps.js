/**
 * Gherkin step executor for the RegelRecht editor engine.
 *
 * Step definitions are derived from the generated canonical grammar
 * (grammar.generated.js) - never hand-listed here. Each definition is
 * { pattern: RegExp, tier: string, execute: async (ctx, engine, match, step) => void }.
 * The actual semantics live in actions.js (the single dispatch file). The editor
 * supports only the `core` tier; non-core steps throw via their action arm.
 */

import { GRAMMAR } from './grammar.generated.js';
import { dispatch, bareValue, quotedValue } from './actions.js';

/** Tiers the editor's WASM engine can execute. */
export const SUPPORTED_TIERS = ['core'];

/**
 * Find the grammar entry a step text belongs to.
 *
 * The one walk over the grammar, shared. Every runner that is handed raw step
 * text has to do this, and it was hand-rolled identically in three places (the
 * demo's Dutch renderer, and both panels on the docs site) before it lived
 * here. A fourth copy is a fourth chance for one of them to match differently
 * from the Rust harness.
 *
 * @param {string} text - the step without its keyword
 * @returns {{entry: object, args: string[]} | null}
 */
export function matchStep(text) {
  for (const entry of GRAMMAR) {
    const m = entry.pattern.exec(text);
    if (m) return { entry, args: m.slice(1) };
  }
  return null;
}

/**
 * Parse a step's captures into typed args.
 *
 * Which capture becomes what is `value_typing` in bdd/grammar.yaml; the Rust
 * dispatcher reads the same three keys. The grammar literals are not appended
 * here, because a caller that only wants to *show* a step wants the captures
 * alone; `buildArgs` adds them for the callers that dispatch.
 *
 * @param {object} entry - a grammar entry
 * @param {string[]} captures - the captures, in order, as matchStep returns them
 */
export function typedArgs(entry, captures) {
  return entry.argTypes.map((t, i) =>
    t === 'number' ? bareValue(captures[i]) : quotedValue(captures[i]),
  );
}

/** Typed captures plus the entry's literals: what `dispatch` takes. */
function buildArgs(entry, match) {
  return [...typedArgs(entry, match.slice(1)), ...entry.literals];
}

/**
 * Create the step definitions registry from the canonical grammar.
 *
 * @param {object} options
 * @param {(lawId: string) => Promise<void>} options.loadDependency - Callback to fetch and load a dependent law
 * @returns {Array<{pattern: RegExp, tier: string, execute: Function}>}
 */
export function createStepDefinitions({ loadDependency }) {
  return GRAMMAR.map((entry) => ({
    pattern: entry.pattern,
    tier: entry.tier,
    execute: async (ctx, engine, match, step) => {
      const args = buildArgs(entry, match);
      const table = step?.dataTable ?? null;
      await dispatch(ctx, engine, entry.action, args, table, { loadDependency });
    },
  }));
}

// parseValue has a single home in actions.js; re-exported for the tests that
// pin which literals it recognises.
export { parseValue } from './actions.js';
