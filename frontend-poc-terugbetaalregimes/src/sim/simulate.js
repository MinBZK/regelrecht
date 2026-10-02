/**
 * Meerjarenprojectie van een studieschuld-terugbetaling.
 *
 * De engine rekent per peildatum (jaarlijks: draagkracht, peiljaar,
 * rente); deze lus doet de maandelijkse boekhouding eromheen:
 * rente bijschrijven, betalen, jokerjaren pauzeren, de aflosperiode
 * verlengen bij de partneropt-out en aan het einde kwijtschelden.
 *
 * Alle geldbedragen zijn integer eurocent.
 */
import { discountFactor, monthlyRate } from './annuity.js';

export const LAW_ID = 'wet_studiefinanciering_2000';
export const NIBUD_ID = 'duo_werkinstructie_afloscapaciteit_nibud';

const CANONICAL_OUTPUTS = [
  'terugbetaalregime',
  'te_betalen_maandbedrag',
  'wettelijk_maandbedrag',
  'termijn_naar_draagkracht',
  'draagkrachtvrije_voet',
  'draagkrachtmeting_van_toepassing',
  'terugbetaalperiode_maanden',
  'rentepercentage',
  'aflosvrije_periode_toegestaan',
  'kwijtschelding_van_toepassing',
  'partner_opt_out_verlengt_aflosfase',
  'maximale_aflosfase_maanden_sf15_oud',
  // Voldoet de inkomensdaling aan de 15%-eis van artikel 6.12? De wet rekent
  // dit uit; de burgerview gebruikt het om peiljaarverlegging als keuze aan
  // te bieden precies wanneer die keuze bestaat.
  'peiljaarverlegging_mogelijk',
];

/**
 * Inkomen van de debiteur in kalenderjaar j. Standaard groeit het inkomen
 * met `inkomensgroei` vanaf startJaar. Een recente inkomensdaling wordt
 * gemodelleerd met `inkomen_voorheen` + `inkomensdaling_jaar`: in jaren
 * vóór het dalingsjaar gold het oude (hogere) inkomen. Daardoor ligt het
 * peiljaarinkomen (t-2) de eerste jaren na de daling boven het actuele
 * inkomen en heeft peiljaarverlegging (artikel 6.12) effect, precies tot
 * het peiljaar de daling heeft ingehaald.
 */
export function inkomenInJaar(record, startJaar, j) {
  if (record.inkomensdaling_jaar && record.inkomen_voorheen != null
      && j < record.inkomensdaling_jaar) {
    return Math.round(record.inkomen_voorheen);
  }
  const groei = record.inkomensgroei ?? 0;
  return Math.round(record.inkomen * Math.pow(1 + groei, j - startJaar));
}

/**
 * @param {object} engine - WasmEngine (wetten al geladen)
 * @param {object} record - persona/populatierecord:
 *   { bsn, geboortejaar, huishoudtype, heeft_partner, partnerinkomen,
 *     inkomen, inkomensgroei, inkomen_voorheen?, inkomensdaling_jaar?,
 *     schuld, eerste_studiefinanciering,
 *     onderwijssoort, is_levenlanglerenkrediet }
 * @param {object} choices -
 *   { overstapNieuw, draagkrachtAangevraagd, peiljaarverlegging,
 *     jokerMaanden, jokerVanafMaand, partnerMeetellen }
 * @param {object} options - { startJaar, maxLeeftijd }
 * @returns {{ timeline: object[], totals: object }}
 */
