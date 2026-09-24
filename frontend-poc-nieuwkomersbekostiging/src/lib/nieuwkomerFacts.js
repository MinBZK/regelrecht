/**
 * Domeinfeiten voor de nieuwkomersbekostiging: wet-id's, codelijsten,
 * peildatum-helpers en weergavelabels. Alles wat de views en de simulatie
 * delen en dat niet uit de engine komt.
 *
 * Datums zijn ISO-strings ('2025-04-01'); bedragen eurocent.
 */

export const LAW_PO = 'regeling_bekostiging_wpo_en_wec';
export const LAW_VO = 'regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo';
export const LAW_UITVOERINGSBESLUIT_WVO = 'uitvoeringsbesluit_wvo_2020';
export const LAW_WERKINSTRUCTIE = 'duo_werkinstructie_bestand_nieuwkomers';

export const LAW_ID_BY_SECTOR = { po: LAW_PO, vo: LAW_VO };

/** Standaard simulatiehorizon. */
export const DEFAULT_JAREN = [2025, 2026, 2027, 2028, 2029, 2030];

/**
 * Peildata van de persona-tijdlijn. Het corpus kent versies vanaf 2025-01-01
 * en de engine weigert een rekendatum vóór de eerste versie, dus de tijdlijn
 * start op 1 januari 2025 (de casussen uit de visual zijn een jaar opgeschoven).
 */
export const PERSONA_PEILDATA_VAN = '2025-01-01';
export const PERSONA_PEILDATA_TOT = '2030-10-01';

/**
 * BRP-verblijfstitelcodes (art. 34 lid 10 en 11, Regeling bekostiging WPO en
 * WEC 2026). De engine bepaalt de categorie; deze lijsten dienen alleen voor
 * de tabblad-indeling en de weergave als de werkinstructie-YAML ontbreekt.
 */
export const CODES_ASIELZOEKER = [26, 27, 32, 39, 44, 46, 50, 51, 52, 53, 54, 55, 56];
export const CODES_OVERIGE_VREEMDELING = [
  22, 23, 24, 25, 28, 29, 30, 31, 35, 36, 37, 38, 40, 41, 42, 43, 45,
];
export const CODES_BESTUUR_BEOORDEELT = [21, 33, 34, 98];

/** Peildata (eerste dag van elk kwartaal) voor een reeks jaren. */
export function peildataVoorJaren(jaren) {
  const out = [];
  for (const jaar of jaren) {
    for (const maand of ['01', '04', '07', '10']) out.push(`${jaar}-${maand}-01`);
  }
  return out;
}

/** Alle kwartaalpeildata van `van` tot en met `tot` (beide ISO, op een peildatum). */
export function peildataTussen(van, tot) {
  const jaarVan = Number(van.slice(0, 4));
  const jaarTot = Number(tot.slice(0, 4));
  const jaren = [];
  for (let j = jaarVan; j <= jaarTot; j++) jaren.push(j);
  return peildataVoorJaren(jaren).filter((p) => p >= van && p <= tot);
}

export function jaarVan(peildatum) {
  return Number(String(peildatum).slice(0, 4));
}

export function isEenJanuari(peildatum) {
  return String(peildatum).slice(5) === '01-01';
}

const MAANDEN = [
  'januari', 'februari', 'maart', 'april', 'mei', 'juni',
  'juli', 'augustus', 'september', 'oktober', 'november', 'december',
];

/** '2025-04-01' → '1 april 2025'. */
export function datumLabel(iso) {
  if (!iso) return '—';
  const [j, m, d] = String(iso).split('-').map(Number);
  if (!j || !m || !d) return String(iso);
  return `${d} ${MAANDEN[m - 1]} ${j}`;
}

/** '2025-04-01' → '1 apr 2025' (kort, voor assen en tabelkoppen). */
export function datumKort(iso) {
  if (!iso) return '—';
  const [j, m, d] = String(iso).split('-').map(Number);
  return `${d} ${MAANDEN[m - 1].slice(0, 3)} ${j}`;
}

/** '2025-04-01' → 'Q2 2025'. */
export function kwartaalLabel(iso) {
  const m = Number(String(iso).slice(5, 7));
  return `Q${Math.floor((m - 1) / 3) + 1} ${String(iso).slice(0, 4)}`;
}

/** Dagen optellen bij een ISO-datum (UTC, geen tijdzone-drift). */
export function addDays(iso, days) {
  const [j, m, d] = String(iso).split('-').map(Number);
  const t = Date.UTC(j, m - 1, d) + days * 86400000;
  return new Date(t).toISOString().slice(0, 10);
}

