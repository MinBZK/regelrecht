/**
 * Simulatie van de nieuwkomersbekostiging over een reeks peildata.
 *
 * Twee passen per run:
 *  1. per leerling: elke peildatum door de engine (po: regeling bekostiging
 *     WPO/WEC art. 34-35; vo: regeling aanvullende bekostiging eerste opvang
 *     nieuwkomers vo). Levert categorie, telt-of-niet, bekostigingsjaar en
 *     het bedrag per leerling.
 *  2. per school en peildatum: uit de leerlinguitkomsten het schoolrecord
 *     (Ap, Vp, At, tweedejaars, nieuwkomers vo, eerste keer) opbouwen, als
 *     databron `scholen` registreren en de schoolniveau-outputs opvragen
 *     (drempel, bedrag met de 1-januari-nuance, toeslag, voorbereidingskosten).
 *
 * Het geld komt van schoolniveau (daar bijt de drempel en art. 34 lid 9);
 * de per-leerling-uitkomsten zijn voor de casus- en schoolschermen. De
 * handelingen-tellingen en de euro's voor uitvoeringslast worden NIET hier
 * maar in metrics.js berekend, zodat minuten/tarieven zonder hersimulatie
 * bijgesteld kunnen worden.
 *
 * Alle bedragen integer eurocent. Datums ISO.
 */
import {
  LAW_ID_BY_SECTOR,
  DEFAULT_JAREN,
  peildataVoorJaren,
  jaarVan,
  isAmbigu,
  addMonths,
} from '../lib/nieuwkomerFacts.js';

export { LAW_PO, LAW_VO } from '../lib/nieuwkomerFacts.js';

export const PO_LEERLING_OUTPUTS = [
  'categorie_nieuwkomer',
  'telt_op_peildatum',
  'bekostigingsjaar',
  'bedrag_kwartaal',
  'aanvraag_vereist',
  'aftrek_maanden_voor_vierde_verjaardag',
  'aftrek_kwartalen',
  'bekostigbare_kwartalen',
  'kwartalen_verstreken',
];
export const VO_LEERLING_OUTPUTS = [
  'is_nieuwkomer',
  'categorie_nieuwkomer_vo',
  'bedrag_kwartaal',
  'aanvraag_vereist',
];
export const PO_SCHOOL_OUTPUTS = [
  'voldoet_aan_drempel',
  'bedrag_eerste_opvang_peildatum',
  'bedrag_tweede_jaar_peildatum',
  'toeslag_eerste_keer',
];
export const VO_SCHOOL_OUTPUTS = ['voorbereidingskosten'];

/** Bestand Nieuwkomers: ROD toont leerlingen tot twee jaar na de eerste inschrijving. */
const BESTAND_VENSTER_MAANDEN = 24;

/** Drempel-fallback (art. 34 lid 2) als het corpus nog geen schoolniveau-outputs heeft. */
const FALLBACK_DREMPEL = 4;

// ---- Engine-aanroepen met uitval-tolerantie ------------------------------

/**
 * Voer een set outputs uit. Bij een fout op de volledige set probeert de
 * eerste aanroep elke output apart en onthoudt per wet welke beschikbaar
 * zijn (het corpus kan nog in aanbouw zijn). Daarna alleen die subset.
 */
function runOutputs(engine, lawId, wanted, params, date, cache) {
  const known = cache.get(lawId);
  const names = known ?? wanted;
  try {
    return engine.executeMultiple(lawId, names, params, date).outputs ?? {};
  } catch (e) {
    if (known) throw e;
    // Eerste keer: probeer per output.
    const available = [];
    const outputs = {};
    let lastError = e;
    for (const name of wanted) {
      try {
        const res = engine.execute(lawId, name, params, date);
        outputs[name] = res.outputs?.[name];
        available.push(name);
      } catch (err) {
        lastError = err;
      }
    }
    if (!available.length) throw lastError;
    cache.set(lawId, available);
    return outputs;
  }
}

