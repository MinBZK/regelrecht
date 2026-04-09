/**
 * demoGherkin — minimal Gherkin parser + runner for the demo's feature files.
 *
 * Supports only the step phrasing used by the leenstelsel demo:
 *   Given the calculation date is "<date>"
 *   Given a citizen with the following data:
 *     | key | value |
 *   When the tegemoetkoming <kind> is executed for <law_id> article <article>
 *   Then the output "<name>" is "<value>"
 *
 * Mirrors the 3 hardcoded step mappings in
 * packages/engine/tests/bdd/steps/when.rs:130-152.
 */

import type { TraceResult, WasmEngine } from './wasmEngine';

export interface DemoAssertion {
  output: string;
  expected: string;
}

export interface DemoScenario {
  name: string;
  calculationDate: string;
  parameters: Record<string, unknown>;
  lawId: string;
  outputName: string;
  article: string;
  assertions: DemoAssertion[];
}

export interface AssertionResult extends DemoAssertion {
  actual: unknown;
  passed: boolean;
}

export interface ScenarioRunResult {
  trace: TraceResult;
  assertions: AssertionResult[];
}

// (kind, article) → (law_id, output_name)
const STEP_MAP: Record<string, [string, string]> = {
  'eligibility|12.30': ['wet_studiefinanciering_2000', 'is_rechthebbende'],
  'toekenning|21b': ['besluit_studiefinanciering_2000', 'wijze_van_toekenning'],
  'verstrekking|21c': ['besluit_studiefinanciering_2000', 'wijze_van_verstrekking'],
};

function coerceValue(raw: string): unknown {
  const trimmed = raw.trim();
  if (trimmed === 'true') return true;
  if (trimmed === 'false') return false;
  if (trimmed === 'null') return null;
  if (/^-?\d+$/.test(trimmed)) {
    const n = parseInt(trimmed, 10);
    if (Number.isSafeInteger(n)) return n;
  }
  if (/^-?\d+\.\d+$/.test(trimmed)) {
    const f = parseFloat(trimmed);
    if (Number.isFinite(f)) return f;
  }
  return trimmed;
}

function parseRow(line: string): string[] {
  // Strip leading/trailing pipe, split, trim.
  const body = line.trim().replace(/^\|/, '').replace(/\|$/, '');
  return body.split('|').map((c) => c.trim());
}

/**
 * Parse a .feature file into demo scenarios. Scenarios that don't match the
 * demo grammar are skipped silently (returned with an empty lawId so the UI
 * can mark them unrunnable if needed).
 */