/** Maanden optellen bij een ISO-datum; de dag wordt geklemd op de maandlengte. */
export function addMonths(iso, months) {
  const [j, m, d] = String(iso).split('-').map(Number);
  const total = (j * 12 + (m - 1)) + months;
  const nj = Math.floor(total / 12);
  const nm = (total % 12) + 1;
  const lastDay = new Date(Date.UTC(nj, nm, 0)).getUTCDate();
  const nd = Math.min(d, lastDay);
  return `${nj}-${String(nm).padStart(2, '0')}-${String(nd).padStart(2, '0')}`;
}

/** Hele maanden tussen twee ISO-datums (a ≤ b), afgerond naar beneden. */
export function monthsBetween(a, b) {
  const [aj, am, ad] = String(a).split('-').map(Number);
  const [bj, bm, bd] = String(b).split('-').map(Number);
  let months = (bj - aj) * 12 + (bm - am);
  if (bd < ad) months -= 1;
  return Math.max(0, months);
}

/**
 * Uiterste aanvraagdatum bij DUO: vier weken na de peildatum, acht weken bij
 * 1 juli (zomervakantie). Benadering van de gepubliceerde DUO-deadlines; de
 * werkinstructie-YAML kan een exacte datum leveren.
 */
export function aanvraagDeadline(peildatum) {
  const weken = String(peildatum).slice(5) === '07-01' ? 8 : 4;
  return addDays(peildatum, weken * 7);
}

/** Datum waarop het Bestand Nieuwkomers in Mijn DUO klaarstaat (de 8e na de peildatum). */
export function bestandDatum(peildatum) {
  return `${String(peildatum).slice(0, 8)}08`;
}

// ---- Labels ---------------------------------------------------------------

export const CATEGORIE_LABELS = {
  ASIELZOEKER: 'Asielzoeker',
  OVERIGE_VREEMDELING: 'Overige vreemdeling',
  BESTUUR_BEOORDEELT: 'Bestuur beoordeelt',
  GEEN: 'Geen nieuwkomer',
  EERSTE: 'Eerste categorie',
  TWEEDE: 'Tweede categorie',
};

export function categorieLabel(code) {
  return CATEGORIE_LABELS[code] ?? code ?? '—';
}

/** Rijkskleur-token per categorie voor tags. */
export const CATEGORIE_COLORS = {
  ASIELZOEKER: 'lintblauw',
  OVERIGE_VREEMDELING: 'oranje',
  BESTUUR_BEOORDEELT: 'paars',
  GEEN: 'neutral',
  EERSTE: 'lintblauw',
  TWEEDE: 'mintgroen',
};

export function categorieColor(code) {
  return CATEGORIE_COLORS[code] ?? 'neutral';
}

/** Hex-kleuren voor echarts (buiten de nldd-tokens om). */
export const KLEUR_HEX = {
  regeling_po: '#154273',
  regeling_vo: '#39870c',
  uitvoeringslast_school: '#d9730d',
  uitvoeringslast_duo: '#8b5cf6',
  uitvoeringslast_overig: '#a90061',
  investering: '#ca005d',
  budget: '#9aa5b1',
};

export const SECTOR_LABELS = { po: 'Primair onderwijs', vo: 'Voortgezet onderwijs' };

export function sectorLabel(sector) {
  return SECTOR_LABELS[sector] ?? sector ?? '—';
}

export const PARTIJ_LABELS = { school: 'School', duo: 'DUO', accountant: 'Accountant', ocw: 'OCW' };

export function partijLabel(partij) {
  return PARTIJ_LABELS[partij] ?? partij ?? '—';
}

export const AANLEIDING_LABELS = {
  per_school_peildatum: 'per school per peildatum',
  per_leerling_peildatum: 'per tellende leerling per peildatum',
  per_ambigu_leerling: 'per ambigue leerling (tabblad 2/3)',
  per_aanvraag: 'per aanvraag',
  per_aanvraag_1_januari: 'per aanvraag op 1 januari',
  per_bezwaar: 'per bezwaar',
  per_school_jaar: 'per school per jaar',
  per_leerling_zonder_bsn: 'per leerling zonder BSN',
  per_aanvraag_voorbereidingskosten: 'per aanvraag voorbereidingskosten',
};

export function aanleidingLabel(aanleiding) {
  return AANLEIDING_LABELS[aanleiding] ?? aanleiding ?? '—';
}

/**
 * Tabblad in het Bestand Nieuwkomers: 1 = eenduidige code, 2 = code waarbij
 * het bestuur beslist, 3 = zonder BSN (onderwijsnummer). Fallback op de
 * codelijsten als de werkinstructie geen `tabblad`-output geeft.
 */
