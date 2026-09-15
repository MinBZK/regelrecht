import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GramPanel from './GramPanel.vue';
import { receiptFixture } from '../testing/receiptFixture.js';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';
import { formatMoment } from '../world/format.js';
import { carriesReceipt } from '../world/receipt.js';
import { allGrams } from '../world/snapshot.js';

// Het receipt komt van de server en niet uit het beeld; deze tests gaan over de
// uitklap die erom vraagt, niet over het verkeer.
const fetchGramReceipt = vi.fn(async () => receiptFixture);
vi.mock('../api/worldApi.js', () => ({
  fetchGramReceipt: (...args) => fetchGramReceipt(...args),
}));

// Het overzicht van alle grammen: de volgorde, de twee filters en de uitklap.
//
// De verwachtingen komen uit de fixture zelf en niet uit een lijst hier: het
// beeld is het contract, en een tweede exemplaar zou stil uit elkaar lopen.

/** De rijen van de tabel: de directe kinderen van de lijst, niet de uitklap. */
function rows(wrapper) {
  return wrapper.findAll('nldd-list > nldd-list-item');
}

/** De rijen die het filter doorlaat: wat eruit valt blijft staan met `hidden`. */
function visibleRows(wrapper) {
  return rows(wrapper).filter((row) => row.attributes('hidden') === undefined);
}

/** De teksten van de kolommen van één rij, in volgorde. */
function columns(row) {
  return row.findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
}

/** Een filter kiezen: het ontwerpsysteem meldt de keuze in `detail.value`. */
function choose(wrapper, index, value) {
  wrapper
    .findAll('nldd-dropdown')
    [index].element.dispatchEvent(new CustomEvent('change', { detail: { value } }));
}

