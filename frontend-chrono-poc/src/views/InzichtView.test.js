import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { fakeServer } from '../testing/fakeServer.js';
import { portaalFixture } from '../testing/portaalFixture.js';
import { useWorld } from '../world/useWorld.js';

// Inzicht in je aanvraag: één kaart per vraag uit het portaal, elk antwoord van
// één cel, en "niets vastgesteld" als "nog niets bekend" en niet als fout.

let complaints;
/** De pagina van de lopende test; afgebroken na afloop, anders luistert ze mee met de volgende. */
let mounted = null;

beforeEach(() => {
  complaints = [];
  for (const channel of ['warn', 'error']) {
    vi.spyOn(console, channel).mockImplementation((...args) => complaints.push(args.join(' ')));
  }
  const world = useWorld();
  world.snapshot.value = null;
  world.portaal.value = undefined;
  world.result.value = null;
  world.dismissError();
  window.location.hash = '#/inzicht';
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const beschikking = {
  cell: 'toeslagen',
  name: 'zorgtoeslagbeschikking',
  op_moment: '2025-02-01',
  outcome: { established: { heeft_recht_op_zorgtoeslag: true, hoogte_zorgtoeslag: 1234 } },
  reductie: null,
};

async function mountInzicht(options) {
  const server = fakeServer(options);
  const wrapper = mount(App);
  mounted = wrapper;
  await flushPromises();
  return { wrapper, server };
}

describe('inzicht in je aanvraag', () => {
  it('vraagt zonder gekozen aanvrager nog niets', async () => {
    const { wrapper, server } = await mountInzicht();
    expect(wrapper.findAll('nldd-card')).toHaveLength(0);
    expect(server.requests.some((request) => request.path.includes('/lexostatus/'))).toBe(false);
    expect(wrapper.findAll('nldd-inline-dialog').map((dialog) => dialog.attributes('text'))).toContain(
      'Kies als wie u kijkt',
    );
    expect(complaints).toStrictEqual([]);
  });

  it('stelt elke vraag apart aan haar eigen cel, met de parameters van de persona', async () => {
    const { wrapper, server } = await mountInzicht({
      persona: 'aanvrager-a',
      answers: { zorgtoeslagbeschikking: beschikking },
    });

    const vragen = portaalFixture.personas[0].inzicht;
    const lexostatus = server.requests.filter((request) => request.path.includes('/lexostatus/'));
    expect(lexostatus.map((request) => request.path)).toStrictEqual(
      vragen.map((vraag) => `/api/cells/${vraag.cell}/lexostatus/${vraag.lexostatus}`),
    );
    for (const request of lexostatus) expect(request.query).toStrictEqual({ zaakkenmerk: 'zorgtoeslag/999993653' });

    const cards = wrapper.findAll('nldd-card');
    expect(cards.map((card) => card.attributes('accessible-label'))).toStrictEqual(vragen.map((vraag) => vraag.label));

    // De eerste cel stelde iets vast: haar uitkomsten staan in de kaart, met het
    // moment erbij.
    const waarden = cards[0].findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(waarden).toContain('Hoogte zorgtoeslag');
    expect(waarden).toContain('1234');
    expect(cards[0].text()).toContain('01-02-2025');

    // De tweede niet: dat is "nog niets bekend", met de reden van de cel, en
    // geen fout.
    const leeg = cards[1].find('nldd-inline-dialog');
    expect(leeg.attributes('text')).toBe('Nog niets bekend');
    expect(leeg.attributes('supporting-text')).toContain("cel 'belastingdienst'");
    expect(wrapper.findAll('nldd-banner').filter((item) => item.attributes('variant') === 'critical')).toHaveLength(0);
    expect(complaints).toStrictEqual([]);
  });

  it('volgt een wissel van aanvrager, en die geldt ook voor het portaal', async () => {
    const { wrapper, server } = await mountInzicht({ persona: 'aanvrager-a' });
    wrapper
      .find('nldd-dropdown')
      .element.dispatchEvent(new CustomEvent('change', { detail: { value: 'aanvrager-b' } }));
    await flushPromises();

    const laatste = server.requests.filter((request) => request.path.includes('/lexostatus/')).at(-1);
    expect(laatste.query).toStrictEqual({ zaakkenmerk: 'zorgtoeslag/999990019' });

    // Dezelfde store: op het portaal staat dezelfde aanvrager.
    window.location.hash = '#/portaal';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    await flushPromises();
    // Het portaal toont haar formulieren, ingevuld met haar bsn, en de
    // keuzelijst staat op haar.
    expect(wrapper.findAll('nldd-card').length).toBeGreaterThan(0);
    expect(wrapper.find('nldd-card nldd-text-field').attributes('value')).toBe('999990019');
    expect(wrapper.find('select').element.value).toBe('aanvrager-b');
    expect(complaints).toStrictEqual([]);
  });
});
