/**
 * The small pieces the scenario view needs to show a Gherkin step as written,
 * only easier to read.
 *
 * Nothing here computes or converts: an amount stays the number in the file,
 * a date stays its ISO text. What a scenario *means* is the engine's business,
 * and only after "Uitvoeren". This module decides how the text is set.
 */
import { matchStep } from '@regelrecht/frontend-shared/gherkin';
import { t } from '../i18n/index.js';

/**
 * A table cell for display.
 *
 * Returns `{ text, quiet }`. The three kinds of "nothing" in a data table read
 * as words, in a quieter colour, instead of as tokens:
 * - `null` and `[]` state an absence and read "geen";
 * - an empty cell states nothing at all, so the engine sees the field as
 *   unknown (RFC-036), and it reads "niet opgegeven".
 * Everything else is the cell text as it stands in the file.
 */
export function displayCell(raw) {
  const text = String(raw ?? '').trim();
  if (text === '') return { text: t('scenario.value.not_stated'), quiet: true };
  if (text === 'null' || text === '[]') return { text: t('format.none'), quiet: true };
  return { text, quiet: false };
}

/**
 * A step's text with its quoted arguments set in bold rather than in quotes:
 * `is "hoogte_toeslag" gelijk aan 165412` becomes
 * `is **hoogte_toeslag** gelijk aan 165412`, which the design system's text
 * cells render as bold. An empty argument keeps its quotes, since `****`
 * would show nothing.
 */
export function emphasiseArguments(text) {
  return String(text).replace(/"([^"]*)"/g, (whole, inner) => (inner ? `**${inner}**` : whole));
}

/**
 * Whether a step's data table starts with a header row. Every table does
 * except the parameters table (`the following parameters:`), whose rows are
 * name/value pairs from the first line on; the runner reads it that way too.
 */
export function hasHeaderRow(step) {
  return matchStep(step.text)?.entry.id !== 'set_parameters_table';
}

/** Whether a step is an expectation (a Then step in the grammar), which gets a pass/fail mark after a run. */
export function isExpectation(step) {
  return matchStep(step.text)?.entry.keyword === 'then';
}
