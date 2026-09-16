import { describe, expect, it } from 'vitest';
import { cloneReceipt, receiptFixture } from '../testing/receiptFixture.js';
import {
  acceptedValues,
  loadedRegulations,
  receiptSections,
  receiptTimestamp,
  receiptTrace,
  resolveSource,
} from './receipt.js';

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
    expect(receiptTrace(null)).toBeNull();
  });

  it('haalt de trace uit `results` en laat haar daar niet nog eens staan', () => {
    // Uitgevouwen tot regels zou de boom tientallen regels opleveren met namen als
    // `trace · children · 1 · children · 0 · result`, en dan is precies de vorm weg
    // die haar leesbaar maakt.
    const results = receiptSections(receiptFixture).find((section) => section.key === 'results');
    expect(results.rows.every((row) => !row.name.startsWith('trace'))).toBe(true);
    expect(results.rows.some((row) => row.name.startsWith('outputs'))).toBe(true);
  });

  it('geeft de trace als boom, met per stap de regeling en het artikel', () => {
    const trace = receiptTrace(receiptFixture);
    expect(trace.nodeType).toBe('article');
    expect(trace.name).toContain('wet_op_de_zorgtoeslag');

    // De sleutel is het pad in de boom: twee zusjes kunnen dezelfde naam en
    // dezelfde uitkomst hebben, dus de plek is het enige wat ze onderscheidt.
    expect(trace.path).toBe('0');
    expect(trace.children.map((child) => child.path)).toStrictEqual(['0.0', '0.1']);

    const rekenend = trace.children[1];
    expect(rekenend.nodeType).toBe('action');
    expect(rekenend.regulation).toBe('wet_op_de_zorgtoeslag');
    expect(rekenend.article).toBe('3');
    expect(rekenend.result).toBe(197178.01);
    expect(rekenend.hasResult).toBe(true);

    // En de stap eronder heeft geen eigen artikel: die hoort bij het artikel
    // erboven, en er wordt er geen bij verzonnen.
    const bewerking = rekenend.children[0];
    expect(bewerking.nodeType).toBe('operation');
    expect(bewerking.article).toBeNull();
  });

  it('houdt een stap zonder uitkomst apart van een stap die niets opleverde', () => {
    const receipt = cloneReceipt();
    receipt.results.trace.children[0].result = null;
    delete receipt.results.trace.children[1].result;

    const trace = receiptTrace(receipt);
    expect(trace.children[0].hasResult).toBe(true);
    expect(trace.children[0].result).toBeNull();
    expect(trace.children[1].hasResult).toBe(false);
  });

  it('zegt in gewone woorden waar een waarde vandaan kwam', () => {
    expect(resolveSource('DATA_SOURCE')).toBe('een databron');
    expect(resolveSource(null)).toBeNull();
    // Een soort die deze lijst niet kent, blijft leesbaar in plaats van weg te
    // vallen: beter een technische naam dan een stilte.
    expect(resolveSource('IETS_NIEUWS')).toBe('IETS_NIEUWS');
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
