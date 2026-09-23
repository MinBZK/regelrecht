/**
 * De Engelse persona's verschillen alléén in hun beschrijving.
 *
 * Dit is de test die ertoe doet, en niet de vraag of er Engels staat.
 * `profiles.yaml` is niet alleen schermtekst: het is de invoer van de wet. De
 * engine leest er zijn tabellen uit (`useDemoEngine.js`), de simulatie bouwt er
 * zijn populatie mee (`population.js`), en de materialisatie registreert de
 * brongegevens eruit (`materialize.js`). Een overlay die per ongeluk een
 * geboortedatum, een inkomen of een BSN raakt, laat de demo in het Engels iets
 * anders uitrekenen dan in het Nederlands.
 *
 * Dat zou niemand opvallen: er komt gewoon een ander bedrag uit, en er is geen
 * scherm dat de twee talen naast elkaar zet. Vandaar deze test, die de twee
 * documenten ontdoet van hun beschrijving en daarna eist dat er niets meer over
 * is om over te verschillen.
 */
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const dataDir = resolve(here, '..', '..', 'public', 'data');
const nlFile = join(dataDir, 'profiles.yaml');
const enFile = join(dataDir, 'profiles.en.yaml');

// De bestanden zijn het resultaat van `copy-demo-corpus.mjs`, dat in `predev` en
// `prebuild` draait. Wie de tests draait zonder ooit gebouwd te hebben, heeft ze
// niet; dan is er niets te toetsen en is overslaan eerlijker dan rood.
const built = existsSync(nlFile) && existsSync(enFile);

describe.skipIf(!built)('de Engelse persona\'s', () => {
  const nl = yaml.load(readFileSync(nlFile, 'utf8'));
  const en = yaml.load(readFileSync(enFile, 'utf8'));

  /** Hetzelfde document zonder de beschrijvingen: dit is wat gelijk moet zijn. */
  function withoutDescriptions(doc) {
    const copy = structuredClone(doc);
    for (const profile of Object.values(copy.profiles ?? {})) delete profile.description;
    return copy;
  }

  it('dragen precies dezelfde gegevens als het Nederlands', () => {
    expect(withoutDescriptions(en)).toEqual(withoutDescriptions(nl));
  });

  it('houden elke persona, op dezelfde sleutel', () => {
    expect(Object.keys(en.profiles)).toEqual(Object.keys(nl.profiles));
  });

  it('houden de BSN als tekst en niet als getal', () => {
    // Een BSN mag met een nul beginnen. Wordt de sleutel een getal, dan valt die
    // nul weg en vindt geen enkele opzoeking de persoon nog.
    for (const key of Object.keys(en.profiles)) expect(typeof key).toBe('string');
  });

  it('hebben overal een beschrijving, en nergens meer de Nederlandse', () => {
    for (const [bsn, profile] of Object.entries(en.profiles)) {
      expect(profile.description, `persona ${bsn} heeft geen Engelse beschrijving`).toBeTruthy();
      expect(profile.description, `persona ${bsn} staat nog in het Nederlands`).not.toBe(
        nl.profiles[bsn].description,
      );
    }
  });

  it('laten de namen staan', () => {
    // Een naam is een naam. Vertalen zou van `Merijn van der Meer` iets maken
    // waar de rest van de demo niet meer naar kan verwijzen.
    for (const [bsn, profile] of Object.entries(en.profiles)) {
      expect(profile.name).toBe(nl.profiles[bsn].name);
    }
  });
});
