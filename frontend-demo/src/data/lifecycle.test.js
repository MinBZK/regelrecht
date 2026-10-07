/**
 * De regels waarmee de demo een zaak aan de Awb-levensloop ophangt (RFC-008).
 *
 * Wat hier bewezen wordt is de verhouding tussen fase en uitkomst: de fase zegt
 * wáár een besluit is, `approved` zegt wát het inhoudt, en die twee mogen niet
 * door elkaar lopen. En dat bezwaar aan de bekendmaking hangt en niet aan het
 * besluit, want daar zat het verschil dat de demo eerder niet liet zien.
 */
import { describe, expect, it } from 'vitest';
import { announced, awbOutcomes, carriedInputs, decisionDates, objectionOpen, procedureStages, reachedStage, statusOf } from './lifecycle.js';

const atStage = (stage, extra = {}) => ({
  stageState: { current_stage: stage, accumulated_outputs: {} },
  ...extra,
});

describe('statusOf', () => {
  it('leidt de status af uit de fase', () => {
    expect(statusOf(atStage('AANVRAAG'))).toBe('SUBMITTED');
    expect(statusOf(atStage('BEHANDELING'))).toBe('IN_REVIEW');
    expect(statusOf(atStage('BEKENDMAKING'))).toBe('DECIDED');
    expect(statusOf(atStage('BEZWAAR'))).toBe('DECIDED');
  });

  it('leest een zaak die op BESLUIT wacht niet als besloten', () => {
    // `current_stage` is de fase waar de levensloop vóór staat, niet de fase
    // die hij heeft afgerond. Een zaak op BESLUIT wacht nog op de
    // besluitdatum: de haken van 3:46 en 6:7 hebben niet gevuurd, er is geen
    // besluit. Dit verkeerd lezen zette elke aanvraag die naar een behandelaar
    // gaat meteen als "Afgewezen" op het portaal, omdat `approved` dan null is.
    expect(statusOf(atStage('BESLUIT'))).toBe('IN_REVIEW');
    expect(statusOf(atStage('BESLUIT', { approved: null, decidedAt: null }))).toBe('IN_REVIEW');
  });

  it('maakt van afwijzen geen aparte fase', () => {
    // Toekennen en afwijzen zijn twee uitkomsten van hetzelfde moment; wat het
    // besluit inhoudt staat in `approved`, niet in de fase.
    expect(statusOf(atStage('BEKENDMAKING', { approved: true }))).toBe('DECIDED');
    expect(statusOf(atStage('BEKENDMAKING', { approved: false }))).toBe('DECIDED');
  });

  it('houdt een zaak zonder levensloop bij haar eigen status', () => {
    // Een wet die geen beschikking geeft, of een zaak uit opgeslagen staat van
    // vóór deze verandering: die hoort gewoon te blijven werken.
    expect(statusOf({ status: 'IN_REVIEW' })).toBe('IN_REVIEW');
    expect(statusOf({ status: 'DECIDED' })).toBe('DECIDED');
    expect(statusOf({})).toBe('IN_REVIEW');
  });

  it('kent een ingetrokken zaak', () => {
    expect(statusOf({ ...atStage('BEZWAAR'), withdrawnAt: '2026-03-12T10:00:00Z' })).toBe('WITHDRAWN');
  });
});

