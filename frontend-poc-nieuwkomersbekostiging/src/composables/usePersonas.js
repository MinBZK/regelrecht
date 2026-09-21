/**
 * Laadt de persona's uit /data/personas.yaml (recordschema = datamodel §1)
 * en evalueert een persona over de peildatum-tijdlijn, per kolom (huidig
 * recht of variant) met de bijbehorende engine, inclusief de trace van één
 * peildatum voor de "dit komt letterlijk uit de regeling"-sheet.
 */
import { ref } from 'vue';
import * as yaml from 'js-yaml';
import { b } from '../basePad.js';
import { evaluateLeerlingTimeline, sectorVan } from '../sim/simulate.js';
import {
  LAW_ID_BY_SECTOR,
  PERSONA_PEILDATA_VAN,
  PERSONA_PEILDATA_TOT,
  peildataTussen,
} from '../lib/nieuwkomerFacts.js';

const personas = ref([]);
const loaded = ref(false);
const loadError = ref(null);
let loadPromise = null;

export const PERSONA_PEILDATA = peildataTussen(PERSONA_PEILDATA_VAN, PERSONA_PEILDATA_TOT);

async function fetchPersonas() {
  if (loadPromise) return loadPromise;
  loadPromise = (async () => {
    try {
      const res = await fetch(b('/data/personas.yaml'));
      if (!res.ok) throw new Error(`Ophalen mislukt (${res.status})`);
      const doc = yaml.load(await res.text());
      personas.value = doc?.personas ?? [];
      loaded.value = true;
    } catch (e) {
      loadError.value = e;
      throw e;
    }
    return personas.value;
  })();
  return loadPromise;
}

/** Tijdlijn van één persona over PERSONA_PEILDATA met de gegeven engine. */
export function personaTimeline(engine, persona, peildata = PERSONA_PEILDATA) {
  return evaluateLeerlingTimeline(engine, persona, peildata);
}

/**
 * Trace van één output op één peildatum. Het record wordt tijdelijk als
 * databron geregistreerd; scholen-context (voor schoolniveau-outputs) kan
 * via `scholen` meegegeven worden.
 */
export function personaTrace(engine, persona, peildatum, output = 'bedrag_kwartaal', scholen = null) {
  const lawId = LAW_ID_BY_SECTOR[sectorVan(persona)];
  engine.registerDataSource('personas', 'bsn', [persona]);
  if (scholen) engine.registerDataSource('scholen', 'school_id', scholen);
  try {
    return engine.executeWithTrace(lawId, output, { bsn: persona.bsn }, peildatum);
  } finally {
    engine.clearDataSources();
  }
}

export function usePersonas() {
  return { personas, loaded, loadError, fetchPersonas, personaTimeline, personaTrace };
}
