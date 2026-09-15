import { mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import LexostatusPanel from './LexostatusPanel.vue';
import { cloneWorld, fixtureCell, worldFixture } from '../testing/worldFixture.js';

/** De cellen die iets publiceren, in de volgorde waarin het beeld ze geeft. */
const publishers = worldFixture.cells.filter((cell) => cell.lexostatussen.length > 0);
/** De cel en de naam waar het paneel uit zichzelf op staat. */
const firstCell = publishers[0];
const firstDefinition = firstCell.lexostatussen[0];

const established = {
  cell: 'belastingdienst',
  name: 'toetsingsinkomen',
  op_moment: '2025-02-01',
  outcome: { established: { toetsingsinkomen: 81000, competent_authority: 'Belastingdienst' } },
  reductie: {
    vorm: {
      soort: 'kroniekfilter',
      chronicle: 'aanslagen',
      key: 'bsn',
      key_value: '999993653',
      where: {},
      regel: { regel: 'laatste' },
      op_moment: '2025-02-01',
    },
    grammen: [
      {
        cell: 'belastingdienst',
        chronicle: 'aanslagen',
        id: 'belastingdienst|aanslagen|1',
        kind: 'decretogram',
        name: 'aanslag_herzien',
        volgnummer: 1,
        op_moment: '2024-12-01',
        regulation_valid_from: null,
        bijdrage: null,
      },
    ],
  },
};

const nothing = {
  cell: 'belastingdienst',
  name: 'toetsingsinkomen',
  op_moment: '2025-02-01',
  outcome: { not_established: { reason: "cel 'belastingdienst' heeft hierover niets vastgelegd" } },
  reductie: {
    vorm: {
      soort: 'kroniekfilter',
      chronicle: 'aanslagen',
      key: 'bsn',
      key_value: '999993653',
      where: {},
      regel: { regel: 'laatste' },
      op_moment: '2025-02-01',
    },
    grammen: [],
    gemist: { in_de_stroom: 2, na_het_moment: 0, andere_sleutel: 2, buiten_de_voorwaarden: 0 },
  },
};

// Een wetsvorm: de engine rekende, en per input staat erbij waar hij vandaan
// kwam — de een uit de vraag, de ander uit een eigen kroniek met het gram dat
// hem droeg.
const berekend = {
  cell: 'toeslagen',
  name: 'zorgtoeslag_rechtstoestand',
  op_moment: '2025-02-01',
  outcome: { established: { heeft_recht_op_zorgtoeslag: true } },
  reductie: {
    vorm: {
      soort: 'wetsvorm',
      regulation: 'wet_op_de_zorgtoeslag',
      regulation_valid_from: '2025-01-01',
      output: 'heeft_recht_op_zorgtoeslag',
      inputs: [
        { name: 'bsn', herkomst: { herkomst: 'parameter', parameter: 'bsn' } },
        {
          name: 'verzamelinkomen',
          herkomst: {
            herkomst: 'eigen_kroniek',
            chronicle: 'inkomensleveringen',
            gram: 'toeslagen|inkomensleveringen|0',
          },
        },
      ],
      op_moment: '2025-02-01',
    },
    grammen: [
      {
        cell: 'toeslagen',
        chronicle: 'inkomensleveringen',
        id: 'toeslagen|inkomensleveringen|0',
        kind: 'lexogram',
        name: 'inkomenslevering',
        volgnummer: 0,
        op_moment: '2024-11-15',
        regulation_valid_from: null,
        bijdrage: null,
      },
    ],
  },
};

function mountPanel(answer) {
  const ask = vi.fn(async () => answer);
  return { wrapper: mount(LexostatusPanel, { props: { snapshot: worldFixture, ask } }), ask };
}

/** Een waarde in een veld zetten zoals een ontwerpsysteem-veld dat meldt. */
async function fill(wrapper, element, value) {
  element.element.dispatchEvent(new CustomEvent('input', { detail: { value } }));
  await wrapper.vm.$nextTick();
}

/** Een keuze maken in de zoveelste dropdown, zoals het ontwerpsysteem dat meldt. */
async function choose(wrapper, index, value) {
  const dropdown = wrapper.findAll('nldd-dropdown')[index];
  dropdown.element.dispatchEvent(new CustomEvent('change', { detail: { value } }));
  await wrapper.vm.$nextTick();
  await wrapper.vm.$nextTick();
}

/** Het formulier versturen en de ronde afwachten. */
async function submit(wrapper) {
  await wrapper.find('form').trigger('submit');
  await wrapper.vm.$nextTick();
  await wrapper.vm.$nextTick();
}

describe('een vraag aan een cel', () => {
  it('biedt alleen cellen aan die iets publiceren, met hun eigen namen', () => {
    const { wrapper } = mountPanel(established);
    const selects = wrapper.findAll('select');
    expect(selects[0].findAll('option').map((option) => option.attributes('value'))).toStrictEqual(
      publishers.map((cell) => cell.id),
    );
    expect(selects[1].findAll('option').map((option) => option.attributes('value'))).toStrictEqual(
      firstCell.lexostatussen.map((definition) => definition.name),
    );
  });

  // Het formulier komt uit de definitie: de cel accepteert precies deze namen,
  // dus ze horen niet geraden te worden.
  it('maakt van elke gedocumenteerde parameter een veld', () => {
    const { wrapper } = mountPanel(established);
    expect(wrapper.findAll('nldd-form-field').map((field) => field.attributes('label'))).toStrictEqual([
      'Cel',
      'Lexostatus',
      'Op moment',
      ...firstDefinition.inputs.map((input) => input.name),
    ]);
  });

  it('zet de toelichting van de gekozen definitie erbij', () => {
    const { wrapper } = mountPanel(established);
    const dialogs = wrapper.findAll('nldd-inline-dialog');
    const toelichting = dialogs.find((dialog) => dialog.attributes('text') === firstDefinition.name);
    expect(toelichting.attributes('supporting-text')).toBe(firstDefinition.doc);
  });

  // Het zaakkenmerk "ontstaat uit het niets" zolang er nergens staat dat het de
  // sleutel van een kroniek is en welke vorm het heeft.
  it('zegt bij de sleutel van een reductie uit welke kroniek ze komt', () => {
    const { wrapper } = mountPanel(established);
    const field = wrapper
      .findAll('nldd-form-field')
      .find((candidate) => candidate.attributes('label') === firstDefinition.key.parameter);
    expect(field.attributes('supporting-label')).toBe(`sleutel van kroniek '${firstDefinition.key.chronicle}'`);
  });

  // En de vorm erbij zodra de besluiten van de cel er een sjabloon voor hebben.
  it('noemt de vorm van het zaakkenmerk bij de cel die erover besluit', async () => {
    const { wrapper } = mountPanel(established);
    const besluitende = worldFixture.cells.find((cell) => cell.besluiten.length > 0);
    const beschikkingen = besluitende.lexostatussen.find((definition) => definition.key?.chronicle === 'beschikkingen');
    await choose(wrapper, 0, besluitende.id);
    await choose(wrapper, 1, beschikkingen.name);

    const field = wrapper
      .findAll('nldd-form-field')
      .find((candidate) => candidate.attributes('label') === beschikkingen.key.parameter);
    expect(field.attributes('supporting-label')).toBe(
      `sleutel van kroniek 'beschikkingen', vorm '${besluitende.besluiten[0].zaakkenmerk}'`,
    );
  });

  // De waarden die er nu liggen, als keuze. Vrije invoer blijft: een vraag over
  // een zaak die er nog niet is, is een geldige vraag.
  it('biedt de bekende sleutels van de kroniek aan, met vrije invoer', () => {
    const { wrapper } = mountPanel(established);
    const combo = wrapper.find('nldd-combo-box');
    expect(combo.attributes('allow-custom')).toBeDefined();
    const chronicle = fixtureCell(firstCell.id).chronicles.find(
      (candidate) => candidate.stream === firstDefinition.key.chronicle,
    );
    const known = [...new Set(chronicle.grams.map((gram) => String(gram.fields[chronicle.key].value)))];
    expect(wrapper.findAll('nldd-menu-item').map((item) => item.attributes('value'))).toStrictEqual(known);
  });

  it('stelt de vraag met de ingevulde parameters, op de stand van de klok', async () => {
    const { wrapper, ask } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-combo-box'), 'zorgtoeslag/999993653');
    await submit(wrapper);

    expect(ask).toHaveBeenCalledWith(
      firstCell.id,
      firstDefinition.name,
      { [firstDefinition.key.parameter]: 'zorgtoeslag/999993653' },
      worldFixture.clock,
    );
  });

  // Een leeg veld gaat niet mee: dan weigert de cel de vraag met "parameter
  // ontbreekt", en dat is de melding die erbij hoort.
  it('laat een leeg veld weg in plaats van er een lege tekst van te maken', async () => {
    const { wrapper, ask } = mountPanel(established);
    await submit(wrapper);
    expect(ask).toHaveBeenCalledWith(firstCell.id, firstDefinition.name, {}, worldFixture.clock);
  });

  // Het tijdreizen zelf: dezelfde cel, dezelfde naam, een eerder moment. Het
  // veld begint op de klok, dus wie niets kiest vraagt naar nu.
  it('vraagt naar een eerder moment zodra de bezoeker er een kiest', async () => {
    const { wrapper, ask } = mountPanel(established);
    const moment = wrapper.find('nldd-date-field');
    expect(moment.attributes('value')).toBe(worldFixture.clock);
    expect(moment.attributes('max')).toBe(worldFixture.clock);

    await fill(wrapper, moment, '2024-06-01');
    await submit(wrapper);

    expect(ask).toHaveBeenCalledWith(expect.any(String), expect.any(String), {}, '2024-06-01');
  });

  // Wie zelf niets kiest, vraagt naar nu — en "nu" verschuift met de klok. Bleef
  // de oude klokstand als waarde in het veld staan, dan gaf dezelfde knop na het
  // vooruitspoelen stil een antwoord over gisteren.
  it('volgt de klok zolang de bezoeker zelf geen moment koos', async () => {
    const { wrapper, ask } = mountPanel(established);
    const later = cloneWorld();
    later.clock = '2025-06-01';
    await wrapper.setProps({ snapshot: later });

    expect(wrapper.find('nldd-date-field').attributes('value')).toBe('2025-06-01');
    await submit(wrapper);
    expect(ask).toHaveBeenCalledWith(expect.any(String), expect.any(String), {}, '2025-06-01');
  });

  // Een gekozen moment is een keuze en geen afspiegeling van de klok:
  // vooruitspoelen maakt een eerder moment niet ongeldig.
  it('houdt een gekozen moment vast als de klok vooruit gaat', async () => {
    const { wrapper, ask } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-date-field'), '2024-06-01');
    const later = cloneWorld();
    later.clock = '2025-06-01';
    await wrapper.setProps({ snapshot: later });

    expect(wrapper.find('nldd-date-field').attributes('value')).toBe('2024-06-01');
    await submit(wrapper);
    expect(ask).toHaveBeenCalledWith(expect.any(String), expect.any(String), {}, '2024-06-01');
  });

  // `POST /api/reset` zet de klok terug op de startdag. Een keuze van ná die dag
  // zou daar stil een 409 opleveren; ze vervalt, en het veld volgt de klok weer.
  it('laat een keuze los die na een reset ná de klok zou liggen', async () => {
    const { wrapper } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-date-field'), '2024-06-01');
    const start = cloneWorld();
    start.clock = '2024-01-01';
    await wrapper.setProps({ snapshot: start });

    const moment = wrapper.find('nldd-date-field');
    expect(moment.attributes('value')).toBe('2024-01-01');
    expect(moment.attributes('invalid')).toBeUndefined();
  });

  // Ná de klok heeft nog niets vastgelegd; een antwoord "op" zo'n moment zou een
  // voorspelling zijn. De server weigert het met een 409 — hier komt het niet
  // eens zo ver.
  it('stelt geen vraag over een moment ná de klok', async () => {
    const { wrapper, ask } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-date-field'), '2030-01-01');

    expect(wrapper.find('nldd-date-field').attributes('invalid')).toBe('true');
    expect(wrapper.find('nldd-form-field-error-text').text()).toContain('01-02-2025');
    expect(wrapper.find('nldd-button').attributes('disabled')).toBe('true');

    await submit(wrapper);
    expect(ask).not.toHaveBeenCalled();
  });

  it('toont een vastgesteld antwoord met zijn waarden en het moment', async () => {
    const { wrapper } = mountPanel(established);
    await submit(wrapper);
    expect(wrapper.text()).toContain('belastingdienst · toetsingsinkomen');
    expect(wrapper.text()).toContain('geldig op 01-02-2025');
    // De rijen van het antwoord zelf; de uitleg eronder staat in haar eigen
    // lijst (zie hieronder).
    const rows = wrapper.findAll('nldd-list:not([type="tree"]) nldd-list-item');
    expect(rows).toHaveLength(2);
    const values = wrapper.findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(values).toContain('Toetsingsinkomen');
    expect(values).toContain('81000');
  });

  it('behandelt "niets vastgesteld" als gewoon antwoord, met de reden van de cel', async () => {
    const { wrapper } = mountPanel(nothing);
    await submit(wrapper);
    const dialogs = wrapper.findAll('nldd-inline-dialog');
    const answer = dialogs.find((dialog) => dialog.attributes('text') === 'Niets vastgesteld');
    expect(answer).toBeDefined();
    // Geen alarm: dit is een antwoord, geen fout.
    expect(answer.attributes('variant')).toBeUndefined();
    expect(answer.attributes('icon')).toBe('info');
    expect(answer.attributes('supporting-text')).toContain('niets vastgelegd');
    expect(wrapper.findAll('nldd-list:not([type="tree"]) nldd-list-item')).toHaveLength(0);
  });

  // Een andere naam is een ander formulier; wat voor de vorige vraag ingevuld
  // stond, hoort dan niet stil mee te gaan naar een parameter die anders heet.
  it('laat een ingevulde waarde los zodra de parameter niet meer bestaat', async () => {
    const { wrapper, ask } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-combo-box'), 'zorgtoeslag/999993653');
    const anders = firstCell.lexostatussen.find((definition) => definition.name !== firstDefinition.name);
    await choose(wrapper, 1, anders.name);
    await submit(wrapper);

    expect(ask).toHaveBeenCalledWith(firstCell.id, anders.name, {}, worldFixture.clock);
  });
});

describe('hoe het antwoord tot stand kwam', () => {
  /** De regel van de uitklap: de enige `nldd-list-item` die geen kind is. */
  function disclosure(wrapper) {
    return wrapper.find('nldd-list[type="tree"] > nldd-list-item');
  }

  /** De rijen ín de uitklap. */
  function children(wrapper) {
    return wrapper.findAll('nldd-list-item[slot="children"]');
  }

  it('zet de regel onder het antwoord, dichtgeklapt', async () => {
    const { wrapper } = mountPanel(established);
    expect(disclosure(wrapper).exists()).toBe(false);

    await submit(wrapper);

    const row = disclosure(wrapper);
    expect(row.exists()).toBe(true);
    const cell = row.find('nldd-text-cell');
    expect(cell.attributes('text')).toBe('Zo is dit vastgesteld');
    expect(cell.attributes('supporting-text')).toBe(
      "laatste vastlegging in kroniek 'aanslagen' met bsn '999993653' op of vóór 01-02-2025",
    );
    // Dicht: de uitkomst is waar de vraag over ging.
    expect(row.attributes('expanded')).toBeUndefined();
    expect(children(wrapper)).toHaveLength(0);
  });

  it('toont na uitklappen het gelezen gram, en brengt de bezoeker erheen', async () => {
    const { wrapper } = mountPanel(established);
    await submit(wrapper);
    await disclosure(wrapper).trigger('click');

    const rows = children(wrapper);
    expect(rows).toHaveLength(1);
    const texts = rows[0].findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(texts).toContain('01-12-2024');
    expect(texts).toContain('aanslag_herzien');

    await rows[0].trigger('click');
    expect(wrapper.emitted('show-gram')).toStrictEqual([['belastingdienst|aanslagen|1']]);
  });

  it('zegt bij niets vastgesteld wat er gezocht is en wat er wél lag', async () => {
    const { wrapper } = mountPanel(nothing);
    await submit(wrapper);
    await disclosure(wrapper).trigger('click');

    const rows = children(wrapper);
    expect(rows).toHaveLength(1);
    const cell = rows[0].find('nldd-text-cell');
    expect(cell.attributes('text')).toBe('Geen gram gelezen');
    expect(cell.attributes('supporting-text')).toContain('2 over een ander onderwerp');
  });

  // Een input die uit een eigen kroniek kwam, draagt het gram dat hem droeg.
  // Alleen zeggen "uit kroniek X" laat de lezer zoeken naar welke vastlegging
  // dat dan was, terwijl het antwoord het weet.
  it('brengt de bezoeker van een input uit een kroniek naar het gram dat hem droeg', async () => {
    const { wrapper } = mountPanel(berekend);
    await submit(wrapper);
    await disclosure(wrapper).trigger('click');

    const rijen = children(wrapper);
    const uitDeVraag = rijen.find((rij) => rij.find('nldd-text-cell').attributes('text') === 'Bsn');
    expect(uitDeVraag.attributes('button')).toBeUndefined();

    const uitDeKroniek = rijen.find(
      (rij) => rij.find('nldd-text-cell').attributes('text') === 'Verzamelinkomen',
    );
    expect(uitDeKroniek.attributes('button')).toBe('true');
    await uitDeKroniek.trigger('click');
    expect(wrapper.emitted('show-gram')).toStrictEqual([['toeslagen|inkomensleveringen|0']]);
  });

  it('laat de uitklap weg als het antwoord niet zegt hoe het tot stand kwam', async () => {
    const { wrapper } = mountPanel({ ...established, reductie: null });
    await submit(wrapper);
    expect(disclosure(wrapper).exists()).toBe(false);
  });
});
