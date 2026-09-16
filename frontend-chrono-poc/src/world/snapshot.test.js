import { describe, expect, it } from 'vitest';
import { cloneWorld, fixtureCell, fixtureGram, worldFixture } from '../testing/worldFixture.js';
import {
  actionsByActor,
  afwijzingsgrondenOf,
  allGrams,
  besluitDefinitions,
  clockIndex,
  competentAuthorityOf,
  decidedAlready,
  decisionTypeOf,
  decretogramSchema,
  decretogramRefOf,
  describeEffect,
  describeHerkomst,
  describeOrigin,
  describeParam,
  describeReductie,
  describeZaak,
  gramByRef,
  gramCounts,
  gramFields,
  gramKind,
  gramsInTimeOrder,
  initialForm,
  isNewGram,
  isPrefilled,
  lexostatusDefinitions,
  lexostatusParams,
  missedDeadlines,
  newGrams,
  obligationsOf,
  placeholderFor,
  readLexostatus,
  regulationOf,
  settingRows,
  timelineMoments,
  zaakkenmerkOf,
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

  // De klok loopt door en er komen feiten bij, dus een veld dat niemand
  // aanraakte hoort te zeggen wat de wereld *nu* al weet. Wat de invuller zelf
  // typte, blijft van hem.
  it('laat een onaangeraakt veld het nieuwe voorstel volgen en houdt wat er getypt is', () => {
    const action = structuredClone(worldFixture.actions.find((candidate) => candidate.form.length > 1));
    const [eerste, tweede] = action.form;
    const getypt = { [eerste.name]: 'zelf ingevuld' };

    action.prefill[tweede.name] = 1999;
    const form = initialForm(action, getypt);

    expect(form[eerste.name]).toBe('zelf ingevuld');
    expect(form[tweede.name]).toBe(1999);
    expect(isPrefilled(action, tweede.name, form[tweede.name])).toBe(true);
    expect(isPrefilled(action, eerste.name, form[eerste.name])).toBe(false);
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

  it('zet het zaakkenmerk voorop en de andere vaste velden daarna', () => {
    const fields = gramFields(gram);
    expect(fields[0].name).toBe('zaakkenmerk');
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

  it('noemt een teruggelezen besluit een besluit, met de zaak en het moment', () => {
    // Een literal en niet de fixture: deze wereld leest geen eerder besluit terug
    // (zie `scenarios/toeslagen_nabetaling.yaml` voor een wereld die dat wel
    // doet). Wat hier getoetst wordt is de weergave van de vorm die het gram
    // opschrijft.
    const origin = describeOrigin({
      herkomst: 'besluit_input',
      recorded_origin: {
        herkomst: 'eerder_besluit',
        besluit: 'zorgtoeslag_toekenning',
        zaakkenmerk: 'zorgtoeslag/999993653',
        moment: '2026-12-01',
      },
    });
    expect(origin.kind).toBe('eerder_besluit');
    expect(origin.label).toContain("eerder besluit 'zorgtoeslag_toekenning'");
    expect(origin.label).toContain('01-12-2026');
    expect(origin.details).toStrictEqual([
      { term: 'zaak', value: 'zorgtoeslag/999993653' },
      { term: 'besloten op', value: '01-12-2026' },
    ]);
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

  it('geeft het besluittype zoals het gram het draagt', () => {
    expect(decisionTypeOf(gram)).toStrictEqual({
      type: gram.fields.decision_type.value,
      label: gram.fields.decision_type.value,
      color: 'donkerblauw',
    });
  });

  // Het onderscheid dat het platform zelf maakt: een beschikking omvat ook de
  // afwijzing van de aanvraag, dus een weigering hoort niet als een gewone
  // toekenning weg te vallen.
  it('geeft een afwijzing een eigen kleur', () => {
    const afwijzing = { fields: { decision_type: { value: 'AFWIJZING' } } };
    expect(decisionTypeOf(afwijzing)).toStrictEqual({
      type: 'AFWIJZING',
      label: 'AFWIJZING',
      color: 'oranje',
    });
  });

  it('geeft geen besluittype voor een gram dat er geen draagt', () => {
    expect(decisionTypeOf({ fields: {} })).toBeNull();
    expect(decisionTypeOf({ fields: { decision_type: { value: null } } })).toBeNull();
  });

  it('geeft geen afwijzingsgrond bij een besluit dat niet afwees', () => {
    expect(afwijzingsgrondenOf(gram)).toStrictEqual([]);
    expect(afwijzingsgrondenOf({ fields: {} })).toStrictEqual([]);
  });

  it('geeft elke afwijzingsgrond met haar uitkomst, waarde en artikel', () => {
    const afwijzing = {
      fields: {
        afwijzingsgrond: {
          value: [{ output: 'heeft_recht_op_zorgtoeslag', value: false, article: '2' }],
        },
      },
    };
    expect(afwijzingsgrondenOf(afwijzing)).toStrictEqual([
      { output: 'heeft_recht_op_zorgtoeslag', value: false, article: '2' },
    ]);
  });

  it('houdt een grond zonder artikel leesbaar', () => {
    const afwijzing = {
      fields: { afwijzingsgrond: { value: [{ output: 'is_verzekerde', value: false, article: null }] } },
    };
    expect(afwijzingsgrondenOf(afwijzing)).toStrictEqual([
      { output: 'is_verzekerde', value: false, article: null },
    ]);
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

describe('de weg van een betaling terug naar het besluit', () => {
  /** Het eerste executogram over een betaling uit de fixture. */
  const betaling = allGrams(worldFixture).find((row) => row.gram.fields?.besluit_gram).gram;

  it('maakt van de losse velden het id waarmee het beeld een gram aanwijst', () => {
    const ref = decretogramRefOf(betaling);
    expect(ref.id).toBe(`${ref.cell}|${ref.chronicle}|${ref.index}`);
    expect(gramByRef(worldFixture, ref)).not.toBeNull();
    expect(gramByRef(worldFixture, ref).kind).toBe('decretogram');
  });

  it('draagt het besluit, het moment en het termijnnummer erbij', () => {
    const ref = decretogramRefOf(betaling);
    expect(ref.besluit).toBe(String(betaling.fields.besluit.value));
    expect(ref.opMoment).toBe(String(betaling.fields.besluit_op_moment.value));
    expect(ref.volgnummer).toBe(Number(betaling.fields.volgnummer.value));
  });

  it('geeft niets voor een gram dat geen betaling is', () => {
    expect(decretogramRefOf({ fields: {} })).toBeNull();
  });

  it('geeft niets voor een halve verwijzing', () => {
    // Half tonen zou een link opleveren die soms nergens op uitkomt, en dat is
    // erger dan geen link.
    const half = structuredClone(betaling);
    delete half.fields.besluit_kroniek;
    expect(decretogramRefOf(half)).toBeNull();

    const slordig = structuredClone(betaling);
    slordig.fields.besluit_gram.value = ' 1 ';
    expect(decretogramRefOf(slordig)).toBeNull();
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

describe('hoe een antwoord tot stand kwam', () => {
  /** Het antwoord over een celgrens uit de fixture draagt een echte uitleg. */
  const crossing = worldFixture.crossings[0].answer;

  it('leest de uitleg mee met het antwoord, met de grammen die gelezen zijn', () => {
    const answer = readLexostatus(crossing);
    expect(answer.reductie.soort).toBe('kroniekfilter');
    expect(answer.reductie.grams.length).toBeGreaterThan(0);

    // Elk gram wijst naar een gram in de kroniek van de cel die antwoordde, met
    // de sleutel waarmee het tabblad Grammen zijn rijen kent.
    for (const gram of answer.reductie.grams) {
      expect(gram.cell).toBe(crossing.cell);
      expect(gram.id).toBe(`${gram.cell}|${gram.chronicle}|${gram.volgnummer}`);
      expect(allGrams(worldFixture).some((row) => row.id === gram.id)).toBe(true);
    }
  });

  it('schrijft de regel van een kroniekfilter als één zin', () => {
    expect(
      describeReductie({
        soort: 'kroniekfilter',
        chronicle: 'beschikkingen',
        key: 'zaakkenmerk',
        key_value: 'zaak/1',
        where: {},
        regel: { regel: 'laatste' },
        op_moment: '2026-12-01',
      }),
    ).toBe("laatste vastlegging in kroniek 'beschikkingen' met zaakkenmerk 'zaak/1' op of vóór 01-12-2026");
  });

  it('noemt de som met haar veld, en de voorwaarden waaronder gefilterd is', () => {
    const zin = describeReductie({
      soort: 'kroniekfilter',
      chronicle: 'betalingen',
      key: 'zaakkenmerk',
      key_value: 'zaak/1',
      where: { soort: 'termijn' },
      regel: { regel: 'som', field: 'bedrag' },
      op_moment: '2026-12-01',
    });
    expect(zin).toContain('som over bedrag');
    expect(zin).toContain('soort = termijn');
  });

  it('noemt bij een wetsvorm de regeling met de versie die gold', () => {
    const zin = describeReductie({
      soort: 'wetsvorm',
      regulation: 'wet_op_de_zorgtoeslag',
      regulation_valid_from: '2025-01-01',
      output: 'heeft_recht_op_zorgtoeslag',
      inputs: [],
      op_moment: '2026-12-01',
    });
    expect(zin).toContain("regeling 'wet_op_de_zorgtoeslag'");
    expect(zin).toContain('versie 01-01-2025');
  });

  it('zegt per input van een wetsvorm waar hij vandaan kwam', () => {
    expect(describeHerkomst({ herkomst: 'parameter', parameter: 'bsn' })).toContain("'bsn'");
    expect(describeHerkomst({ herkomst: 'eigen_kroniek', chronicle: 'relaties', gram: 'a|b|0' })).toContain(
      "'relaties'",
    );
    expect(describeHerkomst({ herkomst: 'regeling', regulation: 'awir', output: 'partner' })).toContain("'awir'");
    expect(describeHerkomst({ herkomst: 'cel', cell: 'brp', output: 'partnerschap' })).toContain("'brp'");
    // Een herkomst die deze app niet kent, hoort het beeld niet om te gooien.
    expect(describeHerkomst({ herkomst: 'iets-nieuws' })).toBe('');
  });

  it('houdt een antwoord zonder uitleg leesbaar', () => {
    expect(readLexostatus({ outcome: {} }).reductie).toBeNull();
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

describe('wat een cel publiceert', () => {
  const toeslagen = fixtureCell('toeslagen');
  const beschikking = lexostatusDefinitions(toeslagen).find(
    (definition) => definition.key?.chronicle === 'beschikkingen',
  );

  it('geeft per definitie haar toelichting, parameters en uitkomsten', () => {
    expect(beschikking.doc).toBeTruthy();
    expect(beschikking.inputs.map((input) => input.name)).toContain('zaakkenmerk');
    expect(beschikking.outputs.length).toBeGreaterThan(0);
  });

  it('geeft per besluit het zaakkenmerk-sjabloon en de kroniek waarin het legt', () => {
    for (const besluit of besluitDefinitions(toeslagen)) {
      expect(besluit.zaakkenmerk).toContain('{');
      expect(besluit.chronicle).toBe('beschikkingen');
    }
  });

  it('valt op een cel zonder definities terug op niets', () => {
    expect(lexostatusDefinitions(null)).toStrictEqual([]);
    expect(besluitDefinitions({ id: 'a' })).toStrictEqual([]);
  });

  // Het schema zegt per veld van het decretogram wie het declareert. Dat is de
  // meting "welk deel van een besluit volgt uit de wet", en een veld dat het
  // wereldbestand zegt is een gat (RFC-022).
  it('geeft per besluit het schema van het decretogram, met een label per herkomst', () => {
    const [besluit] = besluitDefinitions(toeslagen);
    const schema = decretogramSchema(besluit);
    expect(schema.length).toBe(besluit.schema.length);

    const uitkomst = schema.find((field) => field.name === 'hoogte_zorgtoeslag');
    expect(uitkomst.source.label).toBe('wet');
    expect(uitkomst.gat).toBe(false);
    // Type én eenheid: een bedrag zonder eenheid is een getal.
    expect(uitkomst.type).toBe('amount · eurocent');
    // De versie hoort bij het artikel: zonder haar wijst de verwijzing naar iets
    // dat morgen anders kan staan.
    expect(uitkomst.lexogram).toContain('wet_op_de_zorgtoeslag');
    expect(uitkomst.lexogram).toContain('artikel 2');
    expect(uitkomst.lexogram).toContain('versie ');

    const kenmerk = schema.find((field) => field.name === 'zaakkenmerk');
    expect(kenmerk.gat).toBe(true);
    expect(kenmerk.source.label).toBe('wereldbestand');
    expect(kenmerk.toelichting).toBeTruthy();

    // Een platformveld waarvan de wet de waarde levert, noemt die plek erbij.
    const gezag = schema.find((field) => field.name === 'competent_authority');
    expect(gezag.source.label).toBe('platform');
    expect(gezag.lexogram).toContain('op het document');
  });

  it('blijft leesbaar bij een besluit zonder schema of met een onbekende herkomst', () => {
    expect(decretogramSchema(undefined)).toStrictEqual([]);
    const [veld] = decretogramSchema({ schema: [{ name: 'iets', herkomst: 'iets nieuws' }] });
    expect(veld.source.label).toBe('onbekend');
    expect(veld.type).toBe('onbekend');
    expect(veld.lexogram).toBe('');
  });

  // De sleutel van de reductie is wat het zaakkenmerk uit het niets laat komen:
  // zonder die aanwijzing is er geen manier om te weten wat er in dat veld hoort.
  it('wijst de sleutel van de reductie aan, met vorm en bekende waarden', () => {
    const params = lexostatusParams(toeslagen, beschikking);
    expect(params).toHaveLength(1);
    const [zaak] = params;
    expect(zaak.name).toBe('zaakkenmerk');
    expect(zaak.chronicle).toBe('beschikkingen');
    expect(zaak.patterns).toStrictEqual(['zorgtoeslag/{bsn}']);
    expect(zaak.known).toStrictEqual(['zorgtoeslag/999993653']);
    expect(describeParam(zaak)).toBe("sleutel van kroniek 'beschikkingen', vorm 'zorgtoeslag/{bsn}'");
  });

  it('laat een gewone parameter een gewone parameter', () => {
    const aanvraag = lexostatusDefinitions(toeslagen).find((definition) => definition.name === 'ontvangen_aanvraag');
    const [bsn] = lexostatusParams(toeslagen, aanvraag);
    expect(bsn.chronicle).toBe('aanvragen');
    // Geen besluit van deze cel legt in die kroniek, dus er is geen vorm.
    expect(bsn.patterns).toStrictEqual([]);
    expect(describeParam(bsn)).toBe("sleutel van kroniek 'aanvragen'");
  });

  // Het geestschrift in het veld is één welgevormde waarde, of niets. Twee
  // sjablonen aaneenrijgen zou een tekst in het veld zetten die zelf geen geldig
  // kenmerk is; er één uitkiezen zou die vorm stelliger maken dan ze is.
  it('geeft een voorbeeld in het veld zolang er precies één vorm is', () => {
    const [zaak] = lexostatusParams(toeslagen, beschikking);
    expect(placeholderFor(zaak)).toBe('zorgtoeslag/{bsn}');
    expect(placeholderFor({ patterns: ['a/{x}', 'b/{y}'] })).toBe('');
    expect(placeholderFor({ patterns: [] })).toBe('');
    expect(placeholderFor(undefined)).toBe('');
  });

  it('noemt bij de wetsvorm geen sleutel, want daar valt niets op te zoeken', () => {
    const wet = { inputs: [{ name: 'bsn', type: 'string' }], key: null };
    const [bsn] = lexostatusParams(toeslagen, wet);
    expect(bsn.chronicle).toBeNull();
    expect(bsn.known).toStrictEqual([]);
    expect(describeParam(bsn)).toBe('string');
  });
});

describe('de zaak van een gram', () => {
  it('leest het zaakkenmerk uit het gram zelf', () => {
    const { gram } = fixtureGram('toeslagen', 'decretogram');
    expect(zaakkenmerkOf(gram)).toBe('zorgtoeslag/999993653');
    expect(describeZaak(gram)).toBe('zaak zorgtoeslag/999993653');
  });

  it('zwijgt over een gram dat er geen draagt', () => {
    expect(zaakkenmerkOf({ fields: { bsn: { value: '1' } } })).toBeNull();
    expect(describeZaak(null)).toBe('');
  });

  it('vindt het gram terug waar een journaalregel naar wijst', () => {
    const row = allGrams(worldFixture).find((candidate) => candidate.kind === 'decretogram');
    expect(gramByRef(worldFixture, { id: row.id })).toBe(row.gram);
  });

  it('valt op een onleesbare verwijzing terug op niets', () => {
    expect(gramByRef(worldFixture, { id: 'toeslagen|beschikkingen' })).toBeNull();
    expect(gramByRef(worldFixture, { id: 'nergens|iets|0' })).toBeNull();
    expect(gramByRef(worldFixture, null)).toBeNull();
  });

  // Een plek die geen rij cijfers is, hoort niets aan te wijzen. `Number('')` is
  // 0, dus zonder toets op de tekst zou een id met een lege plek het eerste gram
  // van de kroniek opleveren: een verwijzing die klopt lijkt te zijn.
  it('wijst bij een plek die geen getal is niets aan, ook niet het eerste gram', () => {
    const { chronicle } = fixtureGram('toeslagen', 'decretogram');
    for (const plek of ['', ' 0 ', '0x0', '-1', '1e0']) {
      expect(gramByRef(worldFixture, { id: `toeslagen|${chronicle.stream}|${plek}` })).toBeNull();
    }
  });
});
