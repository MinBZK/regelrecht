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
import { z } from 'astro:content';
import configJson from '~/data/roadmap-config.json';
import worksheetJson from '~/data/outcome-mapping.json';
import paperHeadings from '~/research/rules-as-executed.headings.json';
import { getRfcs } from '~/lib/rfcs';

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
  onderzoeksvragen: (string | { vraag: string; paper: string })[];
  samenhangIds: string[];
  onderzoek: string;
  bouw: string;
  rfcs: number[];
}

export const PRIORITEITEN = [
  { id: 'hoog', label: 'Hoog', tagColor: 'critical' },
  { id: 'midden', label: 'Midden', tagColor: 'warning' },
  { id: 'laag', label: 'Laag', tagColor: 'success' },
] as const;

export const OMVANGEN = ['S', 'M', 'L', 'XL'] as const;

/*
 * Two axes, deliberately separate.
 *
 * `onderzoek` is how far the question is answered; `bouw` is how much of it
 * stands in the codebase. They diverge in both directions, which is why one
 * combined status would be a lie in half the cases.
 *
 * The tag colours follow the same reading as elsewhere on the site: neutral
 * for "not started", warning for "under way", success for "done".
 */
export const ONDERZOEK_STANDEN = [
  { id: 'open', label: 'Open', tagColor: 'neutral' },
  { id: 'loopt', label: 'Loopt', tagColor: 'warning' },
  { id: 'beantwoord', label: 'Beantwoord', tagColor: 'success' },
] as const;

export const BOUW_STANDEN = [
  { id: 'niet', label: 'Niet gebouwd', tagColor: 'neutral' },
  { id: 'deels', label: 'Deels gebouwd', tagColor: 'warning' },
  { id: 'wel', label: 'Gebouwd', tagColor: 'success' },
] as const;

export const getOnderzoek = (id: string) =>
  ONDERZOEK_STANDEN.find((s) => s.id === id);
export const getBouw = (id: string) => BOUW_STANDEN.find((s) => s.id === id);

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

/*
 * The id lists the zod schema in content.config.ts builds its enums from.
 *
 * The tuple type is what z.enum() requires. Deriving the enums here rather
 * than repeating the ids in the schema is what makes the "single source"
 * above true: with two copies, adding a value to the schema alone would let
 * it validate while getPrioriteit() returns undefined, and the card would
 * silently render no tag for a value that was set.
 */
type NonEmpty = [string, ...string[]];

/**
 * The value `data-categorie` carries for a werkpakket without a categorie.
 * Six of the nineteen have none, so the filter needs a way to show them:
 * without one, checking any box hides them with no control to bring them
 * back. Shared by the page and the stylesheet's selectors.
 */
export const GEEN_CATEGORIE = 'geen';

/** The filter's checkboxes: every categorie, plus the ones without one. */
export const FILTER_OPTIES = [
  ...CATEGORIEEN.map((c) => ({ id: c.id, label: c.label })),
  { id: GEEN_CATEGORIE, label: 'Zonder categorie' },
];

/**
 * Fail the build when the filter's stylesheet has no show-rule for an option
 * the page renders.
 *
 * The checkboxes come from FILTER_OPTIES, but the rules that show their cards
 * again live in roadmap.css, which cannot read this list. Add a categorie and
 * its checkbox appears while its cards stay hidden behind the blanket
 * hide-rule, with nothing to bring them back — silent, and only on the
 * filtered view. Reading the stylesheet here keeps the two in step.
 */
export function assertFilterRules(css: string): void {
  const missing = FILTER_OPTIES.filter(
    (optie) => !css.includes(`#rr-cat-${optie.id}[checked]`),
  ).map((optie) => optie.id);

  if (missing.length) {
    throw new Error(
      `roadmap.css mist een toon-regel voor filteroptie(s) ${missing
        .map((id) => `"${id}"`)
        .join(', ')}. Voeg een ` +
        `\`.rr-roadmap:has(#rr-cat-<id>[checked]) .rr-wp-card[data-categorie='<id>']\`-regel toe, ` +
        'anders blijven die kaarten verborgen zodra er gefilterd wordt.',
    );
  }
}

