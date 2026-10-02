import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import * as yaml from 'js-yaml';
import { simulate } from '../src/sim/simulate.js';
import { createEngineWithLaws } from './helpers/nodeEngine.js';
import { dataDir } from './helpers/casusPaden.js';

const personasFile = resolve(dataDir, 'personas.yaml');
const { personas } = yaml.load(readFileSync(personasFile, 'utf-8'));
const byName = Object.fromEntries(personas.map((p) => [p.naam, p]));

const OPTIONS = { startJaar: 2026 };

describe('simulate', () => {
  it('Priya (SF15-oud) betaalt het wettelijke maandbedrag en is na 15 jaar klaar', async () => {
    const engine = await createEngineWithLaws();
    const { timeline, totals } = simulate(
      engine,
      byName.Priya,
      { draagkrachtAangevraagd: true, partnerMeetellen: true },
      OPTIONS,
    );
    expect(timeline[0].regime).toBe('SF15_OUD');
    expect(totals.levenslang).toBe(false);
    // Wettelijk maandbedrag (~163 euro) ligt onder haar draagkracht, dus de
    // schuld is binnen de aflosfase van 180 maanden volledig afgelost.
    expect(totals.maanden).toBeLessThanOrEqual(180);
    expect(totals.kwijtgescholden).toBe(0);
    // Conservatie: betaald = hoofdsom + rente (op afrondingsruis na).
    const verschil = Math.abs(
      totals.totaalBetaald - (totals.startSchuld + totals.totaalRente),
    );
    expect(verschil).toBeLessThan(2000);
  });

  it('Wouter (partneropt-out, draagkracht nihil) blijft levenslang debiteur', async () => {
    const engine = await createEngineWithLaws();
    const { timeline, totals } = simulate(
      engine,
      byName.Wouter,
      { draagkrachtAangevraagd: true, partnerMeetellen: false },
      OPTIONS,
    );
    expect(timeline[0].regime).toBe('SF15_OUD');
    expect(timeline[0].maandbedrag).toBe(0);
    expect(totals.levenslang).toBe(true);
    expect(totals.totaalBetaald).toBe(0);
    expect(totals.restschuldBijOverlijden).toBeGreaterThan(0);
  });

  it('Wouter mét partnerinkomen meegeteld lost af en krijgt kwijtschelding-einde', async () => {
    const engine = await createEngineWithLaws();
    const { totals } = simulate(
      engine,
      byName.Wouter,
      { draagkrachtAangevraagd: true, partnerMeetellen: true },
      OPTIONS,
    );
    expect(totals.levenslang).toBe(false);
    expect(totals.totaalBetaald).toBeGreaterThan(0);
  });

  it('Aisha (SF15-nieuw, laag inkomen) krijgt een fors deel kwijtgescholden', async () => {
    const engine = await createEngineWithLaws();
    const { timeline, totals } = simulate(engine, byName.Aisha, {}, OPTIONS);
    expect(timeline[0].regime).toBe('SF15_NIEUW');
    expect(totals.kwijtgescholden).toBeGreaterThan(0);
    expect(totals.maanden).toBe(180);
  });

  it('jokerjaren schuiven het einde op zonder de totale verplichting te vergroten dan met rente', async () => {
    const engine = await createEngineWithLaws();
    const zonder = simulate(engine, byName.Yusuf, {}, OPTIONS);
    const met = simulate(engine, byName.Yusuf, { jokerMaanden: 24 }, OPTIONS);
    expect(met.totals.maanden).toBe(zonder.totals.maanden + 24);
    // Tijdens de pauze loopt de rente door.
    expect(met.totals.totaalRente).toBeGreaterThan(zonder.totals.totaalRente);
  });

  it('peiljaarverlegging (Els, recente inkomensdaling) verlaagt het maandbedrag direct', async () => {
    const engine = await createEngineWithLaws();
    const zonder = simulate(engine, byName.Els, {}, OPTIONS);
    const met = simulate(engine, byName.Els, { peiljaarverlegging: true }, OPTIONS);
    // 2026: peiljaar t-2 = 2024, vóór haar daling (30.000 euro); actueel is
    // 21.000 euro = 70%, onder de 85%-grens van artikel 6.12. Verlegging
    // gebruikt het actuele inkomen en drukt de draagkrachttermijn.
    expect(met.timeline[0].maandbedrag).toBeLessThan(zonder.timeline[0].maandbedrag);
    // De keerzijde: wie nu minder betaalt, houdt een hogere restschuld over
    // en betaalt dus later meer, langer, of krijgt meer kwijtgescholden.
    expect(met.totals.totaalBetaald + met.totals.kwijtgescholden)
      .toBeGreaterThanOrEqual(zonder.totals.totaalBetaald + zonder.totals.kwijtgescholden);
    expect(met.totals.totaalRente).toBeGreaterThan(zonder.totals.totaalRente);
  });

  it('LLLK (Els) kan geen jokerjaren inzetten', async () => {
    const engine = await createEngineWithLaws();
    const zonder = simulate(engine, byName.Els, {}, OPTIONS);
    const met = simulate(engine, byName.Els, { jokerMaanden: 24 }, OPTIONS);
    expect(met.totals.maanden).toBe(zonder.totals.maanden);
  });
});
