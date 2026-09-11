import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App.vue';
import { cloneWorld, worldFixture } from './testing/worldFixture.js';

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

  it('laat de vier panelen van de bediening kiezen', async () => {
    const wrapper = await mountApp();
    const tabs = wrapper.findAll('nldd-tab-bar-item');
    expect(tabs.map((tab) => tab.attributes('text'))).toStrictEqual([
      'Acties',
      'Instellingen',
      'Lexostatus',
      'Observatielog',
    ]);
    expect(tabs[0].attributes('selected')).toBe('true');

    // De tab-bar meldt de keuze; de pagina wisselt van paneel.
    wrapper.find('nldd-tab-bar').element.dispatchEvent(
      new CustomEvent('tabchange', { detail: { item: tabs[3].element } }),
    );
    await flushPromises();
    const banner = wrapper.findAll('nldd-banner').find((item) => item.attributes('icon') === 'binoculars');
    expect(banner.attributes('text')).toBe('Meetinstrument van de testopstelling');
    expect(complaints).toStrictEqual([]);
  });

  it('noemt een verstreken termijn, zonder er een fout van te maken', async () => {
    const world = cloneWorld();
    world.warnings = [
      {
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
});