describe('het grammenoverzicht', () => {
  // Elke test telt haar eigen verzoeken: "er is nog niets opgehaald" is hier een
  // verwachting, en die mag niet van de test ervoor afhangen.
  beforeEach(() => {
    fetchGramReceipt.mockClear();
  });

  it('zet elk gram van elke cel op volgorde van moment', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });

    const expected = allGrams(worldFixture);
    expect(expected.length).toBeGreaterThan(1);
    expect(rows(wrapper)).toHaveLength(expected.length);

    // Chronologisch: geen rij ligt eerder dan de rij erboven.
    const moments = rows(wrapper).map((row) => columns(row)[0]);
    expect(moments).toStrictEqual(expected.map((gram) => formatMoment(gram.opMoment)));
    const sorted = [...moments].sort((a, b) =>
      a.split('-').reverse().join().localeCompare(b.split('-').reverse().join()),
    );
    expect(moments).toStrictEqual(sorted);
  });

  it('zet dezelfde dag altijd in dezelfde volgorde, hoe het beeld hem ook geeft', () => {
    // De dag is de korrel, dus grammen van dezelfde dag hebben geen eigen tijd.
    // Zonder een vaste staartsortering zou de tabel bewegen terwijl er niets
    // gebeurd is: hier staan dezelfde cellen in omgekeerde volgorde in het beeld.
    const world = cloneWorld();
    const reversed = { ...world, cells: [...world.cells].reverse() };
    const expected = allGrams(world).map((gram) => `${gram.opMoment}|${gram.cell}|${gram.name}`);
    const actual = allGrams(reversed).map((gram) => `${gram.opMoment}|${gram.cell}|${gram.name}`);
    expect(actual).toStrictEqual(expected);
  });

  it('geeft elke rij haar kolommen: moment, cel, kroniek, naam, kanaal en grondslag', () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const first = allGrams(worldFixture)[0];

    expect(columns(rows(wrapper)[0])).toStrictEqual([
      formatMoment(first.opMoment),
      first.cell,
      first.chronicle,
      first.name,
      first.intake,
      first.grondslag,
    ]);
    // En het type als tag, zodat de soort ook zonder de kolomtekst te zien is.
    expect(rows(wrapper)[0].find('nldd-tag').attributes('text')).toBe('Executogram');
  });

  // Waar het zaakkenmerk vandaan komt: het staat onder de naam van het besluit
  // dat het invulde, en niet alleen in de uitklap.
  it('zet bij een besluit de zaak onder de naam van het gram', () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const besluit = allGrams(worldFixture).find((row) => row.chronicle === 'beschikkingen');
    const row = rows(wrapper).find((candidate) => columns(candidate).includes(besluit.name));
    const supporting = row.findAll('nldd-text-cell').map((cell) => cell.attributes('supporting-text'));
    expect(supporting).toContain(`zaak ${besluit.gram.fields.zaakkenmerk.value}`);
  });

  it('laat op cel filteren', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const cell = worldFixture.cells[0].id;
    const expected = allGrams(worldFixture).filter((gram) => gram.cell === cell);
    expect(expected.length).toBeGreaterThan(0);

    choose(wrapper, 0, cell);
    await wrapper.vm.$nextTick();

    const kept = visibleRows(wrapper);
    expect(kept).toHaveLength(expected.length);
    for (const row of kept) expect(columns(row)[1]).toBe(cell);
    // De rest verdwijnt niet uit de lijst maar uit het zicht: zo is het de lijst
    // zelf die "niets gevonden" zegt, en blijven de filters de weg terug.
    expect(rows(wrapper)).toHaveLength(allGrams(worldFixture).length);
  });

  it('laat op type filteren en telt wat er overblijft', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const total = allGrams(worldFixture).length;
    const expected = allGrams(worldFixture).filter((gram) => gram.kind === 'decretogram');
    expect(expected.length).toBeGreaterThan(0);

    choose(wrapper, 1, 'decretogram');
    await wrapper.vm.$nextTick();

    const kept = visibleRows(wrapper);
    expect(kept).toHaveLength(expected.length);
    for (const row of kept) expect(row.find('nldd-tag').attributes('text')).toBe('Decretogram');

    const count = wrapper
      .findAll('nldd-tag')
      .map((tag) => tag.attributes('text'))
      .find((text) => text?.endsWith('grammen'));
    expect(count).toBe(`${expected.length} van ${total} grammen`);
  });

  it('combineert de twee filters', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const cell = allGrams(worldFixture).find((gram) => gram.kind === 'decretogram').cell;
    const expected = allGrams(worldFixture).filter(
      (gram) => gram.cell === cell && gram.kind === 'decretogram',
    );

    choose(wrapper, 0, cell);
    choose(wrapper, 1, 'decretogram');
    await wrapper.vm.$nextTick();

    expect(visibleRows(wrapper)).toHaveLength(expected.length);
  });

  it('klapt een rij uit naar het volledige gram als JSON, met de herkomst per veld', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const index = allGrams(worldFixture).findIndex((gram) => gram.kind === 'decretogram');
    const gram = allGrams(worldFixture)[index];
    const row = rows(wrapper)[index];

    expect(wrapper.find('nldd-code-viewer').exists()).toBe(false);
    expect(row.attributes('expanded')).toBeUndefined();

    await row.trigger('click');

    expect(row.attributes('expanded')).toBe('true');
    const viewer = row.find('nldd-code-viewer');
    expect(viewer.attributes('language')).toBe('json');
    // Het gram zelf, niet een uittreksel ervan: wat er in het beeld staat staat
    // er, en de herkomst van elke waarde staat erin.
    expect(JSON.parse(viewer.text())).toStrictEqual(gram.gram);
    expect(viewer.text()).toContain('herkomst');

    // Eén rij tegelijk is geen belofte: een tweede mag er ook bij open staan.
    await rows(wrapper)[0].trigger('click');
    expect(wrapper.findAll('nldd-code-viewer')).toHaveLength(2);

    // En dicht is weer dicht.
    await row.trigger('click');
    expect(row.attributes('expanded')).toBeUndefined();
    expect(row.find('nldd-code-viewer').exists()).toBe(false);
  });

  it('geeft een decretogram uit het besluit-pad een tweede uitklap voor zijn receipt', async () => {
    // Het receipt staat niet in het beeld — het draagt wandkloktijd — dus de
    // uitklap is een eigen verzoek aan de server, per gram en alleen op verzoek.
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const grams = allGrams(worldFixture);
    const decretogram = grams.findIndex((gram) => carriesReceipt(gram.gram));
    const executogram = grams.findIndex((gram) => gram.kind === 'executogram');

    await rows(wrapper)[executogram].trigger('click');
    expect(
      rows(wrapper)[executogram]
        .findAll('nldd-text-cell')
        .some((cell) => cell.attributes('text') === 'Receipt'),
    ).toBe(false);

    const row = rows(wrapper)[decretogram];
    await row.trigger('click');
    const receiptRow = row
      .findAll('nldd-list-item')
      .find((item) => item.findAll('nldd-text-cell').some((cell) => cell.attributes('text') === 'Receipt'));
    expect(receiptRow).toBeDefined();

    // Dicht is nog niets opgehaald: een rij opendoen is geen verzoek.
    expect(fetchGramReceipt).not.toHaveBeenCalled();

    await receiptRow.trigger('click');
    expect(receiptRow.attributes('expanded')).toBe('true');
    await vi.waitFor(() =>
      expect(fetchGramReceipt).toHaveBeenCalledWith(
        grams[decretogram].cell,
        grams[decretogram].chronicle,
        grams[decretogram].index,
      ),
    );
    // En de rij eronder blijft gewoon open staan: de klik zat in de uitklap.
    expect(row.attributes('expanded')).toBe('true');
  });

  it('biedt geen receipt aan bij een decretogram waar nooit een uitvoering achter zat', async () => {
    // Een bron-cel zonder engine legt haar eigen vaststelling ook als
    // decretogram vast. Die draagt geen receipt, en de server weigert hem met een
    // 404 — dus er hoort hier niets aangeboden te worden dat dat oplevert.
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const grams = allGrams(worldFixture);
    const zonder = grams.findIndex(
      (gram) => gram.kind === 'decretogram' && !carriesReceipt(gram.gram),
    );
    expect(zonder, 'de fixture hoort zo’n gram te hebben').toBeGreaterThan(-1);

    const row = rows(wrapper)[zonder];
    await row.trigger('click');
    expect(
      row.findAll('nldd-text-cell').some((cell) => cell.attributes('text') === 'Receipt'),
    ).toBe(false);
    expect(fetchGramReceipt).not.toHaveBeenCalled();
  });

  it('laat de lijst zelf "niets gevonden" zeggen als het filter alles wegneemt', async () => {
    // Het ontwerpsysteem toont `no-results` als de lijst rijen hééft en ze
    // allemaal `hidden` zijn. Die twee samen zijn waar deze aanpak op leunt: de
    // rijen blijven staan, dus de filters blijven de weg terug.
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const all = allGrams(worldFixture);
    const leeg = all
      .flatMap((row) => all.map((other) => ({ cell: row.cell, kind: other.kind })))
      .find((pair) => !all.some((row) => row.cell === pair.cell && row.kind === pair.kind));
    expect(leeg).toBeDefined();

    choose(wrapper, 0, leeg.cell);
    choose(wrapper, 1, leeg.kind);
    await wrapper.vm.$nextTick();

    expect(visibleRows(wrapper)).toHaveLength(0);
    expect(rows(wrapper).length).toBeGreaterThan(0);
    const noResults = wrapper
      .findAll('nldd-list > nldd-inline-dialog')
      .find((item) => item.attributes('slot') === 'no-results');
    expect(noResults.attributes('text')).toBe('Geen gram voldoet aan het filter');
  });

  it('laat een rij open staan bij een klik in de uitklap zelf', async () => {
    // De rij is de knop, dus een klik in de uitklap bubbelt erover heen. Zonder
    // toets zou de kopieerknop van de viewer — of het aanwijzen van een regel
    // JSON om hem te selecteren — de rij onder je handen dichtdoen.
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const index = allGrams(worldFixture).findIndex((gram) => gram.kind === 'decretogram');
    const row = rows(wrapper)[index];

    await row.trigger('click');
    expect(row.attributes('expanded')).toBe('true');

    row.find('nldd-code-viewer').element.dispatchEvent(new Event('click', { bubbles: true }));
    await wrapper.vm.$nextTick();

    expect(row.attributes('expanded')).toBe('true');
    expect(row.find('nldd-code-viewer').exists()).toBe(true);
  });

  it('zegt het zelf als er niets ligt', () => {
    const world = cloneWorld();
    world.cells = [];
    const wrapper = mount(GramPanel, { props: { snapshot: world } });

    expect(rows(wrapper)).toHaveLength(0);
    const empty = wrapper.findAll('nldd-inline-dialog').find((item) => item.attributes('slot') === 'empty');
    expect(empty.attributes('text')).toBe('Nog geen grammen');
  });

  it('biedt alleen soorten aan die er ook liggen', () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });
    const offered = wrapper.findAll('nldd-dropdown')[1].findAll('option').map((option) => option.text());
    const present = new Set(allGrams(worldFixture).map((gram) => gram.kind));

    expect(offered[0]).toBe('Alle typen');
    // Een lexogram ligt in geen enkele kroniek, dus het staat er niet bij.
    expect(offered).not.toContain('Lexogram');
    expect(offered.slice(1)).toHaveLength(present.size);
  });
});

