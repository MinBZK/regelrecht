/*
 * The five things the landing-page demo shows, all read from the repository at
 * build time rather than retyped here.
 *
 * The point of the demo is that one law is the same law in five
 * representations: the prose a citizen can look up, the YAML that makes it
 * executable, the memorandum that says why it reads that way, the scenario that
 * checks it, and the trace of that scenario actually running. Retyping any of
 * them would make the page an illustration of the claim instead of an instance
 * of it, and it would drift the moment the corpus moved.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// Two roots, because the sources sit in two places. Deriving either from
// `import.meta.url` does not survive the build: this module is bundled, so at
// build time it no longer sits where its source does. `process.cwd()` is the
// docs project in `astro dev`, in `astro build` and in the image.
const project = process.cwd();
const repo = join(project, '..');

/** A file inside the docs project. */
const readLocal = (...parts: string[]) =>
  readFileSync(join(project, ...parts), 'utf8');

/** A file elsewhere in the repository, chiefly the corpus. */
const read = (...parts: string[]) => readFileSync(join(repo, ...parts), 'utf8');

const LAW = 'corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml';
const SCENARIO =
  'corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/scenarios/eligibility.feature';
const ANNOTATIONS = 'corpus/annotations/wet_op_de_zorgtoeslag/annotations.yaml';

/**
 * The recorded run, written by `just record-landing-trace`.
 *
 * Read from disk rather than imported: a JSON import resolves differently
 * between dev and build here, and reading the file keeps one code path for all
 * five sources.
 */
export const trace = JSON.parse(readLocal('src', 'data', 'landing-trace.json')) as {
  root: Record<string, any>;
  recording: { total_steps: number; total_duration_us: number };
};

/**
 * Article 2 lid 1, as the law states it.
 *
 * The YAML holds the whole article as one block with the leden separated by
 * blank lines, so lid 1 is the first paragraph. Taking it by position rather
 * than by a regex over legal text keeps the failure loud: if the shape changes,
 * this throws at build time instead of quietly rendering the wrong lid.
 */
function articleTwoLidOne(yaml: string): string {
  const marker = "  - number: '2'\n    text: >-\n";
  const start = yaml.indexOf(marker);
  if (start === -1) throw new Error(`${LAW}: article 2 not found in the expected shape`);

  const body = yaml.slice(start + marker.length);
  const end = body.indexOf('\n    url:');
  if (end === -1) throw new Error(`${LAW}: article 2 has no url line after its text`);

  const text = body
    .slice(0, end)
    .split('\n')
    .map((line) => line.trim())
    .join('\n');

  const lidOne = text.split('\n\n')[0]?.replace(/\n/g, ' ').trim();
  if (!lidOne?.startsWith('1.')) {
    throw new Error(`${LAW}: expected article 2 to open with lid 1, got ${lidOne?.slice(0, 40)}`);
  }
  return lidOne.replace(/^1\.\s*/, '');
}

/**
 * The machine-readable action that computes the amount, lifted from the law
 * file verbatim, including its `legal_basis`. Indentation is normalized so it
 * reads on its own.
 */
function computationYaml(yaml: string): string {
  const start = yaml.indexOf('          - output: hoogte_zorgtoeslag\n');
  if (start === -1) throw new Error(`${LAW}: the hoogte_zorgtoeslag action is not where expected`);
  const rest = yaml.slice(start);
  const end = rest.indexOf('\n          - output: heeft_recht_op_zorgtoeslag');
  if (end === -1) throw new Error(`${LAW}: could not find the end of the hoogte_zorgtoeslag action`);

  return rest
    .slice(0, end)
    .split('\n')
    .map((line) => line.slice(10))
    .join('\n')
    .trimEnd();
}

/**
 * The scenario that CI runs, from `Scenario:` to the end of its Then block.
 *
 * The taper one, because it is the scenario the memorandum panel above it
 * quotes: an income above the threshold, where the allowance runs down. A
 * scenario below the threshold produces a flat amount and shows nothing of the
 * rule the legislature works out.
 */
function scenario(feature: string): string {
  const start = feature.indexOf('  Scenario: Inkomen boven het drempelinkomen');
  if (start === -1) throw new Error(`${SCENARIO}: the demo scenario is missing`);
  // Stop at the blank line that ends this scenario's block. Looking for the
  // next `Scenario:` is not enough: the one after this is introduced by a
  // comment, so the search ran past it and the panel showed two scenarios
  // where the text promises one.
  const lines = feature.slice(start).split('\n');
  const body: string[] = [];
  for (const line of lines) {
    if (body.length > 0 && line.trim() === '') break;
    body.push(line);
  }
  return body
    .map((line) => line.slice(2))
    .join('\n')
    .trimEnd();
}

/**
 * The passage from the parliamentary papers that the scenario rests on.
 *
 * Anchored on the sentence that opens this specific annotation, not on a
 * generic "memorie van toelichting" phrase: the file holds several quotations
 * and the generic one matched whichever came first, which was the definition
 * of the allowance rather than the worked example the scenario executes.
 */
function memorandum(annotations: string): string {
  const marker = 'Verslag houdende een lijst van vragen en antwoorden bij de';
  const start = annotations.indexOf(marker);
  if (start === -1) throw new Error(`${ANNOTATIONS}: the worked-example annotation is missing`);
  const rest = annotations.slice(start);
  const end = rest.indexOf('\n        purpose:');
  if (end === -1) throw new Error(`${ANNOTATIONS}: the annotation body has no end`);
  return rest
    .slice(0, end)
    .split('\n')
    .map((line) => line.trim())
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim();
}

const lawYaml = read(LAW);

export const demo = {
  trace,
  prose: articleTwoLidOne(lawYaml),
  yaml: computationYaml(lawYaml),
  memorandum: memorandum(read(ANNOTATIONS)),
  gherkin: scenario(read(SCENARIO)),
  scenarioPath: SCENARIO,
  lawPath: LAW,
  lawUrl: 'https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2',
  steps: trace.recording.total_steps as number,
  durationMs: (trace.recording.total_duration_us as number) / 1000,
};
