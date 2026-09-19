/**
 * Synthetische populatie van nieuwkomers (po en vo).
 *
 * Genereert N leerlingrecords met exact het persona-schema uit
 * docs/datamodel.md §1, op basis van data/distributions.yaml. Elk record
 * draagt een populatiegewicht (echte leerlingen per record). Scholen worden
 * mee-geschaald: het aantal gesimuleerde scholen is aantal_echte_scholen ×
 * (records_in_de_sector / echte_instroom_per_jaar). Het scholenbestand telt
 * scholen per jaar, dus de noemer is de instroom van één jaar en niet die
 * over de hele horizon.
 *
 * Drie gewichten die uit elkaar gehouden moeten worden:
 * - `gewicht` op het record: echte leerlingen per record.
 * - `school_gewicht`: echte scholen per gesimuleerde school. Niet hetzelfde
 *   als `gewicht`; ze schelen een factor gelijk aan het aantal instroomjaren.
 * - `school_omvang`: echte nieuwkomers per jaar op die school, uit de
 *   staartverdeling `nieuwkomers_per_school`. Hierop slaat de drempel van
 *   vier (art. 34 lid 2); die mag niet uit de getrokken records komen, want
 *   een record telt maar één tot twee jaar mee van een horizon van acht.
 *
 * Interne DUO/OCW-microdata kan later drop-in dezelfde recordvorm aannemen.
 */
import { addDays, addMonths } from '../lib/nieuwkomerFacts.js';

/** Deterministische PRNG (mulberry32) zodat dezelfde seed dezelfde populatie geeft. */
export function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Trek een waarde uit een percentiel-tabel [{p, value}] met lineaire interpolatie. */
export function samplePercentiles(rand, table) {
  const u = rand();
  for (let i = 0; i < table.length; i++) {
    if (u <= table[i].p) {
      if (i === 0) return table[0].value;
      const lo = table[i - 1];
      const hi = table[i];
      const f = (u - lo.p) / (hi.p - lo.p);
      return Math.round(lo.value + f * (hi.value - lo.value));
    }
  }
  return table[table.length - 1].value;
}

/** Als samplePercentiles, maar zonder afronding (voor leeftijden in jaren). */
export function samplePercentilesFloat(rand, table) {
  const u = rand();
  for (let i = 0; i < table.length; i++) {
    if (u <= table[i].p) {
      if (i === 0) return table[0].value;
      const lo = table[i - 1];
      const hi = table[i];
      const f = (u - lo.p) / (hi.p - lo.p);
      return lo.value + f * (hi.value - lo.value);
    }
  }
  return table[table.length - 1].value;
}

/** Kies een categorie uit {naam: kans}; kansen hoeven niet exact tot 1 te sommeren. */
export function sampleCategory(rand, weights) {
  const entries = Object.entries(weights).filter(([, w]) => Number(w) > 0);
  const total = entries.reduce((s, [, w]) => s + Number(w), 0);
  const u = rand() * total;
  let acc = 0;
  for (const [name, w] of entries) {
    acc += Number(w);
    if (u <= acc) return name;
  }
  return entries[entries.length - 1][0];
}

/** Trek een sleutel uit {sleutel: gewicht} en geef hem als getal terug. */
function sampleNumericKey(rand, weights) {
  return Number(sampleCategory(rand, weights));
}

/** Gewichten-object dat per categorie genest kan zijn ({cat: {k: w}}) of plat ({k: w}). */
function pickWeights(block, categorie, code, fallback) {
  if (!block || typeof block !== 'object') return fallback;
  const values = Object.values(block);
  const nested = values.length && values.every((v) => v && typeof v === 'object');
  if (!nested) return block;
  return block[categorieKey(categorie, code, block)] ?? values[0] ?? fallback;
}

/**
 * Categorie-sleutel voor tabellen die per categorie zijn uitgesplitst.
 * Oekraïense ontheemden (code 46) hebben soms een eigen tabel.
 */
function categorieKey(categorie, code, block) {
  if (code === 46 && block && block.oekraine) return 'oekraine';
  if (categorie === 'asielzoeker' || categorie === 'overige_vreemdeling') return categorie;
  return 'overige_vreemdeling';
}

/**
 * Zoek een percentieltabel in een blok dat per sector en/of per categorie
 * genest kan zijn: blok[sector][categorie] → blok[sector] → blok[categorie] → blok.
 */
function pickTable(block, sector, categorie) {
  if (!block) return null;
  if (Array.isArray(block)) return block;
  const bySector = block[sector];
  if (Array.isArray(bySector)) return bySector;
  if (bySector && typeof bySector === 'object') {
    if (Array.isArray(bySector[categorie])) return bySector[categorie];
    const first = Object.values(bySector).find(Array.isArray);
    if (first) return first;
  }
  if (Array.isArray(block[categorie])) return block[categorie];
  const first = Object.values(block).find(Array.isArray);
  return first ?? null;
}

