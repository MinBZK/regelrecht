// Bewijst dat een variant-corpus andere wetten oplevert dan de basis, en dat
// de engine daar ook anders van rekent.
//
// Waarom dit er moet zijn: de beleidsmakersview zet "nu" en een variant naast
// elkaar, elk met een eigen engine en een eigen set wetsteksten. Gaat die
// koppeling stuk, dan tonen beide kolommen hetzelfde, en dat leest als "deze
// maatregel doet niets" in plaats van "de maatregel wordt niet toegepast".
// Geen enkele andere test laadde een variant-corpus, dus die stilte was niet
// te onderscheiden van een kloppende uitkomst.
//
// De keuze voor a2: die variant verandert het regime zelf en daarmee het
// maandbedrag van een SF15-oud-debiteur. Variant b (de aflosfase-cap) leent
// zich er niet voor: die bijt alleen na een partneropt-out en pas boven 480
// maanden, en geen van de vaste persona's komt daar — b en "nu" horen voor
// deze persona's gelijk te zijn.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { createEngineWithLaws, lawsDir } from './helpers/nodeEngine.js';
import { discountFactor } from '../src/sim/annuity.js';

const LAW_ID = 'wet_studiefinanciering_2000';
const publicDir = resolve(lawsDir, '..');
const manifest = JSON.parse(readFileSync(resolve(lawsDir, 'variants.json'), 'utf-8'));

/** Engine met de basiswetten, waarvan de bestanden van één variant de plaats innemen. */
async function engineVoorVariant(variantId) {
  const variant = manifest.find((v) => v.id === variantId);
  expect(variant, `variant ${variantId} staat niet in variants.json`).toBeTruthy();

  const engine = await createEngineWithLaws();
  for (const bestand of variant.files) {
    // Dezelfde $id, dus dit vervangt de basisversie in plaats van ernaast te
    // komen staan — hetzelfde wat lawStore in de app doet.
    engine.loadLaw(readFileSync(resolve(publicDir, bestand.path), 'utf-8'));
  }
  return engine;
}

// Kwame uit personas.yaml: SF15-oud, schuld 30.000, inkomen 32.000. Onder a2
// stapt hij automatisch over naar SF15-nieuw, met andere draagkrachtregels.
const KWAME = {
  bsn: '100000003',
  geboortejaar: 1988,
  huishoudtype: 'alleenstaand',
  heeft_partner: false,
  eerste_studiefinanciering_jaar: 2008,
  onderwijssoort: 'hbo',
  is_levenlanglerenkrediet: false,
  toetsingsinkomen: 3200000,
  toetsingsinkomen_actueel: 3200000,
  toetsingsinkomen_partner: 0,
};

/** Het maandbedrag van één debiteur, met de keuzes die de scenariotabel gebruikt. */
function maandbedrag(engine, persona) {
  // De persoonsgegevens komen via de databron, niet als parameters — net als
  // in useSimulation.js. Zonder deze regel rekent de engine nergens aan en
  // geeft hij aan beide kanten `unknown`, wat net zo gelijk is.
  engine.registerDataSource('personas', 'bsn', [persona]);

  const basis = {
    bsn: persona.bsn,
    restschuld: 3000000,
    draagkracht_aangevraagd: false,
    partner_meetellen: true,
    peiljaarverlegging_toegepast: false,
    overstap_aangevraagd: false,
  };
  const voor = engine.executeMultiple(
    LAW_ID,
    ['terugbetaalperiode_maanden', 'rentepercentage'],
    { ...basis, resterende_maanden: 180, discontofactor: 1 },
    '2026-01-01',
  ).outputs;
  const resterende = Math.max(Math.round(voor.terugbetaalperiode_maanden), 1);
  // Dezelfde afleiding als useSimulation.js: een vaste discontofactor van 1
  // laat de annuiteit door nul delen.
  const discontofactor = discountFactor(voor.rentepercentage, resterende);
  return engine.executeMultiple(
    LAW_ID,
    ['te_betalen_maandbedrag'],
    { ...basis, resterende_maanden: resterende, discontofactor },
    '2026-01-01',
  ).outputs.te_betalen_maandbedrag;
}

describe('variant-corpora', () => {
  it('elke variant in het manifest vervangt een bestand en wijkt daarvan af', () => {
    expect(manifest.length).toBeGreaterThan(0);
    for (const variant of manifest) {
      expect(variant.files.length, `${variant.id} vervangt geen enkel bestand`).toBeGreaterThan(0);
      for (const bestand of variant.files) {
        const varianttekst = readFileSync(resolve(publicDir, bestand.path), 'utf-8');
        const basistekst = readFileSync(resolve(publicDir, bestand.base), 'utf-8');
        expect(
          varianttekst,
          `${variant.id} is identiek aan de basis; dan verandert die kolom nooit iets`,
        ).not.toBe(basistekst);
      }
    }
  });

  it('a2 rekent een SF15-oud-debiteur anders dan huidig recht', async () => {
    const nu = maandbedrag(await createEngineWithLaws(), KWAME);
    const a2 = maandbedrag(await engineVoorVariant('a2-automatische-overstap'), KWAME);

    // Een uitkomst die de engine niet kón bepalen is aan beide kanten gelijk
    // en zou dit vergelijk betekenisloos maken.
    for (const [naam, waarde] of [['huidig recht', nu], ['a2', a2]]) {
      expect(
        typeof waarde,
        `${naam} levert geen getal op (${JSON.stringify(waarde)}); de invoer bereikt de wet niet`,
      ).toBe('number');
    }

    expect(
      a2,
      'a2 geeft hetzelfde maandbedrag als huidig recht; dan wordt de variant niet toegepast',
    ).not.toBe(nu);
  });
});