export function parseFeature(text: string): DemoScenario[] {
  const lines = text.split(/\r?\n/);
  const scenarios: DemoScenario[] = [];

  // Background state (calculation date only, for now)
  let backgroundDate = '2025-01-01';

  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    // Background block
    if (/^Background:/i.test(trimmed)) {
      i++;
      while (i < lines.length && !/^\s*(Scenario|Feature|Background)/i.test(lines[i])) {
        const m = lines[i].trim().match(/^Given the calculation date is "([^"]+)"$/);
        if (m) backgroundDate = m[1];
        i++;
      }
      continue;
    }

    // Scenario block
    const scnMatch = trimmed.match(/^Scenario:\s*(.+)$/);
    if (scnMatch) {
      const scenarioName = scnMatch[1];
      const scenario: DemoScenario = {
        name: scenarioName,
        calculationDate: backgroundDate,
        parameters: {},
        lawId: '',
        outputName: '',
        article: '',
        assertions: [],
      };

      i++;
      while (i < lines.length && !/^\s*(Scenario|Feature|Background)/i.test(lines[i])) {
        const step = lines[i].trim();

        // Inline calculation date override
        const dateM = step.match(/^Given the calculation date is "([^"]+)"$/);
        if (dateM) {
          scenario.calculationDate = dateM[1];
          i++;
          continue;
        }

        // Citizen data table
        if (/^Given a citizen with the following data:/i.test(step)) {
          i++;
          while (i < lines.length && lines[i].trim().startsWith('|')) {
            const row = parseRow(lines[i]);
            if (row.length >= 2) {
              scenario.parameters[row[0]] = coerceValue(row[1]);
            }
            i++;
          }
          continue;
        }

        // The 3 demo "When ... is executed for ... article ..." mappings
        const whenM = step.match(
          /^When the tegemoetkoming (eligibility|toekenning|verstrekking) is executed for (\S+) article (\S+)$/,
        );
        if (whenM) {
          const kind = whenM[1];
          const lawId = whenM[2];
          const article = whenM[3];
          const mapping = STEP_MAP[`${kind}|${article}`];
          if (mapping) {
            scenario.lawId = mapping[0];
            scenario.outputName = mapping[1];
            scenario.article = article;
          } else {
            // Fallback: use law_id from the sentence, best-effort output
            scenario.lawId = lawId;
            scenario.outputName = kind;
            scenario.article = article;
          }
          i++;
          continue;
        }

        // Output assertions
        const thenM = step.match(
          /^(?:Then|And) the output "([^"]+)" is "([^"]*)"$/,
        );
        if (thenM) {
          scenario.assertions.push({ output: thenM[1], expected: thenM[2] });
          i++;
          continue;
        }

        // Unknown step — skip it (don't break the scenario)
        i++;
      }

      // Inject test BSN if parameters are empty of a bsn (matches Rust given.rs:41-46)
      if (!('bsn' in scenario.parameters)) {
        scenario.parameters.bsn = '123456789';
      }

      scenarios.push(scenario);
      continue;
    }

    i++;
  }

  return scenarios;
}

/**
 * Run a scenario against the engine with trace. Auto-loads missing laws via
 * loadDependency() when the engine reports "Law not loaded: X". Retries up to
 * 5 times.
 */
export async function runScenario(
  engine: WasmEngine,
  scn: DemoScenario,
  loadDependency: (lawId: string) => Promise<void>,
): Promise<ScenarioRunResult> {
  if (!scn.lawId || !scn.outputName) {
    throw new Error(`Scenario "${scn.name}" is not runnable (no law/output mapping)`);
  }

  // Make sure the target law is loaded before we even try.
  await loadDependency(scn.lawId);

  let attempts = 0;
  while (attempts < 6) {
    try {
      const result = engine.executeWithTrace(
        scn.lawId,
        scn.outputName,
        scn.parameters,
        scn.calculationDate,
      );

      const assertions: AssertionResult[] = scn.assertions.map((a) => {
        const actual = result.outputs?.[a.output];
        const actualStr = actual === null || actual === undefined ? '' : String(actual);
        return { ...a, actual, passed: actualStr === a.expected };
      });

      return { trace: result, assertions };
    } catch (e: unknown) {
      attempts++;
      const msg = extractErrorMessage(e);
      const missing = matchMissingLaw(msg);
      if (missing && attempts < 6) {
        try {
          await loadDependency(missing);
          continue;
        } catch {
          throw new Error(`Failed to auto-load missing law "${missing}": ${msg}`);
        }
      }
      throw new Error(msg);
    }
  }
  throw new Error('Too many missing-law retries');
}

function extractErrorMessage(e: unknown): string {
  if (!e) return 'Unknown error';
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  // wasm-bindgen throws TracedErrorResult objects — try to pull .error
  if (typeof e === 'object' && 'error' in (e as Record<string, unknown>)) {
    return String((e as { error: unknown }).error);
  }
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

function matchMissingLaw(msg: string): string | null {
  const patterns = [
    /Law ['"]?([^'"\s]+)['"]? not found/i,
    /Law not loaded:\s*['"]?([^'"\s]+)['"]?/i,
    /Unknown law\s*['"]?([^'"\s]+)['"]?/i,
  ];
  for (const re of patterns) {
    const m = msg.match(re);
    if (m) return m[1].trim();
  }
  return null;
}
