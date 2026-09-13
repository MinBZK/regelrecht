import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App.vue';
import { cloneWorld, worldFixture } from './testing/worldFixture.js';
import { allGrams } from './world/snapshot.js';
import { useWorld } from './world/useWorld.js';

// De hele pagina op het beeld van de fixture: de kolommen, de bediening en de
// tijdlijn, zonder één klacht van Vue over een onbekend element of een ontbrekende
// prop. Dit is de test die valt als het contract met de wereld-API verschuift.

/** Het antwoord van de server, in de vorm die apiFetch verwacht. */
function jsonResponse(body) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => 'application/json' },
    json: async () => body,
    text: async () => JSON.stringify(body),
  };
}

/** De weigering van de server, in diezelfde vorm: `{"error": "…"}` bij een 400. */
function rejection(message) {
  const body = JSON.stringify({ error: message });
  return {
    ok: false,
    status: 400,
    headers: { get: () => 'application/json' },
    json: async () => JSON.parse(body),
    text: async () => body,
  };
}

let complaints;

beforeEach(() => {
  complaints = [];
  for (const channel of ['warn', 'error']) {
    vi.spyOn(console, channel).mockImplementation((...args) => complaints.push(args.join(' ')));
  }
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function mountApp(world = worldFixture) {
  vi.stubGlobal('fetch', vi.fn(async () => jsonResponse(world)));
  const wrapper = mount(App);
  await flushPromises();
  return wrapper;
}

describe('de pagina', () => {
  it('haalt het beeld op en tekent een kolom per cel', async () => {
    const wrapper = await mountApp();
    expect(fetch).toHaveBeenCalledWith('/api/world', expect.anything());
    const columns = wrapper.findAll('nldd-card');
    // Eén kaart per cel, plus één per actie in het actiepaneel.
    expect(columns.length).toBe(worldFixture.cells.length + worldFixture.actions.length);
    for (const cell of worldFixture.cells) expect(wrapper.text()).toContain(cell.id);
    expect(complaints).toStrictEqual([]);
  });

  it('zet de klok in de werkbalk en op de tijdlijn', async () => {
    const wrapper = await mountApp();
    const clocks = wrapper.findAll('nldd-tag').filter((tag) => tag.attributes('text')?.startsWith('Klok:'));
    expect(clocks).toHaveLength(2);
    expect(clocks[0].attributes('text')).toBe('Klok: 01-02-2025');
    expect(wrapper.findAll('nldd-step-indicator-item').length).toBeGreaterThan(0);
  });

  it('laat de vijf panelen van de bediening kiezen', async () => {
    const wrapper = await mountApp();
    const tabs = wrapper.findAll('nldd-tab-bar-item');
    expect(tabs.map((tab) => tab.attributes('text'))).toStrictEqual([
      'Acties',
      'Instellingen',
      'Lexostatus',
      'Grammen',
      'Observatielog',
    ]);
    expect(tabs[0].attributes('selected')).toBe('true');

    // De tab-bar meldt de keuze; de pagina wisselt van paneel.
    wrapper.find('nldd-tab-bar').element.dispatchEvent(
      new CustomEvent('tabchange', { detail: { item: tabs[4].element } }),
    );
    await flushPromises();
    const banner = wrapper.findAll('nldd-banner').find((item) => item.attributes('icon') === 'binoculars');
    expect(banner.attributes('text')).toBe('Meetinstrument van de testopstelling');
    expect(complaints).toStrictEqual([]);
  });

  it('zet elk gram van de wereld in het grammenpaneel', async () => {
    const wrapper = await mountApp();
    const tabs = wrapper.findAll('nldd-tab-bar-item');
    wrapper.find('nldd-tab-bar').element.dispatchEvent(
      new CustomEvent('tabchange', { detail: { item: tabs[3].element } }),
    );
    await flushPromises();

    const table = wrapper
      .findAll('nldd-list')
      .find((list) => list.attributes('accessible-label')?.startsWith('Alle grammen'));
    expect(table.findAll('nldd-list-item')).toHaveLength(allGrams(worldFixture).length);
    expect(complaints).toStrictEqual([]);
  });

  // Waar het om begonnen was: naast elkaar paste geen kolom meer heel.
  it('zet de bediening boven en de cellen eronder, met de tijdlijn onderaan', async () => {
    const wrapper = await mountApp();

    // Eén split view, en die stapelt: de panelen liggen boven elkaar en niet
    // meer naast elkaar. Dat er precies vier panelen zijn, pint de indeling vast
    // — de inhoud, de twee helften, en de balk met de tijdlijn.
    const split = wrapper.find('nldd-stacked-split-view');
    expect(split.attributes('panes')).toBe('2');
    expect(wrapper.findAll('nldd-split-view-pane').map((pane) => pane.attributes('slot'))).toStrictEqual([
      'main',
      'pane-1',
      'pane-2',
      'timeline-bar',
    ]);

    const panes = split.findAll('nldd-split-view-pane');
    expect(panes[0].attributes('slot')).toBe('pane-1');
    expect(panes[0].find('nldd-tab-bar').exists()).toBe(true);
    expect(panes[0].find('nldd-collection').exists()).toBe(false);

    expect(panes[1].attributes('slot')).toBe('pane-2');
    expect(panes[1].find('nldd-collection').exists()).toBe(true);
    expect(panes[1].findAll('nldd-card')).toHaveLength(worldFixture.cells.length);

    // De tijdlijn hoort niet in de split view maar in haar eigen balk: die
    // blijft onderaan staan, wat er in de panelen ook gebeurt.
    const bar = wrapper.findAll('nldd-split-view-pane').find((pane) => pane.attributes('slot') === 'timeline-bar');
    expect(bar.find('nldd-step-indicator').exists()).toBe(true);
    expect(split.find('nldd-step-indicator').exists()).toBe(false);
  });

  it('geeft elke celkolom een vaste minimumbreedte en laat het paneel zelf opzij schuiven', async () => {
    const wrapper = await mountApp();
    const collection = wrapper.find('nldd-collection');

    // De kolommen krimpen niet mee tot ze onleesbaar zijn; past het rijtje niet,
    // dan scrolt de collectie binnen zichzelf (nooit de pagina).
    expect(collection.attributes('layout')).toBe('horizontal-scroll');
    expect(collection.attributes('item-width')).toMatch(/^\d+px$/);
  });

  it('noemt een verstreken termijn, zonder er een fout van te maken', async () => {
    const world = cloneWorld();
    world.warnings = [
      {
        // De soort hoort erbij: de lijst gaat over termijnen, en een
        // waarschuwing van een andere soort staat bij het gram waar ze over gaat.
        soort: 'gemiste_termijn',
        label: 'aanvraagtermijn',
        at: '2024-11-01',
        cell: 'toeslagen',
        chronicle: 'aanvragen',
        name: 'aanvraag_ontvangen',
      },
    ];
    const wrapper = await mountApp(world);
    expect(wrapper.text()).toContain('Verstreken termijnen');
    const rows = wrapper.findAll('nldd-text-cell').map((cell) => cell.attributes('supporting-text'));
    expect(rows.some((text) => text?.includes("cel 'toeslagen' had geen 'aanvraag_ontvangen'"))).toBe(true);
    expect(wrapper.findAll('nldd-banner').filter((item) => item.attributes('variant') === 'critical')).toHaveLength(0);
  });

  // De store is er één per pagina, dus deze test maakt hem achteraf weer leeg;
  // hij is de enige die er een fout in achterlaat.
  it('zet een geweigerde actie bovenaan zodra haar eigen paneel niet meer in beeld is', async () => {
    const melding = "parameter 'ondertekend_op' is geen datum: '09-01-2024' (verwacht jjjj-mm-dd)";
    const wrapper = await mountApp();
    try {
      vi.stubGlobal(
        'fetch',
        vi.fn(async (_url, init) => (init?.method === 'POST' ? rejection(melding) : jsonResponse(worldFixture))),
      );

      const world = useWorld();
      await world.act({ id: worldFixture.actions[0].id, label: 'Aanvraag' }, {});
      await flushPromises();
      expect(world.actionError.value.message).toBe(melding);

      // Zolang het actiepaneel op het scherm staat, staat de melding bij de
      // kaart en zwijgt de banner bovenaan: anders zou dezelfde zin er twee
      // keer staan.
      const met = (titel) =>
        wrapper
          .findAll('nldd-banner')
          .filter((item) => item.attributes('text') === titel && item.attributes('supporting-text') === melding);
      expect(met('Deze actie is niet uitgevoerd')).toHaveLength(1);
      expect(met('De server kon dit niet doen')).toHaveLength(0);

      // Een ander tabblad haalt die kaart weg. Bleef de banner dan zwijgen, dan
      // stond de melding nergens meer op het scherm.
      const tabs = wrapper.findAll('nldd-tab-bar-item');
      wrapper.find('nldd-tab-bar').element.dispatchEvent(
        new CustomEvent('tabchange', { detail: { item: tabs[2].element } }),
      );
      await flushPromises();
      expect(met('Deze actie is niet uitgevoerd')).toHaveLength(0);
      expect(met('De server kon dit niet doen')).toHaveLength(1);
    } finally {
      useWorld().dismissError();
    }
  });
});
