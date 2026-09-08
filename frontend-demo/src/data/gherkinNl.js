/**
 * Dutch rendering of the canonical (English) BDD grammar, for the audience.
 *
 * The scenario files stay in the canonical vocabulary from bdd/grammar.yaml so
 * the Rust runner and the browser runner read the same text; this module only
 * decides how a step is *shown*. Every template is keyed by grammar step id, so
 * a new step in the grammar shows up in English until it gets a line here.
 */
import { GRAMMAR } from '@regelrecht/frontend-shared/gherkin';

const KEYWORDS = {
  Given: 'Gegeven',
  When: 'Als',
  Then: 'Dan',
  And: 'En',
  But: 'Maar',
  '*': '*',
};

const TEMPLATES = {
  set_calculation_date: (a) => `de peildatum is ${q(a[0])}`,
  load_law: (a) => `de wet ${q(a[0])} is geladen`,
  set_parameter_string: (a) => `parameter ${q(a[0])} is ${q(a[1])}`,
  set_parameter_number: (a) => `parameter ${q(a[0])} is ${a[1]}`,
  set_parameters_table: () => 'de volgende parameters:',
  set_data_source: (a) => `de volgende gegevens uit ${q(a[0])}, gesleuteld op ${q(a[1])}:`,
  set_data_source_for_law: (a) => `de volgende gegevens van ${q(a[0])} voor ${q(a[2])}, gesleuteld op ${q(a[1])}:`,
  set_parameter_collection: (a) => `parameter ${q(a[0])} is de verzameling:`,
  evaluate: (a) => `${q(a[1])} wordt uitgevoerd voor ${q(a[0])}`,
  evaluate_outputs: (a) => `${q(a[1])} wordt uitgevoerd voor ${q(a[0])}`,
  assert_succeeds: () => 'slaagt de uitvoering',
  assert_fails: () => 'mislukt de uitvoering',
  assert_fails_with: (a) => `mislukt de uitvoering met ${q(a[0])}`,
  assert_boolean_true: (a) => `is ${q(a[0])} waar`,
  assert_boolean_false: (a) => `is ${q(a[0])} onwaar`,
  assert_equals_number: (a) => `is ${q(a[0])} gelijk aan ${a[1]}`,
  assert_equals_string: (a) => `is ${q(a[0])} gelijk aan ${q(a[1])}`,
  assert_null: (a) => `is ${q(a[0])} leeg`,
  assert_contains: (a) => `bevat ${q(a[0])} ${q(a[1])}`,
  assert_exact_outputs: (a) => `bevat de uitkomst precies ${q(a[0])}`,
};

/** Grammar step ids that have a Dutch rendering (for the coverage test). */
export const TRANSLATED_STEP_IDS = new Set(Object.keys(TEMPLATES));

function q(s) {
  return `"${s}"`;
}

/**
 * Match a step text against the grammar.
 * @returns {{entry: object, args: string[]} | null}
 */
export function matchStep(text) {
  for (const entry of GRAMMAR) {
    const m = entry.pattern.exec(text);
    if (m) return { entry, args: m.slice(1) };
  }
  return null;
}

/** Dutch keyword + Dutch phrasing of a step; falls back to the English text. */
export function renderStepNl(step) {
  const keyword = KEYWORDS[step.keyword] ?? step.keyword;
  const match = matchStep(step.text);
  const template = match ? TEMPLATES[match.entry.id] : null;
  return { keyword, text: template ? template(match.args) : step.text, matched: !!match };
}

export const FEATURE_KEYWORDS_NL = { Feature: 'Functionaliteit', Background: 'Achtergrond', Scenario: 'Scenario' };