function toInt(v) {
  const n = Number(v);
  return Number.isFinite(n) ? Math.round(n) : 0;
}

function inBestand(record, peildatum) {
  const start = record.eerste_inschrijfdatum;
  if (!start || start > peildatum) return false;
  return peildatum < addMonths(start, BESTAND_VENSTER_MAANDEN);
}

/** Normaliseer de engine-outputs van één leerling op één peildatum. */
function normalizeLeerling(sector, outputs, record, peildatum) {
  const base = {
    peildatum,
    jaar: jaarVan(peildatum),
    in_bestand: inBestand(record, peildatum),
  };
  if (sector === 'vo') {
    const categorie = outputs.categorie_nieuwkomer_vo ?? 'GEEN';
    const isNieuwkomer = outputs.is_nieuwkomer;
    const telt = (categorie === 'EERSTE' || categorie === 'TWEEDE') && isNieuwkomer !== false;
    return {
      ...base,
      is_nieuwkomer: isNieuwkomer ?? null,
      categorie,
      categorie_effectief: categorie,
      telt,
      bekostigingsjaar: categorie === 'EERSTE' ? 1 : categorie === 'TWEEDE' ? 2 : 0,
      bedrag: telt ? toInt(outputs.bedrag_kwartaal) : 0,
      aanvraag_vereist: outputs.aanvraag_vereist ?? false,
      aftrek_maanden: null,
      aftrek_kwartalen: null,
      bekostigbare_kwartalen: null,
      kwartalen_verstreken: null,
    };
  }
  const categorie = outputs.categorie_nieuwkomer ?? 'GEEN';
  const telt = !!outputs.telt_op_peildatum;
  // Bij BESTUUR_BEOORDEELT beslist het bevoegd gezag (art. 34 lid 11); de
  // simulatie volgt het oordeel uit het record voor de Ap/Vp-verdeling.
  const effectief = categorie === 'BESTUUR_BEOORDEELT'
    ? (record.oordeel_bevoegd_gezag ?? 'BESTUUR_BEOORDEELT')
    : categorie;
  return {
    ...base,
    is_nieuwkomer: categorie !== 'GEEN',
    categorie,
    categorie_effectief: effectief,
    telt,
    bekostigingsjaar: telt ? toInt(outputs.bekostigingsjaar) : 0,
    bedrag: telt ? toInt(outputs.bedrag_kwartaal) : 0,
    aanvraag_vereist: outputs.aanvraag_vereist ?? true,
    aftrek_maanden: outputs.aftrek_maanden_voor_vierde_verjaardag ?? null,
    aftrek_kwartalen: outputs.aftrek_kwartalen ?? null,
    bekostigbare_kwartalen: outputs.bekostigbare_kwartalen ?? null,
    kwartalen_verstreken: outputs.kwartalen_verstreken ?? null,
  };
}

function emptyLeerlingResult(record, peildatum, sector) {
  return normalizeLeerling(sector, {}, record, peildatum);
}

export function sectorVan(record) {
  return record?.sector === 'vo' ? 'vo' : 'po';
}

/**
 * Evalueer één leerling op één peildatum. De databron `personas` moet al
 * geregistreerd zijn (met dit record).
 */
export function evaluateLeerlingOpPeildatum(engine, record, peildatum, cache = new Map()) {
  const sector = sectorVan(record);
  const lawId = LAW_ID_BY_SECTOR[sector];
  const wanted = sector === 'vo' ? VO_LEERLING_OUTPUTS : PO_LEERLING_OUTPUTS;
  const outputs = runOutputs(engine, lawId, wanted, { bsn: record.bsn }, peildatum, cache);
  return normalizeLeerling(sector, outputs, record, peildatum);
}

/**
 * Tijdlijn van één leerling over een reeks peildata (voor de casus-view).
 * Registreert het record zelf en ruimt de databron daarna op.
 */
