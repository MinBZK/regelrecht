/**
 * regimeFacts - haalt de karakteristieke wetsparameters per terugbetaalregime
 * LIVE uit de lawStore-definities, zodat de regime-vergelijkingstabel meebeweegt
 * met beleid-edits en varianten. Niets is hier hardcoded behalve de mapping van
 * "welke definitie hoort bij welke kolom".
 */
import { useLawStore } from '../engine/lawStore.js';

const LAW_ID = 'wet_studiefinanciering_2000';

const { readDefinition } = useLawStore();

function def(article, name) {
  return readDefinition(LAW_ID, article, name);
}

/**
 * @returns {Array} per regime een rij met leesbare feiten. Waarden zijn ruw
 *   (ratio's, maanden); de weergave (percent/duration) doet de component.
 */
export function regimeFacts() {
  const draagkrachtvoetSf15Oud = def('10a.8', 'voet_ratio_overig_sf15_oud');
  const draagkrachtvoetSf15Nieuw = def('12.32', 'voet_ratio_overig_sf15_nieuw');
  const draagkrachtvoetLllk = def('6.19', 'voet_ratio_overig_lllk');
  const draagkrachtvoetSf35 = def('6.10', 'voet_ratio_overig_sf35');

  return [
    {
      regime: 'SF15_OUD',
      voetRatio: draagkrachtvoetSf15Oud,
      // SF15-oud kent een schijventarief (7,9%–30%); toon het toptarief.
      draagkrachtRatio: def('10a.8', 'percentage_meerdere'),
      draagkrachtLabel: '7,9–30% (schijven)',
      periodeMaanden: def('10a.4', 'aflosfase_maanden'),
      aflosvrij: true,
      draagkrachtAutomatisch: false,
    },
    {
      regime: 'SF15_NIEUW',
      voetRatio: draagkrachtvoetSf15Nieuw,
      draagkrachtRatio: def('12.32', 'draagkrachtpercentage_sf15_nieuw'),
      periodeMaanden: def('12.32', 'aflosfase_maanden'),
      aflosvrij: true,
      draagkrachtAutomatisch: true,
    },
    {
      regime: 'SF15_LLLK',
      voetRatio: draagkrachtvoetLllk,
      draagkrachtRatio: def('6.19', 'draagkrachtpercentage_lllk'),
      periodeMaanden: def('6.19', 'aflosfase_maanden'),
      aflosvrij: false,
      draagkrachtAutomatisch: true,
    },
    {
      regime: 'SF35',
      voetRatio: draagkrachtvoetSf35,
      draagkrachtRatio: def('6.10', 'draagkrachtpercentage_sf35'),
      periodeMaanden: def('6.7', 'aflosfase_maanden_sf35'),
      aflosvrij: true,
      draagkrachtAutomatisch: true,
    },
  ];
}

/** snake_case → 'Snake case', voor labels van definities. */
export function leesbaar(naam) {
  const s = String(naam ?? '').replace(/_/g, ' ').trim();
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
}

/** Onderwerp per artikel van de WSF 2000, als kop in de parametereditor. */
export const ARTIKEL_ONDERWERP = {
  '6.7': 'aflosfase SF35',
  '6.9': 'minimum jaarbedrag',
  '6.10': 'draagkracht SF35',
  '6.12': 'peiljaarverlegging',
  '6.19': 'draagkracht levenlanglerenkrediet',
  '10a.1': 'cohortgrenzen',
  '10a.4': 'aflosfase SF15',
  '10a.8': 'draagkracht SF15-oud',
  '12.32': 'draagkracht SF15-nieuw (overgangsrecht)',
};

export function artikelOnderwerp(article) {
  return ARTIKEL_ONDERWERP[String(article)] ?? '';
}

// ---- Woordenlijst: twee assen die makkelijk door elkaar lopen -------------
//
// Een terugbetaalregime en een beleidsvariant zijn niet hetzelfde, en in de
// beleidsview staan ze naast elkaar op het scherm. Vandaar overal het soort
// erbij, zodat "SF35" en "variant b" niet voor elkaar worden aangezien.

/** Een groep debiteuren: onder welke regels iemand valt (volgt uit het cohort). */
export const REGIME_UITLEG =
  'Een terugbetaalregime is de set regels waar een debiteur onder valt. Welk regime dat is, volgt uit het cohort (wanneer je voor het eerst studiefinanciering kreeg en welke opleiding).';