export function simulate(engine, record, choices, options = {}) {
  const startJaar = options.startJaar ?? 2026;
  const maxLeeftijd = options.maxLeeftijd ?? 90;

  let schuld = Math.round(record.schuld);
  const startSchuld = schuld;
  let periodeMaanden = null; // uit de wet, bij de eerste jaarstap
  let verstrekenMaanden = 0;
  let jokerGebruikt = 0;
  let totaalBetaald = 0;
  let totaalRente = 0;
  let kwijtgescholden = 0;
  let levenslang = false;

  const jokerMaanden = Math.min(choices.jokerMaanden ?? 0, 60);
  const jokerVanaf = choices.jokerVanafMaand ?? 0;
  const partnerMeetellen = choices.partnerMeetellen ?? true;

  const timeline = [];
  let jaar = startJaar;

  // Inkomen per kalenderjaar; peiljaar is t-2. Zie inkomenInJaar hierboven.
  const inkomen = (j) => inkomenInJaar(record, startJaar, j);

  while (schuld > 0) {
    const leeftijd = jaar - record.geboortejaar;
    if (leeftijd > maxLeeftijd) {
      levenslang = true;
      break;
    }

    // De wet hanteert het peiljaar (t-2) en regelt zelf de verlegging naar
    // het actuele jaar (artikel 6.12) en het al dan niet meetellen van het
    // partnerinkomen (artikel 10a.11); de simulator levert alleen de kale
    // inkomens aan.
    const inkomenPeiljaar = inkomen(jaar - 2);
    const inkomenActueel = inkomen(jaar);
    const partnerInkomen = record.heeft_partner ? Math.round(record.partnerinkomen ?? 0) : 0;

    engine.registerDataSource('personas', 'bsn', [
      {
        bsn: record.bsn,
        geboortejaar: record.geboortejaar,
        huishoudtype: record.huishoudtype,
        heeft_partner: !!record.heeft_partner,
        eerste_studiefinanciering_jaar: eersteStudiefinancieringJaar(record),
        onderwijssoort: record.onderwijssoort,
        is_levenlanglerenkrediet: !!record.is_levenlanglerenkrediet,
        toetsingsinkomen: inkomenPeiljaar,
        toetsingsinkomen_actueel: inkomenActueel,
        toetsingsinkomen_partner: partnerInkomen,
      },
    ]);

    // Eerste jaarstap zonder discontofactor kan niet: de wet heeft hem als
    // parameter nodig. Gebruik de (nog onbekende) periode uit de vorige stap
    // of een eerste schatting; na de eerste engine-call ligt de periode vast.
    const resterend = periodeMaanden === null
      ? 180 // voorlopige aanname; direct hierna overschreven door de wet
      : Math.max(periodeMaanden - verstrekenMaanden, 1);

    let result = executeYear(engine, record, choices, jaar, schuld, resterend, partnerMeetellen);
    if (periodeMaanden === null) {
      periodeMaanden = result.terugbetaalperiode_maanden;
      const echtResterend = Math.max(periodeMaanden - verstrekenMaanden, 1);
      if (echtResterend !== resterend) {
        result = executeYear(engine, record, choices, jaar, schuld, echtResterend, partnerMeetellen);
      }
    }

    const maandbedrag = Math.round(result.te_betalen_maandbedrag);
    const rente = result.rentepercentage;
    const rMaand = monthlyRate(rente);

    // Artikel 10a.11, tweede lid: voor ieder jaar dat het partnerinkomen
    // niet meetelt, wordt de aflosfase met een jaar verlengd. Een
    // beleidsvariant kan die verlenging maximeren (artikel 10a.4,
    // maximale_aflosfase_maanden; 0 = geen maximum).
    if (result.partner_opt_out_verlengt_aflosfase) {
      periodeMaanden += 12;
      const max = result.maximale_aflosfase_maanden_sf15_oud ?? 0;
      if (max > 0 && periodeMaanden > max) {
        periodeMaanden = max;
      }
    }

    const afloscapaciteit = nibudCapaciteit(engine, record, jaar, inkomenPeiljaar);
    const betalingsprobleem = afloscapaciteit !== null && maandbedrag > afloscapaciteit;

    let betaaldDitJaar = 0;
    let jokerDitJaar = 0;

    for (let m = 0; m < 12 && schuld > 0; m++) {
      const renteMaand = Math.round(schuld * rMaand);
      schuld += renteMaand;
      totaalRente += renteMaand;

      const maandIndex = verstrekenMaanden;
      const jokerToegestaan = !!result.aflosvrije_periode_toegestaan;
      const inJoker = jokerToegestaan
        && jokerGebruikt < jokerMaanden
        && maandIndex >= jokerVanaf;

      if (inJoker) {
        // Aflosvrije maand: niet betalen, de klok staat stil (de
        // terugbetaalperiode schuift mee op).
        jokerGebruikt++;
        jokerDitJaar++;
        periodeMaanden++;
        verstrekenMaanden++;
        continue;
      }

      const betaling = Math.min(maandbedrag, schuld);
      schuld -= betaling;
      betaaldDitJaar += betaling;
      totaalBetaald += betaling;
      verstrekenMaanden++;

      if (verstrekenMaanden >= periodeMaanden) break;
    }

    timeline.push({
      jaar,
      leeftijd,
      maandbedrag,
      wettelijk_maandbedrag: Math.round(result.wettelijk_maandbedrag),
      termijn_naar_draagkracht: result.termijn_naar_draagkracht === null
        ? null
        : Math.round(result.termijn_naar_draagkracht),
      restschuld: schuld,
      betaald: betaaldDitJaar,
      afloscapaciteit,
      betalingsprobleem,
      jokermaanden: jokerDitJaar,
      regime: result.terugbetaalregime,
      // Voor de burgerview: mag deze debiteur dit jaar peiljaarverlegging
      // aanvragen (artikel 6.12, 15%-daling)?
      peiljaarverlegging_mogelijk: result.peiljaarverlegging_mogelijk ?? null,
      // Telt de draagkracht mee? Onder hoofdstuk 6 gaat dat ambtshalve,
      // onder 10a alleen op aanvraag (artikel 10a.7).
      draagkrachtmeting_van_toepassing: result.draagkrachtmeting_van_toepassing ?? null,
    });

    if (schuld <= 0) break;

    if (verstrekenMaanden >= periodeMaanden) {
      // Einde aflosfase: de resterende schuld gaat teniet (artikel 6.16).
      // Wie het partnerinkomen blijft uitsluiten, bereikt dit einde nooit:
      // de verlenging van artikel 10a.11 schuift het elk jaar op.
      if (result.kwijtschelding_van_toepassing) {
        kwijtgescholden = schuld;
        schuld = 0;
      }
    }

    jaar++;
  }

  if (levenslang) {
    // Schuld resteert bij overlijden (leeftijdscap): niets kwijtgescholden
    // tijdens leven; administratief eindigt de verplichting bij overlijden.
    kwijtgescholden = schuld;
  }

  return {
    timeline,
    totals: {
      startSchuld,
      totaalBetaald,
      totaalRente,
      kwijtgescholden,
      restschuldBijOverlijden: levenslang ? schuld : 0,
      maanden: verstrekenMaanden,
      eindejaar: jaar,
      levenslang,
      maxMaandbedrag: Math.max(0, ...timeline.map((t) => t.maandbedrag)),
      jarenBetalingsprobleem: timeline.filter((t) => t.betalingsprobleem).length,
      regime: timeline.length ? timeline[timeline.length - 1].regime : null,
    },
  };
}