export function evaluateLeerlingTimeline(engine, record, peildata, cache = new Map()) {
  engine.registerDataSource('personas', 'bsn', [record]);
  const out = [];
  try {
    for (const pd of peildata) {
      try {
        out.push(evaluateLeerlingOpPeildatum(engine, record, pd, cache));
      } catch (e) {
        out.push({ ...emptyLeerlingResult(record, pd, sectorVan(record)), fout: String(e?.message ?? e) });
      }
    }
  } finally {
    engine.clearDataSources();
  }
  return out;
}

// ---- Schoolniveau ---------------------------------------------------------

/**
 * Bouw het schoolrecord (datamodel §2) voor één peildatum uit de
 * leerlinguitkomsten van die school. `resultaten` = [{record, uitkomst}].
 */
export function buildSchoolRecord(school, resultaten, peildatum, eersteKeer) {
  const rec = {
    school_id: school.school_id,
    schoolsoort: school.schoolsoort,
    aantal_asielzoekers_peildatum: 0,
    aantal_overige_vreemdelingen_peildatum: 0,
    aantal_asielzoekers_telling_1_februari: 0,
    aantal_tweedejaars_asielzoekers_peildatum: 0,
    aantal_nieuwkomers_peildatum: 0,
    eerste_keer_eerste_opvang: !!eersteKeer,
  };
  const facts = {
    in_bestand: 0,
    tellend: 0,
    onbeslist: 0,
    ambigu: 0,
    zonder_bsn: 0,
    eerste_cat_vo: 0,
    tweede_cat_vo: 0,
    bedrag_leerlingen: { ASIELZOEKER: 0, OVERIGE_VREEMDELING: 0, TWEEDEJAARS: 0, EERSTE: 0, TWEEDE: 0, totaal: 0 },
    aanvraag_vereist: false,
    leerlingen_tellend: [],
    // Bedrag van tellende po-leerlingen waarvoor het bestuur zelf de categorie
    // bepaalde (ambigue code of onderwijsnummer): ook onder ambtshalve
    // toekenning blijft dit deel op eigen opgave.
    bedrag_op_oordeel: 0,
  };
  for (const { record, uitkomst } of resultaten) {
    if (uitkomst.in_bestand) {
      facts.in_bestand++;
      if (isAmbigu(record)) facts.ambigu++;
      if (record.heeft_bsn === false) facts.zonder_bsn++;
    }
    if (!uitkomst.telt) continue;
    facts.tellend++;
    facts.leerlingen_tellend.push(record.bsn);
    facts.bedrag_leerlingen.totaal += uitkomst.bedrag;
    if (uitkomst.aanvraag_vereist) facts.aanvraag_vereist = true;
    if (school.sector === 'vo') {
      rec.aantal_nieuwkomers_peildatum++;
      if (uitkomst.categorie === 'EERSTE') {
        facts.eerste_cat_vo++;
        facts.bedrag_leerlingen.EERSTE += uitkomst.bedrag;
      } else if (uitkomst.categorie === 'TWEEDE') {
        facts.tweede_cat_vo++;
        facts.bedrag_leerlingen.TWEEDE += uitkomst.bedrag;
      }
      continue;
    }
    if (isAmbigu(record) || record.heeft_bsn === false) facts.bedrag_op_oordeel += uitkomst.bedrag;
    if (uitkomst.bekostigingsjaar === 2) {
      rec.aantal_tweedejaars_asielzoekers_peildatum++;
      facts.bedrag_leerlingen.TWEEDEJAARS += uitkomst.bedrag;
      continue;
    }
    if (uitkomst.categorie_effectief === 'ASIELZOEKER') {
      rec.aantal_asielzoekers_peildatum++;
      facts.bedrag_leerlingen.ASIELZOEKER += uitkomst.bedrag;
      if (record.in_telling_1_februari) rec.aantal_asielzoekers_telling_1_februari++;
    } else if (uitkomst.categorie_effectief === 'OVERIGE_VREEMDELING') {
      rec.aantal_overige_vreemdelingen_peildatum++;
      facts.bedrag_leerlingen.OVERIGE_VREEMDELING += uitkomst.bedrag;
    } else {
      facts.onbeslist++;
    }
  }

  // De tellingen hierboven zijn ruwe records; de wet toetst de drempel op één
  // échte school. Daarom gaan de tellingen naar de schaal van één school
  // (begrensd op de jaaromvang uit nieuwkomers_per_school): de engine past
  // dan zijn eigen drempelwaarde toe, ook als een variant die verandert.
  // De bedragen gaan mee naar dezelfde schaal en worden in metrics.js met het
  // schoolgewicht weer opgeschaald naar alle échte scholen.
  // De tellingen worden begrensd op de jaaromvang van de school, want daar
  // gaat de drempeltoets over. De bedragen krijgen de volle factor mee: die
  // vertegenwoordigen alle leerlingen van dit record, ongeacht hoeveel er op
  // één school passen. In metrics.js gaan ze daarna maal het schoolgewicht.
  schaalNaarEchteSchool(rec, school);
  schaalBedragen(facts, schaalFactor(school));

  return { record: rec, facts };
}