describe('een procedure met eigen fasen', () => {
  // De procedure van de Awir: twee besluiten, elk een fase die een BESLUIT `is`.
  const awir = {
    $id: 'awir',
    procedure: [
      {
        id: 'tegemoetkoming',
        stages: [
          { name: 'AANVRAAG' },
          { name: 'VOORSCHOT', is: 'BESLUIT', requires: [{ name: 'dagtekening_voorschot', type: 'date' }] },
          { name: 'VOORSCHOT_BEKENDMAKING', is: 'BEKENDMAKING', requires: [{ name: 'bekendmaking_datum', type: 'date' }] },
          { name: 'TOEKENNING', is: 'BESLUIT', requires: [{ name: 'dagtekening_toekenning', type: 'date' }] },
          { name: 'TOEKENNING_BEKENDMAKING', is: 'BEKENDMAKING', requires: [{ name: 'bekendmaking_datum', type: 'date' }] },
        ],
      },
    ],
  };
  const stages = procedureStages([{ $id: 'wet' }, awir], 'tegemoetkoming');
  const at = (stage) => atStage(stage, { procedureStages: stages });

  it('leest de fasen uit de wet die de procedure vastlegt', () => {
    expect(stages).toEqual([
      { name: 'AANVRAAG', is: null, requires: [] },
      { name: 'VOORSCHOT', is: 'BESLUIT', requires: [{ name: 'dagtekening_voorschot', type: 'date' }] },
      { name: 'VOORSCHOT_BEKENDMAKING', is: 'BEKENDMAKING', requires: [{ name: 'bekendmaking_datum', type: 'date' }] },
      { name: 'TOEKENNING', is: 'BESLUIT', requires: [{ name: 'dagtekening_toekenning', type: 'date' }] },
      { name: 'TOEKENNING_BEKENDMAKING', is: 'BEKENDMAKING', requires: [{ name: 'bekendmaking_datum', type: 'date' }] },
    ]);
    expect(procedureStages([awir], 'beschikking')).toBeNull();
    expect(procedureStages([awir], undefined)).toBeNull();
  });

  it('leest een zaak die op het voorschot wacht niet als besloten', () => {
    expect(statusOf(at('AANVRAAG'))).toBe('SUBMITTED');
    expect(statusOf(at('VOORSCHOT'))).toBe('IN_REVIEW');
  });

  it('opent bezwaar per besluit: na elke bekendmaking, en niet tussen een besluit en zijn bekendmaking', () => {
    expect(objectionOpen(at('VOORSCHOT_BEKENDMAKING'))).toBe(false);
    expect(objectionOpen(at('TOEKENNING'))).toBe(true);
    expect(announced(at('TOEKENNING_BEKENDMAKING'))).toBe(false);
    expect(objectionOpen(at('BEZWAAR'))).toBe(true);
    const out = { bezwaartermijn_einddatum: '2025-01-02' };
    const naToekenning = { stageState: { current_stage: 'TOEKENNING_BEKENDMAKING', accumulated_outputs: out }, procedureStages: stages };
    expect(awbOutcomes(naToekenning).bezwaartermijnEinde).toBeNull();
  });

  it('geeft een bekendmakingsdatum niet mee aan een volgende bekendmaking', () => {
    const inputs = { dagtekening_voorschot: '2024-11-20', bekendmaking_datum: '2024-11-20' };
    expect(carriedInputs(inputs, stages, 'TOEKENNING')).toEqual({ dagtekening_voorschot: '2024-11-20' });
    expect(carriedInputs(inputs, stages, 'VOORSCHOT')).toEqual({});
    // Zonder bekende fasen: alles, zoals voorheen.
    expect(carriedInputs(inputs, null, 'BESLUIT')).toEqual(inputs);
  });

  it('leest een zaak na het voorschot als besloten, ook al komt de toekenning nog', () => {
    expect(statusOf(at('TOEKENNING'))).toBe('DECIDED');
    // Klaar met de levensloop: een fase buiten de procedure.
    expect(statusOf(at('BEZWAAR'))).toBe('DECIDED');
  });
});

describe('decisionDates', () => {
  const stages = [
    { name: 'BESLUIT', is: null, requires: [{ name: 'besluit_datum', type: 'date' }, { name: 'kenmerk', type: 'string' }] },
  ];
  const waiting = (pendingInputs) => ({ pendingInputs, procedureStages: stages, stageState: { current_stage: 'BESLUIT' } });

  it('geeft de dagtekening die de wet bij het besluit noemt, en niets anders', () => {
    expect(decisionDates(waiting(['dagtekening_voorschot', 'kenmerk']), '2026-03-12', 'dagtekening_voorschot')).toEqual({
      dagtekening_voorschot: '2026-03-12',
    });
    expect(decisionDates(waiting(['kenmerk']), '2026-03-12', 'dagtekening_voorschot')).toEqual({});
  });

  it('geeft zonder cel alleen wat de fase vraagt en een datum is', () => {
    expect(decisionDates(waiting(['besluit_datum', 'kenmerk']), '2026-03-12')).toEqual({ besluit_datum: '2026-03-12' });
    expect(decisionDates({ pendingInputs: ['besluit_datum'] }, '2026-03-12')).toEqual({});
    expect(decisionDates({}, '2026-03-12')).toEqual({});
  });
});

describe('reachedStage', () => {
  it('weet welke fase eerder komt', () => {
    expect(reachedStage(atStage('BEKENDMAKING'), 'BESLUIT')).toBe(true);
    expect(reachedStage(atStage('BESLUIT'), 'BEKENDMAKING')).toBe(false);
    expect(reachedStage(atStage('BEZWAAR'), 'BEZWAAR')).toBe(true);
  });

  it('zegt nee als er geen levensloop is', () => {
    expect(reachedStage({}, 'BESLUIT')).toBe(false);
  });
});

