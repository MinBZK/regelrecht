import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { fakeServer } from '../testing/fakeServer.js';
import { portaalFixture } from '../testing/portaalFixture.js';
import { useWorld } from '../world/useWorld.js';

// Het aanvraagportaal: een keuzelijst "Aanvragen als", en daaronder alleen de
// acties van de portaal-actor met formulieren die de server voor die persona
// invulde.

let complaints;
/** De pagina van de lopende test; afgebroken na afloop, anders luistert ze mee met de volgende. */
let mounted = null;

beforeEach(() => {
  complaints = [];
  for (const channel of ['warn', 'error']) {
    vi.spyOn(console, channel).mockImplementation((...args) => complaints.push(args.join(' ')));
  }
  // De store is er één per pagina; elke test begint met een lege.
  const world = useWorld();
  world.snapshot.value = null;
  world.portaal.value = undefined;
  world.result.value = null;
  world.dismissError();
  window.location.hash = '#/portaal';
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function mountPortaal(options) {
  const server = fakeServer(options);
  const wrapper = mount(App);
  mounted = wrapper;
  await flushPromises();
  return { wrapper, server };
}

/** De keuzelijst "Aanvragen als". */
function picker(wrapper) {
  return wrapper.findAll('nldd-form-field').find((field) => field.attributes('label') === 'Aanvragen als');
}

async function choose(wrapper, id) {
  picker(wrapper)
    .find('nldd-dropdown')
    .element.dispatchEvent(new CustomEvent('change', { detail: { value: id } }));
  await flushPromises();
}

describe('het aanvraagportaal', () => {
  it('vraagt zonder gekozen aanvrager om een keuze en toont nog geen formulier', async () => {
    const { wrapper } = await mountPortaal();
    const options = picker(wrapper).findAll('option');
    expect(options.map((option) => option.text())).toStrictEqual([
      'Kies een aanvrager',
      ...portaalFixture.personas.map((persona) => persona.label),
    ]);
    expect(wrapper.findAll('nldd-card')).toHaveLength(0);
    const leeg = wrapper.findAll('nldd-inline-dialog').map((dialog) => dialog.attributes('text'));
    expect(leeg).toContain('Kies als wie u aanvraagt');
    // Geen spoor van "Achter de schermen": geen cellen, geen journaal.
    expect(wrapper.find('nldd-stacked-split-view').exists()).toBe(false);
    expect(complaints).toStrictEqual([]);
  });

  it('kiest een aanvrager op de server en toont alleen haar acties, ingevuld met haar gegevens', async () => {
    const { wrapper, server } = await mountPortaal();
    await choose(wrapper, 'aanvrager-b');

    expect(server.requests).toContainEqual(
      expect.objectContaining({ method: 'PUT', path: '/api/persona', body: { id: 'aanvrager-b' } }),
    );
    expect(useWorld().persona.value?.id).toBe('aanvrager-b');

    const cards = wrapper.findAll('nldd-card');
    const eigen = useWorld().snapshot.value.actions.filter((action) => action.actor === portaalFixture.actor);
    expect(cards).toHaveLength(eigen.length);
    expect(cards.map((card) => card.attributes('accessible-label'))).toStrictEqual(eigen.map((action) => action.label));

    const bsn = cards[0].find('nldd-text-field');
    expect(bsn.attributes('value')).toBe('999990019');
    expect(cards[0].find('nldd-number-field').attributes('value')).toBe('2024');
    expect(complaints).toStrictEqual([]);
  });

  it('laat wat er voor de ene aanvrager getypt is niet staan onder de volgende', async () => {
    const { wrapper, server } = await mountPortaal({ persona: 'aanvrager-a' });
    // Zelf getypt volgt de voorinvulling niet meer (zie ActionCard)…
    wrapper
      .find('nldd-card nldd-text-field')
      .element.dispatchEvent(new CustomEvent('input', { detail: { value: '123456782' } }));
    await flushPromises();
    expect(wrapper.find('nldd-card nldd-text-field').attributes('value')).toBe('123456782');

    // …maar wie wisselt, is iemand anders, en krijgt haar eigen formulier.
    await choose(wrapper, 'aanvrager-b');
    expect(wrapper.find('nldd-card nldd-text-field').attributes('value')).toBe('999990019');
    await wrapper.find('nldd-card form').trigger('submit');
    await flushPromises();
    const post = server.requests.find((request) => request.method === 'POST');
    expect(post.body).toMatchObject({ bsn: '999990019' });
    expect(complaints).toStrictEqual([]);
  });

  it('bevestigt een ingediende aanvraag, met een link naar inzicht', async () => {
    const { wrapper, server } = await mountPortaal({ persona: 'aanvrager-a' });
    await wrapper.find('nldd-card form').trigger('submit');
    await flushPromises();

    const post = server.requests.find((request) => request.method === 'POST');
    expect(post.path).toBe(`/api/actions/${encodeURIComponent('burger.aanvraag')}`);
    expect(post.body).toMatchObject({ bsn: '999993653', jaar: 2024 });

    const banner = wrapper.findAll('nldd-banner').find((item) => item.attributes('variant') === 'success');
    expect(banner.attributes('text')).toContain('ingediend');
    expect(banner.attributes('supporting-text')).toContain('Aanvrager A (fictief)');
    expect(banner.find('nldd-button').attributes('href')).toBe('#/inzicht');

    // Wie wisselt, is iemand anders en heeft niets ingediend.
    await choose(wrapper, 'aanvrager-b');
    expect(wrapper.findAll('nldd-banner').filter((item) => item.attributes('variant') === 'success')).toHaveLength(0);
    expect(complaints).toStrictEqual([]);
  });

  it('kiest niemand met de lege keuze', async () => {
    const { wrapper, server } = await mountPortaal({ persona: 'aanvrager-a' });
    await choose(wrapper, '');
    expect(server.requests).toContainEqual(
      expect.objectContaining({ method: 'PUT', path: '/api/persona', body: { id: null } }),
    );
    expect(wrapper.findAll('nldd-card')).toHaveLength(0);
  });
});
