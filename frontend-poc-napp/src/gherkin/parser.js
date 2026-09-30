/**
 * Gherkin parser — wraps @cucumber/gherkin to produce a simplified AST.
 *
 * Returns { feature, scenarios[] } where each scenario has steps[]
 * and optional dataTables.
 */
import * as Gherkin from '@cucumber/gherkin';
import * as Messages from '@cucumber/messages';

/**
 * Parse a .feature text into a structured object.
 *
 * @param {string} featureText - Raw Gherkin feature text
 * @returns {{ feature: string, background: Step[]|null, scenarios: Scenario[] }}
 */
export function parseFeature(featureText) {
  const uuidFn = Messages.IdGenerator.uuid();
  const builder = new Gherkin.AstBuilder(uuidFn);
  const matcher = new Gherkin.GherkinClassicTokenMatcher();
  const parser = new Gherkin.Parser(builder, matcher);

  const gherkinDocument = parser.parse(featureText);
  const feature = gherkinDocument.feature;

  if (!feature) {
    return { feature: '', background: null, scenarios: [] };
  }

  let background = null;
  const scenarios = [];

  for (const child of feature.children) {
    if (child.background) {
      background = child.background.steps.map(mapStep);
    }
    if (child.scenario) {
      scenarios.push({
        name: child.scenario.name,
        tags: child.scenario.tags.map((t) => t.name),
        steps: child.scenario.steps.map(mapStep),
      });
    }
  }

  return {
    feature: feature.name,
    background,
    scenarios,
  };
}

/**
 * Map a Gherkin step AST node to our simplified step format.
 */
function mapStep(step) {
  const result = {
    keyword: step.keyword.trim(),
    text: step.text,
  };

  if (step.dataTable) {
    result.dataTable = step.dataTable.rows.map((row) =>
      row.cells.map((cell) => cell.value),
    );
  }

  if (step.docString) {
    result.docString = step.docString.content;
  }

  return result;
}
