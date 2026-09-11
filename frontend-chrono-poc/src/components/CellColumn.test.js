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

describe('een kolom per cel', () => {
  it('zet de cel, haar regelingen, lexostatussen en besluiten in de kop', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    expect(wrapper.text()).toContain('toeslagen');
    const tags = attrs(wrapper, 'nldd-tag', 'text');
    for (const law of cell.laws) expect(tags).toContain(law);
    for (const name of cell.lexostatussen) expect(tags).toContain(name);
    for (const besluit of cell.besluiten) expect(tags).toContain(besluit);
  });

  it('noemt een cel zonder wetten een bron-cel', () => {
    expect(mountCell('burger').text()).toContain('bron-cel');
    expect(mountCell('toeslagen').text()).toContain('regelingen geladen');
  });

  it('geeft elke kroniek een eigen lijst, met haar sleutel erbij', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    expect(wrapper.findAll('nldd-list')).toHaveLength(cell.chronicles.length);
    for (const chronicle of cell.chronicles) {
      expect(wrapper.text()).toContain(chronicle.stream);
      expect(wrapper.text()).toContain(`sleutel: ${chronicle.key}`);
    }
  });

  it('maakt een kroniek met besluiten een boom en een kroniek zonder een lijst', () => {
    const wrapper = mountCell('toeslagen');
    const cell = fixtureCell('toeslagen');
    const types = wrapper.findAll('nldd-list').map((list) => list.attributes('type'));
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

  it('scheidt op de lijn wat vóór en wat ná de klok ligt', () => {
    const wrapper = mount(CellColumn, { props: { cell: fixtureCell('toeslagen'), clock: '2024-05-01' } });
    const steps = attrs(wrapper, 'nldd-timeline-track-cell', 'step');
    expect(steps).toContain('past');
    expect(steps).toContain('future');
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

  it('klapt weer dicht', async () => {
    const wrapper = mountCell('toeslagen');
    const row = decisionRow(wrapper);
    await row.trigger('click');
    await row.trigger('click');
    expect(row.attributes('expanded')).toBe('false');
  });
});
