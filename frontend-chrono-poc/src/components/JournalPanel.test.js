import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import JournalPanel from './JournalPanel.vue';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';

// Het journaal op de fixture van de simulator zelf: het beeld dat de wereld
// geeft, niet een verzonnen exemplaar. Verschuift het contract, dan valt deze
// test en niet de browser.

/** De rijen van het journaal: de bovenste laag, zonder de uitklap eronder. */
function rows(wrapper) {
  return wrapper.findAll('nldd-list-item').filter((item) => item.attributes('slot') !== 'children');
}

/** De uitgeklapte regels van één geopende rij. */
function children(wrapper) {
  return wrapper.findAll('nldd-list-item').filter((item) => item.attributes('slot') === 'children');
}

/** Alle teksten van een rij, uit de attributen waarin ze staan. */
function texts(row) {
  return row
    .findAll('nldd-text-cell')
    .flatMap((cell) => [cell.attributes('text'), cell.attributes('supporting-text')])
    .filter(Boolean);
}

function mountPanel(props = {}) {
  return mount(JournalPanel, { props: { snapshot: worldFixture, ...props } });
}

describe('het journaal', () => {
  it('zet elke gebeurtenis op een rij, in de volgorde van het beeld', () => {
    const wrapper = mountPanel();
    const lijst = rows(wrapper);
    expect(lijst).toHaveLength(worldFixture.journal.length);

    // Het moment, de actor en de omschrijving staan in de rij zelf.
    const eerste = texts(lijst[0]);
    expect(eerste).toContain('10-01-2024');
    expect(eerste).toContain('burger');
    expect(eerste).toContain(worldFixture.journal[0].description);
  });

  it('zet het verschil in de stand van de zaak onder de omschrijving', () => {
    const wrapper = mountPanel();
    const besluit = worldFixture.journal.find((entry) => entry.kind === 'besluit');
    const rij = rows(wrapper)[besluit.seq];
    const regel = texts(rij).join(' | ');

    // "was → is", en "niets vastgesteld" is een stand en geen lege waarde.
    expect(regel).toContain(besluit.changes[0].label);
    expect(regel).toContain('niets vastgesteld →');
  });

  it('springt een vraag over de celgrens in onder het besluit dat haar uitlokte', () => {
    const wrapper = mountPanel();
    const vraag = worldFixture.journal.find((entry) => entry.kind === 'vraag');
    expect(vraag.parent).not.toBeNull();

    // De inspringing is de eerste cel van de rij; het besluit erboven begint bij
    // zijn icoon.
    const vraagRij = rows(wrapper)[vraag.seq];
    const besluitRij = rows(wrapper)[vraag.parent];
    expect(vraagRij.element.firstElementChild.tagName.toLowerCase()).toBe('nldd-spacer-cell');
    expect(besluitRij.element.firstElementChild.tagName.toLowerCase()).toBe('nldd-icon-cell');
    expect(texts(vraagRij)).toContain(vraag.description);
  });

  it('klapt een regel uit naar haar grammen, wat ze accepteerde en wat ze veranderde', async () => {
    const wrapper = mountPanel();
    const besluit = worldFixture.journal.find((entry) => entry.kind === 'besluit');
    expect(children(wrapper)).toHaveLength(0);

    await rows(wrapper)[besluit.seq].trigger('click');
    const uitklap = children(wrapper).map((item) => texts(item).join(' | '));

    expect(uitklap.some((regel) => regel.includes(besluit.grams[0].name))).toBe(true);
    expect(uitklap.some((regel) => regel.includes('accepteerde toetsingsinkomen'))).toBe(true);
    expect(uitklap.some((regel) => regel.includes(besluit.changes[0].label))).toBe(true);
  });

  it('zet de zaak vooraan bij een gram dat er een draagt', async () => {
    const wrapper = mountPanel();
    const besluit = worldFixture.journal.find((entry) => entry.kind === 'besluit');
    await rows(wrapper)[besluit.seq].trigger('click');

    const gramRij = children(wrapper).find((item) => texts(item).includes(besluit.grams[0].name));
    expect(texts(gramRij).some((tekst) => tekst.startsWith('zaak zorgtoeslag/'))).toBe(true);
  });

  it('opent het gram achter een regel, zoals de cel het in haar kroniek toont', async () => {
    const wrapper = mountPanel();
    const besluit = worldFixture.journal.find((entry) => entry.kind === 'besluit');
    await rows(wrapper)[besluit.seq].trigger('click');

    const gramRij = children(wrapper).find((item) => texts(item).includes(besluit.grams[0].name));
    await gramRij.trigger('click');

    const dialog = wrapper.find('nldd-modal-dialog');
    expect(dialog.attributes('text')).toBe(besluit.grams[0].name);
    expect(dialog.attributes('supporting-text')).toContain(besluit.grams[0].chronicle);
    // Het ruwe gram gaat mee, zonder uittreksel: het receipt zit er niet in,
    // want het beeld draagt hem niet.
    expect(dialog.find('nldd-code-viewer').text()).toContain('"op_moment"');
  });

  it('markeert wat er sinds de vorige stap bij kwam', () => {
    const wrapper = mountPanel({ previousLength: worldFixture.journal.length - 1 });
    const nieuw = wrapper.findAll('nldd-tag').filter((tag) => tag.attributes('text') === 'nieuw');
    expect(nieuw).toHaveLength(1);
  });

  it('markeert niets zolang er geen stap geweest is', () => {
    const wrapper = mountPanel();
    expect(wrapper.findAll('nldd-tag').filter((tag) => tag.attributes('text') === 'nieuw')).toHaveLength(0);
  });

  it('filtert op actor en laat de rest staan als weg terug', async () => {
    const wrapper = mountPanel();
    const dropdown = wrapper
      .findAll('nldd-dropdown')
      .find((item) => item.attributes('accessible-label') === 'Filter op actor');
    dropdown.element.dispatchEvent(new CustomEvent('change', { detail: { value: 'klok' } }));
    await wrapper.vm.$nextTick();

    const zichtbaar = rows(wrapper).filter((row) => row.attributes('hidden') === undefined);
    const verwacht = worldFixture.journal.filter((entry) => entry.actor.soort === 'klok');
    expect(zichtbaar).toHaveLength(verwacht.length);
    // Gefilterd is niet weg: de rijen blijven staan met `hidden`, zodat de lijst
    // zelf kan zeggen dat er niets gevonden is.
    expect(rows(wrapper)).toHaveLength(worldFixture.journal.length);
  });

  it('filtert op cel, en telt een cel mee waar alleen de stand veranderde', async () => {
    const wrapper = mountPanel();
    const dropdown = wrapper
      .findAll('nldd-dropdown')
      .find((item) => item.attributes('accessible-label') === 'Filter op cel');
    dropdown.element.dispatchEvent(new CustomEvent('change', { detail: { value: 'belastingdienst' } }));
    await wrapper.vm.$nextTick();

    const zichtbaar = rows(wrapper).filter((row) => row.attributes('hidden') === undefined);
    expect(zichtbaar.length).toBeGreaterThan(0);
    expect(zichtbaar.length).toBeLessThan(worldFixture.journal.length);
  });

  it('houdt alleen de dag over waarop de tijdlijn wijst, met een weg terug', async () => {
    const dag = worldFixture.journal[0].moment;
    const wrapper = mountPanel({ focusMoment: dag });
    const zichtbaar = rows(wrapper).filter((row) => row.attributes('hidden') === undefined);
    expect(zichtbaar).toHaveLength(worldFixture.journal.filter((entry) => entry.moment === dag).length);

    const knop = wrapper.findAll('nldd-button').find((button) => button.attributes('text')?.includes('toon alles'));
    await knop.trigger('click');
    expect(wrapper.emitted('clear-focus')).toHaveLength(1);
  });

  it('zegt zelf dat er nog niets gebeurd is, in plaats van leeg te blijven', () => {
    const leeg = cloneWorld();
    leeg.journal = [];
    const wrapper = mountPanel({ snapshot: leeg });
    expect(rows(wrapper)).toHaveLength(0);
    const dialogen = wrapper.findAll('nldd-inline-dialog').map((item) => item.attributes('text'));
    expect(dialogen).toContain('Er is nog niets gebeurd');
  });

  it('valt niet om op een beeld zonder journaal', () => {
    const wrapper = mountPanel({ snapshot: {} });
    expect(rows(wrapper)).toHaveLength(0);
  });
});
