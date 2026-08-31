/*
 * Roadmap: constants, data loading and the integrity checks behind /roadmap.
 *
 * The pages are a read-only rendering of content that lives in the repo:
 * werkpakketten as a content collection (src/content/roadmap/werkpakketten/),
 * the fase/discipline matrix and the outcome-mapping worksheet as JSON under
 * src/data/. Editing goes through a pull request; there is no write path.
 *
 * The lists below were the app's shared/constants.js and outcome-constants.js,
 * imported by both its server (validation) and its client (rendering). Here
 * they are the single source for the zod enums in content.config.ts and for
 * the labels the pages render, so a value can never render as a tag the schema
 * would have rejected.
 */
import config from '~/data/roadmap-config.json';
import worksheet from '~/data/outcome-mapping.json';

export interface Fase {
  id: string;
  volgnummer: number;
  naam: string;
  ondertitel: string;
}

export interface Discipline {
  id: string;
  naam: string;
  ondertitel: string;
}

/** A werkpakket's frontmatter, mirroring the zod schema in content.config.ts. */
export interface WerkpakketData {
  id: string;
  titel: string;
  faseId: string;
  disciplineId: string;
  prioriteit: string;
  omvang: string;
  categorie: string;
  capability: string;
  capaciteit: string;
  toelichting: string;
  volgorde: number;
  onderzoeksvragen: string[];
  samenhangIds: string[];
}

export const PRIORITEITEN = [
  { id: 'hoog', label: 'Hoog', tagColor: 'critical' },
  { id: 'midden', label: 'Midden', tagColor: 'warning' },
  { id: 'laag', label: 'Laag', tagColor: 'success' },
] as const;

export const OMVANGEN = ['S', 'M', 'L', 'XL'] as const;

export const CATEGORIEEN = [
  { id: 'lat', label: 'Lat' },
  { id: 'pivot', label: 'Pivot' },
  { id: 'bet', label: 'Bet' },
] as const;

export const CAPABILITIES = [
  { id: 'basis', label: 'Basis' },
  { id: 'ontwikkelen', label: 'Ontwikkelen van wet- en regelgeving' },
  { id: 'simuleren', label: 'Simuleren van wet- en regelgeving' },
  { id: 'publiceren', label: 'Publiceren van wet- en regelgeving' },
  { id: 'analyseren', label: 'Analyseren van wet- en regelgeving' },
  { id: 'implementeren', label: 'Implementeren van wet- en regelgeving' },
  { id: 'verifieren', label: 'Verifiëren en simuleren van besluitvorming' },
] as const;

export const PRIORITEIT_IDS = PRIORITEITEN.map((p) => p.id);
export const CATEGORIE_IDS = CATEGORIEEN.map((c) => c.id);
export const CAPABILITY_IDS = CAPABILITIES.map((c) => c.id);

export const getPrioriteit = (id: string) =>
  PRIORITEITEN.find((p) => p.id === id);
export const getCategorie = (id: string) => CATEGORIEEN.find((c) => c.id === id);
export const getCapability = (id: string) =>
  CAPABILITIES.find((c) => c.id === id);

export const fases: Fase[] = [...config.fases].sort(
  (a, b) => a.volgnummer - b.volgnummer,
);
export const disciplines: Discipline[] = config.disciplines;

export const getFase = (id: string) => fases.find((f) => f.id === id);
export const getDiscipline = (id: string) =>
  disciplines.find((d) => d.id === id);

/** Placeholder wording for fields the roadmap has not filled in yet. */
export const NIET_BEPAALD = 'Nog niet bepaald';
export const NIET_GESPECIFICEERD = 'Niet gespecificeerd';

/**
 * The werkpakketten of one matrix cell, in the order the roadmap puts them.
 * `volgorde` was maintained by drag-and-drop in the app; here it is just a
 * number in the frontmatter, so a gap or a duplicate is harmless.
 */
export function werkpakkettenInCel<T extends { data: WerkpakketData }>(
  alle: T[],
  faseId: string,
  disciplineId: string,
): T[] {
  return alle
    .filter(
      (w) => w.data.faseId === faseId && w.data.disciplineId === disciplineId,
    )
    .sort((a, b) => (a.data.volgorde ?? 0) - (b.data.volgorde ?? 0));
}

/**
 * Fail the build on a reference that goes nowhere.
 *
 * check-links.mjs only reads <a href>, and every link the roadmap renders sits
 * on an NLDD component attribute (nldd-card, nldd-list-item), which that gate
 * cannot see. Without this the samenhang links would be the one part of the
 * site where a dangling reference ships unnoticed — and the app relied on its
 * server to keep them consistent, which is exactly what we removed.
 *
 * zod cannot do this: a per-entry schema never sees its sibling entries, nor
 * the fase/discipline JSON.
 */
