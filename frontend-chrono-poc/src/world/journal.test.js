import { describe, expect, it } from 'vitest';
import { worldFixture } from '../testing/worldFixture.js';
import {
  askingCell,
  describeAccepted,
  describeAnswer,
  describeAnswerMoment,
  describeChange,
  describeExecutedRegulation,
  describeStand,
  isNewEntry,
  journalActor,
  journalCells,
  journalEntries,
  journalGrams,
  journalKind,
  journalRows,
  readExecution,
} from './journal.js';

// Het journaal zoals de simulator het geeft, gelezen zoals de weergave het nodig
// heeft. Op de fixture van de simulator zelf, want dat is het contract.

const besluit = worldFixture.journal.find((entry) => entry.kind === 'besluit');
const vraag = worldFixture.journal.find((entry) => entry.kind === 'vraag');
const betaling = worldFixture.journal.find((entry) => entry.kind === 'betaling');

describe('het journaal lezen', () => {
  it('geeft de regels terug zoals het beeld ze geeft', () => {
    expect(journalEntries(worldFixture)).toBe(worldFixture.journal);
    expect(journalEntries({})).toStrictEqual([]);
    expect(journalEntries(null)).toStrictEqual([]);
  });

  it('houdt de drie soorten actor uit elkaar', () => {
    expect(journalActor(worldFixture.journal[0])).toStrictEqual({
      id: 'burger',
      label: 'burger',
      icon: 'person',
    });
    expect(journalActor(vraag).icon).toBe('building');
    expect(journalActor(betaling)).toStrictEqual({ id: 'klok', label: 'de klok', icon: 'clock' });
    // Een soort die deze app niet kent, is de klok noch een cel; hij hoort
    // leesbaar te blijven en niet op iemands naam te komen.
    expect(journalActor({ actor: { soort: 'iets-nieuws' } }).id).toBe('klok');
  });

  it('geeft elke soort gebeurtenis een eigen kleur en icoon', () => {
    expect(journalKind('besluit').color).toBe('donkerblauw');
    expect(journalKind('betaling').icon).toBe('euro-sign');
    expect(journalKind('iets-nieuws').label).toBe('Gebeurtenis');
  });

  it('noemt bij een vraag beide kanten van de grens', () => {
    const cells = journalCells(vraag);
    expect(cells).toContain('belastingdienst');
    expect(cells).toContain('toeslagen');
    expect(askingCell(vraag.question)).toBe('toeslagen');
    expect(askingCell({ asked_by: 'toeslagen' })).toBe('toeslagen');
    expect(askingCell({})).toBeNull();
  });

  it('noemt bij een gebeurtenis elke cel waar een gram landde of iets veranderde', () => {
    const cells = journalCells(betaling);
    expect(cells).toContain('belastingdienst');
    expect(cells).toContain('toeslagen');
  });

  it('markeert alleen wat er sinds de vorige stap bij kwam', () => {
    const laatste = worldFixture.journal.at(-1);
    expect(isNewEntry(worldFixture.journal.length - 1, laatste)).toBe(true);
    expect(isNewEntry(worldFixture.journal.length, laatste)).toBe(false);
    // Zonder vorige stand is er niets nieuw: een opgehaald beeld is de stand en
    // geen gebeurtenis.
    expect(isNewEntry(null, laatste)).toBe(false);
  });

  it('filtert op actor, op cel en op dag, en laat de rest staan', () => {
    const alle = journalRows(worldFixture);
    expect(alle).toHaveLength(worldFixture.journal.length);
    expect(alle.every((row) => row.matches)).toBe(true);

    const vanDeKlok = journalRows(worldFixture, { actor: 'klok' }).filter((row) => row.matches);
    expect(vanDeKlok.length).toBe(worldFixture.journal.filter((entry) => entry.actor.soort === 'klok').length);

    const opDag = journalRows(worldFixture, { moment: besluit.moment }).filter((row) => row.matches);
    expect(opDag.length).toBe(worldFixture.journal.filter((entry) => entry.moment === besluit.moment).length);

    // Gefilterd is niet weg: elke regel blijft in de lijst staan.
    expect(journalRows(worldFixture, { actor: 'klok' })).toHaveLength(worldFixture.journal.length);
  });

  it('springt een vraag in en laat de rest op de eerste laag staan', () => {
    const rows = journalRows(worldFixture);
    expect(rows[vraag.seq].indented).toBe(true);
    expect(rows[besluit.seq].indented).toBe(false);
  });

  it('schrijft een stand uit zonder er iets bij te verzinnen', () => {
    expect(describeStand(null)).toBe('niets vastgesteld');
    expect(describeStand({})).toBe('niets vastgesteld');
    expect(describeStand({ bedrag: 25000 })).toBe('25000');
    expect(describeStand({ recht: true, bedrag: 25000 })).toBe('recht: ja · bedrag: 25000');
  });

  it('schrijft een verandering als was → is', () => {
    expect(describeChange(besluit.changes[0])).toContain('niets vastgesteld →');
    expect(describeChange({ voor: { bedrag: 1 }, na: { bedrag: 2 } })).toBe('1 → 2');
  });

  it('zegt van een geaccepteerde waarde bij wie ze vandaan komt', () => {
    const regel = describeAccepted(besluit.accepted[0]);
    expect(regel).toContain('accepteerde toetsingsinkomen');
    expect(regel).toContain('van belastingdienst');
    expect(regel).toContain('vastgesteld');
  });

  // Het kenmerk komt uit het gram waarnaar de regel wijst: het journaal draagt
  // alleen een verwijzing, en houdt geen kopie van wat er in een kroniek ligt.
  it('zet bij een besluit de zaak waar het over ging', () => {
    const [gram] = journalGrams(worldFixture, besluit);
    expect(gram.zaak).toBe('zaak zorgtoeslag/999993653');
    expect(gram.id).toBe(besluit.grams[0].id);
  });

  it('laat de zaak leeg bij een gram dat er geen draagt', () => {
    const grams = journalGrams(worldFixture, { grams: [{ id: 'burger|aanvragen|0', name: 'x' }] });
    expect(grams[0].zaak).toBe('');
    expect(journalGrams(worldFixture, null)).toStrictEqual([]);
  });

  it('geeft het antwoord op een cross-cel-vraag terug, ook als het er geen is', () => {
    expect(describeAnswer(vraag.question)).toContain('81000');
    expect(
      describeAnswer({ answer: { outcome: { not_established: { reason: 'geen aanslag' } } } }),
    ).toBe('geen aanslag');
    expect(describeAnswer({})).toBe('niets vastgesteld');
  });

  it('zegt van wanneer het antwoord is, in de notatie van het scherm', () => {
    expect(describeAnswerMoment(vraag.question)).toBe(
      `geldig op ${vraag.question.answer.op_moment.split('-').reverse().join('-')}`,
    );
    expect(describeAnswerMoment({})).toBe('');
  });

  it('leest bij een besluit wat het uitvoerde: het recht, de inputs en de uitkomsten', () => {
    const executed = readExecution(besluit, worldFixture.journal);

    // Meer dan de regeling waarop het besluit gaat: wat zij aanriep hoort er even
    // goed bij, want zonder dat is niet te zien onder welk recht dit tot stand kwam.
    expect(executed.regulations.length).toBeGreaterThan(1);
    expect(executed.zin).toContain(besluit.executed.regulations[0].regulation);
    // De versie in de notatie van het scherm, en geen verzonnen datum waar er
    // geen versiedatum is.
    expect(executed.zin).toContain('01-01-2024');
    expect(describeExecutedRegulation({ regulation: 'een_regeling' })).toBe('een_regeling');

    expect(executed.inputs.map((input) => input.name)).toStrictEqual(
      besluit.executed.inputs.map((input) => input.name),
    );
    expect(executed.outputs.map((output) => output.name)).toStrictEqual(
      besluit.executed.outputs.map((output) => output.name),
    );

    // Elke input draagt haar herkomst in de woorden van het gram zelf.
    const geaccepteerd = executed.inputs.find((input) => input.origin.kind === 'geaccepteerd');
    expect(geaccepteerd.origin.label).toContain('belastingdienst');
    // En wijst naar de vraag-regel waarlangs ze binnenkwam.
    expect(geaccepteerd.question).toBe(`journaal-${vraag.seq}`);
    // Een input die de cel zelf vaststelde heeft die weg niet: er is geen vraag.
    const eigen = executed.inputs.find((input) => input.origin.kind !== 'geaccepteerd');
    expect(eigen.question).toBeNull();
  });

  it('laat een regel die niets uitvoerde zonder uitvoering', () => {
    expect(readExecution(worldFixture.journal[0], worldFixture.journal)).toBeNull();
    expect(readExecution(vraag, worldFixture.journal)).toBeNull();
    expect(readExecution({}, [])).toBeNull();

    const rows = journalRows(worldFixture);
    expect(rows[besluit.seq].executed).not.toBeNull();
    expect(rows[vraag.seq].executed).toBeNull();
  });
});
