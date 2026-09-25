// Assert every Gherkin step shown in the docs exists in the canonical grammar.
//
// The step vocabulary is not free text: `bdd/grammar.yaml` is the single
// source of truth and every engine's bindings are generated from it, so a step
// that is not in that file does not run anywhere. The docs nevertheless showed
// invented steps for a long time (`When the law "x" is executed for outputs
// "y"`, `And the output "y" is "500"`) next to a sentence saying the scenario
// came from a real feature file. A reader who copied them wrote a scenario that
// failed on its first line.
//
// ## What is checked
//
// Every fenced ```gherkin block under `src/content/docs/`. RFCs are left out
// on purpose: an RFC may propose wording that the grammar does not have yet,
// and an accepted RFC is not rewritten afterwards.
//
// A doc block is usually a fragment (a few steps, no `Feature:` header), so it
// is not parsed as a feature file. Each step line (`Given`, `When`, `Then`,
// `And`, `But`, `*`) is matched instead against the patterns in
// `packages/frontend-shared/src/gherkin/grammar.generated.js`, which codegen
// writes from the grammar, and its keyword is checked against the keyword the
// grammar gives that step. `And` and `But` take the keyword of the step before
// them, as cucumber does. Table rows, comments, tags, doc strings and the
// structural lines (`Feature:`, `Scenario:`, ...) are skipped.
//
// Known gaps: a `Scenario Outline` step with a `<placeholder>` is skipped
// rather than matched, and a step that needs a data table is not checked for
// having one.
//
// It reads source files only, so it runs without `astro build`. It runs in the
// CI checkout through `npm run check:source`; should the generated grammar be
// missing (a checkout without packages/), it skips rather than fails.
// Non-zero exit fails the gate.

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const DOCS_DIR = fileURLToPath(new URL('../src/content/docs', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));
const GRAMMAR = fileURLToPath(
  new URL(
    '../../packages/frontend-shared/src/gherkin/grammar.generated.js',
    import.meta.url,
  ),
);

if (!existsSync(GRAMMAR)) {
  console.log(
    'check-gherkin-steps: generated grammar not present, skipping (not a full-repo checkout)',
  );
  process.exit(0);
}

const { GRAMMAR: STEPS } = await import(pathToFileURL(GRAMMAR).href);

const STEP_KEYWORDS = {
  Given: 'given',
  When: 'when',
  Then: 'then',
  And: null,
  But: null,
  '*': null,
};
const STRUCTURAL =
  /^(Feature|Rule|Background|Scenario|Scenario Outline|Scenario Template|Example|Examples|Scenarios):/;

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* walk(path);
    else if (/\.mdx?$/.test(name)) yield path;
  }
}

// Yield [lineNumber, lines[]] for each ```gherkin fence in a markdown file.
function* gherkinBlocks(text) {
  const lines = text.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const open = lines[i].match(/^(\s*)(`{3,}|~{3,})\s*(?:gherkin|feature)\b.*$/i);
    if (!open) continue;
    const fence = open[2];
    const body = [];
    const start = i + 1;
    for (i = i + 1; i < lines.length; i++) {
      if (lines[i].trim().startsWith(fence)) break;
      body.push(lines[i]);
    }
    yield [start, body];
  }
}

function checkBlock(file, start, body) {
  const errors = [];
  let previous = null;
  let inDocString = false;
  body.forEach((raw, offset) => {
    const line = raw.trim();
    const where = `${file}:${start + offset + 1}`;
    if (line.startsWith('"""') || line.startsWith('```')) {
      inDocString = !inDocString;
      return;
    }
    if (inDocString || line === '' || line.startsWith('#') || line.startsWith('@')) return;
    if (line.startsWith('|')) return;
    if (STRUCTURAL.test(line)) {
      previous = null;
      return;
    }

    const match = line.match(/^(Given|When|Then|And|But|\*)\s+(.*)$/);
    if (!match) {
      // A line that is neither a step nor structure is feature/scenario
      // description text only when it follows a structural line; inside a
      // block of steps it is a continuation cucumber would reject.
      if (previous !== null) errors.push(`${where}: not a step: "${line}"`);
      return;
    }
    const keyword = STEP_KEYWORDS[match[1]] ?? previous;
    const text = match[2];
    if (/<[^>]+>/.test(text)) {
      previous = keyword ?? previous;
      return;
    }
    const step = STEPS.find((s) => s.pattern.test(text));
    if (!step) {
      errors.push(`${where}: no step in bdd/grammar.yaml matches "${text}"`);
    } else if (keyword && step.keyword !== keyword) {
      errors.push(
        `${where}: "${text}" is a ${step.keyword} step in bdd/grammar.yaml, used here as ${keyword}`,
      );
    }
    previous = keyword ?? step?.keyword ?? previous;
  });
  return errors;
}

const errors = [];
let blocks = 0;
for (const path of walk(DOCS_DIR)) {
  const text = readFileSync(path, 'utf8');
  const file = relative(REPO_ROOT, path);
  for (const [start, body] of gherkinBlocks(text)) {
    blocks++;
    errors.push(...checkBlock(file, start, body));
  }
}

if (errors.length > 0) {
  console.error(`check-gherkin-steps: ${errors.length} step(s) do not match bdd/grammar.yaml:\n`);
  for (const e of errors) console.error(`  ${e}`);
  console.error(
    '\nCopy the step from a real feature file, or add the wording to bdd/grammar.yaml and run `just bdd-codegen`.',
  );
  process.exit(1);
}
console.log(`check-gherkin-steps: ${blocks} gherkin block(s), every step matches bdd/grammar.yaml`);