export function assertReferencesResolve(
  werkpakketten: { data: WerkpakketData }[],
): void {
  const ids = new Set(werkpakketten.map((w) => w.data.id));
  const problems: string[] = [];

  for (const { data } of werkpakketten) {
    const waar = `werkpakket ${data.id} (${data.titel})`;
    if (!getFase(data.faseId)) {
      problems.push(`${waar}: onbekende faseId "${data.faseId}"`);
    }
    if (!getDiscipline(data.disciplineId)) {
      problems.push(`${waar}: onbekende disciplineId "${data.disciplineId}"`);
    }
    for (const samenhangId of data.samenhangIds) {
      if (!ids.has(samenhangId)) {
        problems.push(`${waar}: samenhangId "${samenhangId}" bestaat niet`);
      }
    }
  }

  if (problems.length) {
    throw new Error(
      `Roadmap-verwijzingen kloppen niet:\n  ${problems.join('\n  ')}`,
    );
  }
}

// --- Outcome mapping -------------------------------------------------------

export interface Worksheet {
  vision: string;
  mission: string;
  boundaryPartners: string[];
  outcomeChallenges: string[];
  progressMarkers: {
    expectToSee: string[];
    likeToSee: string[];
    loveToSee: string[];
  }[];
  strategyMaps: { individual: string[]; environment: string[] }[];
  organizationalPractices: { keyActions: string; disabled: boolean }[];
}

export const outcomeMapping = worksheet as Worksheet;

export interface Step {
  id: string;
  label: string;
  section: keyof Worksheet;
  /** Rendered once per boundary partner rather than once for the worksheet. */
  perPartner?: boolean;
}

export const STEPS: Step[] = [
  { id: 'vision', label: 'Vision', section: 'vision' },
  { id: 'mission', label: 'Mission', section: 'mission' },
  { id: 'boundary-partners', label: 'Boundary Partners', section: 'boundaryPartners' },
  { id: 'outcome-challenges', label: 'Outcome Challenges', section: 'outcomeChallenges' },
  { id: 'progress-markers', label: 'Progress Markers', section: 'progressMarkers', perPartner: true },
  { id: 'strategy-maps', label: 'Strategy Maps', section: 'strategyMaps', perPartner: true },
  { id: 'organizational-practices', label: 'Organizational Practices', section: 'organizationalPractices' },
];

export const STRATEGY_COLUMNS = ['Causal', 'Persuasive', 'Supportive'] as const;

export const STRATEGY_ROWS = [
  {
    key: 'individual',
    code: 'I',
    label: 'Strategies and Activities Aimed at a Specific Individual or Group',
  },
  {
    key: 'environment',
    code: 'E',
    label: "Strategies and Activities Aimed at Individual's or Group's Environment",
  },
] as const;

export const PRACTICE_LABELS = [
  {
    title: 'Prospecting for new ideas, opportunities, and resources',
    nl: 'Actief zoeken naar nieuwe ideeën, kansen en middelen buiten de eigen kring',
  },
  {
    title: 'Seeking feedback from key informants',
    nl: 'Systematisch feedback ophalen bij sleutelinformanten, ook kritische stemmen',
  },
  {
    title: 'Obtaining the support of your next highest power',
    nl: 'Steun en legitimiteit verwerven bij het naasthogere niveau (bestuur, moederorganisatie, donor, ministerie)',
  },
  {
    title: 'Assessing and (re)designing products, services, systems, and procedures',
    nl: 'De eigen producten, diensten en werkwijzen periodiek tegen het licht houden en herontwerpen',
  },
  {
    title: 'Checking up on those already served to add value',
    nl: 'Terugkeren naar partners die je eerder hebt ondersteund, om waarde toe te voegen in plaats van door te schuiven naar de volgende',
  },
  {
    title: 'Sharing your best wisdom with the world',
    nl: 'Kennis en ervaring actief naar buiten brengen, niet alleen intern houden',
  },
  {
    title: 'Experimenting to remain innovative',
    nl: 'Ruimte houden voor experiment en risico, ook als de uitkomst onzeker is',
  },
  {
    title: 'Engaging in organizational reflection',
    nl: 'Gestructureerde reflectiemomenten inbouwen over wat werkt en wat niet',
  },
] as const;

/** The partner's own name where the worksheet has one, else a stable label. */
export function partnerLabel(index: number): string {
  return outcomeMapping.boundaryPartners[index] || `Partner ${index + 1}`;
}

/** True when a worksheet field is still empty, so pages can say so uniformly. */
export const isLeeg = (value: string | undefined) => !value || !value.trim();