/** Bedragen van recordschaal naar de schaal van één echte school. */
export function schaalBedragen(facts, factor) {
  if (!Number.isFinite(factor) || factor === 1) return facts;
  for (const key of Object.keys(facts.bedrag_leerlingen)) {
    facts.bedrag_leerlingen[key] = Math.round(facts.bedrag_leerlingen[key] * factor);
  }
  facts.bedrag_op_oordeel = Math.round(facts.bedrag_op_oordeel * factor);
  return facts;
}

/**
 * records → leerlingen per echte school.
 *
 * De ruwe verhouding leerlinggewicht/schoolgewicht schaalt de bedragen goed,
 * maar niet de tellingen waar de drempel op slaat. Een record hoort levenslang
 * bij één school en telt maar één tot twee jaar mee, dus op één peildatum is
 * er ongeveer één record actief; dat blind maal acht doen zet élke school
 * boven de drempel. `school_omvang` (nieuwkomers per jaar op de échte school,
 * uit de staartverdeling) is de bovengrens die dat voorkomt: meer nieuwkomers
 * dan de school er in een jaar heeft, kunnen er op een peildatum niet zijn.
 */
function schaalFactor(school) {
  const leerlinggewicht = Number(school?.leerlinggewicht ?? 1);
  const schoolgewicht = Number(school?.gewicht ?? 1);
  if (!(leerlinggewicht > 0) || !(schoolgewicht > 0)) return 1;
  const factor = leerlinggewicht / schoolgewicht;
  return Number.isFinite(factor) ? factor : 1;
}

/** Aantal actieve records op deze peildatum, uit de tellingen. */
function actieveRecords(rec) {
  return rec.aantal_asielzoekers_peildatum
    + rec.aantal_overige_vreemdelingen_peildatum
    + rec.aantal_tweedejaars_asielzoekers_peildatum;
}

/** De tellingen van recordschaal naar de schaal van één echte school. */
const TELVELDEN = [
  'aantal_asielzoekers_peildatum',
  'aantal_overige_vreemdelingen_peildatum',
  'aantal_asielzoekers_telling_1_februari',
  'aantal_tweedejaars_asielzoekers_peildatum',
  'aantal_nieuwkomers_peildatum',
];

