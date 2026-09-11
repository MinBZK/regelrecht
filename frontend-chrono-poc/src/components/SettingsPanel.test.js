import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import SettingsPanel from './SettingsPanel.vue';
import { cloneWorld, worldFixture } from '../testing/worldFixture.js';

function mountPanel(snapshot = worldFixture) {
  return mount(SettingsPanel, { props: { snapshot } });
}

/** Hetzelfde beeld met een instelling die nog vrij is. */
function withFreeSetting() {
  const snapshot = cloneWorld();
  snapshot.settings = { ...snapshot.settings, drempel: 25 };
  return snapshot;
}

describe('het instellingen-paneel', () => {
  it('zet elke instelling uit het wereldbestand neer, met haar type', () => {
    const wrapper = mountPanel(withFreeSetting());
    const labels = wrapper.findAll('nldd-form-field').map((field) => field.attributes('label'));
    expect(labels).toStrictEqual(['Betalingsritme', 'Drempel']);
    expect(wrapper.findAll('nldd-number-field')).toHaveLength(1);
  });

  it('zegt van een vaste instelling waardoor ze vastligt, en laat haar niet wijzigen', () => {
    const wrapper = mountPanel();
    const locked = worldFixture.locked_settings.betalingsritme;
    const field = wrapper.find('nldd-form-field');
    expect(field.attributes('supporting-label')).toContain(`besluit '${locked.besluit}'`);
    expect(field.attributes('supporting-label')).toContain(`cel '${locked.cell}'`);
    expect(wrapper.find('nldd-text-field').attributes('disabled')).toBe('true');
    const summary = wrapper.findAll('nldd-text-cell').map((cell) => cell.attributes('supporting-text'));
    expect(summary.some((text) => text?.includes('rekende er al mee'))).toBe(true);
  });

  it('slaat alleen op wat veranderd is', async () => {
    const wrapper = mountPanel(withFreeSetting());
    const save = wrapper.findAll('nldd-button').find((button) => button.attributes('type') === 'submit');
    expect(save.attributes('disabled')).toBe('true');

    wrapper.find('nldd-number-field').element.dispatchEvent(new CustomEvent('input', { detail: { value: 40 } }));
    await wrapper.vm.$nextTick();
    expect(save.attributes('disabled')).toBeUndefined();

    await wrapper.find('form').trigger('submit');
    expect(wrapper.emitted('save')).toStrictEqual([[{ drempel: 40 }]]);
  });

  // Een leeg getalveld is niets ingevuld. `Number('')` is 0, dus zonder die
  // scheiding zou het leegmaken van een veld de instelling op nul zetten.
  it('houdt een leeggemaakt getalveld voor niets ingevuld en niet voor nul', async () => {
    const wrapper = mountPanel(withFreeSetting());
    const field = wrapper.find('nldd-number-field');
    const save = wrapper.findAll('nldd-button').find((button) => button.attributes('type') === 'submit');

    field.element.dispatchEvent(new CustomEvent('input', { detail: { value: '' } }));
    await wrapper.vm.$nextTick();
    expect(save.attributes('disabled')).toBe('true');

    await wrapper.find('form').trigger('submit');
    expect(wrapper.emitted('save')).toBeUndefined();
  });

  it('vraagt om bevestiging voordat de wereld teruggaat naar de startstand', async () => {
    const wrapper = mountPanel();
    const dialog = wrapper.find('nldd-modal-dialog');
    expect(dialog.attributes('variant')).toBe('alert');
    expect(dialog.attributes('supporting-text')).toContain('klok gaat terug');

    // De knop in de dialoog zet het terugzetten door; de knop ernaast opent hem.
    const confirm = wrapper
      .findAll('nldd-button')
      .find((button) => button.attributes('text') === 'Terugzetten');
    await confirm.trigger('click');
    expect(wrapper.emitted('reset')).toHaveLength(1);
  });

  it('zegt het als een wereldbestand geen instellingen heeft', () => {
    const snapshot = cloneWorld();
    snapshot.settings = {};
    snapshot.locked_settings = {};
    const wrapper = mountPanel(snapshot);
    expect(wrapper.find('nldd-inline-dialog').attributes('text')).toBe('Geen instellingen');
  });
});