const DEFAULT_LEEFTIJD = {
  po: [{ p: 0.1, value: 3 }, { p: 0.5, value: 6.5 }, { p: 0.9, value: 10.5 }, { p: 1, value: 12 }],
  vo: [{ p: 0.1, value: 12 }, { p: 0.5, value: 14.5 }, { p: 0.9, value: 17 }, { p: 1, value: 18 }],
};
const DEFAULT_WACHTTIJD = [{ p: 0.25, value: 21 }, { p: 0.5, value: 49 }, { p: 0.9, value: 150 }, { p: 1, value: 365 }];
const DEFAULT_AANKOMSTMAAND = Object.fromEntries(Array.from({ length: 12 }, (_, i) => [i + 1, 1 / 12]));
const DEFAULT_NIEUWKOMERS_PER_SCHOOL = [
  { p: 0.5, value: 2 }, { p: 0.8, value: 5 }, { p: 0.95, value: 15 }, { p: 0.99, value: 50 }, { p: 1, value: 120 },
];

/**
 * Instroom per jaar en sector uit het blok `instroom_per_jaar`. Accepteert
 * {jaar: {po, vo}} en {po: {jaar: n}, vo: {jaar: n}}.
 */
function instroomTabel(d) {
  const block = d.instroom_per_jaar ?? {};
  const rows = [];
  const keys = Object.keys(block);
  if (keys.every((k) => /^\d{4}$/.test(k))) {
    for (const jaar of keys) {
      for (const sector of ['po', 'vo']) {
        const n = Number(block[jaar]?.[sector] ?? 0);
        if (n > 0) rows.push({ jaar: Number(jaar), sector, aantal: n });
      }
    }
  } else {
    for (const sector of ['po', 'vo']) {
      for (const [jaar, n] of Object.entries(block[sector] ?? {})) {
        if (Number(n) > 0) rows.push({ jaar: Number(jaar), sector, aantal: Number(n) });
      }
    }
  }
  return rows;
}

/**
 * Scholenbestand voor één sector: elke gesimuleerde school krijgt een
 * "grootte" uit de lange-staartverdeling (nieuwkomers per school per jaar);
 * leerlingen worden daarna evenredig aan die grootte toegewezen. Zo ontstaat
 * dezelfde staart als in de verdeling, met multinomiale ruis.
 *
 * Twee grootheden per school, en die zijn niet hetzelfde:
 * - `omvang`: hoeveel nieuwkomers deze school in het echt per jaar heeft.
 *   Dit is de grootheid waar de drempel van vier (art. 34 lid 2) op slaat,
 *   dus die mag niet uit het aantal getrokken records worden afgeleid.
 * - `gewicht`: hoeveel echte scholen deze gesimuleerde school voorstelt
 *   (echte scholen ÷ gesimuleerde scholen). Voor de schooltellers.
 *
 * Waarom `omvang` apart moet: een record hoort levenslang bij één school,
 * maar telt maar één tot twee jaar mee van een horizon van acht. Het aantal
 * records op een gesimuleerde school (levensduur) is dus een veelvoud van
 * het aantal dat op één peildatum actief is. Wie de drempel op dat actieve
 * aantal toetst, toetst vier tegen ongeveer één en zet vrijwel alle
 * po-bekostiging uit.
 */
function buildSchools(rand, d, sector, nSector, echteLeerlingenPerJaar) {
  const block = d.scholen?.[sector] ?? {};
  const echteScholen = Number(block.aantal ?? (sector === 'po' ? 3000 : 450));
  const tabel = Array.isArray(block.nieuwkomers_per_school)
    ? block.nieuwkomers_per_school
    : DEFAULT_NIEUWKOMERS_PER_SCHOOL;
  // `aantal` telt scholen met nieuwkomers per jaar, dus schalen tegen de
  // instroom van één jaar. Schalen tegen de instroom over alle jaren zou het
  // scholenbestand door het aantal jaren delen; elke school kreeg dan een
  // veelvoud van de leerlingen uit nieuwkomers_per_school.
  const schaal = echteLeerlingenPerJaar > 0 ? nSector / echteLeerlingenPerJaar : 1;
  const aantal = Math.max(1, Math.round(echteScholen * schaal));
  const gewicht = echteScholen / aantal;
  const prefix = sector === 'po' ? 'P' : 'V';
  const schools = [];
  let totalSize = 0;
  for (let i = 0; i < aantal; i++) {
    const omvang = Math.max(1, samplePercentiles(rand, tabel));
    totalSize += omvang;
    schools.push({ school_id: `${prefix}${String(i + 1).padStart(4, '0')}`, omvang, gewicht });
  }
  // Cumulatieve kansen voor snelle toewijzing: een grotere school trekt naar
  // rato meer leerlingen.
  let acc = 0;
  for (const s of schools) {
    acc += s.omvang / totalSize;
    s.cum = acc;
  }
  return schools;
}