export function tabbladVoor(record, categorie) {
  if (record?.heeft_bsn === false) return 3;
  if (categorie === 'BESTUUR_BEOORDEELT') return 2;
  const code = record?.verblijfstitel_code;
  if (code === null || code === undefined) return 3;
  if (CODES_BESTUUR_BEOORDEELT.includes(Number(code))) return 2;
  return 1;
}

export const TABBLAD_LABELS = {
  1: 'Tabblad 1: eenduidig',
  2: 'Tabblad 2: bestuur beslist',
  3: 'Tabblad 3: zonder BSN',
};

/** Is de code een code waarbij het bevoegd gezag beslist (of ontbreekt het BSN)? */
export function isAmbigu(record) {
  if (record?.heeft_bsn === false) return true;
  const code = record?.verblijfstitel_code;
  if (code === null || code === undefined) return true;
  return CODES_BESTUUR_BEOORDEELT.includes(Number(code));
}

/** Leeftijd in hele jaren op een datum. */
export function leeftijdOp(geboortedatum, datum) {
  if (!geboortedatum || !datum) return null;
  return Math.floor(monthsBetween(geboortedatum, datum) / 12);
}

/**
 * Vanaf welke datum een variant geldt: de vroegste versiedatum in de bestanden
 * van de branch (een nieuwe versie 2028-01-01.yaml geldt vanaf 2028; een
 * bewerkte 2026-versie vanaf 2026). null als dat niet uit de paden blijkt.
 */
export function variantVanaf(variant) {
  const datums = (variant?.files ?? [])
    .map((file) => (String(file.base ?? '').match(/(\d{4}-\d{2}-\d{2})\.yaml$/) || [])[1])
    .filter(Boolean)
    .sort();
  return datums[0] ?? null;
}

/** Korte titel per handeling (data/handelingen.yaml); de omschrijving is de toelichting. */
export const HANDELING_TITELS = {
  bestand_nieuwkomers_doorlopen: 'Bestand Nieuwkomers doorlopen',
  categorie_bepalen_ambigu: 'Categorie bepalen (tabblad 2 en 3)',
  aanvraag_indienen: 'Aanvraag indienen',
  aanvraag_beoordelen: 'Aanvraag beoordelen en beschikken',
  at_correctie_1_januari: 'At-correctie op 1 januari',
  bezwaar_indienen: 'Bezwaar indienen',
  bezwaar_afhandelen: 'Bezwaar afhandelen',
  verantwoording_jaarverslag: 'Verantwoording in het jaarverslag',
  rod_uitlezing: 'ROD uitlezen',
  accountantsvalidatie_nieuwkomers: 'Accountantsvalidatie',
  bewijsstuk_onderwijsnummer: 'Bewijsstuk bij onderwijsnummer',
  aanvraag_voorbereidingskosten: 'Aanvraag voorbereidingskosten',
  ambtshalve_po_it: 'IT voor ambtshalve toekennen in het po',
  een_definitie_beleidstraject: 'Beleidstraject één definitie',
};

/** snake_case → 'Snake case', voor labels van definities en handelingen. */
export function leesbaar(naam) {
  const s = String(naam ?? '').replace(/_/g, ' ').trim();
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
}

export function handelingTitel(h) {
  if (!h) return '';
  return h.titel ?? HANDELING_TITELS[h.id] ?? leesbaar(h.id);
}

/** Onderwerp per artikel, als kop in de parametereditor (de YAML kent geen artikeltitels). */
export const ARTIKEL_ONDERWERP = {
  [LAW_PO]: {
    '34': 'eerste opvang: duur en leeftijdsgrens',
    '34, tweede lid': 'drempel per school',
    '34, zevende lid': 'eenmalige toeslag',
    '34, negende lid': 'bedragen per leerling',
    '35': 'tweede jaar asielzoekers',
    '35, tweede lid': 'tweede jaar: bedrag',
    '36': 'speciaal basisonderwijs',
    '37': 'POL/GLO',
    '43': 'kleine scholen',
  },
  [LAW_VO]: {
    '1': 'begrippen',
    '3': 'ambtshalve toekenning',
    '4': 'berekening per school',
    '5': 'telling in ROD',
    '6': 'vaststelling en betaling',
    '9': 'inwerkingtreding en vervaldatum',
  },
};

export function artikelOnderwerp(lawId, article) {
  return ARTIKEL_ONDERWERP[lawId]?.[String(article)] ?? '';
}
