import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import CellColumn from './CellColumn.vue';
import { fixtureCell, worldFixture } from '../testing/worldFixture.js';
import { gramCounts, streamKey } from '../world/snapshot.js';

const clock = worldFixture.clock;

function mountCell(id, props = {}) {
  return mount(CellColumn, { props: { cell: fixtureCell(id), clock, ...props } });
}

/** Elke waarde van een attribuut over alle elementen van een soort. */
function attrs(wrapper, selector, name) {
  return wrapper.findAll(selector).map((element) => element.attributes(name));
}

/**
 * De lijsten die een kroniek tonen.
 *
 * Op hun toegankelijke naam en niet op `nldd-list` alleen: de kolom draagt ook
 * de lijst met het decretogram-schema van de besluiten, en die gaat over de
 * vorm van een gram en niet over wat er in een kroniek ligt.
 */
function chronicleLists(wrapper) {
  return wrapper
    .findAll('nldd-list')
    .filter((list) => (list.attributes('accessible-label') ?? '').startsWith('Kroniek '));
}

describe('een kolom per cel', () => {
  it('zet de cel, haar regelingen, lexostatussen en besluiten in de kop', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    expect(wrapper.text()).toContain('toeslagen');
    const tags = attrs(wrapper, 'nldd-tag', 'text');
    for (const law of cell.laws) expect(tags).toContain(law);
    for (const definition of cell.lexostatussen) expect(tags).toContain(definition.name);
    // Bij een besluit staat het zaakkenmerk-sjabloon erbij: dat is de vorm die
    // de sleutel van de kroniek met beschikkingen krijgt, en die vorm hoort te
    // staan op de plek waar ze vandaan komt.
    for (const besluit of cell.besluiten) {
      expect(tags).toContain(`${besluit.name} · ${besluit.zaakkenmerk}`);
    }
  });

  it('noemt een cel zonder wetten een bron-cel', () => {
    expect(mountCell('burger').text()).toContain('bron-cel');
    expect(mountCell('toeslagen').text()).toContain('regelingen geladen');
  });

  it('geeft elke kroniek een eigen lijst, met haar sleutel erbij', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    expect(chronicleLists(wrapper)).toHaveLength(cell.chronicles.length);
    for (const chronicle of cell.chronicles) {
      expect(wrapper.text()).toContain(chronicle.stream);
      expect(wrapper.text()).toContain(`sleutel: ${chronicle.key}`);
    }
  });

  it('maakt een kroniek met besluiten een boom en een kroniek zonder een lijst', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    const types = chronicleLists(wrapper).map((list) => list.attributes('type'));
    const expected = cell.chronicles.map((chronicle) =>
      chronicle.grams.some((gram) => gram.kind === 'decretogram') ? 'tree' : 'list',
    );
    expect(types).toStrictEqual(expected);
    expect(types).toContain('tree');
    expect(types).toContain('list');
  });

  it('zet elk gram als rij, met zijn soort en zijn moment', () => {
    const wrapper = mountCell('belastingdienst');
    const cell = fixtureCell('belastingdienst');
    const total = cell.chronicles.reduce((sum, chronicle) => sum + chronicle.grams.length, 0);
    expect(wrapper.findAll('nldd-list-item:not([slot="children"])')).toHaveLength(total);

    const texts = attrs(wrapper, 'nldd-text-cell', 'text');
    for (const chronicle of cell.chronicles) {
      for (const gram of chronicle.grams) expect(texts).toContain(gram.name);
    }
    const tags = attrs(wrapper, 'nldd-tag', 'text');
    expect(tags).toContain('Decretogram');
    expect(tags).toContain('Executogram');

    const supporting = attrs(wrapper, 'nldd-text-cell', 'supporting-text');
    expect(supporting.some((text) => text?.includes('01-03-2024'))).toBe(true);
  });

  // `status` en `position` zijn de namen van de tijdlijn-cel zelf. Onder een
  // andere naam blijft het attribuut op het element staan en doet het niets: de
  // lijn zou dan overal 'past' en overal 'between' tekenen zonder dat iets faalt.
  it('scheidt op de lijn wat vóór en wat ná de klok ligt', () => {
    const wrapper = mount(CellColumn, { props: { cell: fixtureCell('toeslagen'), clock: '2024-05-01' } });
    const steps = attrs(wrapper, 'nldd-timeline-track-cell', 'status');
    expect(steps).toContain('past');
    expect(steps).toContain('future');
  });

  it('laat de lijn beginnen, doorlopen en eindigen met de kroniek', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    const expected = cell.chronicles.flatMap((chronicle) =>
      chronicle.grams.map((_, index, grams) => {
        if (grams.length === 1) return 'only';
        if (index === 0) return 'first';
        return index === grams.length - 1 ? 'last' : 'between';
      }),
    );
    expect(attrs(wrapper, 'nldd-timeline-track-cell', 'position')).toStrictEqual(expected);
    // Een kroniek met één gram: een spoor van één punt krijgt aan geen van beide
    // kanten een lijn. 'none' bestaat wel op de cel, maar als status en niet als plek.
    expect(expected).toContain('only');
  });

  it('markeert een gram dat erbij kwam sinds de vorige stand', () => {
    const previousCounts = gramCounts(worldFixture);
    // Doe alsof er één betaling minder lag: de laatste is dan nieuw.
    const key = streamKey('belastingdienst', 'betalingen');
    previousCounts.set(key, previousCounts.get(key) - 1);
    const wrapper = mount(CellColumn, {
      props: { cell: fixtureCell('belastingdienst'), clock, previousCounts },
    });
    expect(attrs(wrapper, 'nldd-tag', 'text').filter((text) => text === 'nieuw')).toHaveLength(1);
  });

  it('markeert niets zonder een vorige stand', () => {
    expect(attrs(mountCell('belastingdienst'), 'nldd-tag', 'text')).not.toContain('nieuw');
  });
});

