/**
 * Aggregatie van een simulatie (simulate.js) naar de uitkomsten van
 * docs/datamodel.md §6: per jaar en totaal de regeling-uitgaven (po/vo, per
 * categorie), de uitvoeringslast per partij (handelingen × minuten × tarief),
 * de investeringen (afschrijving), de budgettoets en de stabiliteit.
 *
 * De handelingen-parameters (minuten, tarieven, investeringen, budget) komen
 * uit data/handelingen.yaml (of een bewerkte kopie) en worden hier pas in
 * geld omgezet: bijstellen kost geen hersimulatie.
 *
 * Alle bedragen integer eurocent; gewichten (echte leerlingen/scholen per
 * record) worden overal toegepast.
 */
import { isEenJanuari } from '../lib/nieuwkomerFacts.js';

export const AANLEIDINGEN = [
  'per_school_peildatum',
  'per_leerling_peildatum',
  'per_ambigu_leerling',
  'per_aanvraag',
  'per_aanvraag_1_januari',
  'per_bezwaar',
  'per_school_jaar',
  'per_leerling_zonder_bsn',
  'per_aanvraag_voorbereidingskosten',
];

function add(counts, sector, aanleiding, jaar, w) {
  if (!w) return;
  const s = (counts[sector] ??= {});
  const a = (s[aanleiding] ??= {});
  a[jaar] = (a[jaar] ?? 0) + w;
}

/**
 * Gewogen aantallen per (sector, aanleiding, jaar) uit de simulatie.
 * `per_bezwaar` wordt in aggregate() afgeleid als fractie van `per_aanvraag`.
 */
export function countAanleidingen(sim) {
  const counts = {};
  for (const school of sim.scholen) {
    // Handelingen per school tellen met het schoolgewicht, handelingen per
    // leerling met het leerlinggewicht: pp.tellend en pp.ambigu zijn ruwe
    // recordaantallen, en één record staat voor `leerlinggewicht` leerlingen.
    const w = school.gewicht ?? 1;
    const wl = school.leerlinggewicht ?? w;
    const sector = school.sector;
    const jarenMetTellend = new Set();
    for (const pp of school.perPeildatum) {
      if (pp.in_bestand > 0) add(counts, sector, 'per_school_peildatum', pp.jaar, w);
      add(counts, sector, 'per_leerling_peildatum', pp.jaar, pp.tellend * wl);
      add(counts, sector, 'per_ambigu_leerling', pp.jaar, pp.ambigu * wl);
      if (pp.aanvraag) {
        add(counts, sector, 'per_aanvraag', pp.jaar, w);
        if (isEenJanuari(pp.peildatum)) add(counts, sector, 'per_aanvraag_1_januari', pp.jaar, w);
      }
      if (pp.aanvraag_voorbereidingskosten) add(counts, sector, 'per_aanvraag_voorbereidingskosten', pp.jaar, w);
      if (pp.tellend > 0) jarenMetTellend.add(pp.jaar);
    }
    for (const jaar of jarenMetTellend) add(counts, sector, 'per_school_jaar', jaar, w);
  }
  for (const l of sim.leerlingen) {
    if (l.heeft_bsn) continue;
    const eerste = l.perPeildatum.find((pp) => pp.telt);
    if (eerste) add(counts, l.sector, 'per_leerling_zonder_bsn', eerste.jaar, l.gewicht ?? 1);
  }
  return counts;
}

function emptyJaar() {
  return {
    regeling_po: {
      totaal: 0,
      eerste_opvang: 0,
      tweede_jaar: 0,
      toeslag: 0,
      per_categorie: { ASIELZOEKER: 0, OVERIGE_VREEMDELING: 0 },
    },
    regeling_vo: {
      totaal: 0,
      leerlingen: 0,
      voorbereidingskosten: 0,
      per_categorie: { EERSTE: 0, TWEEDE: 0 },
    },
    regeling_totaal: 0,
    uitvoeringslast: { school: 0, duo: 0, overig: 0, totaal: 0, per_partij: {} },
    investering: 0,
    investering_per_partij: {},
    uitvoering_totaal: 0,
    handelingen: {},
    leerlingen_bekostigd: { po: 0, vo: 0, totaal: 0 },
    leerlingen_1_oktober: { po: 0, vo: 0 },
    leerlingen_onder_drempel: 0,
    leerlingen_per_categorie: {},
    leerling_kwartalen: { po: 0, vo: 0 },
    scholen_bekostigd: { po: 0, vo: 0 },
    scholen_onder_drempel: 0,
    scholen_met_nieuwkomers: { po: 0, vo: 0 },
    binnen_budget: { regeling_po: null, regeling_vo: null, uitvoering: null },
    // Rechtmatigheid en risico: geld dat op eigen opgave van de school wordt
    // uitbetaald (ADR-onzekerheid) en geld dat scholen missen door te late aanvragen.
    grondslag: { op_aanvraag: 0, op_oordeel: 0, zonder_verifieerbare_grondslag: 0, gemist_te_laat: 0 },
  };
}

