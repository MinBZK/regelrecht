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
import { dispatch } from './actions.js';

/** Tiers the editor's WASM engine can execute. */
export const SUPPORTED_TIERS = ['core'];

/** Parse a regex match's captures into typed args, then append the grammar literals. */
function buildArgs(entry, match) {
  const args = entry.argTypes.map((t, i) => {
    const raw = match[i + 1];
    return t === 'number' ? Number(raw) : raw;
  });
  return [...args, ...entry.literals];
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

// Keep the public re-export ScenarioForm.vue relies on (single home in actions.js).
export { parseValue } from './actions.js';
