/**
 * ScenarioRunner — executes parsed Gherkin scenarios against the WasmEngine.
 */
import { ExecutionContext } from './context.js';
import { createStepDefinitions } from './steps.js';

/**
 * @typedef {'passed'|'failed'|'undefined'|'skipped'} StepStatus
 * @typedef {{ text: string, keyword: string, status: StepStatus, error?: string }} StepResult
 * @typedef {{ name: string, status: StepStatus, steps: StepResult[] }} ScenarioResult
 */

/**
 * Run all scenarios from a parsed feature against the engine.
 *
 * @param {object} parsed - Output of parseFeature()
 * @param {object} engine - WasmEngine instance
 * @param {object} options
 * @param {(lawId: string) => Promise<void>} options.loadDependency - Fetch and load a dependent law
 * @returns {Promise<ScenarioResult[]>}
 */
export async function runFeature(parsed, engine, { loadDependency }) {
  const stepDefs = createStepDefinitions({ loadDependency });
  const results = [];

  for (const scenario of parsed.scenarios) {
    const result = await runScenario(
      scenario,
      parsed.background,
      engine,
      stepDefs,
    );
    results.push(result);
  }

  return results;
}

/**
 * Run a single scenario (including background steps).
 */
async function runScenario(scenario, background, engine, stepDefs) {
  const ctx = new ExecutionContext();
  const stepResults = [];
  let failed = false;

  // Execute background steps first
  const allSteps = [...(background || []), ...scenario.steps];

  for (const step of allSteps) {
    if (failed) {
      stepResults.push({
        text: `${step.keyword} ${step.text}`,
        keyword: step.keyword,
        status: 'skipped',
      });
      continue;
    }

    const result = await executeStep(step, ctx, engine, stepDefs);
    stepResults.push(result);

    if (result.status === 'failed' || result.status === 'undefined') {
      failed = true;
    }
  }

  // Clean up data sources after each scenario
  engine.clearDataSources();

  const scenarioStatus = stepResults.every((s) => s.status === 'passed')
    ? 'passed'
    : 'failed';

  return {
    name: scenario.name,
    status: scenarioStatus,
    steps: stepResults,
  };
}

/**
 * Execute a single step by matching it against step definitions.
 */
async function executeStep(step, ctx, engine, stepDefs) {
  const fullText = step.text;

  for (const def of stepDefs) {
    const match = fullText.match(def.pattern);
    if (match) {
      try {
        await def.execute(ctx, engine, match, step);
        return {
          text: `${step.keyword} ${fullText}`,
          keyword: step.keyword,
          status: 'passed',
        };
      } catch (e) {
        return {
          text: `${step.keyword} ${fullText}`,
          keyword: step.keyword,
          status: 'failed',
          error: e.message || String(e),
        };
      }
    }
  }

  return {
    text: `${step.keyword} ${fullText}`,
    keyword: step.keyword,
    status: 'undefined',
    error: `No matching step definition for: ${fullText}`,
  };
}
