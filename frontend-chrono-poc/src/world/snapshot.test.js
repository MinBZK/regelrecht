import { describe, expect, it } from 'vitest';
import { cloneWorld, fixtureGram, worldFixture } from '../testing/worldFixture.js';
import {
  actionsByActor,
  allGrams,
  clockIndex,
  competentAuthorityOf,
  decidedAlready,
  describeEffect,
  describeOrigin,
  gramCounts,
  gramFields,
  gramKind,
  gramsInTimeOrder,
  initialForm,
  isNewGram,
  isPrefilled,
  missedDeadlines,
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
    const action = structuredClone(worldFixture.actions.find((candidate) => candidate.form.length > 1));
    delete action.prefill;
    const form = initialForm(action);
    for (const field of action.form) {
      expect(form).toHaveProperty(field.name);
      if (field.type === 'number') expect(form[field.name]).toBeNull();
      if (field.type === 'string') expect(form[field.name]).toBe('');
    }
  });

  // De voorinvulling is al opgelost door de wereld: hier staat een waarde en
  // geen verwijzing, dus deze app hoeft niets in de kronieken op te zoeken.
  it('begint met wat de wereld al weet, en houdt het soort van de waarde', () => {
    const action = worldFixture.actions.find((candidate) => candidate.form.length > 1);
    const form = initialForm(action);
    expect(Object.keys(action.prefill).length).toBeGreaterThan(0);
    for (const [name, value] of Object.entries(action.prefill)) {
      expect(form[name]).toStrictEqual(value);
      expect(isPrefilled(action, name, form[name])).toBe(true);
    }
  });

  it('laat een veld leeg waarover de wereld niets aanlevert', () => {
    const action = structuredClone(worldFixture.actions.find((candidate) => candidate.form.length > 1));
    const [eerste] = action.form;
    delete action.prefill[eerste.name];
    const form = initialForm(action);
    expect(form[eerste.name]).toBe(eerste.type === 'number' ? null : '');
    expect(isPrefilled(action, eerste.name, form[eerste.name])).toBe(false);
  });

  // Zodra iemand er iets anders van maakt, is het zijn opgave en niet meer een
  // voorinvulling — anders zou het label iets beweren wat niet meer waar is.
  it('rekent een gewijzigde waarde niet meer als voorgevuld', () => {
    const action = worldFixture.actions.find((candidate) => candidate.form.length > 1);
    const [name] = Object.keys(action.prefill);
    expect(isPrefilled(action, name, 'iets anders')).toBe(false);
  });

  it('valt terug op een leeg formulier als het beeld geen voorinvulling geeft', () => {
    expect(initialForm({ form: [{ name: 'bsn', type: 'string' }] })).toStrictEqual({ bsn: '' });
    expect(initialForm(undefined)).toStrictEqual({});
  });

  it('schrijft uit wat een actie uitwerkt, in de woorden van de wereld', () => {
    const records = worldFixture.actions.find((action) => action.effect.soort === 'records');
    expect(describeEffect(records.effect)).toContain(`kroniek '${records.effect.chronicle}'`);
    expect(describeEffect(records.effect)).toContain(`cel '${records.effect.delivers_to}'`);

    const decides = worldFixture.actions.find((action) => action.effect.soort === 'decides');
    expect(describeEffect(decides.effect)).toContain(`besluit '${decides.effect.besluit}'`);
  });
});

