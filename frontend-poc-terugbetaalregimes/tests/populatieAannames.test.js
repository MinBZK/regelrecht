import { describe, it, expect, beforeEach } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import * as yaml from 'js-yaml';
import { usePopulatieAannames } from '../src/composables/usePopulatieAannames.js';
import { generatePopulation } from '../src/sim/population.js';
import { dataDir } from './helpers/casusPaden.js';

const DIST = yaml.load(
  readFileSync(resolve(dataDir, 'distributions.yaml'), 'utf-8'),
);

const {
  basis, effectief, versie, hasOverrides, regimeVerdeling, totaalDebiteuren,
  setTotaalDebiteuren, setRegimeAandeel, setDraagkrachtPrior, setHuishouden,
  setGedrag, setInkomensgroei, resetOverrides,
} = usePopulatieAannames();

describe('populatie-aannames', () => {
  beforeEach(() => {
    basis.value = DIST;
    resetOverrides();
  });

  it('geeft zonder overrides de basis terug', () => {
    expect(hasOverrides.value).toBe(false);
    expect(effectief.value.cohorten).toEqual(DIST.cohorten);
    expect(effectief.value.regimes).toEqual(DIST.regimes);
    expect(effectief.value.huishoudtypen).toEqual(DIST.huishoudtypen);
  });

  it('past het aantal debiteuren aan zonder de basis te raken', () => {
    setTotaalDebiteuren(500000);
    expect(totaalDebiteuren.value).toBe(500000);
    expect(basis.value.cohorten.totaal_debiteuren).toBe(DIST.cohorten.totaal_debiteuren);
    expect(hasOverrides.value).toBe(true);
  });

  it('normaliseert de regimeverdeling zoals de generator dat doet', () => {
    setRegimeAandeel('SF35', 0.5);
    const sf35 = regimeVerdeling.value.find((r) => r.regime === 'SF35');
    expect(sf35.waarde).toBe(0.5);
    // De vier tellen nu op tot meer dan 1; het getoonde aandeel is genormaliseerd.
    const som = regimeVerdeling.value.reduce((s, r) => s + r.waarde, 0);
    expect(sf35.aandeel).toBeCloseTo(0.5 / som, 6);
  });

  it('zet de draagkrachtmeting-prior per regime', () => {
    setDraagkrachtPrior('SF15_OUD', 0.9);
    expect(effectief.value.regimes.SF15_OUD.draagkrachtmeting_prior).toBe(0.9);
    // De andere regimes blijven ongemoeid.
    expect(effectief.value.regimes.SF35.draagkrachtmeting_prior)
      .toBe(DIST.regimes.SF35.draagkrachtmeting_prior);
  });

  it('bumpt de versie en levert een andere populatie bij dezelfde seed', () => {
    const voor = versie.value;
    const populatieVoor = generatePopulation(effectief.value, 200, 42);
    setRegimeAandeel('SF15_OUD', 0.9);
    expect(versie.value).toBeGreaterThan(voor);
    const populatieNa = generatePopulation(effectief.value, 200, 42);
    expect(populatieNa).not.toEqual(populatieVoor);
    expect(populatieNa).toHaveLength(200);
  });

  it('begrenst fracties en herstelt naar de basis', () => {
    setGedrag('partner_opt_out_prior', 1.4);
    expect(effectief.value.gedrag.partner_opt_out_prior).toBe(1);
    setHuishouden('paar', -0.3);
    expect(effectief.value.huishoudtypen.paar).toBe(0);
    setInkomensgroei(0.05);
    expect(effectief.value.inkomen.groei_per_jaar).toBe(0.05);

    resetOverrides();
    expect(hasOverrides.value).toBe(false);
    expect(effectief.value.gedrag.partner_opt_out_prior).toBe(DIST.gedrag.partner_opt_out_prior);
    expect(effectief.value.inkomen.groei_per_jaar).toBe(DIST.inkomen.groei_per_jaar);
  });
});
