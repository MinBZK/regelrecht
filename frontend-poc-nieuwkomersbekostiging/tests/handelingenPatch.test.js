/**
 * Het uitvoeringslastmodel bewerken zoals de beleidsassistent dat doet.
 *
 * De aanleiding voor deze module staat in de laatste test: de instructie "in
 * het primair onderwijs hoeft er geen accountant aan te pas te komen" raakt
 * geen regelgeving maar handelingen.yaml, en liep daarom dood op "de assistent
 * heeft niets gewijzigd". De test toetst het langs de echte variant-YAML en de
 * echte aggregatie, zodat hij ook omvalt als de simulatie de handeling anders
 * gaat tellen.
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import yaml from 'js-yaml';
import { aggregate } from '../src/sim/metrics.js';
import {
  addHandeling,
  listHandelingen,
  listTarieven,
  patchHandeling,
  patchTarief,
  removeHandeling,
} from '../src/lib/handelingenPatch.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const casusDir = resolve(__dirname, '..', '..', 'corpus-poc', 'nieuwkomersbekostiging');

function leesBasis() {
  return readFileSync(resolve(casusDir, 'data', 'handelingen.yaml'), 'utf-8');
}

function leesVariant(id) {
  return readFileSync(resolve(casusDir, 'varianten', id, 'data', 'handelingen.yaml'), 'utf-8');
}

/**
 * Kleinste simulatie waarin een po- en een vo-school in 2028 allebei een
 * nieuwkomer hebben. 2028 omdat de po-accountant in de variant `vanaf_jaar:
 * 2028` draagt; in een eerder jaar telt hij niet en zou de test niets bewijzen.
 */
function simulatieMetPoEnVoSchool() {
  const school = (id, sector) => ({
    school_id: id,
    sector,
    gewicht: 1,
    leerlinggewicht: 1,
    perPeildatum: [
      { peildatum: '2028-10-01', jaar: 2028, in_bestand: 1, tellend: 1, ambigu: 0, aanvraag: true, bedrag_totaal: 0 },
    ],
  });
  return {
    jaren: [2028],
    scholen: [school('po-1', 'po'), school('vo-1', 'vo')],
    leerlingen: [],
  };
}

describe('handelingen lezen', () => {
  it('geeft de handelingen met hun velden terug', () => {
    const lijst = listHandelingen(leesBasis());
    expect(lijst.length).toBeGreaterThan(5);
    const accountant = lijst.find((h) => h.id === 'accountantsvalidatie_nieuwkomers');
    expect(accountant).toMatchObject({ partij: 'school', sector: 'vo', tarief: 'accountant' });
  });

  it('geeft de tarieven terug', () => {
    expect(listTarieven(leesBasis())).toHaveProperty('accountant');
  });
});

describe('een handeling wijzigen', () => {
  it('past minuten aan en meldt wat er veranderde', () => {
    const { yaml: nieuw, veranderd } = patchHandeling(leesBasis(), 'accountantsvalidatie_nieuwkomers', { minuten: 45 });
    expect(veranderd).toEqual(['minuten: 90 -> 45']);
    const na = listHandelingen(nieuw).find((h) => h.id === 'accountantsvalidatie_nieuwkomers');
    expect(na.minuten).toBe(45);
    // De rest van het model blijft staan.
    expect(listHandelingen(nieuw).length).toBe(listHandelingen(leesBasis()).length);
  });

  it('haalt een veld weg bij een waarde van null', () => {
    const { yaml: nieuw } = patchHandeling(leesBasis(), 'accountantsvalidatie_nieuwkomers', { sector: null });
    expect(listHandelingen(nieuw).find((h) => h.id === 'accountantsvalidatie_nieuwkomers').sector).toBeNull();
  });

  it('weigert een onbekende aanleiding, want de simulatie telt hem niet', () => {
    expect(() => patchHandeling(leesBasis(), 'accountantsvalidatie_nieuwkomers', { aanleiding: 'per_maand' }))
      .toThrow(/Onbekende aanleiding/);
  });

  it('weigert een tarief dat niet bestaat', () => {
    expect(() => patchHandeling(leesBasis(), 'accountantsvalidatie_nieuwkomers', { tarief: 'notaris' }))
      .toThrow(/Onbekend tarief/);
  });

  it('meldt een onbekende handeling in plaats van stil niets te doen', () => {
    expect(() => patchHandeling(leesBasis(), 'bestaat_niet', { minuten: 10 })).toThrow(/bestaat niet/);
  });
});