export function schaalNaarEchteSchool(rec, school) {
  const ruw = schaalFactor(school);
  if (ruw === 1) return 1;
  // Eén échte school heeft er `omvang` per jaar; meer kunnen er op een
  // peildatum niet meetellen. Zonder die grens zet de schaling elke school
  // boven de drempel, want er is meestal maar één record actief.
  const actief = actieveRecords(rec);
  const omvang = Number(school?.omvang);
  const factor = actief > 0 && Number.isFinite(omvang) && omvang > 0
    ? Math.min(ruw, Math.max(1, omvang) / actief)
    : ruw;
  for (const veld of TELVELDEN) {
    if (rec[veld] > 0) rec[veld] = Math.max(1, Math.round(rec[veld] * factor));
  }
  return factor;
}

/**
 * Schoolniveau-outputs voor één school op één peildatum. De databron
 * `scholen` moet al geregistreerd zijn. Bij ontbrekende outputs (corpus in
 * aanbouw) valt de berekening terug op de leerlingbedragen met een lokale
 * drempeltoets; `fallback: true` markeert dat.
 */
export function evaluateSchoolOpPeildatum(engine, school, schoolRecord, facts, peildatum, cache = new Map()) {
  const sector = school.sector;
  const lawId = LAW_ID_BY_SECTOR[sector];
  const wanted = sector === 'vo' ? VO_SCHOOL_OUTPUTS : PO_SCHOOL_OUTPUTS;
  let outputs = {};
  let fallback = false;
  let fout = null;
  try {
    outputs = runOutputs(engine, lawId, wanted, { school_id: school.school_id }, peildatum, cache);
  } catch (e) {
    fallback = true;
    fout = String(e?.message ?? e);
  }

  const eerstejaars = schoolRecord.aantal_asielzoekers_peildatum + schoolRecord.aantal_overige_vreemdelingen_peildatum;
  let out;
  if (sector === 'vo') {
    out = {
      voldoet_aan_drempel: true,
      bedrag_eerste_opvang: 0,
      bedrag_tweede_jaar: 0,
      toeslag_eerste_keer: 0,
      voorbereidingskosten: fallback ? 0 : toInt(outputs.voorbereidingskosten),
      bedrag_leerlingen_vo: facts.bedrag_leerlingen.EERSTE + facts.bedrag_leerlingen.TWEEDE,
    };
    out.bedrag_totaal = out.bedrag_leerlingen_vo + out.voorbereidingskosten;
    out.per_categorie = {
      EERSTE: facts.bedrag_leerlingen.EERSTE,
      TWEEDE: facts.bedrag_leerlingen.TWEEDE,
    };
  } else {
    // De drempelwaarde komt uit de wet, niet uit deze code: variant nk-4 zet
    // hem op 1, en dat moet doorwerken. Het schoolrecord staat inmiddels op de
    // schaal van één échte school, dus de engine toetst het juiste getal.
    const drempelOk = fallback
      ? eerstejaars >= FALLBACK_DREMPEL
      : outputs.voldoet_aan_drempel !== undefined
        ? !!outputs.voldoet_aan_drempel
        : eerstejaars >= FALLBACK_DREMPEL;
    // De engine rekent met de begrensde tellingen (die horen bij de
    // drempeltoets), dus zijn bedragen staan op die kleinere schaal. De
    // leerlingbedragen in `facts` dragen wél alle leerlingen van dit record;
    // die zijn hier de maat, met de engine-uitkomst als verhouding.
    const eersteOpvang = drempelOk
      ? facts.bedrag_leerlingen.ASIELZOEKER + facts.bedrag_leerlingen.OVERIGE_VREEMDELING
      : 0;
    const tweedeJaar = facts.bedrag_leerlingen.TWEEDEJAARS;
    const toeslag = fallback ? 0 : toInt(outputs.toeslag_eerste_keer);
    // Verdeel het schoolbedrag eerste opvang over de categorieën naar rato van
    // de leerlingbedragen (of de aantallen als die nul zijn).
    let wA = facts.bedrag_leerlingen.ASIELZOEKER;
    let wV = facts.bedrag_leerlingen.OVERIGE_VREEMDELING;
    if (wA + wV === 0) {
      wA = schoolRecord.aantal_asielzoekers_peildatum;
      wV = schoolRecord.aantal_overige_vreemdelingen_peildatum;
    }
    const som = wA + wV;
    out = {
      voldoet_aan_drempel: drempelOk,
      bedrag_eerste_opvang: eersteOpvang,
      bedrag_tweede_jaar: tweedeJaar,
      toeslag_eerste_keer: toeslag,
      voorbereidingskosten: 0,
      bedrag_leerlingen_vo: 0,
      per_categorie: {
        ASIELZOEKER: som ? Math.round((eersteOpvang * wA) / som) : 0,
        OVERIGE_VREEMDELING: som ? Math.round((eersteOpvang * wV) / som) : 0,
      },
    };
    out.bedrag_totaal = eersteOpvang + tweedeJaar + toeslag;
  }
  out.fallback = fallback;
  if (fout) out.fout = fout;
  out.aanvraag_vereist = facts.aanvraag_vereist;
  // Rechtmatigheid: op aanvraag is het hele schoolbedrag op eigen opgave;
  // ambtshalve blijft alleen het deel op bestuursoordeel over (geschaald naar
  // het schoolbedrag, dat door drempel en formule kan afwijken van de som).
  const leerlingSom = facts.bedrag_leerlingen?.totaal || 0;
  const geldLeerlingen = (out.bedrag_eerste_opvang ?? 0) + (out.bedrag_tweede_jaar ?? 0) + (out.bedrag_leerlingen_vo ?? 0);
  const schaal = leerlingSom > 0 ? geldLeerlingen / leerlingSom : 0;
  out.bedrag_op_oordeel = sector === 'po' ? Math.round((facts.bedrag_op_oordeel ?? 0) * schaal) : 0;
  out.bedrag_op_aanvraag = sector === 'po' && facts.aanvraag_vereist ? out.bedrag_totaal : 0;
  // Een aanvraag (po, ist) volgt als er iets te vragen valt: eerstejaars boven
  // de drempel, of tweedejaars asielzoekers (art. 35, zonder drempel).
  out.aanvraag = sector === 'po'
    && facts.aanvraag_vereist
    && ((out.voldoet_aan_drempel && eerstejaars > 0) || schoolRecord.aantal_tweedejaars_asielzoekers_peildatum > 0);
  out.aanvraag_voorbereidingskosten = sector === 'vo' && out.voorbereidingskosten > 0;
  return out;
}

