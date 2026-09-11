import { describe, expect, it } from 'vitest';
import { cloneWorld, fixtureGram, worldFixture } from '../testing/worldFixture.js';
import {
  actionsByActor,
  clockIndex,
  describeEffect,
  describeOrigin,
  emptyForm,
  gramCounts,
  gramFields,
  gramKind,
  gramsInTimeOrder,
  isNewGram,
  newGrams,
  obligationsOf,
  readLexostatus,
  regulationOf,
  settingRows,
  timelineMoments,
} from './snapshot.js';

describe('het beeld lezen', () => {
  it('geeft elk gram-soort een eigen label en kleur', () => {
    expect(gramKind('decretogram').label).toBe('Decretogram');
    expect(gramKind('executogram').label).toBe('Executogram');
    expect(gramKind('lexogram').label).toBe('Lexogram');
    // Drie verschillende kleuren, zodat de soorten in beeld te scheiden zijn.
    const colors = ['lexogram', 'decretogram', 'executogram'].map((kind) => gramKind(kind).color);
    expect(new Set(colors).size).toBe(3);
  });

  it('houdt een onbekend gram-soort leesbaar in plaats van om te vallen', () => {
    expect(gramKind('iets-nieuws').label).toBe('Gram');
  });

  it('zet de grammen van een kroniek in tijdsvolgorde, met hun plek in het beeld', () => {
    const cell = worldFixture.cells.find((candidate) => candidate.id === 'belastingdienst');
    const chronicle = cell.chronicles.find((candidate) => candidate.stream === 'betalingen');
    const entries = gramsInTimeOrder(chronicle);
    const moments = entries.map((entry) => entry.gram.op_moment);
    expect([...moments]).toStrictEqual([...moments].sort());
    expect(entries.map((entry) => entry.index)).toStrictEqual(entries.map((_, index) => index));
  });
});

describe('wat er nieuw is', () => {
  it('telt de grammen per kroniek', () => {
    const counts = gramCounts(worldFixture);
    const cell = worldFixture.cells.find((candidate) => candidate.id === 'belastingdienst');
    const chronicle = cell.chronicles[0];
    expect(counts.get(`belastingdienst|${chronicle.stream}`)).toBe(chronicle.grams.length);
  });

  it('noemt een gram nieuw zodra het achter de vorige stand staat', () => {
    const counts = gramCounts(worldFixture);
    const cell = worldFixture.cells.find((candidate) => candidate.id === 'belastingdienst');
    const stream = cell.chronicles[0].stream;
    const last = cell.chronicles[0].grams.length - 1;
    expect(isNewGram(counts, 'belastingdienst', stream, last)).toBe(false);
    expect(isNewGram(counts, 'belastingdienst', stream, last + 1)).toBe(true);
  });

  it('noemt niets nieuw zonder een vorige stand: een beeld is geen stap', () => {
    expect(isNewGram(null, 'belastingdienst', 'betalingen', 99)).toBe(false);
    expect(newGrams(worldFixture, null)).toStrictEqual([]);
  });

  it('somt een gram op dat erbij kwam, met cel, kroniek en moment', () => {
    const before = gramCounts(worldFixture);
    const after = cloneWorld();
    const chronicle = after.cells.find((cell) => cell.id === 'burger').chronicles[0];
    chronicle.grams.push({
      kind: 'executogram',
      name: 'aanvraag_ingediend',
      intake: 'aanvraag',
      recording_actor: 'burger',
      grondslag: '',
      op_moment: '2025-02-01',
      fields: {},
    });

    expect(newGrams(after, before)).toStrictEqual([
      {
        cell: 'burger',
        stream: chronicle.stream,
        kind: 'executogram',
        name: 'aanvraag_ingediend',
        opMoment: '2025-02-01',
      },
    ]);
  });

  it('markeert na terugzetten niets als nieuw', () => {
    const before = gramCounts(worldFixture);
    const shorter = cloneWorld();
    for (const cell of shorter.cells) for (const chronicle of cell.chronicles) chronicle.grams = [];
    expect(newGrams(shorter, before)).toStrictEqual([]);
  });
});

