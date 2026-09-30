/**
 * Synthetische populatie van studieschuld-debiteuren.
 *
 * Genereert N records met exact het persona-schema uit data/personas.yaml,
 * op basis van verdelingsparameters uit data/distributions.yaml. Elk record
 * draagt een populatiegewicht zodat gewogen totalen naar de echte
 * cohortgroottes schalen. Interne DUO/OCW-microdata kan later drop-in
 * dezelfde recordvorm aannemen (andere loader, zelfde simulator).
 */

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

/** Kies een categorie uit {naam: kans}. */
export function sampleCategory(rand, weights) {
  const u = rand();
  let acc = 0;
  const entries = Object.entries(weights);
  for (const [name, w] of entries) {
    acc += w;
    if (u <= acc) return name;
  }
  return entries[entries.length - 1][0];
}

/**
 * Genereer een populatie.
 *
 * @param {object} distributions - geparste data/distributions.yaml
 * @param {number} n - aantal records
 * @param {number} seed
 * @returns {Array<object>} records met persona-schema + { gewicht, keuzes }
 */
export function generatePopulation(distributions, n, seed = 42) {
  const rand = mulberry32(seed);
  const d = distributions;
  const totaal = d.cohorten.totaal_debiteuren;
  const records = [];

  for (let i = 0; i < n; i++) {
    const regime = sampleCategory(rand, d.cohorten.regime_verdeling);
    const cohort = d.regimes[regime];

    const leeftijd = Math.round(
      cohort.leeftijd.min + rand() * (cohort.leeftijd.max - cohort.leeftijd.min),
    );
    const geboortejaar = d.basisjaar - leeftijd;
    const huishoudtype = sampleCategory(rand, d.huishoudtypen);
    const heeftPartner = huishoudtype === 'paar' || huishoudtype === 'paar_met_kind';

    const inkomensTabel = leeftijd < 35
      ? d.inkomen.percentielen_25_35
      : leeftijd < 45
        ? d.inkomen.percentielen_35_45
        : d.inkomen.percentielen_45_55;
    const inkomen = samplePercentiles(rand, inkomensTabel);
    const partnerinkomen = heeftPartner ? samplePercentiles(rand, inkomensTabel) : 0;
    const schuld = samplePercentiles(rand, cohort.schuld_percentielen);

    // Gedragskeuzes uit priors (Stand van de Uitvoering): wie vraagt
    // draagkracht aan, wie gebruikt de partneropt-out.
    const draagkrachtAangevraagd = rand() < cohort.draagkrachtmeting_prior;
    const partnerOptOut = regime === 'SF15_OUD'
      && heeftPartner
      && rand() < d.gedrag.partner_opt_out_prior;

    records.push({
      bsn: String(200000000 + i),
      geboortejaar,
      huishoudtype,
      heeft_partner: heeftPartner,
      partnerinkomen,
      inkomen,
      inkomensgroei: d.inkomen.groei_per_jaar,
      schuld,
      eerste_studiefinanciering: cohort.eerste_studiefinanciering,
      onderwijssoort: sampleCategory(rand, cohort.onderwijssoort),
      is_levenlanglerenkrediet: regime === 'SF15_LLLK',
      gewicht: totaal / n,
      keuzes: {
        draagkrachtAangevraagd,
        partnerMeetellen: !partnerOptOut,
        overstapAangevraagd: false,
        peiljaarverlegging: false,
        jokerMaanden: 0,
      },
    });
  }

  return records;
}