function leegSchoolResultaat(sector) {
  return {
    bedrag_op_oordeel: 0,
    bedrag_op_aanvraag: 0,
    voldoet_aan_drempel: false,
    bedrag_eerste_opvang: 0,
    bedrag_tweede_jaar: 0,
    toeslag_eerste_keer: 0,
    voorbereidingskosten: 0,
    bedrag_leerlingen_vo: 0,
    bedrag_totaal: 0,
    per_categorie: sector === 'vo' ? { EERSTE: 0, TWEEDE: 0 } : { ASIELZOEKER: 0, OVERIGE_VREEMDELING: 0 },
    fallback: false,
    aanvraag_vereist: false,
    aanvraag: false,
    aanvraag_voorbereidingskosten: false,
  };
}

// ---- Volledige simulatie --------------------------------------------------

/**
 * @param {object} engine - WasmEngine met het corpus (ist of variant) geladen
 * @param {Array<object>} records - populatie- of persona-records (datamodel §1)
 * @param {object} options - { jaren?, peildata?, onProgress?(done, total) }
 * @returns {{ peildata, jaren, leerlingen, scholen, fouten, outputs }}
 */
/** Herkent de engine-fout "geen versie in werking" (regeling vervallen of nog niet geldig). */
export function isNietInWerking(error) {
  return /no version of law .* in force/i.test(String(error?.message ?? error));
}

