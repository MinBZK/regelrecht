/*
 * What the landing-page demo shows, all read from the repository at build time
 * rather than retyped here: the prose a citizen can look up, the YAML that
 * makes it executable, the memorandum that says why it reads that way, and the
 * scenario that checks it. Retyping any of them would make the page an
 * illustration of the claim instead of an instance of it, and it would drift
 * the moment the corpus moved.
 *
 * The fifth thing, the run itself, is not here: it happens in the visitor's
 * browser (see ~/scripts/landing-run.ts). There used to be a recording of it
 * alongside, and that was one execution too many -- two traces of the same law
 * that had to be kept saying the same thing, on a page whose whole argument is
 * that one law should have one execution.
 */
import { read } from './corpus';
// The feature file has an owner of its own, because the runner on
// /concepts/scenarios wants a different cut of the same scenario. See
// ~/lib/scenario-demo.ts.
import { scenarioDemo } from './scenario-demo';

const LAW = 'corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml';
const ANNOTATIONS = 'corpus/annotations/wet_op_de_zorgtoeslag/annotations.yaml';

/**
 * Article 3 lid 1, as the law states it: the capital test.
 *
 * One sentence, one rule, and an amount a reader recognises. Article 2 states
 * the allowance itself, which takes three leden and a page of arithmetic to
 * model; this panel is there to show that a statute and its executable form say
 * the same thing, and that reads better on a rule you can hold in your head.
 *
 * The YAML holds the whole article as one block with the leden separated by
 * blank lines, so lid 1 is the first paragraph. Taking it by position rather
 * than by a regex over legal text keeps the failure loud: if the shape changes,
 * this throws at build time instead of quietly rendering the wrong lid.
 */
function articleThreeLidOne(yaml: string): string {
  const marker = "  - number: '3'\n    text: >-\n";
  const start = yaml.indexOf(marker);
  if (start === -1) throw new Error(`${LAW}: article 3 not found in the expected shape`);

  const body = yaml.slice(start + marker.length);
  const end = body.indexOf('\n    url:');
  if (end === -1) throw new Error(`${LAW}: article 3 has no url line after its text`);

  const text = body
    .slice(0, end)
    .split('\n')
    .map((line) => line.trim())
    .join('\n');

  const lidOne = text.split('\n\n')[0]?.replace(/\n/g, ' ').trim();
  if (!lidOne?.startsWith('1.')) {
    throw new Error(`${LAW}: expected article 3 to open with lid 1, got ${lidOne?.slice(0, 40)}`);
  }
  return lidOne.replace(/^1\.\s*/, '');
}

/**
 * The machine-readable form of that same article, whole, lifted from the law
 * file verbatim. Indentation is normalized so it reads on its own.
 *
 * The whole block rather than just the action that carries the rule: without
 * the declarations around it, `$vermogen` and `$vermogensgrens_verzekerde`
 * arrive out of nowhere. With them, the panel shows the two things that make
 * this an interesting comparison -- the amounts from the statute appear
 * literally (141.896 euro as 14189600 eurocent), and the capital being tested
 * turns out to be fetched from another law entirely.
 */
function computationYaml(yaml: string): string {
  const marker = "  - number: '3'\n";
  const article = yaml.indexOf(marker);
  if (article === -1) throw new Error(`${LAW}: article 3 not found in the expected shape`);
  const start = yaml.indexOf('    machine_readable:\n', article);
  if (start === -1) throw new Error(`${LAW}: article 3 has no machine_readable block`);
  const rest = yaml.slice(start + '    machine_readable:\n'.length);
  const end = rest.indexOf("\n  - number: '4'");
  if (end === -1) throw new Error(`${LAW}: could not find the end of article 3`);

  return (
    rest
      .slice(0, end)
      .split('\n')
      .map((line) => line.slice(6))
      // Comments in the law file are notes to whoever maintains the corpus:
      // why a bound is modelled the way it is, which RFC governs a field, what
      // a past bug was. A visitor reading the panel is being shown the rule,
      // not our working notes, and an English aside about RFC-039 in the middle
      // of a Dutch statute reads as part of the law when it is not.
      .filter((line) => !line.trimStart().startsWith('#'))
      .join('\n')
      .trimEnd()
  );
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

/**
 * Where the quoted passage comes from: the citation as the annotation states
 * it, and the link to the published document.
 *
 * Both are read out of the one annotation that holds the quotation, rather
 * than written out here, so a citation can never end up pointing at a
 * different document than the text beside it. A quotation from the
 * parliamentary papers without a way to check it is the kind of claim this
 * page exists to avoid making.
 */
function memorandumSource(annotations: string): { citation: string; url: string } {
  const marker = 'Verslag houdende een lijst van vragen en antwoorden bij de';
  const quoted = annotations.indexOf(marker);
  if (quoted === -1) throw new Error(`${ANNOTATIONS}: the worked-example annotation is missing`);

  // The `creator:` line sits above the body, the `source:` link below it, both
  // inside the same annotation. Bound the search at the next annotation so a
  // missing field borrows neither from the one before nor the one after.
  const blockStart = annotations.lastIndexOf('\n  - type: Annotation', quoted);
  const after = annotations.indexOf('\n  - type: Annotation', quoted);
  const block = annotations.slice(
    blockStart === -1 ? 0 : blockStart,
    after === -1 ? annotations.length : after,
  );

  const citation = block.match(/^\s*creator:\s*(.+)$/m)?.[1]?.trim();
  if (!citation) throw new Error(`${ANNOTATIONS}: the worked-example annotation has no creator`);

  const url = block.match(/^\s*source:\s*(https:\/\/\S+)$/m)?.[1]?.trim();
  if (!url) throw new Error(`${ANNOTATIONS}: the worked-example annotation has no source link`);

  return { citation, url };
}

const lawYaml = read(LAW);
const annotationsYaml = read(ANNOTATIONS);
const memorandumFrom = memorandumSource(annotationsYaml);

export const demo = {
  prose: articleThreeLidOne(lawYaml),
  yaml: computationYaml(lawYaml),
  memorandum: memorandum(annotationsYaml),
  // Dutch in both languages: a citation is a findable address, not a
  // description. Translating it would name a document that does not exist
  // under that name in the official record.
  memorandumCitation: memorandumFrom.citation,
  memorandumUrl: memorandumFrom.url,
  gherkin: scenarioDemo.scenario,
  scenarioPath: scenarioDemo.path,
  lawPath: LAW,
  lawUrl: 'https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel3',
};
