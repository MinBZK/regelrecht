/**
 * Gewogen aggregaties over gesimuleerde populatierecords.
 *
 * Input: array van { record, totals } (uit simulate.js), waarbij record.gewicht
 * het aantal echte debiteuren is dat dit synthetische record vertegenwoordigt.
 */

export function aggregate(results) {
  const perRegime = {};
  let gewichtTotaal = 0;
  let betalingsprobleemGewicht = 0;
  let levenslangGewicht = 0;
  let kwijtgescholdenTotaal = 0; // gewogen eurocent
  let betaaldTotaal = 0; // wat er over de hele looptijd binnenkomt (gewogen eurocent)
  let renteTotaal = 0;
  let startSchuldTotaal = 0;
  // Uitvoeringsvolumes voor DUO: aantallen, geen euro's. Er is in deze casus
  // geen uitvoeringslastmodel (anders dan bij nieuwkomersbekostiging), dus
  // hier staan de aantallen handelingen en niet de kosten ervan.
  let draagkrachtGewicht = 0;
  let optOutGewicht = 0;
  let peiljaarGewicht = 0;

  for (const { record, totals } of results) {
    const w = record.gewicht ?? 1;
    gewichtTotaal += w;
    if (totals.jarenBetalingsprobleem > 0) betalingsprobleemGewicht += w;
    if (totals.levenslang) levenslangGewicht += w;
    kwijtgescholdenTotaal += totals.kwijtgescholden * w;
    betaaldTotaal += (totals.totaalBetaald ?? 0) * w;
    renteTotaal += (totals.totaalRente ?? 0) * w;
    startSchuldTotaal += (totals.startSchuld ?? 0) * w;
    if (record.keuzes?.draagkrachtAangevraagd) draagkrachtGewicht += w;
    if (record.heeft_partner && record.keuzes?.partnerMeetellen === false) optOutGewicht += w;
    if (record.keuzes?.peiljaarverlegging) peiljaarGewicht += w;

    const regime = totals.regime ?? 'ONBEKEND';
    if (!perRegime[regime]) {
      perRegime[regime] = {
        gewicht: 0,
        maandbedragSom: 0,
        betalingsprobleemGewicht: 0,
        kwijtgescholden: 0,
      };
    }
    const r = perRegime[regime];
    r.gewicht += w;
    r.maandbedragSom += totals.maxMaandbedrag * w;
    if (totals.jarenBetalingsprobleem > 0) r.betalingsprobleemGewicht += w;
    r.kwijtgescholden += totals.kwijtgescholden * w;
  }

  const regimes = {};
  for (const [naam, r] of Object.entries(perRegime)) {
    regimes[naam] = {
      aantal: Math.round(r.gewicht),
      gemiddeldMaxMaandbedrag: r.gewicht ? Math.round(r.maandbedragSom / r.gewicht) : 0,
      pctBetalingsprobleem: r.gewicht ? r.betalingsprobleemGewicht / r.gewicht : 0,
      kwijtgescholdenTotaal: Math.round(r.kwijtgescholden),
    };
  }

  return {
    aantalDebiteuren: Math.round(gewichtTotaal),
    aantalBetalingsprobleem: Math.round(betalingsprobleemGewicht),
    pctBetalingsprobleem: gewichtTotaal ? betalingsprobleemGewicht / gewichtTotaal : 0,
    aantalLevenslang: Math.round(levenslangGewicht),
    kwijtgescholdenTotaal: Math.round(kwijtgescholdenTotaal),
    // Wat er over de hele looptijd binnenkomt en wat er wordt afgeboekt: de
    // twee posten die op de OCW-begroting landen.
    betaaldTotaal: Math.round(betaaldTotaal),
    renteTotaal: Math.round(renteTotaal),
    startSchuldTotaal: Math.round(startSchuldTotaal),
    // Uitvoeringsvolumes bij DUO (aantallen debiteuren, geen kosten).
    uitvoering: {
      draagkrachtmetingen: Math.round(draagkrachtGewicht),
      partnerOptOuts: Math.round(optOutGewicht),
      peiljaarverleggingen: Math.round(peiljaarGewicht),
    },
    regimes,
  };
}