describe('de tijdlijn', () => {
  const moments = timelineMoments(worldFixture);

  it('zet één punt per dag, op datum gesorteerd', () => {
    const dates = moments.map((point) => point.moment);
    expect(new Set(dates).size).toBe(dates.length);
    expect([...dates]).toStrictEqual([...dates].sort());
  });

  it('zet de klok op de lijn, ook als er die dag niets ligt', () => {
    const clockPoint = moments.find((point) => point.isClock);
    expect(clockPoint.moment).toBe(worldFixture.clock);
    expect(clockPoint.step).toBe('current');
    expect(clockIndex(moments)).toBe(moments.indexOf(clockPoint) + 1);
  });

  it('scheidt verleden van toekomst op de stand van de klok', () => {
    for (const point of moments) {
      if (point.moment < worldFixture.clock) expect(point.step).toBe('past');
      if (point.moment > worldFixture.clock) expect(point.step).toBe('future');
    }
  });

  it('draagt elk gram van die dag mee, met zijn cel en kroniek', () => {
    const total = moments.reduce((sum, point) => sum + point.grams.length, 0);
    const inWorld = worldFixture.cells.reduce(
      (sum, cell) => sum + cell.chronicles.reduce((rows, chronicle) => rows + chronicle.grams.length, 0),
      0,
    );
    expect(total).toBe(inWorld);
    expect(moments.every((point) => point.grams.every((gram) => gram.cell && gram.stream))).toBe(true);
  });

  it('markeert het punt waarop een nieuw gram ligt', () => {
    const before = gramCounts(worldFixture);
    const after = cloneWorld();
    after.cells
      .find((cell) => cell.id === 'burger')
      .chronicles[0].grams.push({
        kind: 'executogram',
        name: 'aanvraag_ingediend',
        intake: 'aanvraag',
        recording_actor: 'burger',
        grondslag: '',
        op_moment: '2025-03-09',
        fields: {},
      });

    const marked = timelineMoments(after, { newGramCounts: before }).filter((point) => point.hasNew);
    expect(marked.map((point) => point.moment)).toStrictEqual(['2025-03-09']);
  });
});

describe('de acties', () => {
  it('groepeert per actor en houdt elke actie', () => {
    const groups = actionsByActor(worldFixture);
    expect(groups.map((group) => group.actor)).toStrictEqual([
      ...new Set(worldFixture.actions.map((action) => action.actor)),
    ]);
    const total = groups.reduce((sum, group) => sum + group.actions.length, 0);
    expect(total).toBe(worldFixture.actions.length);
  });

  it('maakt een leeg formulier met het type van elk veld', () => {
    const action = worldFixture.actions.find((candidate) => candidate.form.length > 1);
    const form = emptyForm(action);
    for (const field of action.form) {
      expect(form).toHaveProperty(field.name);
      if (field.type === 'number') expect(form[field.name]).toBeNull();
      if (field.type === 'string') expect(form[field.name]).toBe('');
    }
  });

  it('schrijft uit wat een actie uitwerkt, in de woorden van de wereld', () => {
    const records = worldFixture.actions.find((action) => action.effect.soort === 'records');
    expect(describeEffect(records.effect)).toContain(`kroniek '${records.effect.chronicle}'`);
    expect(describeEffect(records.effect)).toContain(`cel '${records.effect.delivers_to}'`);

    const decides = worldFixture.actions.find((action) => action.effect.soort === 'decides');
    expect(describeEffect(decides.effect)).toContain(`besluit '${decides.effect.besluit}'`);
  });
});