function pickSchool(rand, schools) {
  const u = rand();
  let lo = 0;
  let hi = schools.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (schools[mid].cum < u) lo = mid + 1;
    else hi = mid;
  }
  return schools[lo];
}

/**
 * Genereer een populatie.
 *
 * @param {object} distributions - geparste data/distributions.yaml
 * @param {number} n - aantal records
 * @param {number} seed
 * @returns {Array<object>} records met persona-schema + gewicht
 */
const KNOWN_KEYS = new Set([
  'basisjaar', 'jaren', 'peildata', 'steekproef_records_per_jaar', 'stand_1_oktober',
  'instroom_per_jaar', 'categorie_verdeling', 'verblijfstitel_codes', 'leeftijd_bij_vestiging',
  'sector_regel', 'gat_na_vierde_verjaardag_dagen', 'wachttijd_tot_inschrijving_dagen',
  'aankomstmaand', 'scholen', 'schoolsoort_verdeling_po', 'internationaal_georienteerd_fractie',
  'in_telling_1_februari_prior', 'oordeel_bevoegd_gezag_asielzoeker_fractie', 'oordeel_ontbreekt_fractie',
  'gedrag', 'werkelijk_schoolgaand_fractie', 'bedragen_index_per_jaar', 'uitgaven_realisatie', 'bronnen',
]);
let unknownKeysWarned = false;