/** Een voorgestelde wetswijziging, die op meerdere regimes tegelijk kan ingrijpen. */
export const VARIANT_UITLEG =
  'Een beleidsvariant is een voorgestelde wijziging van de wet. Een variant kan meerdere regimes tegelijk raken; de werkversie is de variant waarop je nu bewerkt.';

/** "SF35" → "regime SF35", zodat de as herkenbaar blijft in grafiek en tabel. */
export function regimeMetSoort(label) {
  return label ? `regime ${label}` : label;
}

/** "a1: overstap…" → "variant a1: overstap…". */
export function variantMetSoort(label) {
  if (!label) return label;
  return /^(huidig recht|variant)/i.test(label) ? label : `variant ${label}`;
}

/**
 * Kostenraming per beleidsvariant uit de Stand van de Uitvoering OCW 2026.
 * Een andere grootheid dan de simulatie (kosten voor het Rijk volgens de
 * brief, tegenover wat de gesimuleerde populatie over haar looptijd betaalt),
 * dus bedoeld als orde-van-grootte-toets, niet als vergelijking.
 */
export const VARIANT_RAMING = {
  'a1-overstap-naar-sf15-nieuw': '€ 100–150 mln',
  'a2-automatische-overstap': '€ 400–500 mln',
  'a3-draagkrachtregels-sf15-nieuw-op-sf15-oud': '€ 550–650 mln',
  'b-aflosfase-maximeren': '€ 75–200 mln (opties samen)',
};

// ---- Cohortroutering ------------------------------------------------------
//
// Welk terugbetaalregime voor iemand geldt, kiest hij niet: het volgt uit zijn
// cohort. De wet doet die routering zelf (artikel 10a.1 en het overgangsrecht
// in hoofdstuk 12); deze functie is de leesbare kopie daarvan, voor schermen
// die het regime willen tonen zonder de engine aan te roepen. De grenzen staan
// als definities in de WSF 2000: cohortgrens_sf15_oud (2009),
// cohortgrens_sf15_nieuw_mbo (2023) en cohortgrens_sf15_nieuw_ho (2015).

/** De cohortgrenzen uit de WSF 2000, vereenvoudigd tot kalenderjaren. */
export const COHORTGRENS = { sf15_oud: 2009, mbo: 2023, ho: 2015 };

/**
 * Het regime dat volgt uit het cohort van een record.
 *
 * @param {object} record - met eerste_studiefinanciering, onderwijssoort en
 *   is_levenlanglerenkrediet
 * @returns {'SF15_OUD'|'SF15_NIEUW'|'SF15_LLLK'|'SF35'}
 */
export function regimeVanCohort(record) {
  if (record?.is_levenlanglerenkrediet) return 'SF15_LLLK';
  const jaar = Number(String(record?.eerste_studiefinanciering ?? '').slice(0, 4));
  if (!Number.isFinite(jaar) || jaar <= 0) return 'SF15_NIEUW';
  if (jaar < COHORTGRENS.sf15_oud) return 'SF15_OUD';
  const grens = record.onderwijssoort === 'mbo' ? COHORTGRENS.mbo : COHORTGRENS.ho;
  return jaar >= grens ? 'SF35' : 'SF15_NIEUW';
}

/** Waarom dit regime volgt, in één zin; voor het scenarioformulier. */
export function cohortUitleg(record) {
  if (record?.is_levenlanglerenkrediet) {
    return 'Een levenlanglerenkrediet valt onder eigen regels (artikel 6.19).';
  }
  const jaar = Number(String(record?.eerste_studiefinanciering ?? '').slice(0, 4));
  if (!Number.isFinite(jaar) || jaar <= 0) return '';
  if (jaar < COHORTGRENS.sf15_oud) {
    return `Je begon in ${jaar}, vóór ${COHORTGRENS.sf15_oud}. Daarom gelden de oudste regels.`;
  }
  const mbo = record.onderwijssoort === 'mbo';
  const grens = mbo ? COHORTGRENS.mbo : COHORTGRENS.ho;
  const opleiding = mbo ? 'mbo' : 'hbo of universiteit';
  return jaar >= grens
    ? `Je begon in ${jaar}, vanaf ${grens} voor ${opleiding}. Daarom gelden de nieuwste regels.`
    : `Je begon in ${jaar}, tussen ${COHORTGRENS.sf15_oud} en ${grens} voor ${opleiding}.`;
}
