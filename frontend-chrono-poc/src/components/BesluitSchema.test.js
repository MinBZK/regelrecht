import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import BesluitSchema from './BesluitSchema.vue';
import { fixtureCell } from '../testing/worldFixture.js';

/** Elke waarde van een attribuut over alle elementen van een soort. */
function attrs(wrapper, selector, name) {
  return wrapper.findAll(selector).map((element) => element.attributes(name));
}

function mountSchema(id) {
  return mount(BesluitSchema, { props: { cell: fixtureCell(id) } });
}

describe('het schema van een decretogram', () => {
  it('geeft elk besluit een rij, met hoeveel velden het heeft en hoeveel gaten', () => {
    const wrapper = mountSchema('toeslagen');
    const cell = fixtureCell('toeslagen');
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(cell.besluiten.length);
    const namen = attrs(wrapper, 'nldd-text-cell', 'text');
    const samenvattingen = attrs(wrapper, 'nldd-text-cell', 'supporting-text');
    for (const besluit of cell.besluiten) {
      expect(namen).toContain(besluit.name);
      const gaten = besluit.schema.filter((field) => field.gat).length;
      expect(samenvattingen).toContain(`${besluit.schema.length} velden · ${gaten} gaten`);
    }
  });

  // Vijftien rijen per besluit horen niet standaard open te staan in een kolom
  // die over de kronieken gaat; wie het schema wil zien, klapt het open.
  it('toont de tabel pas als de rij opengeklapt is', async () => {
    const wrapper = mountSchema('toeslagen');
    expect(wrapper.find('nldd-table').exists()).toBe(false);

    await wrapper.find('nldd-list-item').trigger('click');
    const table = wrapper.find('nldd-table');
    expect(table.exists()).toBe(true);
    expect(table.attributes('accessible-label')).toContain(fixtureCell('toeslagen').besluiten[0].name);
  });

  it('zet elk veld in de tabel, met zijn type en een label per herkomst', async () => {
    const wrapper = mountSchema('toeslagen');
    await wrapper.find('nldd-list-item').trigger('click');

    const besluit = fixtureCell('toeslagen').besluiten[0];
    // De koprij plus één rij per veld.
    expect(wrapper.findAll('nldd-table-row')).toHaveLength(besluit.schema.length + 1);
    const tags = attrs(wrapper, 'nldd-tag', 'text');
    expect(tags).toContain('wet');
    expect(tags).toContain('platform');
    expect(tags).toContain('wereldbestand');
    expect(attrs(wrapper, 'nldd-text-cell', 'text')).toContain('Hoogte zorgtoeslag');
  });

  // Gaten zijn de reden dat dit schema bestaat, dus ze staan er anders bij — en
  // niet alleen in een kleur: een tekstlabel doet het ook zonder kleurverschil.
  it('markeert elk gat met een eigen label', async () => {
    const wrapper = mountSchema('toeslagen');
    await wrapper.find('nldd-list-item').trigger('click');

    const gaten = fixtureCell('toeslagen').besluiten[0].schema.filter((field) => field.gat);
    expect(gaten.length).toBeGreaterThan(0);
    expect(attrs(wrapper, 'nldd-tag', 'text').filter((text) => text === 'gat')).toHaveLength(gaten.length);
  });

  it('toont niets voor een cel die niet kan besluiten', () => {
    expect(mountSchema('burger').find('nldd-list').exists()).toBe(false);
  });
});