describe('een aangewezen gram', () => {
  /** Het gram waar de uitleg bij een lexostatus naar zou wijzen. */
  const target = allGrams(worldFixture)[2];

  it('klapt de rij open en meldt de aanwijzing als verwerkt', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture, focusGram: target.id } });
    await wrapper.vm.$nextTick();

    const row = rows(wrapper).find((candidate) => candidate.attributes('id') === `gram-${target.id}`);
    expect(row.attributes('expanded')).toBe('true');
    // Terugmelden hoort erbij: zonder dat zou hetzelfde gram een tweede keer
    // aanwijzen niets doen, want de waarde verandert dan niet.
    expect(wrapper.emitted('clear-focus')).toBeTruthy();
  });

  it('haalt alleen het filter weg dat het gram zou verbergen', async () => {
    const other = allGrams(worldFixture).find((row) => row.cell !== target.cell);
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture } });

    // Een filter op een andere cel verbergt het aangewezen gram: dat gaat eraf.
    choose(wrapper, 0, other.cell);
    await wrapper.vm.$nextTick();
    expect(visibleRows(wrapper).length).toBeLessThan(rows(wrapper).length);

    await wrapper.setProps({ focusGram: target.id });
    await wrapper.vm.$nextTick();
    const row = rows(wrapper).find((candidate) => candidate.attributes('id') === `gram-${target.id}`);
    expect(row.attributes('hidden')).toBeUndefined();
  });

  it('meldt een gram dat niet bestaat terug zonder iets te openen', async () => {
    const wrapper = mount(GramPanel, { props: { snapshot: worldFixture, focusGram: 'bestaat|niet|9' } });
    await wrapper.vm.$nextTick();
    expect(rows(wrapper).every((row) => row.attributes('expanded') === undefined)).toBe(true);
    expect(wrapper.emitted('clear-focus')).toBeTruthy();
  });
});