function tariefVan(handeling, tarieven) {
  if (handeling.tarief === null || handeling.tarief === undefined) return 0;
  if (typeof handeling.tarief === 'number') return handeling.tarief;
  const t = tarieven?.[handeling.tarief];
  return Number.isFinite(Number(t)) ? Number(t) : 0;
}

function partijBucket(partij) {
  if (partij === 'school') return 'school';
  if (partij === 'duo') return 'duo';
  return 'overig';
}

/**
 * Spreiding van de bekostiging per school van jaar op jaar: variatie-
 * coëfficiënt (sd/gemiddelde over de jaren) per school, gewogen gemiddeld
 * over scholen die in minstens één jaar bekostiging kregen. Lager = stabieler.
 */
export function stabiliteit(sim) {
  const per = { po: { som: 0, w: 0 }, vo: { som: 0, w: 0 } };
  for (const school of sim.scholen) {
    const perJaar = {};
    for (const pp of school.perPeildatum) perJaar[pp.jaar] = (perJaar[pp.jaar] ?? 0) + (pp.bedrag_totaal ?? 0);
    const waarden = sim.jaren.map((j) => perJaar[j] ?? 0);
    const som = waarden.reduce((a, b) => a + b, 0);
    if (som <= 0) continue;
    const mean = som / waarden.length;
    const varsom = waarden.reduce((a, v) => a + (v - mean) ** 2, 0) / waarden.length;
    const cv = Math.sqrt(varsom) / mean;
    const w = school.gewicht ?? 1;
    const bucket = per[school.sector] ?? (per[school.sector] = { som: 0, w: 0 });
    bucket.som += cv * w;
    bucket.w += w;
  }
  const out = {};
  let somAll = 0;
  let wAll = 0;
  for (const [sector, b] of Object.entries(per)) {
    out[sector] = b.w ? b.som / b.w : null;
    somAll += b.som;
    wAll += b.w;
  }
  out.totaal = wAll ? somAll / wAll : null;
  return out;
}

/**
 * @param {object} sim - resultaat van simulate()
 * @param {object} handelingen - geparste handelingen.yaml (tarieven, handelingen, fracties, investeringen, budget)
 * @param {object} options - { index?: {jaar: factor}, budget?: {regeling_po, regeling_vo, uitvoering} }
 */