describe('objectionOpen', () => {
  it('gaat pas open als de bezwaartermijn loopt', () => {
    // Awb 6:8: de termijn begint de dag ná de bekendmaking. Een besluit dat is
    // genomen maar niet verstuurd, heeft de burger nog niet bereikt.
    expect(objectionOpen(atStage('BESLUIT'))).toBe(false);
    expect(objectionOpen(atStage('BEKENDMAKING'))).toBe(false);
    expect(objectionOpen(atStage('BEZWAAR'))).toBe(true);
  });

  it('gaat weer dicht zodra er bezwaar is gemaakt', () => {
    expect(objectionOpen(atStage('BEZWAAR', { objection: { status: 'PENDING' } }))).toBe(false);
  });
});

describe('awbOutcomes', () => {
  it('leest wat de Awb aan het besluit heeft toegevoegd', () => {
    const c = {
      stageState: {
        current_stage: 'BEZWAAR',
        accumulated_outputs: {
          motivering_vereist: true,
          bezwaartermijn_weken: 6,
          bezwaartermijn_startdatum: '2026-03-13',
          bezwaartermijn_einddatum: '2026-04-23',
        },
      },
    };
    expect(awbOutcomes(c)).toEqual({
      motiveringVereist: true,
      bezwaartermijnWeken: 6,
      bezwaartermijnStart: '2026-03-13',
      bezwaartermijnEinde: '2026-04-23',
    });
  });

  it('geeft niets terug voor een zaak die er nog niet is', () => {
    expect(awbOutcomes(null).bezwaartermijnEinde).toBeNull();
    expect(awbOutcomes(atStage('BESLUIT')).bezwaartermijnEinde).toBeNull();
  });
});

describe('de volgorde waarin de demo een zaak door de fasen brengt', () => {
  // Nagelopen tegen de echte engine over het democorpus: na indienen en
  // toekennen staat de zaak op BEKENDMAKING met de termijn van artikel 6:7
  // erbij, en pas na het bekendmaken komt de einddatum van artikel 6:8 erbij.
  // Wat hier vastligt is wat die fasen voor de rest van de demo betekenen.
  const naToekenning = {
    stageState: {
      current_stage: 'BEKENDMAKING',
      accumulated_outputs: { toegekend: true, motivering_vereist: true, bezwaartermijn_weken: 6 },
    },
    approved: true,
  };
  const naBekendmaking = {
    stageState: {
      current_stage: 'BEZWAAR',
      accumulated_outputs: {
        ...naToekenning.stageState.accumulated_outputs,
        bezwaartermijn_startdatum: '2026-03-13',
        bezwaartermijn_einddatum: '2026-04-23',
      },
    },
    approved: true,
    publishedAt: '2026-03-12T10:00:00Z',
  };

  it('telt een toegekend maar niet bekendgemaakt besluit als besloten', () => {
    // Voor het bord hoort het bij "besloten"; dat het nog de deur uit moet,
    // zegt `publishedAt` en niet de status.
    expect(statusOf(naToekenning)).toBe('DECIDED');
  });

  it('laat nog geen bezwaar toe voordat het besluit is verstuurd', () => {
    expect(objectionOpen(naToekenning)).toBe(false);
    expect(awbOutcomes(naToekenning).bezwaartermijnEinde).toBeNull();
  });

  it('opent bezwaar met een datum zodra het is bekendgemaakt', () => {
    expect(objectionOpen(naBekendmaking)).toBe(true);
    expect(awbOutcomes(naBekendmaking).bezwaartermijnEinde).toBe('2026-04-23');
    expect(awbOutcomes(naBekendmaking).bezwaartermijnWeken).toBe(6);
  });

  // De andere weg: een aanvraag die naar een behandelaar gaat. De levensloop
  // stopt dan vóór BESLUIT en wacht op de besluitdatum, die er pas is als er
  // besloten wordt. Dit is de stand waarin verreweg de meeste zaken staan
  // zodra "alles handmatig beoordelen" aanstaat.
  const wachtOpBehandelaar = {
    stageState: { current_stage: 'BESLUIT', accumulated_outputs: {} },
    pendingInputs: ['besluit_datum'],
    approved: null,
    decidedAt: null,
  };

  it('laat een aanvraag die op een behandelaar wacht niet als besloten zien', () => {
    expect(statusOf(wachtOpBehandelaar)).toBe('IN_REVIEW');
  });

  it('laat op een aanvraag die nog loopt geen bezwaar toe, en geeft er geen termijn bij', () => {
    expect(objectionOpen(wachtOpBehandelaar)).toBe(false);
    expect(awbOutcomes(wachtOpBehandelaar).bezwaartermijnWeken).toBeNull();
  });
});
