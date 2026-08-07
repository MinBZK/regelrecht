/*
 * Meetgegevens achter de CI-doorlooptijdpagina (/operations/ci-doorlooptijd).
 *
 * De cijfers verouderen zodra een workflow verandert. `script/meet-ci.sh` in de
 * repo-root meet ze opnieuw en spuugt exact het `ciWorkflows`-blok hieronder
 * uit; bijwerken is het script draaien en de array vervangen (en `measuredOn`
 * meenemen).
 */

export interface CiJob {
  /** Jobnaam zoals GitHub hem toont. */
  name: string;
  /** Minuten tussen het aanmaken van de run en het starten van deze job. */
  wait: number;
  /** Minuten dat de job zelf draait. */
  work: number;
  /** Staat als verplichte check in de branch protection van `main`. */
  required?: boolean;
  /** Doet zelf niets: wacht via `needs` tot andere jobs klaar zijn. */
  gate?: boolean;
}

export interface CiWorkflow {
  name: string;
  /** Mediane doorlooptijd van de hele workflow, of null als hij niet als één run loopt. */
  total: number | null;
  jobs: CiJob[];
}

/** Meetmoment en -wijze, zichtbaar op de pagina zodat oude cijfers herkenbaar zijn. */
export const ciMeasurement = {
  measuredOn: '7 augustus 2026',
  sampleSize: 60,
  repository: 'MinBZK/regelrecht',
} as const;

/** Bovengrens van de tijdas in minuten; bepaalt de schaal van elke balk. */
export const ciScaleMinutes = 18;

/** Afstand tussen de labels op de tijdas, in minuten. */
export const ciTickMinutes = 3;

export const ciWorkflows: CiWorkflow[] = [
  {
    name: 'CI',
    total: 12.8,
    jobs: [
      { name: 'Detect changes', wait: 2.9, work: 0.1 },
      { name: 'Security Audit', wait: 5.1, work: 0.9, required: true },
      { name: 'Pre-commit', wait: 8.2, work: 1.1, required: true },
      { name: 'Protect schema versions', wait: 8.2, work: 0.4, required: true },
      { name: 'WASM Build', wait: 4.7, work: 6.2, required: true },
      { name: 'Rust tests (unit)', wait: 4.7, work: 6.9 },
      { name: 'Rust tests (db)', wait: 4.7, work: 6.9 },
      { name: 'Frontend tests', wait: 8.2, work: 0.5 },
      { name: 'Test', wait: 15.1, work: 0.1, required: true, gate: true },
      { name: 'E2E (mocked)', wait: 4.7, work: 6.2 },
      { name: 'Provenance checks (RFC-013)', wait: 4.7, work: 6.9 },
      { name: 'Cross-law integrity', wait: 8.2, work: 0.4 },
      { name: 'Docs accessibility gate', wait: 8.2, work: 0.4 },
    ],
  },
  {
    name: 'Build and Deploy',
    total: 12.7,
    jobs: [
      { name: 'changes', wait: 6.0, work: 0.3 },
      { name: 'build', wait: 7.5, work: 4.4 },
      { name: 'build-admin', wait: 7.5, work: 4.4 },
      { name: 'build-docs', wait: 7.5, work: 4.4 },
      { name: 'build-lawmaking', wait: 11.8, work: 4.4 },
      { name: 'build-harvester-worker', wait: 6.4, work: 8.2 },
      { name: 'build-pipeline-api', wait: 6.4, work: 8.2 },
      { name: 'build-enrich-worker', wait: 6.3, work: 9.7 },
      { name: 'deploy-preview', wait: 13.9, work: 3.6, gate: true },
    ],
  },
  {
    name: 'Claude Code Review',
    total: 12.2,
    jobs: [
      { name: 'claude-review', wait: 5.5, work: 5.2 },
      { name: 'Claude review completed', wait: 3.9, work: 6.2, gate: true },
    ],
  },
  {
    name: 'Overig',
    total: null,
    jobs: [
      { name: 'Validate PR title', wait: 2.9, work: 0.1 },
      { name: 'Mutation Testing (diff)', wait: 2.5, work: 1.6 },
    ],
  },
];

/** Eén decimaal met een komma, zoals de rest van de Nederlandstalige pagina. */
export function nlNumber(value: number): string {
  return value.toFixed(1).replace('.', ',');
}
