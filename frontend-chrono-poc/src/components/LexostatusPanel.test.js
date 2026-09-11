import { mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import LexostatusPanel from './LexostatusPanel.vue';
import { worldFixture } from '../testing/worldFixture.js';

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

  it('stelt de vraag met de opgegeven parameters', async () => {
    const { wrapper, ask } = mountPanel(established);
    const params = wrapper.findAll('nldd-text-field');
    params[0].element.dispatchEvent(new CustomEvent('input', { detail: { value: 'bsn' } }));
    params[1].element.dispatchEvent(new CustomEvent('input', { detail: { value: '999993653' } }));
    await wrapper.vm.$nextTick();
    await submit(wrapper);

    const publishers = worldFixture.cells.filter((cell) => cell.lexostatussen.length > 0);
    expect(ask).toHaveBeenCalledWith(publishers[0].id, publishers[0].lexostatussen[0], { bsn: '999993653' });
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
