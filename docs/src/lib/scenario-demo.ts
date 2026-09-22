/*
 * The zorgtoeslag feature file, and the blocks cut out of it.
 *
 * Read from the corpus at build time rather than retyped, for the same reason
 * the rest of the landing demo is (~/lib/landing-demo.ts): a scenario retyped
 * into a page is a claim about the corpus rather than a piece of it, and it
 * goes stale the first time the law moves.
 *
 * One module for the file, because two pages want different cuts of it. The
 * landing page shows the scenario alone, under a wipe against the memorandum
 * that works the same sum out. The runner on /concepts/scenarios needs a
 * feature that stands on its own, so it gets the Feature line and the
 * Background that loads the laws around the same scenario. Both used to walk
 * the file themselves, with the same off-by-a-blank-line rule written out
 * twice.
 */
import { read } from './corpus';

const SCENARIO =
  'corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature';

/**
 * The scenario both pages use: an income above the threshold, where the
 * allowance tapers off.
 *
 * The landing page shows this one because it is what the memorandum panel
 * beside it quotes. The runner opens with it for a related reason: a scenario
 * below the threshold pays out a flat maximum, so editing the income in it
 * changes nothing on screen and the reader learns the opposite of the lesson.
 */
const TAPER = 'Inkomen boven het drempelinkomen';

const feature = read(SCENARIO);

/**
 * One block of the feature file, from a line that starts it to the blank line
 * that ends it.
 *
 * Stopping at the blank line rather than at the next keyword, because the
 * scenario that follows the taper one is introduced by a comment: a search for
 * the next `Scenario:` runs straight past it and returns two scenarios where
 * both pages promise one.
 */
function block(opening: string): string {
  const start = feature.indexOf(opening);
  if (start === -1) throw new Error(`${SCENARIO}: no block opening with ${opening.trim()}`);
  const out: string[] = [];
  for (const line of feature.slice(start).split('\n')) {
    if (out.length > 0 && line.trim() === '') break;
    out.push(line);
  }
  return out.join('\n').trimEnd();
}

const featureName = feature.match(/^Feature:\s*(.+)$/m)?.[1]?.trim();
if (!featureName) throw new Error(`${SCENARIO}: no Feature line`);

export const scenarioDemo = {
  /** The scenario on its own, dedented, for the landing page's wipe panel. */
  scenario: block(`  Scenario: ${TAPER}`)
    .split('\n')
    .map((line) => line.slice(2))
    .join('\n'),

  /** A complete, runnable feature file: Feature, Background, one Scenario. */
  text: [
    `Feature: ${featureName}`,
    '',
    block('  Background:'),
    '',
    block(`  Scenario: ${TAPER}`),
    '',
  ].join('\n'),

  /** Where it comes from, for the citation under the runner. */
  path: SCENARIO,
  url: `https://github.com/MinBZK/regelrecht/blob/main/${SCENARIO}`,
};