export function generatePopulation(distributions, n, seed = 42) {
  const rand = mulberry32(seed);
  const d = distributions ?? {};
  if (!unknownKeysWarned) {
    const unknown = Object.keys(d).filter((k) => !KNOWN_KEYS.has(k));
    if (unknown.length) console.warn(`distributions.yaml: onbekende blokken genegeerd: ${unknown.join(', ')}`);
    unknownKeysWarned = true;
  }
  const instroom = instroomTabel(d);
  if (!instroom.length) throw new Error('distributions.yaml: instroom_per_jaar ontbreekt of is leeg');

  const totaal = instroom.reduce((s, r) => s + r.aantal, 0);
  const gewicht = totaal / n;
  const instroomWeights = Object.fromEntries(instroom.map((r, i) => [i, r.aantal]));

  // Verwacht aantal records per sector, voor de schaal van het scholenbestand.
  const echtPerSector = { po: 0, vo: 0 };
  const jarenPerSector = { po: new Set(), vo: new Set() };
  for (const r of instroom) {
    echtPerSector[r.sector] += r.aantal;
    jarenPerSector[r.sector].add(r.jaar);
  }
  // Gemiddelde instroom per jaar: het scholenbestand is een jaarbestand.
  const echtPerJaar = {
    po: echtPerSector.po / Math.max(1, jarenPerSector.po.size),
    vo: echtPerSector.vo / Math.max(1, jarenPerSector.vo.size),
  };
  const nPerSector = {
    po: Math.round((n * echtPerSector.po) / totaal),
    vo: Math.round((n * echtPerSector.vo) / totaal),
  };
  const scholen = {
    po: buildSchools(rand, d, 'po', nPerSector.po, echtPerJaar.po),
    vo: buildSchools(rand, d, 'vo', nPerSector.vo, echtPerJaar.vo),
  };

  const categorieVerdeling = d.categorie_verdeling ?? {};
  const codes = d.verblijfstitel_codes ?? {};
  const priorTelling = Number(d.in_telling_1_februari_prior ?? 0.3);
  const fractieOordeelAsiel = Number(d.oordeel_bevoegd_gezag_asielzoeker_fractie ?? 0.5);
  const fractieOordeelOntbreekt = Number(d.oordeel_ontbreekt_fractie ?? d.gedrag?.oordeel_ontbreekt_fractie ?? 0);
  const fractieSchoolgaand = Number(d.werkelijk_schoolgaand_fractie ?? 1);
  const fractieInternationaal = d.internationaal_georienteerd_fractie ?? {};
  const gatNaVierde = Array.isArray(d.gat_na_vierde_verjaardag_dagen) ? d.gat_na_vierde_verjaardag_dagen : null;
  const schoolsoortPo = d.schoolsoort_verdeling_po ?? d.scholen?.po?.schoolsoort_verdeling ?? { basisschool: 1 };

  const records = [];
  for (let i = 0; i < n; i++) {
    const row = instroom[Number(sampleCategory(rand, instroomWeights))];
    const { jaar, sector } = row;

    const verdeling = categorieVerdeling[sector]
      ?? categorieVerdeling
      ?? { asielzoeker: 0.5, overige_vreemdeling: 0.4, ambigu_code: 0.06, zonder_bsn: 0.04 };
    const categorie = sampleCategory(rand, verdeling);

    // Verblijfstitelcode en BSN volgen uit de categorie.
    let verblijfstitelCode = null;
    let heeftBsn = true;
    let oordeel = null;
    const trekOordeel = () => {
      if (rand() < fractieOordeelOntbreekt) return null;
      return rand() < fractieOordeelAsiel ? 'ASIELZOEKER' : 'OVERIGE_VREEMDELING';
    };
    if (categorie === 'zonder_bsn') {
      heeftBsn = false;
      verblijfstitelCode = null;
      oordeel = trekOordeel();
    } else if (categorie === 'ambigu_code') {
      verblijfstitelCode = codes.ambigu_code ? sampleNumericKey(rand, codes.ambigu_code) : 21;
      oordeel = trekOordeel();
    } else if (categorie === 'asielzoeker') {
      verblijfstitelCode = codes.asielzoeker ? sampleNumericKey(rand, codes.asielzoeker) : 26;
    } else {
      verblijfstitelCode = codes.overige_vreemdeling ? sampleNumericKey(rand, codes.overige_vreemdeling) : 22;
    }

    // Aankomst: maand uit de verdeling (per categorie of plat), dag uniform; vestiging = aankomst.
    const aankomstmaand = pickWeights(d.aankomstmaand, categorie, verblijfstitelCode, DEFAULT_AANKOMSTMAAND);
    const maand = sampleNumericKey(rand, aankomstmaand);
    const dag = 1 + Math.floor(rand() * 28);
    const datumVestiging = `${jaar}-${String(maand).padStart(2, '0')}-${String(dag).padStart(2, '0')}`;

    // Leeftijd bij vestiging → geboortedatum (met uniforme dagruis binnen het jaar).
    const leeftijdTabel = pickTable(d.leeftijd_bij_vestiging, sector, categorieKey(categorie, verblijfstitelCode))
      ?? DEFAULT_LEEFTIJD[sector];
    const leeftijd = samplePercentilesFloat(rand, leeftijdTabel);
    const geboortedatum = addDays(addMonths(datumVestiging, -Math.round(leeftijd * 12)), -Math.floor(rand() * 30));

    // Wachttijd tot de eerste schooldag; wie jonger dan vier aankomt, start pas
    // rond de vierde verjaardag (plus een gat), en dat deel gaat van het recht af.
    const wachtTabel = Array.isArray(d.wachttijd_tot_inschrijving_dagen)
      ? d.wachttijd_tot_inschrijving_dagen
      : (pickTable(d.wachttijd_tot_inschrijving_dagen, sector, categorieKey(categorie, verblijfstitelCode, d.wachttijd_tot_inschrijving_dagen)) ?? DEFAULT_WACHTTIJD);
    const wacht = samplePercentiles(rand, wachtTabel);
    let eersteInschrijfdatum = addDays(datumVestiging, wacht);
    const vierdeVerjaardag = addMonths(geboortedatum, 48);
    if (eersteInschrijfdatum < vierdeVerjaardag) {
      const gat = gatNaVierde ? samplePercentiles(rand, gatNaVierde) : 0;
      eersteInschrijfdatum = addDays(vierdeVerjaardag, gat);
    }

    const school = pickSchool(rand, scholen[sector]);
    const schoolsoort = sector === 'vo' ? 'vo' : sampleCategory(rand, schoolsoortPo);

    // At (art. 34 lid 9): alleen relevant voor asielzoekers in het po die al in
    // de reguliere 1-februari-telling zaten.
    const asielzoekerAchtig = categorie === 'asielzoeker' || oordeel === 'ASIELZOEKER';
    const inTelling = sector === 'po' && asielzoekerAchtig && rand() < priorTelling;

    records.push({
      bsn: heeftBsn ? String(300000000 + i) : `ON${String(900000 + i)}`,
      naam: '',
      omschrijving: '',
      geboortedatum,
      is_vreemdeling: true,
      verblijfstitel_code: verblijfstitelCode,
      heeft_bsn: heeftBsn,
      datum_vestiging_nederland: datumVestiging,
      eerste_inschrijfdatum: eersteInschrijfdatum,
      woonachtig_in_nederland: true,
      werkelijk_schoolgaand: rand() < fractieSchoolgaand,
      sector,
      schoolsoort,
      internationaal_georienteerd: rand() < Number(fractieInternationaal[sector] ?? 0),
      school_id: school.school_id,
      // Echte nieuwkomers per jaar op deze school (drempel art. 34 lid 2) en
      // het aantal echte scholen dat dit record vertegenwoordigt.
      school_omvang: school.omvang,
      school_gewicht: school.gewicht,
      in_telling_1_februari: inTelling,
      oordeel_bevoegd_gezag: oordeel,
      gewicht,
    });
  }

  return records;
}