export const ONDERZOEK_IDS = ONDERZOEK_STANDEN.map((s) => s.id) as NonEmpty;
export const BOUW_IDS = BOUW_STANDEN.map((s) => s.id) as NonEmpty;
export const PRIORITEIT_IDS = PRIORITEITEN.map((p) => p.id) as NonEmpty;
export const OMVANG_IDS = [...OMVANGEN] as NonEmpty;
export const CATEGORIE_IDS = CATEGORIEEN.map((c) => c.id) as NonEmpty;
export const CAPABILITY_IDS = CAPABILITIES.map((c) => c.id) as NonEmpty;

export const getPrioriteit = (id: string) =>
  PRIORITEITEN.find((p) => p.id === id);
export const getCategorie = (id: string) => CATEGORIEEN.find((c) => c.id === id);
export const getCapability = (id: string) =>
  CAPABILITIES.find((c) => c.id === id);

/*
 * The two JSON files get the same build-time validation the werkpakketten get
 * from their collection schema. Without it a hand-edit that drops a key fails
 * far from its cause: a missing `individual` on a strategyMap surfaces as
 * "Cannot read properties of undefined" out of a page template, naming
 * neither the file nor the partner. parse() throws during the build instead,
 * with the path to the offending field.
 */
const configSchema = z.object({
  fases: z
    .array(
      z.object({
        id: z.string().min(1),
        volgnummer: z.number(),
        naam: z.string().min(1),
        ondertitel: z.string(),
      }),
    )
    .min(1),
  disciplines: z
    .array(
      z.object({
        id: z.string().min(1),
        naam: z.string().min(1),
        ondertitel: z.string(),
      }),
    )
    .min(1),
});

const config = configSchema.parse(configJson);

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
    .sort((a, b) => a.data.volgorde - b.data.volgorde);
}

/**
 * A research question as the pages render it: the text, plus the paper section
 * it belongs to when there is one.
 */
export interface Onderzoeksvraag {
  vraag: string;
  /** The paper section, resolved from its anchor. Absent when unlinked. */
  paper?: PaperSectie;
}

/** A section of the position paper, addressable by its anchor. */
export interface PaperSectie {
  /** The anchor, e.g. "sec:traceaccess". */
  slug: string;
  /** The section number, e.g. "4.5". */
  nummer: string;
  /** The section title without its number, e.g. "The Recipient's Check". */
  titel: string;
  /** The href a link should use. */
  href: string;
}

export const PAPER_PAD = '/research/rules-as-executed';

/*
 * The paper's sections, keyed by anchor.
 *
 * Read from the headings JSON the research page already ships, so a section
 * number or title can never drift from the paper: both are the paper's own
 * words. The heading text is "4.5 The Recipient's Check", number and title in
 * one string, which is why they are split here.
 */
const paperSecties = new Map<string, PaperSectie>(
  (paperHeadings as { slug: string; text: string }[]).map((h) => {
    const m = /^([\d.]+)\s+(.*)$/.exec(h.text);
    return [
      h.slug,
      {
        slug: h.slug,
        nummer: m ? m[1] : '',
        titel: m ? m[2] : h.text,
        href: `${PAPER_PAD}#${h.slug}`,
      },
    ];
  }),
);

export const getPaperSectie = (slug: string) => paperSecties.get(slug);

/**
 * One shape for the page: both the plain-string and the linked form of a
 * research question come out as an Onderzoeksvraag.
 */
export function onderzoeksvraagLijst(
  vragen: WerkpakketData['onderzoeksvragen'],
): Onderzoeksvraag[] {
  return vragen.map((v) =>
    typeof v === 'string'
      ? { vraag: v }
      : { vraag: v.vraag, paper: getPaperSectie(v.paper) },
  );
}