export function simulate(engine, records, options = {}) {
  const jaren = options.jaren ?? DEFAULT_JAREN;
  const peildata = options.peildata ?? peildataVoorJaren(jaren);
  const onProgress = options.onProgress ?? (() => {});
  const cache = new Map();
  const fouten = { aantal: 0, voorbeelden: [] };
  const noteer = (melding) => {
    fouten.aantal++;
    if (fouten.voorbeelden.length < 5) fouten.voorbeelden.push(melding);
  };
  // Peildata waarop een regeling niet in werking is (bijvoorbeeld de
  // vo-regeling die per 1-1-2027 vervalt): één keer vastgesteld per sector,
  // daarna overgeslagen in plaats van per record als fout geteld.
  const nietInWerking = { po: new Set(), vo: new Set() };
  const total = records.length + peildata.length;
  let done = 0;

  // Pas 1: leerlingen.
  const leerlingen = [];
  for (const record of records) {
    const sector = sectorVan(record);
    engine.registerDataSource('personas', 'bsn', [record]);
    const perPeildatum = [];
    for (const pd of peildata) {
      if (nietInWerking[sector].has(pd)) {
        perPeildatum.push({ ...emptyLeerlingResult(record, pd, sector), niet_in_werking: true });
        continue;
      }
      try {
        perPeildatum.push(evaluateLeerlingOpPeildatum(engine, record, pd, cache));
      } catch (e) {
        if (isNietInWerking(e)) {
          nietInWerking[sector].add(pd);
          perPeildatum.push({ ...emptyLeerlingResult(record, pd, sector), niet_in_werking: true });
          continue;
        }
        noteer(`${record.bsn} @ ${pd}: ${String(e?.message ?? e)}`);
        perPeildatum.push(emptyLeerlingResult(record, pd, sector));
      }
    }
    // Reset de databron-cache van de engine; anders groeit die lineair mee.
    engine.clearDataSources();
    leerlingen.push({
      bsn: record.bsn,
      naam: record.naam ?? '',
      sector,
      school_id: record.school_id ?? null,
      schoolsoort: record.schoolsoort ?? (sector === 'vo' ? 'vo' : 'basisschool'),
      gewicht: record.gewicht ?? 1,
      heeft_bsn: record.heeft_bsn !== false,
      ambigu: isAmbigu(record),
      verblijfstitel_code: record.verblijfstitel_code ?? null,
      eerste_inschrijfdatum: record.eerste_inschrijfdatum,
      geboortedatum: record.geboortedatum,
      perPeildatum,
    });
    done++;
    if (done % 25 === 0) onProgress(done, total);
  }

  // Pas 2: scholen.
  const bySchool = new Map();
  records.forEach((record, i) => {
    const key = record.school_id ?? `?${sectorVan(record)}`;
    if (!bySchool.has(key)) {
      bySchool.set(key, {
        school_id: key,
        sector: sectorVan(record),
        schoolsoort: record.schoolsoort ?? (sectorVan(record) === 'vo' ? 'vo' : 'basisschool'),
        // Het gewicht van een school is het aantal échte scholen dat deze
        // gesimuleerde school voorstelt, niet het leerlinggewicht. Die twee
        // schelen een factor gelijk aan het aantal instroomjaren.
        gewicht: record.school_gewicht ?? record.gewicht ?? 1,
        // Nieuwkomers per jaar op de échte school: hierop slaat de drempel
        // van vier (art. 34 lid 2).
        omvang: record.school_omvang ?? null,
        leerlinggewicht: record.gewicht ?? 1,
        eerste_inschrijfdatum_min: record.eerste_inschrijfdatum,
        leerlingIdx: [],
      });
    }
    const s = bySchool.get(key);
    s.leerlingIdx.push(i);
    if (record.eerste_inschrijfdatum && record.eerste_inschrijfdatum < s.eerste_inschrijfdatum_min) {
      s.eerste_inschrijfdatum_min = record.eerste_inschrijfdatum;
    }
  });

  const scholen = [...bySchool.values()].map((s) => ({
    school_id: s.school_id,
    sector: s.sector,
    schoolsoort: s.schoolsoort,
    gewicht: s.gewicht,
    omvang: s.omvang,
    leerlinggewicht: s.leerlinggewicht,
    leerlingen: s.leerlingIdx.map((i) => records[i].bsn),
    _idx: s.leerlingIdx,
    _nieuwInVenster: s.eerste_inschrijfdatum_min >= peildata[0],
    _eersteKeerGebruikt: false,
    perPeildatum: [],
  }));

  peildata.forEach((pd, k) => {
    // Schoolrecords van alle scholen met iets in het bestand op deze peildatum.
    const actief = [];
    for (const school of scholen) {
      const resultaten = school._idx.map((i) => ({ record: records[i], uitkomst: leerlingen[i].perPeildatum[k] }));
      const eersteKeer = !school._eersteKeerGebruikt
        && school._nieuwInVenster
        && resultaten.some(({ uitkomst }) => uitkomst.telt);
      const { record: schoolRecord, facts } = buildSchoolRecord(school, resultaten, pd, eersteKeer);
      if (eersteKeer) school._eersteKeerGebruikt = true;
      const entry = {
        peildatum: pd,
        jaar: jaarVan(pd),
        Ap: schoolRecord.aantal_asielzoekers_peildatum,
        Vp: schoolRecord.aantal_overige_vreemdelingen_peildatum,
        At: schoolRecord.aantal_asielzoekers_telling_1_februari,
        tweedejaars: schoolRecord.aantal_tweedejaars_asielzoekers_peildatum,
        nieuwkomers_vo: schoolRecord.aantal_nieuwkomers_peildatum,
        eerste_cat_vo: facts.eerste_cat_vo,
        tweede_cat_vo: facts.tweede_cat_vo,
        eerste_keer: eersteKeer,
        in_bestand: facts.in_bestand,
        tellend: facts.tellend,
        onbeslist: facts.onbeslist,
        ambigu: facts.ambigu,
        zonder_bsn: facts.zonder_bsn,
        bedrag_leerlingen: facts.bedrag_leerlingen,
        leerlingen_tellend: facts.leerlingen_tellend,
        schoolRecord,
        facts,
      };
      school.perPeildatum.push(entry);
      if (facts.in_bestand > 0 || facts.tellend > 0) actief.push({ school, entry });
    }

    engine.registerDataSource('scholen', 'school_id', actief.map(({ entry }) => entry.schoolRecord));
    for (const { school, entry } of actief) {
      if (nietInWerking[school.sector].has(pd)) {
        Object.assign(entry, leegSchoolResultaat(school.sector), { niet_in_werking: true });
        delete entry.schoolRecord;
        delete entry.facts;
        continue;
      }
      const res = evaluateSchoolOpPeildatum(engine, school, entry.schoolRecord, entry.facts, pd, cache);
      if (res.fout) noteer(`${school.school_id} @ ${pd}: ${res.fout}`);
      Object.assign(entry, res);
      delete entry.schoolRecord;
      delete entry.facts;
    }
    for (const school of scholen) {
      const entry = school.perPeildatum[k];
      if (entry.schoolRecord) {
        Object.assign(entry, leegSchoolResultaat(school.sector));
        delete entry.schoolRecord;
        delete entry.facts;
      }
    }
    engine.clearDataSources();
    done++;
    onProgress(done, total);
  });

  for (const school of scholen) {
    delete school._idx;
    delete school._nieuwInVenster;
    delete school._eersteKeerGebruikt;
  }

  return {
    peildata,
    jaren,
    leerlingen,
    scholen,
    fouten,
    niet_in_werking: { po: [...nietInWerking.po].sort(), vo: [...nietInWerking.vo].sort() },
    outputs: Object.fromEntries(cache),
    gewichtTotaal: records.reduce((s, r) => s + (r.gewicht ?? 1), 0),
  };
}