describe('een handeling toevoegen en verwijderen', () => {
  it('voegt een handeling toe die de simulatie kan tellen', () => {
    const { yaml: nieuw } = addHandeling(leesBasis(), {
      id: 'steekproef_ocw',
      omschrijving: 'OCW trekt jaarlijks een steekproef door de toekenningen.',
      partij: 'ocw',
      aanleiding: 'per_school_jaar',
      minuten: 20,
      tarief: 'ocw_beleidsmedewerker',
    });
    const toegevoegd = listHandelingen(nieuw).find((h) => h.id === 'steekproef_ocw');
    expect(toegevoegd).toMatchObject({ partij: 'ocw', minuten: 20, sector: null });
  });

  it('weigert een dubbel id', () => {
    expect(() => addHandeling(leesBasis(), {
      id: 'accountantsvalidatie_nieuwkomers',
      partij: 'school',
      aanleiding: 'per_school_jaar',
      minuten: 10,
      tarief: 'accountant',
    })).toThrow(/bestaat al/);
  });

  it('verwijdert een handeling en geeft terug welke', () => {
    const { yaml: nieuw, verwijderd } = removeHandeling(leesBasis(), 'accountantsvalidatie_nieuwkomers');
    expect(verwijderd.id).toBe('accountantsvalidatie_nieuwkomers');
    expect(listHandelingen(nieuw).some((h) => h.id === 'accountantsvalidatie_nieuwkomers')).toBe(false);
  });
});

describe('een tarief wijzigen', () => {
  it('past het tarief aan en meldt oud en nieuw', () => {
    const { yaml: nieuw, oud, nieuw: na } = patchTarief(leesBasis(), 'accountant', 12000);
    expect([oud, na]).toEqual([15000, 12000]);
    expect(listTarieven(nieuw).accountant).toBe(12000);
  });
});

// De acceptatietest: "geen accountant in het po".
describe('geen accountant in het primair onderwijs', () => {
  const variant = 'nk-3-harmonisatie-po-vo';

  it('de variant voert een po-accountant op', () => {
    const po = listHandelingen(leesVariant(variant)).find((h) => h.id === 'accountantsvalidatie_nieuwkomers_po');
    expect(po).toMatchObject({ partij: 'school', sector: 'po', tarief: 'accountant' });
  });

  it('verwijdert die handeling en laat de vo-accountant staan', () => {
    const { yaml: nieuw } = removeHandeling(leesVariant(variant), 'accountantsvalidatie_nieuwkomers_po');
    const na = listHandelingen(nieuw);
    expect(na.some((h) => h.id === 'accountantsvalidatie_nieuwkomers_po')).toBe(false);
    // De accountant in het vo is een andere handeling en blijft.
    expect(na.find((h) => h.id === 'accountantsvalidatie_nieuwkomers')).toMatchObject({ sector: 'vo' });
  });

  it('bestaat niet in de basis, dus de werkversie moet meegestuurd worden', () => {
    // Waarom dit ertoe doet: de assistent las eerst altijd de basis. Een
    // instructie over deze handeling liep dan dood op "bestaat niet", ook al
    // stond hij op het scherm. De backend krijgt het model van de kolom mee.
    expect(listHandelingen(leesBasis()).some((h) => h.id === 'accountantsvalidatie_nieuwkomers_po')).toBe(false);
    expect(listHandelingen(leesVariant(variant)).some((h) => h.id === 'accountantsvalidatie_nieuwkomers_po')).toBe(true);
  });

  it('haalt daarmee de accountantskosten uit de po-uitvoeringslast', () => {
    // Door de echte aggregatie heen, niet alleen over de YAML: de handeling
    // telt via `aanleiding` en `sector`, en een test die alleen het veld leest
    // zou het blijven doen als de simulatie hem anders ging tellen.
    const sim = simulatieMetPoEnVoSchool();
    const kosten = (doc) => {
      const m = aggregate(sim, doc, {}).totaal;
      return {
        po: m.handelingen.accountantsvalidatie_nieuwkomers_po?.kosten ?? 0,
        vo: m.handelingen.accountantsvalidatie_nieuwkomers?.kosten ?? 0,
        school: m.uitvoeringslast.school,
      };
    };

    const voor = kosten(yaml.load(leesVariant(variant)));
    const { yaml: naYaml } = removeHandeling(leesVariant(variant), 'accountantsvalidatie_nieuwkomers_po');
    const na = kosten(yaml.load(naYaml));

    expect(voor.po).toBeGreaterThan(0);
    expect(na.po).toBe(0);
    // De accountant in het vo blijft betaald worden.
    expect(na.vo).toBe(voor.vo);
    // En de school is precies de po-accountant goedkoper uit.
    expect(na.school).toBeCloseTo(voor.school - voor.po, 6);
  });
});
