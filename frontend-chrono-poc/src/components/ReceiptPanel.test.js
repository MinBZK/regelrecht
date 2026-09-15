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
    expect(titles.join(' ')).toContain('Herkomst van de uitvoering');
    expect(titles.join(' ')).toContain('Resultaten');

    const all = wrapper.findAll('nldd-list-item').map((item) =>
      item.findAll('nldd-text-cell').map((cell) => cell.attributes('text')),
    );
    expect(all).toContainEqual(['Regulation id', 'wet_op_de_zorgtoeslag']);
    expect(all).toContainEqual(['Outputs · hoogte zorgtoeslag', '197178.01']);
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

  it('haalt opnieuw op als het paneel een ander gram aanwijst', async () => {
    const wrapper = await panel();
    await wrapper.setProps({ index: 1 });
    await vi.waitFor(() =>
      expect(fetchGramReceipt).toHaveBeenLastCalledWith('toeslagen', 'beschikkingen', 1),
    );
  });
});