describe('de herkomst in een decretogram', () => {
  /** De rij van het eerste besluit in deze cel. */
  function decisionRow(wrapper) {
    return wrapper.findAll('nldd-list-item').find((row) => row.attributes('button') !== undefined);
  }

  it('laat een decretogram uitklappen en een executogram niet', () => {
    const decisions = mountCell('toeslagen');
    expect(decisionRow(decisions)).toBeDefined();

    const source = mountCell('burger');
    expect(decisionRow(source)).toBeUndefined();
  });

  // De rij zelf is de disclosure: de kinderrijen staan in de `children`-groep die
  // het ontwerpsysteem verbergt zolang `expanded` uit staat. De uitklap is dus het
  // attribuut, niet het in- en uitbouwen van de rijen.
  it('toont per waarde waar ze vandaan komt, achter de uitklap van de rij', async () => {
    const wrapper = mountCell('toeslagen');
    const row = decisionRow(wrapper);
    expect(row.attributes('expanded')).toBe('false');
    expect(wrapper.findAll('[slot="children"]').length).toBeGreaterThan(0);

    await row.trigger('click');
    expect(row.attributes('expanded')).toBe('true');

    const labels = attrs(wrapper, 'nldd-tag', 'text');
    expect(labels).toContain("geaccepteerd van cel 'belastingdienst'");
    expect(labels).toContain('berekend door de regeling');
    expect(labels).toContain("uit de eigen kroniek 'inkomensleveringen'");
    expect(labels).toContain('opgave bij de actie');

    // De onderbouwing van een geaccepteerde waarde: lexostatus, moment, ondertekening.
    const details = attrs(wrapper, 'nldd-text-cell', 'supporting-text').filter(Boolean);
    expect(details.some((text) => text.includes('lexostatus:') && text.includes('ondertekend:'))).toBe(true);
  });

  it('toont de wetsversie en de verplichtingen van het besluit', async () => {
    const wrapper = mountCell('toeslagen');
    await decisionRow(wrapper).trigger('click');
    expect(wrapper.find('nldd-icon-cell[disclosure]').exists()).toBe(true);

    const texts = attrs(wrapper, 'nldd-text-cell', 'text');
    expect(texts).toContain('Wetsversie');
    expect(texts).toContain('Verplichting 1');
    expect(texts).toContain('wet_op_de_zorgtoeslag');

    const supporting = attrs(wrapper, 'nldd-text-cell', 'supporting-text').filter(Boolean);
    expect(supporting.some((text) => text.startsWith('in werking vanaf'))).toBe(true);
    expect(supporting.some((text) => text.includes('vervaldatum:'))).toBe(true);
  });

  // De wet bepaalt wie het bevoegd gezag is. Declareert ze niemand, dan viel er
  // niets te toetsen, en dat hoort bij het gram te staan waar het over gaat —
  // niet alleen in een lijst waarschuwingen elders op de pagina. De besluiten
  // van de fixture zijn wél door hun bevoegd gezag genomen, dus de melding hoort
  // daar juist niet te staan: zonder dat tweede geval meet de eerste niets.
  it('meldt bij een gram dat de regeling geen bevoegd gezag declareert', async () => {
    const wrapper = mountCell('toeslagen');
    await decisionRow(wrapper).trigger('click');
    expect(attrs(wrapper, 'nldd-text-cell', 'text')).not.toContain('Regeling declareert geen bevoegd gezag');

    const cell = structuredClone(fixtureCell('toeslagen'));
    for (const chronicle of cell.chronicles) {
      for (const gram of chronicle.grams) {
        if (gram.kind === 'decretogram') gram.fields.competent_authority.value = null;
      }
    }
    const zonder = mount(CellColumn, { props: { cell, clock } });
    await decisionRow(zonder).trigger('click');
    expect(attrs(zonder, 'nldd-text-cell', 'text')).toContain('Regeling declareert geen bevoegd gezag');
  });

  // Het soort gram zegt dát er besloten is, het type zegt wát er besloten is.
  // Een beschikking omvat ook de afwijzing van de aanvraag, dus zonder dit tweede
  // label zou een weigering in de lijst op een toekenning lijken — en aan het
  // bedrag is het verschil niet te zien: de regeling rekent er ook bij een
  // afwijzing nog een uit.
  it('zet het besluittype als tag naast het gram-soort', () => {
    const tags = attrs(mountCell('toeslagen'), 'nldd-tag', 'text');
    expect(tags).toContain('Decretogram');
    expect(tags).toContain('TOEKENNING');
    expect(tags).not.toContain('AFWIJZING');
  });

  it('toont een afwijzing als zodanig, met haar grond achter de uitklap', async () => {
    const cell = structuredClone(fixtureCell('toeslagen'));
    for (const chronicle of cell.chronicles) {
      for (const gram of chronicle.grams) {
        if (gram.kind !== 'decretogram') continue;
        gram.fields.decision_type.value = 'AFWIJZING';
        gram.fields.afwijzingsgrond.value = [
          { output: 'heeft_recht_op_zorgtoeslag', value: false, article: '2' },
        ];
      }
    }
    const wrapper = mount(CellColumn, { props: { cell, clock } });
    expect(attrs(wrapper, 'nldd-tag', 'text')).toContain('AFWIJZING');

    await decisionRow(wrapper).trigger('click');
    const texts = attrs(wrapper, 'nldd-text-cell', 'text');
    expect(texts).toContain('Afwijzingsgrond: Heeft recht op zorgtoeslag');
    const supporting = attrs(wrapper, 'nldd-text-cell', 'supporting-text').filter(Boolean);
    expect(supporting).toContain('nee · artikel 2');
  });

  it('klapt weer dicht', async () => {
    const wrapper = mountCell('toeslagen');
    const row = decisionRow(wrapper);
    await row.trigger('click');
    await row.trigger('click');
    expect(row.attributes('expanded')).toBe('false');
  });
});
