/*
 * What the scenario runner on /concepts/scenarios starts from.
 *
 * Read out of the corpus at build time, like the landing-page demo
 * (~/lib/landing-demo.ts) and for the same reason: a scenario retyped into a
 * documentation page is a claim about the corpus rather than a piece of it, and
 * it goes stale the first time the law moves. What the reader edits in the
 * browser is the file CI runs, minus the scenarios they are not looking at.
 *
 * The runner needs a feature that stands on its own, so this assembles three
 * parts of the file into one: the Feature line, the Background that loads the
 * laws, and a single Scenario.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// The repository is one level above the docs project, in `astro dev`, in
// `astro build` and in the image. Same reasoning as ~/lib/landing-demo.ts.
const repo = join(process.cwd(), '..');

const SCENARIO =
  'corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature';

/**
 * The scenario the runner opens with: an income above the threshold, where the
 * allowance tapers off.
 *
 * The same one the landing page executes. A scenario below the threshold pays
 * out a flat maximum, so editing the income in it changes nothing on screen and
 * the reader learns the opposite of the lesson.
 */
const OPENS_WITH = 'Inkomen boven het drempelinkomen';

const feature = readFileSync(join(repo, SCENARIO), 'utf8');

/** Everything from `Background:` up to the blank line that ends its block. */
function background(text: string): string {
  const start = text.indexOf('  Background:');
  if (start === -1) throw new Error(`${SCENARIO}: no Background block`);
  const out: string[] = [];
  for (const line of text.slice(start).split('\n')) {
    if (out.length > 0 && line.trim() === '') break;
    out.push(line);
  }
  return out.join('\n');
}

/**
 * One scenario, from its `Scenario:` line to the blank line after its last
 * step.
 *
 * Stopping at the blank line rather than at the next `Scenario:`, because the
 * scenario that follows this one is introduced by a comment block: a search for
 * the next keyword runs straight past it and hands the runner two scenarios
 * where the page promises one.
 */
function scenario(text: string, name: string): string {
  const start = text.indexOf(`  Scenario: ${name}`);
  if (start === -1) throw new Error(`${SCENARIO}: no scenario named ${name}`);
  const out: string[] = [];
  for (const line of text.slice(start).split('\n')) {
    if (out.length > 0 && line.trim() === '') break;
    out.push(line);
  }
  return out.join('\n');
}

const featureName = feature.match(/^Feature:\s*(.+)$/m)?.[1]?.trim();
if (!featureName) throw new Error(`${SCENARIO}: no Feature line`);

export const scenarioDemo = {
  /** A complete, runnable feature file: Feature, Background, one Scenario. */
  text: [
    `Feature: ${featureName}`,
    '',
    background(feature),
    '',
    scenario(feature, OPENS_WITH),
    '',
  ].join('\n'),
  /** Where it comes from, for the citation under the runner. */
  path: SCENARIO,
  url: `https://github.com/MinBZK/regelrecht/blob/main/${SCENARIO}`,
};
