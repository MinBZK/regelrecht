/**
 * How a BDD step is shown to the audience.
 *
 * The scenario files stay in the canonical vocabulary from bdd/grammar.yaml so
 * the Rust runner and the browser runner read the same text; this module only
 * decides how a step is *shown*. Every template is keyed by grammar step id, so
 * a new step in the grammar shows up in its canonical form until it gets a
 * line here.
 *
 * That canonical form is English, which is why the English side of this module
 * is empty: showing a step in English means not translating it at all. The
 * Dutch templates are the work; English is what the file already says.
 */
import { matchStep } from '@regelrecht/frontend-shared/gherkin';
import { DEFAULT_LOCALE, currentLocale } from '../i18n/index.js';
import titles from '../i18n/scenarioTitles.generated.js';

// Re-exported, not redefined: matching a step against the canonical grammar is
// the shared runner's job, and this module only decides how a matched step is
// shown. ScenariosView and the tests here keep importing it from this module.
export { matchStep };

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
  assert_null: (a) => `is ${q(a[0])} afwezig`,
  assert_unknown: (a) => `is ${q(a[0])} onbekend`,
  assert_unknown_for: (a) => `is ${q(a[0])} onbekend bij gebrek aan ${q(a[1])}`,
  assert_contains: (a) => `bevat ${q(a[0])} ${q(a[1])}`,
  assert_exact_outputs: (a) => `bevat de uitkomst precies ${q(a[0])}`,
};

/** Grammar step ids that have a Dutch rendering (for the coverage test). */
export const TRANSLATED_STEP_IDS = new Set(Object.keys(TEMPLATES));

function q(s) {
  return `"${s}"`;
}

/**
 * A step as the audience sees it, in the language that is on.
 *
 * In English the step is printed as it stands in the file: that *is* the
 * canonical grammar, so there is nothing to render. `matched` still says
 * whether the step is one the grammar knows, because the view marks an
 * unknown step either way.
 */
export function renderStep(step) {
  const match = matchStep(step.text);
  if (currentLocale() !== DEFAULT_LOCALE) return { keyword: step.keyword, text: step.text, matched: !!match };
  const keyword = KEYWORDS[step.keyword] ?? step.keyword;
  const template = match ? TEMPLATES[match.entry.id] : null;
  return { keyword, text: template ? template(match.args) : step.text, matched: !!match };
}

/** Kept for callers that still import the old name. */
export const renderStepNl = renderStep;

const FEATURE_KEYWORDS = {
  nl: { Feature: 'Functionaliteit', Background: 'Achtergrond', Scenario: 'Scenario' },
  en: { Feature: 'Feature', Background: 'Background', Scenario: 'Scenario' },
};

/** The Gherkin keywords, in the language that is on. */
export function featureKeywords() {
  return FEATURE_KEYWORDS[currentLocale()] ?? FEATURE_KEYWORDS[DEFAULT_LOCALE];
}

export const FEATURE_KEYWORDS_NL = FEATURE_KEYWORDS.nl;
/**
 * De titel van een feature of scenario, in de taal die aanstaat.
 *
 * De `.feature`-bestanden blijven Nederlands: de Rust-runner en de
 * browser-runner lezen dezelfde bestanden, en `just bdd-demo` toetst erop. De
 * vertaling staat er dus naast, gesleuteld op de Nederlandse titel zelf.
 *
 * Een titel die er niet in staat blijft Nederlands. Dat is bij een scenario
 * minder erg dan het klinkt: de stappen eronder zijn canoniek Engels en dragen
 * de inhoud.
 */
export function scenarioTitle(title) {
  if (!title || currentLocale() === DEFAULT_LOCALE) return title ?? '';
  return titles[currentLocale()]?.[title] ?? title;
}