export function aggregate(sim, handelingen = {}, options = {}) {
  const jaren = sim.jaren;
  const counts = countAanleidingen(sim);
  const fractieBezwaar = Number(handelingen?.fracties?.bezwaar_per_aanvraag ?? 0);
  for (const sector of Object.keys(counts)) {
    const aanvragen = counts[sector].per_aanvraag ?? {};
    for (const [jaar, n] of Object.entries(aanvragen)) add(counts, sector, 'per_bezwaar', Number(jaar), n * fractieBezwaar);
  }

  const tarieven = handelingen?.tarieven ?? {};
  const lijst = Array.isArray(handelingen?.handelingen) ? handelingen.handelingen : [];
  // Investeringen met `actief: false` horen bij een variant die ze aanzet
  // (branch of override); in de basis tellen ze niet mee.
  const investeringen = (Array.isArray(handelingen?.investeringen) ? handelingen.investeringen : [])
    .filter((inv) => inv.actief !== false);
  const budget = options.budget ?? handelingen?.budget ?? null;
  const index = options.index ?? {};

  const perJaar = {};
  for (const jaar of jaren) perJaar[jaar] = emptyJaar();

  // Regeling-uitgaven en schooltellingen.
  for (const school of sim.scholen) {
    const w = school.gewicht ?? 1;
    const bekostigdInJaar = new Set();
    const drempelInJaar = new Set();
    const nieuwkomersInJaar = new Set();
    for (const pp of school.perPeildatum) {
      const m = perJaar[pp.jaar];
      if (!m) continue;
      const f = w * (index[pp.jaar] ?? 1);
      if (school.sector === 'vo') {
        m.regeling_vo.leerlingen += (pp.bedrag_leerlingen_vo ?? 0) * f;
        m.regeling_vo.voorbereidingskosten += (pp.voorbereidingskosten ?? 0) * f;
        m.regeling_vo.per_categorie.EERSTE += (pp.per_categorie?.EERSTE ?? 0) * f;
        m.regeling_vo.per_categorie.TWEEDE += (pp.per_categorie?.TWEEDE ?? 0) * f;
      } else {
        m.regeling_po.eerste_opvang += (pp.bedrag_eerste_opvang ?? 0) * f;
        m.regeling_po.tweede_jaar += (pp.bedrag_tweede_jaar ?? 0) * f;
        m.regeling_po.toeslag += (pp.toeslag_eerste_keer ?? 0) * f;
        m.regeling_po.per_categorie.ASIELZOEKER += (pp.per_categorie?.ASIELZOEKER ?? 0) * f;
        m.regeling_po.per_categorie.OVERIGE_VREEMDELING += (pp.per_categorie?.OVERIGE_VREEMDELING ?? 0) * f;
        if ((pp.Ap ?? 0) + (pp.Vp ?? 0) > 0 && !pp.voldoet_aan_drempel) drempelInJaar.add(pp.jaar);
        if (pp.aanvraag_vereist) m.grondslag.op_aanvraag += (pp.bedrag_op_aanvraag ?? 0) * f;
        else m.grondslag.op_oordeel += (pp.bedrag_op_oordeel ?? 0) * f;
      }
      m.leerling_kwartalen[school.sector] += (pp.tellend ?? 0) * (school.leerlinggewicht ?? w);
      if ((pp.bedrag_totaal ?? 0) > 0) bekostigdInJaar.add(pp.jaar);
      if ((pp.tellend ?? 0) > 0) nieuwkomersInJaar.add(pp.jaar);
    }
    for (const jaar of bekostigdInJaar) perJaar[jaar].scholen_bekostigd[school.sector] += w;
    for (const jaar of drempelInJaar) perJaar[jaar].scholen_onder_drempel += w;
    for (const jaar of nieuwkomersInJaar) perJaar[jaar].scholen_met_nieuwkomers[school.sector] += w;
  }

  // Leerlingen bekostigd (uniek per jaar) en per categorie. Een tellende
  // leerling is pas bekostigd als de school op die peildatum geld krijgt:
  // eerstejaars in het po vallen weg onder de drempel (art. 34 lid 2),
  // tweedejaars (art. 35) en vo-nieuwkomers kennen geen drempel.
  const leerlingByBsn = new Map(sim.leerlingen.map((l) => [l.bsn, l]));
  const bekostigdInJaar = new Map(); // bsn -> Set(jaar)
  const onderDrempelInJaar = new Map();
  for (const school of sim.scholen) {
    school.perPeildatum.forEach((pp, k) => {
      for (const bsn of pp.leerlingen_tellend ?? []) {
        const l = leerlingByBsn.get(bsn);
        const u = l?.perPeildatum[k];
        if (!u) continue;
        const bekostigd = school.sector === 'vo' || u.bekostigingsjaar === 2 || pp.voldoet_aan_drempel;
        const target = bekostigd ? bekostigdInJaar : onderDrempelInJaar;
        if (!target.has(bsn)) target.set(bsn, new Set());
        target.get(bsn).add(pp.jaar);
        // Stand op 1 oktober: vergelijkbaar met de OCW-factsheetreeks.
        if (bekostigd && String(pp.peildatum).slice(5) === '10-01' && perJaar[pp.jaar]) {
          perJaar[pp.jaar].leerlingen_1_oktober[school.sector] += l.gewicht ?? 1;
        }
      }
    });
  }
  for (const [bsn, jarenSet] of bekostigdInJaar) {
    const l = leerlingByBsn.get(bsn);
    const w = l.gewicht ?? 1;
    for (const jaar of jarenSet) {
      const m = perJaar[jaar];
      if (!m) continue;
      m.leerlingen_bekostigd[l.sector] += w;
      m.leerlingen_bekostigd.totaal += w;
      const pp = l.perPeildatum.find((u) => u.jaar === jaar && u.telt);
      const cat = pp?.categorie_effectief ?? pp?.categorie ?? 'ONBEKEND';
      const key = l.sector === 'po' && pp?.bekostigingsjaar === 2 ? 'TWEEDEJAARS' : cat;
      m.leerlingen_per_categorie[key] = (m.leerlingen_per_categorie[key] ?? 0) + w;
    }
  }
  for (const [bsn, jarenSet] of onderDrempelInJaar) {
    const l = leerlingByBsn.get(bsn);
    for (const jaar of jarenSet) {
      if (bekostigdInJaar.get(bsn)?.has(jaar)) continue;
      if (perJaar[jaar]) perJaar[jaar].leerlingen_onder_drempel += l.gewicht ?? 1;
    }
  }

  // Handelingen, investeringen, budget.
  for (const jaar of jaren) {
    const m = perJaar[jaar];
    m.regeling_po.totaal = m.regeling_po.eerste_opvang + m.regeling_po.tweede_jaar + m.regeling_po.toeslag;
    m.regeling_vo.totaal = m.regeling_vo.leerlingen + m.regeling_vo.voorbereidingskosten;
    m.regeling_totaal = m.regeling_po.totaal + m.regeling_vo.totaal;

    for (const h of lijst) {
      // Een handeling kan een geldigheidsvenster hebben: `vanaf_jaar` (nieuw proces
      // vanaf de invoering van een variant) en `tot_jaar` (vervalt bij invoering).
      if (h.vanaf_jaar != null && jaar < Number(h.vanaf_jaar)) continue;
      if (h.tot_jaar != null && jaar >= Number(h.tot_jaar)) continue;
      const sectoren = h.sector ? [h.sector] : ['po', 'vo'];
      let aantal = 0;
      for (const sector of sectoren) aantal += counts[sector]?.[h.aanleiding]?.[jaar] ?? 0;
      const uren = (aantal * Number(h.minuten ?? 0)) / 60;
      const kosten = uren * tariefVan(h, tarieven);
      m.handelingen[h.id] = { aantal, uren, kosten, partij: h.partij, sector: h.sector ?? null, aanleiding: h.aanleiding };
      const bucket = partijBucket(h.partij);
      m.uitvoeringslast[bucket] += kosten;
      m.uitvoeringslast.per_partij[h.partij] = (m.uitvoeringslast.per_partij[h.partij] ?? 0) + kosten;
    }
    m.uitvoeringslast.totaal = m.uitvoeringslast.school + m.uitvoeringslast.duo + m.uitvoeringslast.overig;

    for (const inv of investeringen) {
      const vanaf = Number(inv.vanaf_jaar ?? jaren[0]);
      const duur = Math.max(1, Number(inv.afschrijving_jaren ?? 1));
      if (jaar >= vanaf && jaar < vanaf + duur) {
        const deel = Number(inv.bedrag ?? 0) / duur;
        m.investering += deel;
        m.investering_per_partij[inv.partij ?? 'duo'] = (m.investering_per_partij[inv.partij ?? 'duo'] ?? 0) + deel;
      }
    }
    m.uitvoering_totaal = m.uitvoeringslast.totaal + m.investering;
    m.grondslag.zonder_verifieerbare_grondslag = m.grondslag.op_aanvraag + m.grondslag.op_oordeel;
    m.grondslag.gemist_te_laat = m.grondslag.op_aanvraag * Number(handelingen?.fracties?.aanvraag_te_laat ?? 0);

    m.binnen_budget = {
      regeling_po: budget?.regeling_po == null ? null : m.regeling_po.totaal <= Number(budget.regeling_po),
      regeling_vo: budget?.regeling_vo == null ? null : m.regeling_vo.totaal <= Number(budget.regeling_vo),
      uitvoering: budget?.uitvoering == null ? null : m.uitvoering_totaal <= Number(budget.uitvoering),
    };
  }

  // Totalen over de horizon.
  const totaal = emptyJaar();
  const sumInto = (dst, src) => {
    for (const [k, v] of Object.entries(src)) {
      if (typeof v === 'number') dst[k] = (dst[k] ?? 0) + v;
      else if (v && typeof v === 'object') sumInto((dst[k] ??= {}), v);
    }
  };
  for (const jaar of jaren) {
    const m = perJaar[jaar];
    for (const key of ['regeling_po', 'regeling_vo', 'uitvoeringslast', 'investering_per_partij', 'leerlingen_bekostigd', 'leerlingen_1_oktober', 'leerlingen_per_categorie', 'leerling_kwartalen', 'scholen_bekostigd', 'scholen_met_nieuwkomers', 'grondslag']) {
      sumInto(totaal[key], m[key]);
    }
    totaal.regeling_totaal += m.regeling_totaal;
    totaal.investering += m.investering;
    totaal.uitvoering_totaal += m.uitvoering_totaal;
    totaal.scholen_onder_drempel += m.scholen_onder_drempel;
    totaal.leerlingen_onder_drempel += m.leerlingen_onder_drempel;
    for (const [id, h] of Object.entries(m.handelingen)) {
      const t = (totaal.handelingen[id] ??= { aantal: 0, uren: 0, kosten: 0, partij: h.partij, sector: h.sector, aanleiding: h.aanleiding });
      t.aantal += h.aantal;
      t.uren += h.uren;
      t.kosten += h.kosten;
    }
  }
  const alle = (post) => {
    const vals = jaren.map((j) => perJaar[j].binnen_budget[post]);
    if (vals.every((v) => v === null)) return null;
    return vals.every((v) => v !== false);
  };
  totaal.binnen_budget = { regeling_po: alle('regeling_po'), regeling_vo: alle('regeling_vo'), uitvoering: alle('uitvoering') };
  // Gemiddelden per jaar voor de tel-metrieken (die zijn per jaar uniek, niet optelbaar).
  const n = jaren.length || 1;
  totaal.gemiddeld_per_jaar = {
    leerlingen_bekostigd: {
      po: totaal.leerlingen_bekostigd.po / n,
      vo: totaal.leerlingen_bekostigd.vo / n,
      totaal: totaal.leerlingen_bekostigd.totaal / n,
    },
    scholen_onder_drempel: totaal.scholen_onder_drempel / n,
    leerlingen_onder_drempel: totaal.leerlingen_onder_drempel / n,
    scholen_bekostigd: { po: totaal.scholen_bekostigd.po / n, vo: totaal.scholen_bekostigd.vo / n },
  };

  const investeringTotaal = investeringen.reduce((s, inv) => s + Number(inv.bedrag ?? 0), 0);

  return {
    jaren,
    perJaar,
    totaal,
    stabiliteit: stabiliteit(sim),
    budget,
    investering_totaal: investeringTotaal,
    handelingen: lijst.map((h) => ({
      ...h,
      tarief_bedrag: tariefVan(h, tarieven),
      perJaar: Object.fromEntries(jaren.map((j) => [j, perJaar[j].handelingen[h.id]])),
      totaal: totaal.handelingen[h.id],
    })),
    aantallen: counts,
    fouten: sim.fouten,
    niet_in_werking: sim.niet_in_werking ?? { po: [], vo: [] },
    outputs: sim.outputs,
    gewichtTotaal: sim.gewichtTotaal,
  };
}