describe('een besluit dat er al ligt', () => {
  /** De eerste besluit-actie uit de fixture; in het beeld ligt haar gram al. */
  const decides = worldFixture.actions.find((action) => action.effect.soort === 'decides');

  it('vindt het gram van dezelfde zaak, met zijn zaakkenmerk en moment', () => {
    const decided = decidedAlready(worldFixture, decides, { bsn: '999993653' });
    expect(decided).toStrictEqual({
      opMoment: '2024-04-01',
      zaakkenmerk: 'zorgtoeslag/999993653',
      count: 1,
    });
  });

  it('houdt zaken uit elkaar: een andere parameter is een andere zaak', () => {
    expect(decidedAlready(worldFixture, decides, { bsn: '999999999' })).toBeNull();
  });

  it('zwijgt zolang het formulier niet ingevuld is, en over een actie die vastlegt', () => {
    expect(decidedAlready(worldFixture, decides, { bsn: '' })).toBeNull();
    expect(decidedAlready(worldFixture, decides, {})).toBeNull();
    const records = worldFixture.actions.find((action) => action.effect.soort === 'records');
    expect(decidedAlready(worldFixture, records, { bsn: '999993653' })).toBeNull();
  });

  it('telt elk besluit over dezelfde zaak, en niets van ná de klok', () => {
    const world = cloneWorld();
    const chronicle = world.cells
      .find((cell) => cell.id === decides.effect.cell)
      .chronicles.find((stream) => stream.grams.some((gram) => gram.kind === 'decretogram'));
    const first = chronicle.grams.find((gram) => gram.fields.besluit?.value === decides.effect.besluit);

    const later = structuredClone(first);
    later.op_moment = '2024-09-01';
    chronicle.grams.push(later);
    expect(decidedAlready(world, decides, { bsn: '999993653' })).toMatchObject({
      opMoment: '2024-09-01',
      count: 2,
    });

    // Wat na de klok ligt, ligt er nog niet: daar hoort geen bevestiging over te
    // gaan.
    const ahead = structuredClone(first);
    ahead.op_moment = '2099-01-01';
    chronicle.grams.push(ahead);
    expect(decidedAlready(world, decides, { bsn: '999993653' })).toMatchObject({
      opMoment: '2024-09-01',
      count: 2,
    });
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

  // De wet wijst het bevoegd gezag aan en de cel beweert wie ze is; het gram
  // draagt allebei. Ze zijn hier gelijk, want een besluit door iemand anders
  // wordt geweigerd en komt dus nooit in een kroniek terecht.
  it('geeft het bevoegd gezag van de regeling naast wie besloot', () => {
    expect(competentAuthorityOf(gram)).toStrictEqual({
      authority: gram.fields.competent_authority.value,
      decidedBy: gram.fields.besloten_door.value,
    });
  });

  // Het geval waarvoor dit bestaat: de regeling zegt niets, dus er viel niets te
  // toetsen. Dat hoort te onderscheiden te zijn van een gram dat geen besluit is.
  it('onderscheidt "geen gezag aangewezen" van "geen besluit"', () => {
    const zonder = { fields: { competent_authority: { value: null }, besloten_door: { value: 'uitvoerder' } } };
    expect(competentAuthorityOf(zonder)).toStrictEqual({ authority: null, decidedBy: 'uitvoerder' });
    expect(competentAuthorityOf({ fields: {} })).toBeNull();
  });
});

describe('de waarschuwingen van de wereld', () => {
  it('houdt de soorten uit elkaar', () => {
    const world = cloneWorld();
    world.warnings = [
      { soort: 'gemiste_termijn', label: 'aanvraagtermijn', at: '2024-11-01' },
      { soort: 'geen_bevoegd_gezag', at: '2024-06-01', cell: 'uitvoerder' },
    ];
    expect(missedDeadlines(world)).toStrictEqual([world.warnings[0]]);
  });

  it('geeft een lege lijst voor een beeld zonder waarschuwingen', () => {
    expect(missedDeadlines(worldFixture)).toStrictEqual([]);
    expect(missedDeadlines(undefined)).toStrictEqual([]);
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

describe('alle grammen op één hoop', () => {
  it('neemt elk gram van elke kroniek van elke cel mee', () => {
    const counted = worldFixture.cells.reduce(
      (total, cell) => total + cell.chronicles.reduce((sum, chronicle) => sum + chronicle.grams.length, 0),
      0,
    );
    expect(allGrams(worldFixture)).toHaveLength(counted);
  });

  it('zet ze op volgorde van moment', () => {
    const moments = allGrams(worldFixture).map((gram) => gram.opMoment);
    expect(moments).toStrictEqual([...moments].sort());
  });

  it('houdt dezelfde dag in een vaste volgorde', () => {
    // Cel, dan kroniek, dan de plek in die kroniek. De dag is de korrel, dus
    // zonder die staart zou een tabel per beeld van volgorde wisselen.
    const sameDay = [
      {
        id: 'b',
        chronicles: [{ stream: 'tweede', grams: [{ name: 'y', op_moment: '2025-01-01' }] }],
      },
      {
        id: 'a',
        chronicles: [
          { stream: 'eerste', grams: [{ name: 'x', op_moment: '2025-01-01' }] },
          { stream: 'ander', grams: [{ name: 'w', op_moment: '2025-01-01' }] },
        ],
      },
    ];
    expect(allGrams({ cells: sameDay }).map((gram) => gram.name)).toStrictEqual(['w', 'x', 'y']);
  });

  it('draagt het gram zelf mee, ongefilterd', () => {
    const { gram } = fixtureGram('toeslagen', 'decretogram');
    const row = allGrams(worldFixture).find((candidate) => candidate.gram === gram);
    expect(row.kind).toBe('decretogram');
    expect(row.gram.fields).toBe(gram.fields);
  });

  it('valt op een leeg beeld terug op niets', () => {
    expect(allGrams(null)).toStrictEqual([]);
    expect(allGrams({ cells: [{ id: 'a' }] })).toStrictEqual([]);
  });
});
