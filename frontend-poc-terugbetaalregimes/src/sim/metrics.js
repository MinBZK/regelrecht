/**
 * Gewogen aggregaties over gesimuleerde populatierecords.
 *
 * Input: array van { record, totals } (uit simulate.js), waarbij record.gewicht
 * het aantal echte debiteuren is dat dit synthetische record vertegenwoordigt.
 */

export function aggregate(results, handelingen = null) {
  const perRegime = {};
  let gewichtTotaal = 0;
  let betalingsprobleemGewicht = 0;
  let levenslangGewicht = 0;
  let kwijtgescholdenTotaal = 0; // gewogen eurocent
  let betaaldTotaal = 0; // wat er over de hele looptijd binnenkomt (gewogen eurocent)
  let renteTotaal = 0;
  let startSchuldTotaal = 0;
  // Uitvoeringsvolumes: de aantallen waarop het uitvoeringslastmodel rekent.
  // Zonder model (`handelingen` is null) blijven het alleen aantallen.
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

  const volumes = {
    per_aflossende_debiteur: gewichtTotaal,
    per_draagkrachtmeting: draagkrachtGewicht,
    per_partner_opt_out: optOutGewicht,
    // Per constructie nul: population.js zet peiljaarverlegging hard op false.
    // Blijft staan zodat het model klopt zodra er een prior is; handelingen.yaml
    // draagt er daarom bewust nog geen handelingen voor.
    per_peiljaarverlegging: peiljaarGewicht,
    per_debiteur_betalingsprobleem: betalingsprobleemGewicht,
  };

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
    uitvoeringslast: uitvoeringslastVan(handelingen, volumes),
    regimes,
  };
}

/**
 * Wat de uitvoering kost, per partij, gegeven het aantal aanleidingen.
 *
 * DUO en de debiteur tellen bewust niet in dezelfde eenheid. DUO is een
 * uitvoeringsorganisatie: zijn tijd is loonkosten en die staan in euro's. Voor
 * de tijd van een debiteur bestaat geen tarief, en er een op plakken zou een
 * uitspraak zijn over wat die tijd waard is. Handelingen van de debiteur
 * dragen daarom wel minuten maar geen tarief, en tellen op tot uren.
 *
 * Zonder model komt er niets terug: het is dan niet nul, het is onbekend.
 */
export function uitvoeringslastVan(handelingen, volumes) {
  if (!handelingen?.handelingen?.length) return null;
  const tarieven = handelingen.tarieven ?? {};
  const perHandeling = [];
  const perPartij = {};

  for (const h of handelingen.handelingen) {
    const aantal = volumes[h.aanleiding] ?? 0;
    const uren = (aantal * Number(h.minuten ?? 0)) / 60;
    // Geen tarief betekent: deze tijd wordt niet in geld uitgedrukt.
    const tarief = h.tarief ? Number(tarieven[h.tarief] ?? 0) : null;
    const kosten = tarief === null ? null : uren * tarief;
    perHandeling.push({ id: h.id, omschrijving: h.omschrijving, partij: h.partij, aanleiding: h.aanleiding, minuten: Number(h.minuten ?? 0), aantal: Math.round(aantal), uren, kosten });
    const p = (perPartij[h.partij] ??= { uren: 0, kosten: 0, inGeld: false });
    p.uren += uren;
    if (kosten !== null) { p.kosten += kosten; p.inGeld = true; }
  }

  // Alleen partijen met een tarief tellen mee in het eurototaal; de uren van de
  // debiteur staan ernaast en worden er niet bij opgeteld.
  let kostenTotaal = 0;
  let urenBurger = 0;
  for (const p of Object.values(perPartij)) {
    if (p.inGeld) kostenTotaal += p.kosten;
    else urenBurger += p.uren;
  }

  return { perHandeling, perPartij, kostenTotaal, urenBurger };
}