/** An RFC a werkpakket points at, with the RFC's own implementation state. */
export interface RfcVerwijzing {
  /** Zero-padded id, e.g. "RFC-013". */
  id: string;
  title: string;
  /** "Implemented" | "Partially implemented" | "Not implemented". */
  implementation: string;
  link: string;
}

/**
 * The RFCs a werkpakket points at, resolved against the RFC collection.
 *
 * The implementation state is read from the RFC and never copied into the
 * werkpakket: the RFC is the thing that gets built, so it owns that fact. A
 * copy would be a second truth that nobody updates.
 */
export function rfcVerwijzingen(nummers: number[]): RfcVerwijzing[] {
  const alle = new Map(getRfcs().map((r) => [r.num, r]));
  // Deduped: the same number twice used to render two identical rows, and the
  // build said nothing. Harmless to write by accident, invisible once shipped.
  return [...new Set(nummers)]
    .map((n) => alle.get(n))
    .filter((r): r is NonNullable<typeof r> => Boolean(r))
    .map((r) => ({
      id: r.id,
      title: r.title,
      implementation: r.implementation ?? 'Not implemented',
      link: r.link,
    }));
}

/**
 * Fail the build on a werkpakket pointing at an RFC that does not exist.
 *
 * Same reason as assertPaperSections: check-links.mjs would catch the dead
 * link once it is in the HTML, but it reports the route, not the werkpakket
 * that wrote it.
 */
export function assertRfcReferences(
  werkpakketten: { data: WerkpakketData }[],
): void {
  const bestaande = new Set(getRfcs().map((r) => r.num));
  const problems: string[] = [];

  for (const { data } of werkpakketten) {
    for (const nummer of data.rfcs) {
      if (bestaande.has(nummer)) continue;
      problems.push(
        `werkpakket ${data.id} (${data.titel}): RFC ${nummer} bestaat niet`,
      );
    }
  }

  if (problems.length) {
    throw new Error(
      `Verwijzingen naar RFC's kloppen niet:\n  ${problems.join('\n  ')}`,
    );
  }
}

/**
 * The implemented RFCs no werkpakket points at.
 *
 * Reported, not enforced. An RFC that lands before anyone updates the roadmap
 * is a redactional gap, not a defect, and a gate that blocked the RFC on it
 * would put the roadmap in the way of the work it describes. Printing the
 * list at build time keeps it visible without that cost.
 */
export function ongekoppeldeRfcs(
  werkpakketten: { data: WerkpakketData }[],
): RfcVerwijzing[] {
  const gekoppeld = new Set(werkpakketten.flatMap((w) => w.data.rfcs));
  return getRfcs()
    .filter((r) => r.implementation === 'Implemented' && !gekoppeld.has(r.num))
    .map((r) => ({
      id: r.id,
      title: r.title,
      implementation: r.implementation ?? '',
      link: r.link,
    }));
}

/**
 * Fail the build on a research question pointing at a paper section that does
 * not exist.
 *
 * check-links.mjs does catch a dead anchor once the link is in the HTML, but
 * it reports the route and the anchor, not which werkpakket wrote it — with 53
 * questions that is a search. This names the file and the question instead,
 * and it is what keeps the mapping honest when the paper is revised: drop a
 * section and the build says which werkpakket pointed at it.
 */
