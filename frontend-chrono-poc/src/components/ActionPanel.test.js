import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import ActionPanel from './ActionPanel.vue';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';

/** Een waarde in een veld zetten zoals een ontwerpsysteem-veld dat meldt. */
async function fill(wrapper, element, value) {
  element.element.dispatchEvent(new CustomEvent('input', { detail: { value } }));
  await wrapper.vm.$nextTick();
}

function mountPanel(snapshot = worldFixture) {
  return mount(ActionPanel, { props: { snapshot } });
}

describe('het actiepaneel', () => {
  it('groepeert de acties per actor', () => {
    const wrapper = mountPanel();
    const actors = [...new Set(worldFixture.actions.map((action) => action.actor))];
    for (const actor of actors) expect(wrapper.text()).toContain(actor);
    expect(wrapper.findAll('nldd-card')).toHaveLength(worldFixture.actions.length);
  });

  it('zet het formulier uit de actie, met het veld van het juiste type', () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    const action = worldFixture.actions[0];
    expect(action.form.map((field) => field.type)).toStrictEqual(['string', 'number']);
    expect(card.findAll('nldd-text-field')).toHaveLength(1);
    expect(card.findAll('nldd-number-field')).toHaveLength(1);
    const labels = card.findAll('nldd-form-field').map((field) => field.attributes('label'));
    expect(labels).toStrictEqual(['Bsn', 'Jaar']);
    const types = card.findAll('nldd-form-field').map((field) => field.attributes('supporting-label'));
    expect(types).toStrictEqual(action.form.map((field) => field.type));
  });

  it('zegt of een actie nu kan, en waarom niet', () => {
    const snapshot = cloneWorld();
    snapshot.actions[1].available = false;
    snapshot.actions[1].unavailable_reason = "cel 'toeslagen' heeft nog geen aanvraag in kroniek 'aanvragen'";
    const wrapper = mountPanel(snapshot);
    const tags = wrapper.findAll('nldd-tag').map((tag) => tag.attributes('text'));
    expect(tags).toContain('kan nu');
    expect(tags).toContain('kan nu niet');
    const banner = wrapper.find('nldd-banner');
    expect(banner.attributes('variant')).toBe('warning');
    expect(banner.attributes('supporting-text')).toContain('nog geen aanvraag');
  });

  it('voert de actie uit met de ingevulde waarden, elk in zijn eigen type', async () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    await fill(wrapper, card.find('nldd-text-field'), '999993653');
    await fill(wrapper, card.find('nldd-number-field'), 2025);
    await card.find('form').trigger('submit');

    expect(wrapper.emitted('run')).toHaveLength(1);
    const [{ action, values }] = wrapper.emitted('run')[0];
    expect(action.id).toBe(worldFixture.actions[0].id);
    expect(values).toStrictEqual({ bsn: '999993653', jaar: 2025 });
  });

  it('vraagt eerst om een leeg veld in plaats van het als niets te versturen', async () => {
    const wrapper = mountPanel();
    const card = wrapper.findAll('nldd-card')[0];
    await card.find('form').trigger('submit');

    expect(wrapper.emitted('run')).toBeUndefined();
    expect(card.findAll('nldd-form-field-error-text')).toHaveLength(2);
    expect(card.find('nldd-text-field').attributes('invalid')).toBe('true');
    expect(card.find('nldd-text-field').attributes('error-message')).toBe(
      card.find('nldd-form-field-error-text').attributes('id'),
    );

    // Zodra er iets staat, is de melding weg.
    await fill(wrapper, card.find('nldd-text-field'), '999993653');
    expect(card.findAll('nldd-form-field-error-text')).toHaveLength(1);
  });

  it('zegt het als het wereldbestand geen acties beschrijft', () => {
    const snapshot = cloneWorld();
    snapshot.actions = [];
    const wrapper = mountPanel(snapshot);
    expect(wrapper.find('nldd-inline-dialog').attributes('text')).toBe('Geen acties');
  });
});