function eersteStudiefinancieringJaar(record) {
  if (typeof record.eerste_studiefinanciering_jaar === 'number') {
    return record.eerste_studiefinanciering_jaar;
  }
  return Number(String(record.eerste_studiefinanciering).slice(0, 4));
}

function executeYear(engine, record, choices, jaar, schuld, resterendeMaanden, partnerMeetellen) {
  // Rente van dit jaar is nodig voor de discontofactor; haal die eerst op.
  const renteResult = engine.executeMultiple(
    LAW_ID,
    ['rentepercentage'],
    baseParams(record, choices, schuld, resterendeMaanden, 1, partnerMeetellen),
    `${jaar}-01-01`,
  );
  const rente = renteResult.outputs.rentepercentage;
  const factor = discountFactor(rente, resterendeMaanden);

  const result = engine.executeMultiple(
    LAW_ID,
    CANONICAL_OUTPUTS,
    baseParams(record, choices, schuld, resterendeMaanden, factor, partnerMeetellen),
    `${jaar}-01-01`,
  );
  return result.outputs;
}

function baseParams(record, choices, schuld, resterendeMaanden, discontofactor, partnerMeetellen) {
  return {
    bsn: record.bsn,
    restschuld: schuld,
    resterende_maanden: resterendeMaanden,
    discontofactor,
    draagkracht_aangevraagd: choices.draagkrachtAangevraagd ?? false,
    partner_meetellen: partnerMeetellen,
    peiljaarverlegging_toegepast: !!choices.peiljaarverlegging,
    overstap_aangevraagd: choices.overstapAangevraagd ?? false,
  };
}

function nibudCapaciteit(engine, record, jaar, inkomenPeiljaar) {
  try {
    const res = engine.execute(
      NIBUD_ID,
      'redelijke_afloscapaciteit_per_maand',
      { bsn: record.bsn },
      `${jaar}-01-01`,
    );
    const value = res.outputs.redelijke_afloscapaciteit_per_maand;
    return value === null || value === undefined ? null : Math.round(value);
  } catch {
    return null; // NIBUD-document niet geladen: geen betalingsprobleem-vlag
  }
}
