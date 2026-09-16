import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ReceiptPanel from './ReceiptPanel.vue';
import { cloneReceipt, receiptFixture } from '../testing/receiptFixture.js';

const fetchGramReceipt = vi.fn();
vi.mock('../api/worldApi.js', () => ({
  fetchGramReceipt: (...args) => fetchGramReceipt(...args),
}));

/** Het paneel, gemonteerd en met het antwoord al binnen. */
async function panel(answer = receiptFixture) {
  fetchGramReceipt.mockResolvedValue(answer);
  const wrapper = mount(ReceiptPanel, {
    props: { cell: 'toeslagen', chronicle: 'beschikkingen', index: 0 },
  });
  await vi.waitFor(() => expect(wrapper.find('nldd-activity-indicator').exists()).toBe(false));
  return wrapper;
}

/** De rijen van één tabel: de header telt niet mee. */
function tableRows(wrapper, index) {
  return wrapper
    .findAll('nldd-table')
    [index].findAll('nldd-table-row')
    .filter((row) => row.attributes('slot') !== 'header');
}

describe('het receipt van een decretogram', () => {
  beforeEach(() => {
    fetchGramReceipt.mockReset();
  });

  it('haalt het receipt op bij het gram dat het paneel aanwijst', async () => {
    await panel();
    expect(fetchGramReceipt).toHaveBeenCalledWith('toeslagen', 'beschikkingen', 0);
  });

  it('zegt erbij dat de tijdstempel wandkloktijd is', async () => {
    // Het label is het hele punt: dit is geen moment in de logische tijd van de
    // wereld, en daarom draagt het beeld dit receipt niet.
    const wrapper = await panel();
    const banner = wrapper.find('nldd-banner');
    expect(banner.attributes('text')).toContain(receiptFixture.timestamp.wall_clock);
    expect(banner.attributes('supporting-text')).toContain('wandkloktijd');
  });

  it('zet de geaccepteerde waarden in een tabel met bron en bevoegd gezag', async () => {
    const wrapper = await panel();
    const [row] = tableRows(wrapper, 0);
    const texts = row.findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(texts).toContain('Toetsingsinkomen');
    expect(texts).toContain("cel 'belastingdienst'");
    // Het gezag als tag, zodat het niet op een gewone waarde lijkt.
    expect(row.find('nldd-tag').attributes('text')).toBe('Belastingdienst');
    expect(
      row.findAll('nldd-text-cell').some((cell) => /lexostatus: toetsingsinkomen/.test(cell.attributes('supporting-text') ?? '')),
    ).toBe(true);
  });

  it('zegt het als de bron geen bevoegd gezag noemde, en verzint er geen', async () => {
    const receipt = cloneReceipt();
    receipt.accepted_values[0].authority = null;
    const wrapper = await panel(receipt);
    const [row] = tableRows(wrapper, 0);
    expect(row.find('nldd-tag').exists()).toBe(false);
    expect(
      row.findAll('nldd-text-cell').map((cell) => cell.attributes('text')),
    ).toContain('niet genoemd door de bron');
  });

  it('zet de geladen regelingen met hun hash in een tabel', async () => {
    const wrapper = await panel();
    const rows = tableRows(wrapper, 1);
    expect(rows).toHaveLength(receiptFixture.scope.loaded_regulations.length);
    const texts = rows[0].findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(texts[0]).toBe('wet_op_de_zorgtoeslag');
    expect(texts[1]).toContain('vanaf 01-01-2024');
    expect(texts[2]).toBe(receiptFixture.scope.loaded_regulations[0].hash);
  });

  it('toont elke sectie van het receipt met haar regels', async () => {
    const wrapper = await panel();
    const titles = wrapper.findAll('nldd-title').map((title) => title.text());
    // Het gram staat erbij, met zijn moment in de logische tijd naast de
    // wandkloktijd van de banner: dat die twee verschillen is het hele punt.
    expect(titles.join(' ')).toContain('Het gram waar dit receipt bij hoort');
    expect(titles.join(' ')).toContain('Herkomst van de uitvoering');
    expect(titles.join(' ')).toContain('Resultaten');

    const all = wrapper.findAll('nldd-list-item').map((item) =>
      item.findAll('nldd-text-cell').map((cell) => cell.attributes('text')),
    );
    expect(all).toContainEqual(['Op moment', receiptFixture.gram.op_moment]);
    expect(all).toContainEqual(['Regulation id', 'wet_op_de_zorgtoeslag']);
    expect(all).toContainEqual(['Outputs · hoogte zorgtoeslag', '197178.01']);
  });

  it('zegt het als een besluit niets van een ander overnam, in plaats van te zwijgen', async () => {
    // "Niets geaccepteerd" is zelf een uitspraak over invariant I5; een tabel die
    // verdwijnt laat een lezer raden of er niets was of niets geladen werd.
    const receipt = cloneReceipt();
    receipt.accepted_values = [];
    const wrapper = await panel(receipt);
    expect(tableRows(wrapper, 0)).toHaveLength(0);
    expect(wrapper.findAll('nldd-table')[0].attributes('empty-text')).toBe(
      'Geen geaccepteerde waarden',
    );
  });

  it('zegt het als het receipt geen wandkloktijd draagt', async () => {
    const receipt = cloneReceipt();
    receipt.timestamp.wall_clock = null;
    const wrapper = await panel(receipt);
    expect(wrapper.find('nldd-banner').attributes('text')).toBe(
      'Geen wandkloktijd vastgelegd bij deze uitvoering',
    );
  });

  it('zegt "geen receipt" met de melding van de server als er geen is', async () => {
    fetchGramReceipt.mockRejectedValue(new Error('dit gram draagt geen uitvoeringsreceipt'));
    const wrapper = mount(ReceiptPanel, {
      props: { cell: 'toeslagen', chronicle: 'aanvragen', index: 0 },
    });
    await vi.waitFor(() => expect(wrapper.find('nldd-inline-dialog').exists()).toBe(true));
    expect(wrapper.find('nldd-inline-dialog').attributes('supporting-text')).toBe(
      'dit gram draagt geen uitvoeringsreceipt',
    );
    expect(wrapper.find('nldd-table').exists()).toBe(false);
  });

  it('zet de uitvoeringstrace als boom neer, met de wortel al open', async () => {
    const wrapper = await panel();
    const tree = wrapper.findAll('nldd-list').find((list) => list.attributes('type') === 'tree');
    expect(tree).toBeDefined();

    const rows = tree.findAll('nldd-list-item');
    // De wortel staat open, dus haar directe stappen staan eronder; wat daar weer
    // onder hangt, staat er niet: dicht is dicht.
    expect(rows).toHaveLength(3);
    expect(rows[0].attributes('expanded')).toBe('true');
    expect(rows.slice(1).every((row) => row.attributes('slot') === 'children')).toBe(true);
  });

  it('noemt per stap van de trace de regeling en het artikel', async () => {
    const wrapper = await panel();
    const tree = wrapper.findAll('nldd-list').find((list) => list.attributes('type') === 'tree');
    const supporting = tree
      .findAll('nldd-text-cell')
      .map((cell) => cell.attributes('supporting-text') ?? '');
    expect(supporting.some((text) => text.includes('wet_op_de_zorgtoeslag, artikel 3'))).toBe(true);

    // En de soort stap in gewone woorden, niet de naam die de engine eraan geeft.
    const tags = tree.findAll('nldd-tag').map((tag) => tag.attributes('text'));
    expect(tags).toContain('Uitvoering');
    expect(tags).toContain('Artikel rekent');
  });

  it('klapt een tak open en weer dicht', async () => {
    const wrapper = await panel();
    const tree = () => wrapper.findAll('nldd-list').find((list) => list.attributes('type') === 'tree');
    const rekenend = tree().findAll('nldd-list-item')[2];

    await rekenend.trigger('click');
    // De bewerking eronder komt erbij; de wortel blijft staan waar hij stond.
    expect(tree().findAll('nldd-list-item')).toHaveLength(4);

    await tree().findAll('nldd-list-item')[2].trigger('click');
    expect(tree().findAll('nldd-list-item')).toHaveLength(3);
  });

  it('zegt het als een receipt geen trace draagt', async () => {
    // Een besluit van vóór deze versie, of een gram dat nooit langs een engine
    // kwam: dan staat er wat er is en niet een lege boom.
    const receipt = cloneReceipt();
    delete receipt.results.trace;
    const wrapper = await panel(receipt);
    expect(wrapper.findAll('nldd-list').some((list) => list.attributes('type') === 'tree')).toBe(
      false,
    );
    expect(
      wrapper.findAll('nldd-inline-dialog').some((dialog) => dialog.attributes('text') === 'Geen uitvoeringstrace'),
    ).toBe(true);
  });

  it('doet de trace dicht als het paneel een ander gram aanwijst', async () => {
    // De paden van de ene trace zeggen niets over de andere: een tak die "nog
    // open stond" zou in een ander besluit een willekeurige tak zijn.
    const wrapper = await panel();
    const tree = () => wrapper.findAll('nldd-list').find((list) => list.attributes('type') === 'tree');
    await tree().findAll('nldd-list-item')[2].trigger('click');
    expect(tree().findAll('nldd-list-item')).toHaveLength(4);

    await wrapper.setProps({ index: 1 });
    await vi.waitFor(() => expect(tree().findAll('nldd-list-item')).toHaveLength(3));
  });

  it('haalt opnieuw op als het paneel een ander gram aanwijst', async () => {
    const wrapper = await panel();
    await wrapper.setProps({ index: 1 });
    await vi.waitFor(() =>
      expect(fetchGramReceipt).toHaveBeenLastCalledWith('toeslagen', 'beschikkingen', 1),
    );
  });
});