/**
 * Terugverdientijd in jaren van de investeringen van een variant, uit de
 * jaarlijkse besparing op de uitvoeringslast (school + DUO + overig) ten
 * opzichte van de ist-situatie. null = verdient zich niet terug; 0 = geen
 * investering nodig.
 */
export function terugverdientijd(metrics, ist) {
  if (!metrics || !ist) return null;
  const investering = metrics.investering_totaal ?? 0;
  if (investering <= 0) return 0;
  const jaren = metrics.jaren.filter((j) => ist.perJaar[j]);
  if (!jaren.length) return null;
  let besparing = 0;
  for (const j of jaren) besparing += ist.perJaar[j].uitvoeringslast.totaal - metrics.perJaar[j].uitvoeringslast.totaal;
  besparing /= jaren.length;
  if (besparing <= 0) return null;
  return investering / besparing;
}

/** Verschil tussen twee metrics-sets op de hoofdposten (variant − ist), totaal over de horizon. */
export function verschil(metrics, ist) {
  if (!metrics || !ist) return null;
  const d = (a, b) => (a ?? 0) - (b ?? 0);
  return {
    regeling_po: d(metrics.totaal.regeling_po.totaal, ist.totaal.regeling_po.totaal),
    regeling_vo: d(metrics.totaal.regeling_vo.totaal, ist.totaal.regeling_vo.totaal),
    regeling_totaal: d(metrics.totaal.regeling_totaal, ist.totaal.regeling_totaal),
    uitvoeringslast_school: d(metrics.totaal.uitvoeringslast.school, ist.totaal.uitvoeringslast.school),
    uitvoeringslast_duo: d(metrics.totaal.uitvoeringslast.duo, ist.totaal.uitvoeringslast.duo),
    uitvoeringslast_totaal: d(metrics.totaal.uitvoeringslast.totaal, ist.totaal.uitvoeringslast.totaal),
    investering: d(metrics.totaal.investering, ist.totaal.investering),
    uitvoering_totaal: d(metrics.totaal.uitvoering_totaal, ist.totaal.uitvoering_totaal),
    per_categorie_po: {
      ASIELZOEKER: d(metrics.totaal.regeling_po.per_categorie.ASIELZOEKER, ist.totaal.regeling_po.per_categorie.ASIELZOEKER),
      OVERIGE_VREEMDELING: d(metrics.totaal.regeling_po.per_categorie.OVERIGE_VREEMDELING, ist.totaal.regeling_po.per_categorie.OVERIGE_VREEMDELING),
    },
  };
}