export function assertPaperSections(
  werkpakketten: { data: WerkpakketData; id?: string }[],
): void {
  const problems: string[] = [];

  for (const { data } of werkpakketten) {
    for (const vraag of data.onderzoeksvragen) {
      if (typeof vraag === 'string') continue;
      if (paperSecties.has(vraag.paper)) continue;
      problems.push(
        `werkpakket ${data.id} (${data.titel}): onbekende papersectie ` +
          `"${vraag.paper}" bij de vraag "${vraag.vraag.slice(0, 60)}…"`,
      );
    }
  }

  if (problems.length) {
    throw new Error(
      `Verwijzingen naar het position paper kloppen niet:\n  ${problems.join(
        '\n  ',
      )}`,
    );
  }
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
  werkpakketten: { data: WerkpakketData; id?: string }[],
): void {
  const ids = new Set(werkpakketten.map((w) => w.data.id));
  const problems: string[] = [];

  /*
   * Two files carrying the same id would otherwise collapse into one Set
   * entry and pass unnoticed, while getStaticPaths emits the route twice:
   * Astro drops the second with a warning, the build still succeeds, and one
   * werkpakket renders a card that links to another one's page. No gate
   * catches it — check-links.mjs sees a link that resolves.
   *
   * This is the likelier mistake now that there is no write path: a new
   * werkpakket starts as a copy of an existing file, and the filename is the
   * part you remember to change.
   */
  const seen = new Set<string>();
  for (const { data, id: bestandsnaam } of werkpakketten) {
    if (seen.has(data.id)) {
      problems.push(`id "${data.id}" wordt door meer dan één bestand gebruikt`);
    }
    seen.add(data.id);
    // The filename is the werkpakket's id; keeping the two equal is what makes
    // the content directory navigable.
    if (bestandsnaam !== undefined && bestandsnaam !== data.id) {
      problems.push(
        `bestand "${bestandsnaam}.md" bevat id "${data.id}"; die horen gelijk te zijn`,
      );
    }
  }

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

/*
 * The worksheet's arrays are positional: index i of outcomeChallenges,
 * progressMarkers and strategyMaps all describe boundaryPartners[i], and
 * organizationalPractices lines up with PRACTICE_LABELS. The page templates
 * loop over one array and index into another, so a length mismatch renders
 * one partner's data under another's name, or throws out of a template with
 * no mention of the file it came from. Pinning the lengths against the lists
 * that drive the rendering turns that into a build error naming the field.
 */
// Derived from the data, not a literal: the invariant is that these arrays
// agree with each other, not that there are exactly four partners. Pinning the
// number here would mean a fifth boundary partner needs a code change.
//
// Validated on its own first, because the count is needed to build the schema
// that validates everything else. Reading it straight off the JSON would let a
// missing or renamed boundaryPartners throw "Cannot read properties of
// undefined" from this line — the contextless failure the schema below exists
// to replace, for the one field it depends on.
const PARTNER_COUNT = z
  .object({ boundaryPartners: z.array(z.unknown()).min(1) })
  .parse(worksheetJson).boundaryPartners.length;
const strategieRij = z.array(z.string()).length(STRATEGY_COLUMNS.length);

const worksheetSchema = z.object({
  vision: z.string(),
  mission: z.string(),
  boundaryPartners: z.array(z.string()).length(PARTNER_COUNT),
  outcomeChallenges: z.array(z.string()).length(PARTNER_COUNT),
  progressMarkers: z
    .array(
      z.object({
        expectToSee: z.array(z.string()),
        likeToSee: z.array(z.string()),
        loveToSee: z.array(z.string()),
      }),
    )
    .length(PARTNER_COUNT),
  strategyMaps: z
    .array(z.object({ individual: strategieRij, environment: strategieRij }))
    .length(PARTNER_COUNT),
  organizationalPractices: z
    .array(z.object({ keyActions: z.string(), disabled: z.boolean() }))
    .length(PRACTICE_LABELS.length),
});

export const outcomeMapping: Worksheet = worksheetSchema.parse(worksheetJson);

/** The partner's own name where the worksheet has one, else a stable label. */
export function partnerLabel(index: number): string {
  return outcomeMapping.boundaryPartners[index] || `Partner ${index + 1}`;
}

/** True when a worksheet field is still empty, so pages can say so uniformly. */
export const isLeeg = (value: string | undefined) => !value || !value.trim();
