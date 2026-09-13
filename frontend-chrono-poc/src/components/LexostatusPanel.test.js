import { mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import LexostatusPanel from './LexostatusPanel.vue';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';

const established = {
  cell: 'belastingdienst',
  name: 'toetsingsinkomen',
  op_moment: '2025-02-01',
  outcome: { established: { toetsingsinkomen: 81000, competent_authority: 'Belastingdienst' } },
};

const nothing = {
  cell: 'belastingdienst',
  name: 'toetsingsinkomen',
  op_moment: '2025-02-01',
  outcome: { not_established: { reason: "cel 'belastingdienst' heeft hierover niets vastgelegd" } },
};

function mountPanel(answer) {
  const ask = vi.fn(async () => answer);
  return { wrapper: mount(LexostatusPanel, { props: { snapshot: worldFixture, ask } }), ask };
}

/** Een waarde in een veld zetten zoals een ontwerpsysteem-veld dat meldt. */
async function fill(wrapper, element, value) {
  element.element.dispatchEvent(new CustomEvent('input', { detail: { value } }));
  await wrapper.vm.$nextTick();
}

/** Het formulier versturen en de ronde afwachten. */
async function submit(wrapper) {
  await wrapper.find('form').trigger('submit');
  await wrapper.vm.$nextTick();
  await wrapper.vm.$nextTick();
}

describe('een vraag aan een cel', () => {
  it('biedt alleen cellen aan die iets publiceren, met hun eigen namen', () => {
    const { wrapper } = mountPanel(established);
    const selects = wrapper.findAll('select');
    const publishers = worldFixture.cells.filter((cell) => cell.lexostatussen.length > 0);
    expect(selects[0].findAll('option').map((option) => option.attributes('value'))).toStrictEqual(
      publishers.map((cell) => cell.id),
    );
    expect(selects[1].findAll('option').map((option) => option.attributes('value'))).toStrictEqual(
      publishers[0].lexostatussen,
    );
  });

  it('stelt de vraag met de opgegeven parameters, op de stand van de klok', async () => {
    const { wrapper, ask } = mountPanel(established);
    const params = wrapper.findAll('nldd-text-field');
    params[0].element.dispatchEvent(new CustomEvent('input', { detail: { value: 'bsn' } }));
    params[1].element.dispatchEvent(new CustomEvent('input', { detail: { value: '999993653' } }));
    await wrapper.vm.$nextTick();
    await submit(wrapper);

    const publishers = worldFixture.cells.filter((cell) => cell.lexostatussen.length > 0);
    expect(ask).toHaveBeenCalledWith(
      publishers[0].id,
      publishers[0].lexostatussen[0],
      { bsn: '999993653' },
      worldFixture.clock,
    );
  });

  // Het tijdreizen zelf: dezelfde cel, dezelfde naam, een eerder moment. Het
  // veld begint op de klok, dus wie niets kiest vraagt naar nu.
  it('vraagt naar een eerder moment zodra de bezoeker er een kiest', async () => {
    const { wrapper, ask } = mountPanel(established);
    const moment = wrapper.find('nldd-date-field');
    expect(moment.attributes('value')).toBe(worldFixture.clock);
    expect(moment.attributes('max')).toBe(worldFixture.clock);

    await fill(wrapper, moment, '2024-06-01');
    await submit(wrapper);

    expect(ask).toHaveBeenCalledWith(expect.any(String), expect.any(String), {}, '2024-06-01');
  });

  // Wie zelf niets kiest, vraagt naar nu — en "nu" verschuift met de klok. Bleef
  // de oude klokstand als waarde in het veld staan, dan gaf dezelfde knop na het
  // vooruitspoelen stil een antwoord over gisteren.
  it('volgt de klok zolang de bezoeker zelf geen moment koos', async () => {
    const { wrapper, ask } = mountPanel(established);
    const later = cloneWorld();
    later.clock = '2025-06-01';
    await wrapper.setProps({ snapshot: later });

    expect(wrapper.find('nldd-date-field').attributes('value')).toBe('2025-06-01');
    await submit(wrapper);
    expect(ask).toHaveBeenCalledWith(expect.any(String), expect.any(String), {}, '2025-06-01');
  });

  // Een gekozen moment is een keuze en geen afspiegeling van de klok:
  // vooruitspoelen maakt een eerder moment niet ongeldig.
  it('houdt een gekozen moment vast als de klok vooruit gaat', async () => {
    const { wrapper, ask } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-date-field'), '2024-06-01');
    const later = cloneWorld();
    later.clock = '2025-06-01';
    await wrapper.setProps({ snapshot: later });

    expect(wrapper.find('nldd-date-field').attributes('value')).toBe('2024-06-01');
    await submit(wrapper);
    expect(ask).toHaveBeenCalledWith(expect.any(String), expect.any(String), {}, '2024-06-01');
  });

  // `POST /api/reset` zet de klok terug op de startdag. Een keuze van ná die dag
  // zou daar stil een 409 opleveren; ze vervalt, en het veld volgt de klok weer.
  it('laat een keuze los die na een reset ná de klok zou liggen', async () => {
    const { wrapper } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-date-field'), '2024-06-01');
    const start = cloneWorld();
    start.clock = '2024-01-01';
    await wrapper.setProps({ snapshot: start });

    const moment = wrapper.find('nldd-date-field');
    expect(moment.attributes('value')).toBe('2024-01-01');
    expect(moment.attributes('invalid')).toBeUndefined();
  });

  // Ná de klok heeft nog niets vastgelegd; een antwoord "op" zo'n moment zou een
  // voorspelling zijn. De server weigert het met een 409 — hier komt het niet
  // eens zo ver.
  it('stelt geen vraag over een moment ná de klok', async () => {
    const { wrapper, ask } = mountPanel(established);
    await fill(wrapper, wrapper.find('nldd-date-field'), '2030-01-01');

    expect(wrapper.find('nldd-date-field').attributes('invalid')).toBe('true');
    expect(wrapper.find('nldd-form-field-error-text').text()).toContain('01-02-2025');
    expect(wrapper.find('nldd-button').attributes('disabled')).toBe('true');

    await submit(wrapper);
    expect(ask).not.toHaveBeenCalled();
  });

  it('toont een vastgesteld antwoord met zijn waarden en het moment', async () => {
    const { wrapper } = mountPanel(established);
    await submit(wrapper);
    expect(wrapper.text()).toContain('belastingdienst · toetsingsinkomen');
    expect(wrapper.text()).toContain('geldig op 01-02-2025');
    const rows = wrapper.findAll('nldd-list-item');
    expect(rows).toHaveLength(2);
    const values = wrapper.findAll('nldd-text-cell').map((cell) => cell.attributes('text'));
    expect(values).toContain('Toetsingsinkomen');
    expect(values).toContain('81000');
  });

  it('behandelt "niets vastgesteld" als gewoon antwoord, met de reden van de cel', async () => {
    const { wrapper } = mountPanel(nothing);
    await submit(wrapper);
    const dialogs = wrapper.findAll('nldd-inline-dialog');
    const answer = dialogs.find((dialog) => dialog.attributes('text') === 'Niets vastgesteld');
    expect(answer).toBeDefined();
    // Geen alarm: dit is een antwoord, geen fout.
    expect(answer.attributes('variant')).toBeUndefined();
    expect(answer.attributes('icon')).toBe('info');
    expect(answer.attributes('supporting-text')).toContain('niets vastgelegd');
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(0);
  });

  it('laat een parameter toevoegen en weglaten', async () => {
    const { wrapper } = mountPanel(established);
    expect(wrapper.findAll('nldd-text-field')).toHaveLength(2);
    await wrapper.findAll('nldd-button')[1].trigger('click');
    expect(wrapper.findAll('nldd-text-field')).toHaveLength(4);
    await wrapper.findAll('nldd-icon-button')[0].trigger('click');
    expect(wrapper.findAll('nldd-text-field')).toHaveLength(2);
  });
});
