/*
 * Meetgegevens achter de CI-doorlooptijdpagina (/operations/ci-doorlooptijd).
 *
 * Dit is één echte uitvoering, geen gemiddelde. Medianen per job komen uit
 * verschillende runs, en zodra je die naast elkaar zet met `needs`-pijlen
 * ertussen ontstaat een schema dat nergens zo gelopen heeft.
 *
 * `script/measure-ci.sh` in de repo-root haalt een nieuwe run op en spuugt exact
 * de `ciWorkflows`-array hieronder uit, inclusief de `needs`-graaf waar de
 * pijlen uit getekend worden.
 */

export interface CiJob {
  /** Sleutel binnen de workflow; de `edges` verwijzen hiernaar. */
  id: string;
  /** Jobnaam zoals GitHub hem toont. */
  name: string;
  /** Minuten na het aanmaken van de run waarop de job startte. */
  start: number;
  /** Minuten na het aanmaken van de run waarop de job klaar was. */
  end: number;
  /** Staat als verplichte check in de branch protection van `main`. */
  required?: boolean;
  /** Doet zelf niets: leest alleen de uitkomst van de jobs waar hij op wacht. */
  gate?: boolean;
}

export interface CiEdge {
  /** Job-id van de voorganger. */
  from: string;
  /** Job-ids die via `needs` op `from` wachten. */
  to: string[];
  /**
   * Fan-out: één voorganger met veel opvolgers wordt als verticale spine met
   * korte pijlen getekend in plaats van als bundel losse knieën.
   */
  spine?: boolean;
}

export interface CiWorkflow {
  name: string;
  /** Korte duiding naast de workflownaam, redactioneel. */
  note?: string;
  jobs: CiJob[];
  edges: CiEdge[];
}

/** Welke uitvoering hier staat. Zichtbaar op de pagina, zodat de plaat naspeelbaar is. */
export const ciRun = {
  commit: 'afa1408c',
  pullRequest: 1179,
  startedAt: '7 augustus 2026 om 16:14 UTC',
  repository: 'MinBZK/regelrecht',
} as const;

/** Bovengrens van de tijdas in minuten; bepaalt de schaal van elke balk. */
export const ciScaleMinutes = 18.4;

/** Afstand tussen de labels op de tijdas, in minuten. */
export const ciTickMinutes = 3;

/** Rijhoogte in pixels; de SVG-laag met de pijlen rekent hierin. */
export const ciRowHeight = 26;

/** Ondergrens voor een werkbalk, zodat een job van seconden zichtbaar blijft. */
export const ciMinimumBar = 0.12;

export const ciWorkflows: CiWorkflow[] = [
  {
    name: 'CI',
    note: 'klaar op 5,8',
    jobs: [
      { id: 'changes', name: 'Detect changes', start: 0.05, end: 0.18 },
      { id: 'audit', name: 'Security Audit', start: 0.15, end: 0.88, required: true },
      { id: 'a11y', name: 'Docs accessibility gate', start: 0.27, end: 0.43 },
      { id: 'schema', name: 'Protect schema versions', start: 0.47, end: 0.62, required: true },
      { id: 'cross', name: 'Cross-law integrity', start: 0.47, end: 0.68 },
      { id: 'rsu', name: 'Rust tests (unit)', start: 0.48, end: 2.97 },
      { id: 'fe', name: 'Frontend tests', start: 0.65, end: 1.15 },
      { id: 'prov', name: 'Provenance checks (RFC-013)', start: 0.72, end: 0.88 },
      { id: 'e2e', name: 'E2E (mocked)', start: 0.73, end: 3.32 },
      { id: 'wasm', name: 'WASM Build', start: 0.93, end: 1.45, required: true },
      { id: 'pre', name: 'Pre-commit', start: 1.1, end: 2.22, required: true },
      { id: 'rsd', name: 'Rust tests (db)', start: 1.12, end: 5.63 },
      { id: 'test', name: 'Test', start: 5.68, end: 5.75, required: true, gate: true },
    ],
    edges: [
      {
        from: 'changes',
        to: ['a11y', 'schema', 'cross', 'rsu', 'fe', 'prov', 'e2e', 'wasm', 'pre', 'rsd'],
        spine: true,
      },
      { from: 'rsu', to: ['test'] },
      { from: 'fe', to: ['test'] },
      { from: 'rsd', to: ['test'] },
    ],
  },
  {
    name: 'Build and Deploy',
    note: 'klaar op 17,6 · niets hiervan is verplicht',
    jobs: [
      { id: 'dch', name: 'changes', start: 0.05, end: 0.42 },
      { id: 'b5', name: 'build-pipeline-api', start: 0.92, end: 10.23 },
      { id: 'b3', name: 'build-enrich-worker', start: 0.93, end: 11.05 },
      { id: 'b1', name: 'build', start: 1.18, end: 13.7 },
      { id: 'b4', name: 'build-harvester-worker', start: 1.72, end: 11.67 },
      { id: 'b2', name: 'build-admin', start: 2.25, end: 12.32 },
      { id: 'dp', name: 'deploy-preview', start: 13.9, end: 17.55, gate: true },
    ],
    edges: [
      { from: 'dch', to: ['b5', 'b3', 'b1', 'b4', 'b2'], spine: true },
      { from: 'b5', to: ['dp'] },
      { from: 'b3', to: ['dp'] },
      { from: 'b1', to: ['dp'] },
      { from: 'b4', to: ['dp'] },
      { from: 'b2', to: ['dp'] },
    ],
  },
  {
    name: 'Claude Code Review',
    note: 'klaar op 5,4',
    jobs: [
      { id: 'crc', name: 'Claude review completed', start: 0.13, end: 5.43, gate: true },
      { id: 'cr', name: 'claude-review', start: 0.32, end: 5.37 },
    ],
    edges: [],
  },
  {
    name: 'Overig',
    jobs: [
      { id: 'mut', name: 'Mutation Testing (diff)', start: 0.05, end: 0.8 },
      { id: 'ttl', name: 'Validate PR title', start: 0.17, end: 0.23 },
    ],
    edges: [],
  },
];

/** Eén decimaal met een komma, zoals de rest van de Nederlandstalige pagina. */
export function nlNumber(value: number): string {
  return value.toFixed(1).replace('.', ',');
}
