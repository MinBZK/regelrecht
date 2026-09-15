import { describe, expect, it } from 'vitest';
import { cloneReceipt, receiptFixture } from '../testing/receiptFixture.js';
import { acceptedValues, loadedRegulations, receiptSections, receiptTimestamp } from './receipt.js';

// Het receipt lezen: ordenen en benoemen, nooit selecteren.

describe('het uitvoeringsreceipt lezen', () => {
  it('zet de secties in leesvolgorde en laat er geen weg', () => {
    const sections = receiptSections(receiptFixture);
    expect(sections.map((section) => section.key)).toStrictEqual([
      'gram',
      'provenance',
      'scope',
      'execution',
      'results',
      'engine_config',
    ]);
    // Elke sectie die het receipt draagt komt in beeld — dat is de belofte, en
    // daarom staat hier de lijst van het antwoord zelf en niet een tweede lijst.
    const own = ['accepted_values', 'timestamp'];
    const expected = Object.keys(receiptFixture).filter((key) => !own.includes(key));
    expect([...sections.map((section) => section.key)].sort()).toStrictEqual(expected.sort());
  });

  it('toont een sectie die deze module niet bij naam kent onder haar eigen naam', () => {
    // RFC-013 mag morgen iets toevoegen; dat hoort te verschijnen en niet stil
    // te ontbreken.
    const receipt = cloneReceipt();
    receipt.nieuwe_sectie = { iets: 'waarde' };
    const sections = receiptSections(receipt);
    const nieuw = sections.find((section) => section.key === 'nieuwe_sectie');
    expect(nieuw.label).toBe('Nieuwe sectie');
    expect(nieuw.rows).toStrictEqual([{ name: 'iets', value: 'waarde' }]);
  });

  it('vouwt een geneste sectie uit tot regels met een samengestelde naam', () => {
    const results = receiptSections(receiptFixture).find((section) => section.key === 'results');
    const names = results.rows.map((row) => row.name);
    expect(names).toContain('outputs · hoogte_zorgtoeslag');
    expect(names).toContain('output_provenance · heeft_recht_op_zorgtoeslag · article');
    expect(results.rows.find((row) => row.name === 'outputs · hoogte_zorgtoeslag').value).toBe(
      197178.01,
    );
  });

  it('laat niets weg uit een lijst waarin objecten en losse waarden door elkaar staan', () => {
    // De belofte van deze module is "niets weggelaten". Een lijst met een object
    // én een losse waarde erin zou zonder eigen regel die tweede stil kwijtraken.
    const receipt = cloneReceipt();
    receipt.nieuwe_sectie = { tags: [{ id: 1 }, 'extra', null] };
    const nieuw = receiptSections(receipt).find((section) => section.key === 'nieuwe_sectie');
    expect(nieuw.rows).toStrictEqual([
      { name: 'tags · 1 · id', value: 1 },
      { name: 'tags · 2', value: 'extra' },
      { name: 'tags · 3', value: null },
    ]);
  });

  it('geeft een leeg object in zo’n lijst een eigen regel in plaats van niets', () => {
    const receipt = cloneReceipt();
    receipt.nieuwe_sectie = { tags: [{ id: 1 }, {}] };
    const nieuw = receiptSections(receipt).find((section) => section.key === 'nieuwe_sectie');
    expect(nieuw.rows).toStrictEqual([
      { name: 'tags · 1 · id', value: 1 },
      { name: 'tags · 2', value: null },
    ]);
  });

  it('haalt de geladen regelingen uit `scope` en laat ze daar niet nog eens staan', () => {
    const loaded = loadedRegulations(receiptFixture);
    expect(loaded).toHaveLength(2);
    expect(loaded[0].id).toBe('wet_op_de_zorgtoeslag');
    expect(loaded[0].validFrom).toBe('2024-01-01');
    expect(loaded[0].hash).toMatch(/^sha256:/);
    // Een regeling zonder versie is er een zonder versie, en niet een lege tekst.
    expect(loaded[1].validFrom).toBeNull();

    const scope = receiptSections(receiptFixture).find((section) => section.key === 'scope');
    expect(scope.rows.every((row) => !row.name.startsWith('loaded_regulations'))).toBe(true);
  });

  it('geeft per geaccepteerde waarde de bron-cel én het bevoegd gezag van die bron', () => {
    const [accepted] = acceptedValues(receiptFixture);
    expect(accepted.output).toBe('toetsingsinkomen');
    expect(accepted.cell).toBe('belastingdienst');
    expect(accepted.authority).toBe('Belastingdienst');
    expect(accepted.opMoment).toBe('2024-03-01');
    expect(accepted.zaakkenmerk).toBe('zorgtoeslag/999993653');
  });

  it('zegt niets over een gezag dat de bron niet noemde', () => {
    // Een adres is geen gezag: zwijgt de bron, dan blijft het leeg in plaats van
    // dat het cel-id er onder een andere naam nog eens staat.
    const receipt = cloneReceipt();
    receipt.accepted_values[0].authority = null;
    expect(acceptedValues(receipt)[0].authority).toBeNull();
    expect(acceptedValues(receipt)[0].cell).toBe('belastingdienst');
  });

  it('geeft de tijdstempel met de toelichting van de server', () => {
    const timestamp = receiptTimestamp(receiptFixture);
    expect(timestamp.wallClock).toBe(receiptFixture.timestamp.wall_clock);
    expect(timestamp.note).toContain('wandkloktijd');
  });

  it('valt niet om over een antwoord dat er niet is', () => {
    expect(receiptSections(null)).toStrictEqual([]);
    expect(loadedRegulations(null)).toStrictEqual([]);
    expect(acceptedValues(null)).toStrictEqual([]);
    expect(receiptTimestamp(null)).toBeNull();
  });

  it('zet het gram met zijn moment in de logische tijd als eerste sectie', () => {
    // Naast de wandkloktijd van de uitvoering, en dat die twee náást elkaar te
    // lezen zijn is waarom de server ze uit elkaar houdt.
    const [gram] = receiptSections(receiptFixture);
    expect(gram.key).toBe('gram');
    expect(gram.rows).toContainEqual({ name: 'op_moment', value: receiptFixture.gram.op_moment });
    expect(gram.rows).toContainEqual({
      name: 'zaakkenmerk',
      value: receiptFixture.gram.zaakkenmerk,
    });
  });
});
