/**
 * useSimulation - dunne cache-laag rond simulate() op de hoofdthread.
 *
 * De burger-view draait per persona tientallen simulaties (elke keuze-kaart
 * vergelijkt aan/uit). Dat is snel (~ms per run), maar we cachen per
 * (bsn, choices-key, wet-versie) zodat herteken-cycli niet dezelfde run
 * herhalen. De cache wordt gewist zodra de wet-versie (lawStore) verandert.
 */
import { simulate, inkomenInJaar, LAW_ID } from '../sim/simulate.js';
import { discountFactor } from '../sim/annuity.js';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';

const { getEngine } = useEngine();
const { version } = useLawStore();

const cache = new Map();
let cacheVersion = -1;

function choicesKey(choices) {
  return [
    choices.overstapAangevraagd ? 1 : 0,
    choices.draagkrachtAangevraagd ? 1 : 0,
    choices.peiljaarverlegging ? 1 : 0,
    choices.jokerMaanden ?? 0,
    choices.jokerVanafMaand ?? 0,
    choices.partnerMeetellen === false ? 0 : 1,
  ].join('|');
}

/** Standaardkeuzes voor een persona: partner telt mee, geen extra opties. */
export function defaultChoices(persona) {
  return {
    overstapAangevraagd: false,
    draagkrachtAangevraagd: false,
    peiljaarverlegging: false,
    jokerMaanden: 0,
    jokerVanafMaand: 0,
    partnerMeetellen: true,
    ...(persona?.keuzes ?? {}),
  };
}

/** Gecachte simulate(): key = bsn + keuzes + wet-versie. */
export function runSimulation(persona, choices, options = { startJaar: 2026 }) {
  const engine = getEngine();
  if (!engine) return null;
  if (version.value !== cacheVersion) {
    cache.clear();
    cacheVersion = version.value;
  }
  const key = `${persona.bsn}::${choicesKey(choices)}`;
  if (cache.has(key)) return cache.get(key);
  const result = simulate(engine, persona, choices, options);
  cache.set(key, result);
  return result;
}

/**
 * Voer één output uit met volledige trace, met dezelfde parameters als de
 * eerste jaarstap van simulate(). Levert het rauwe trace-resultaat (boom +
 * provenance) terug voor de trace-sheet.
 */
export function traceOutput(persona, choices, options = { startJaar: 2026 }) {
  const engine = getEngine();
  if (!engine) return null;
  const startJaar = options.startJaar ?? 2026;

  // Peiljaar t-2; exact hetzelfde inkomensmodel als simulate() bij de
  // eerste jaarstap (inclusief een eventuele recente inkomensdaling).
  const inkomenPeiljaar = inkomenInJaar(persona, startJaar, startJaar - 2);
  const inkomenActueel = inkomenInJaar(persona, startJaar, startJaar);
  const partnerInkomen = persona.heeft_partner ? Math.round(persona.partnerinkomen ?? 0) : 0;
  const eersteJaar = typeof persona.eerste_studiefinanciering_jaar === 'number'
    ? persona.eerste_studiefinanciering_jaar
    : Number(String(persona.eerste_studiefinanciering).slice(0, 4));

  engine.registerDataSource('personas', 'bsn', [
    {
      bsn: persona.bsn,
      geboortejaar: persona.geboortejaar,
      huishoudtype: persona.huishoudtype,
      heeft_partner: !!persona.heeft_partner,
      eerste_studiefinanciering_jaar: eersteJaar,
      onderwijssoort: persona.onderwijssoort,
      is_levenlanglerenkrediet: !!persona.is_levenlanglerenkrediet,
      toetsingsinkomen: inkomenPeiljaar,
      toetsingsinkomen_actueel: inkomenActueel,
      toetsingsinkomen_partner: partnerInkomen,
    },
  ]);

  // De trace moet exact het maandbedrag van het EERSTE jaar reproduceren dat
  // de burger-view als "nu" toont. simulate.js gebruikt daar de aflosperiode
  // uit de wet als resterende_maanden en een discontofactor die uit het
  // rentepercentage van dat jaar volgt. Leid die hier op dezelfde manier af,
  // in plaats van vaste aannames, anders wijkt de annuïteit (en dus het
  // getoonde maandbedrag) af.
  const choiceParams = {
    bsn: persona.bsn,
    restschuld: Math.round(persona.schuld),
    draagkracht_aangevraagd: choices.draagkrachtAangevraagd ?? false,
    partner_meetellen: choices.partnerMeetellen === false ? false : true,
    peiljaarverlegging_toegepast: !!choices.peiljaarverlegging,
    overstap_aangevraagd: choices.overstapAangevraagd ?? false,
  };
  const date = `${startJaar}-01-01`;

  // Stap 1: aflosperiode en rente ophalen (voorlopige maanden voor de eerste
  // engine-call, net als simulate.js).
  const voor = engine.executeMultiple(
    LAW_ID,
    ['terugbetaalperiode_maanden', 'rentepercentage'],
    { ...choiceParams, resterende_maanden: 180, discontofactor: 1 },
    date,
  ).outputs;
  const resterendeMaanden = Math.max(Math.round(voor.terugbetaalperiode_maanden), 1);
  const discontofactor = discountFactor(voor.rentepercentage, resterendeMaanden);

  // Stap 2: de trace met exact deze parameters, zodat de uitkomst gelijk is
  // aan timeline[0].maandbedrag.
  return engine.executeWithTrace(
    LAW_ID,
    'te_betalen_maandbedrag',
    { ...choiceParams, resterende_maanden: resterendeMaanden, discontofactor },
    date,
  );
}

export function useSimulation() {
  return { runSimulation, traceOutput, defaultChoices };
}