describe('de herkomst van een waarde', () => {
  const { gram } = fixtureGram('toeslagen', 'decretogram');

  it('zet de vaste velden van het besluit vooraan', () => {
    const fields = gramFields(gram);
    const firstOther = fields.findIndex((field) => field.origin?.herkomst !== 'besluit');
    expect(fields.slice(0, firstOther).every((field) => field.origin.herkomst === 'besluit')).toBe(true);
  });

  it('noemt een geaccepteerde waarde geaccepteerd, met bron, moment en ondertekening', () => {
    const accepted = gramFields(gram).find(
      (field) => field.origin?.recorded_origin?.herkomst === 'geaccepteerd',
    );
    const origin = describeOrigin(accepted.origin);
    expect(origin.kind).toBe('geaccepteerd');
    expect(origin.label).toContain(`cel '${accepted.origin.recorded_origin.cell}'`);
    const terms = origin.details.map((detail) => detail.term);
    expect(terms).toContain('lexostatus');
    expect(terms).toContain('op moment');
    expect(terms).toContain('ondertekend');
  });

  it('houdt geaccepteerd en berekend uit elkaar', () => {
    const computed = gramFields(gram).find((field) => field.origin?.herkomst === 'computed');
    const origin = describeOrigin(computed.origin);
    expect(origin.kind).toBe('computed');
    expect(origin.details).toStrictEqual([{ term: 'regeling', value: computed.origin.regulation }]);
  });

  it('leest een eigen kroniek en een opgave als zulke', () => {
    const fields = gramFields(gram);
    const own = fields.find((field) => field.origin?.recorded_origin?.herkomst === 'eigen_kroniek');
    expect(describeOrigin(own.origin).label).toContain(`kroniek '${own.origin.recorded_origin.chronicle}'`);

    const parameter = fields.find((field) => field.origin?.recorded_origin?.herkomst === 'parameter');
    expect(describeOrigin(parameter.origin).kind).toBe('parameter');
  });

  it('zegt van een vastgelegd gram dat de cel het zelf vastlegde', () => {
    const { gram: executogram } = fixtureGram('belastingdienst', 'executogram');
    const field = gramFields(executogram)[0];
    const origin = describeOrigin(field.origin);
    expect(origin.kind).toBe('recorded');
    expect(origin.details.map((detail) => detail.term)).toContain('kanaal');
  });

  it('blijft leesbaar als de herkomst niet in het gram staat', () => {
    expect(describeOrigin(undefined).label).toBe('herkomst niet vastgelegd');
    expect(describeOrigin({ herkomst: 'besluit_input', recorded_origin: null }).label).toContain('niet te lezen');
  });
});

describe('het besluit zelf', () => {
  const { gram } = fixtureGram('toeslagen', 'decretogram');

  it('geeft de regeling met haar versie', () => {
    expect(regulationOf(gram)).toStrictEqual({
      regulation: gram.fields.regulation.value,
      validFrom: gram.fields.regulation_valid_from.value,
    });
  });

  it('geeft niets voor een gram zonder regeling', () => {
    expect(regulationOf({ fields: {} })).toBeNull();
  });

  it('geeft de verplichtingen met de kolommen die ze zelf dragen', () => {
    const { columns, rows } = obligationsOf(gram);
    expect(rows).toStrictEqual(gram.fields.obligations.value);
    for (const row of rows) for (const name of Object.keys(row)) expect(columns).toContain(name);
  });

  it('geeft geen verplichtingen waar er geen zijn', () => {
    expect(obligationsOf({ fields: {} })).toStrictEqual({ columns: [], rows: [] });
  });
});

describe('een antwoord van een cel', () => {
  it('leest een vastgesteld antwoord uit met zijn waarden', () => {
    const answer = readLexostatus(worldFixture.crossings[0].answer);
    expect(answer.established).toBe(true);
    expect(answer.cell).toBe(worldFixture.crossings[0].answer.cell);
    expect(answer.values.length).toBeGreaterThan(0);
    expect(answer.reason).toBeNull();
  });

  it('leest "niets vastgesteld" als antwoord en niet als fout', () => {
    const answer = readLexostatus({
      cell: 'belastingdienst',
      name: 'toetsingsinkomen',
      op_moment: '2025-02-01',
      outcome: { not_established: { reason: 'in deze cel is hierover niets vastgesteld' } },
    });
    expect(answer.established).toBe(false);
    expect(answer.values).toStrictEqual([]);
    expect(answer.reason).toContain('niets vastgesteld');
  });

  it('geeft ook zonder reden een leesbaar antwoord', () => {
    expect(readLexostatus({ outcome: {} }).reason).toContain('niets vastgesteld');
  });
});

describe('de instellingen', () => {
  it('zet bij elke instelling of ze al vastligt en waardoor', () => {
    const rows = settingRows(worldFixture);
    const locked = rows.find((row) => row.name === 'betalingsritme');
    expect(locked.value).toBe(worldFixture.settings.betalingsritme);
    expect(locked.locked).toStrictEqual(worldFixture.locked_settings.betalingsritme);
  });

  it('laat een vrije instelling vrij', () => {
    const rows = settingRows({ settings: { ritme: 'maand' }, locked_settings: {} });
    expect(rows).toStrictEqual([{ name: 'ritme', value: 'maand', locked: null }]);
  });
});
