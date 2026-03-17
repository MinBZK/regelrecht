import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import MachineReadable from './MachineReadable.vue';

// Minimal article fixture with all section types
function createArticle(overrides = {}) {
  return {
    number: '2',
    machine_readable: {
      definitions: {
        drempelinkomen: { value: 3971900 },
        percentage_drempel: { value: 0.01896 },
        standaard_bedrag: { value: 150000, type_spec: { unit: 'eurocent' } },
      },
      execution: {
        produces: {
          legal_character: 'BESCHIKKING',
          decision_type: 'TOEKENNING',
        },
        parameters: [
          { name: 'bsn', type: 'string', required: true },
        ],
        input: [
          {
            name: 'leeftijd',
            type: 'number',
            source: {
              regulation: 'wet_brp',
              output: 'leeftijd',
              parameters: { bsn: '$bsn' },
            },
          },
          {
            name: 'is_verzekerde',
            type: 'boolean',
            source: {
              regulation: 'zorgverzekeringswet',
              output: 'is_verzekerd',
            },
          },
        ],
        output: [
          { name: 'heeft_recht', type: 'boolean' },
          { name: 'hoogte', type: 'amount', type_spec: { unit: 'eurocent' } },
        ],
        actions: [
          { output: 'hoogte', value: 0, operation: { type: 'LITERAL', value: 0 } },
        ],
      },
      ...overrides,
    },
  };
}

function mountEditable(articleOverrides = {}) {
  return mount(MachineReadable, {
    props: {
      article: createArticle(articleOverrides),
      editable: true,
    },
  });
}

// Helper: find all Bewerk buttons in a wrapper
function findBewerkButtons(wrapper) {
  return wrapper.findAll('rr-button').filter((b) => b.text() === 'Bewerk');
}

// Helper: click the nth Bewerk button (0-indexed)
async function clickBewerk(wrapper, index) {
  const buttons = findBewerkButtons(wrapper);
  await buttons[index].trigger('click');
}

describe('MachineReadable', () => {
  describe('display mode', () => {
    it('renders all sections', () => {
      const wrapper = mountEditable();
      const headings = wrapper.findAll('h3');
      const titles = headings.map((h) => h.text());
      expect(titles).toContain('Definities');
      expect(titles).toContain('Parameters');
      expect(titles).toContain('Inputs');
      expect(titles).toContain('Outputs');
      expect(titles).toContain('Acties');
    });

    it('shows produces metadata', () => {
      const wrapper = mountEditable();
      expect(wrapper.text()).toContain('BESCHIKKING');
      expect(wrapper.text()).toContain('TOEKENNING');
    });

    it('shows empty state when no machine_readable', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: { number: '1' }, editable: true },
      });
      expect(wrapper.text()).toContain('Geen machine-leesbare gegevens');
    });

    it('formats percentage values (0 < v < 1)', () => {
      const wrapper = mountEditable();
      // 0.01896 → "1,896%"
      expect(wrapper.text()).toMatch(/1,896\s*%/);
    });

    it('formats eurocent values as currency', () => {
      const wrapper = mountEditable();
      // 150000 eurocent → €1.500,00
      expect(wrapper.text()).toMatch(/1\.500,00/);
    });

    it('shows plain number when no unit', () => {
      const wrapper = mountEditable();
      expect(wrapper.text()).toContain('3971900');
    });

    it('shows Bewerk buttons for each editable item', () => {
      const wrapper = mountEditable();
      const buttons = findBewerkButtons(wrapper);
      // 3 defs + 1 param + 2 inputs + 2 outputs + 1 action = 9
      expect(buttons.length).toBe(9);
    });
  });

  describe('definitions editing', () => {
    it('opens edit form on Bewerk click', async () => {
      const wrapper = mountEditable();
      // First Bewerk is first definition (drempelinkomen)
      await clickBewerk(wrapper, 0);
      const editForm = wrapper.find('.edit-form');
      expect(editForm.exists()).toBe(true);
    });

    it('shows editable name field', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      const nameInput = wrapper.find('.edit-form input[type="text"]');
      expect(nameInput.exists()).toBe(true);
      expect(nameInput.element.value).toBe('drempelinkomen');
      expect(nameInput.element.disabled).toBe(false);
    });

    it('uses number input for plain numbers', async () => {
      const wrapper = mountEditable();
      // drempelinkomen (value: 3971900, no unit, not 0-1)
      await clickBewerk(wrapper, 0);
      const numberInput = wrapper.find('.edit-form input[type="number"]');
      expect(numberInput.exists()).toBe(true);
      expect(numberInput.element.value).toBe('3971900');
    });

    it('uses percentage control for 0-1 values', async () => {
      const wrapper = mountEditable();
      // percentage_drempel (value: 0.01896) → second definition
      await clickBewerk(wrapper, 1);
      const suffix = wrapper.find('.edit-input-suffix');
      expect(suffix.exists()).toBe(true);
      expect(suffix.text()).toBe('%');
      const input = wrapper.find('.edit-input-group input[type="number"]');
      // displayed as 1.896 (percentage)
      expect(Number(input.element.value)).toBeCloseTo(1.896, 2);
    });

    it('uses currency control for eurocent values', async () => {
      const wrapper = mountEditable();
      // standaard_bedrag (value: 150000, unit: eurocent) → third definition
      await clickBewerk(wrapper, 2);
      const prefix = wrapper.find('.edit-input-prefix');
      expect(prefix.exists()).toBe(true);
      expect(prefix.text()).toBe('€');
      const input = wrapper.find('.edit-input-group input[type="number"]');
      // 150000 eurocent → 1500.00 euros
      expect(Number(input.element.value)).toBeCloseTo(1500, 0);
    });

    it('emits save with converted value on Opslaan', async () => {
      const wrapper = mountEditable();
      // Edit percentage_drempel
      await clickBewerk(wrapper, 1);
      const input = wrapper.find('.edit-input-group input[type="number"]');
      await input.setValue(2.5);
      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events).toHaveLength(1);
      expect(events[0][0].section).toBe('definition');
      expect(events[0][0].key).toBe('percentage_drempel');
      expect(events[0][0].newKey).toBe('percentage_drempel');
      // 2.5% → stored as 0.025
      expect(events[0][0].data.value).toBeCloseTo(0.025, 6);
    });

    it('emits save with eurocent conversion', async () => {
      const wrapper = mountEditable();
      // Edit standaard_bedrag (eurocent)
      await clickBewerk(wrapper, 2);
      const input = wrapper.find('.edit-input-group input[type="number"]');
      await input.setValue(2000);
      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('definition');
      expect(events[0][0].key).toBe('standaard_bedrag');
      // €2000 → 200000 eurocent
      expect(events[0][0].data.value).toBe(200000);
      // Preserves type_spec
      expect(events[0][0].data.type_spec).toEqual({ unit: 'eurocent' });
    });

    it('emits save with renamed key', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      const nameInput = wrapper.find('.edit-form input[type="text"]');
      await nameInput.setValue('vermogensgrens_alleenstaand');
      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0].key).toBe('drempelinkomen');
      expect(events[0][0].newKey).toBe('vermogensgrens_alleenstaand');
      expect(events[0][0].data.value).toBe(3971900);
    });

    it('closes edit form on Annuleer', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      expect(wrapper.find('.edit-form').exists()).toBe(true);
      const annuleer = wrapper.findAll('rr-button').find((b) => b.text() === 'Annuleer');
      await annuleer.trigger('click');
      expect(wrapper.find('.edit-form').exists()).toBe(false);
    });

    it('closes edit form after Opslaan', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');
      expect(wrapper.find('.edit-form').exists()).toBe(false);
    });

    it('only one item editable at a time', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      expect(wrapper.findAll('.edit-form').length).toBe(1);
      // Click Bewerk on second definition (now at different index since first is in edit mode)
      const bewerkButtons = findBewerkButtons(wrapper);
      await bewerkButtons[0].trigger('click');
      expect(wrapper.findAll('.edit-form').length).toBe(1);
    });
  });

  describe('parameter editing', () => {
    it('opens edit form with name, type select, and required checkbox', async () => {
      const wrapper = mountEditable();
      // Parameters Bewerk: after 3 definitions → index 3
      await clickBewerk(wrapper, 3);
      const editForm = wrapper.find('.edit-form');
      expect(editForm.exists()).toBe(true);

      const textInput = editForm.find('input[type="text"]');
      expect(textInput.element.value).toBe('bsn');

      const select = editForm.find('select');
      expect(select.element.value).toBe('string');

      const checkbox = editForm.find('input[type="checkbox"]');
      expect(checkbox.element.checked).toBe(true);
    });

    it('emits save with updated parameter', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 3);
      const editForm = wrapper.find('.edit-form');

      await editForm.find('input[type="text"]').setValue('burger_service_nummer');
      await editForm.find('select').setValue('number');
      await editForm.find('input[type="checkbox"]').setValue(false);

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events).toHaveLength(1);
      expect(events[0][0]).toEqual({
        section: 'parameter',
        index: 0,
        data: { name: 'burger_service_nummer', type: 'number', required: false },
      });
    });
  });

  describe('input editing', () => {
    it('opens edit form with name, type, and source fields', async () => {
      const wrapper = mountEditable();
      // Inputs Bewerk: after 3 defs + 1 param → index 4
      await clickBewerk(wrapper, 4);
      const editForm = wrapper.find('.edit-form');
      expect(editForm.exists()).toBe(true);

      const textInputs = editForm.findAll('input[type="text"]');
      // name, source regulation, source output
      expect(textInputs[0].element.value).toBe('leeftijd');
      expect(textInputs[1].element.value).toBe('wet_brp');
      expect(textInputs[2].element.value).toBe('leeftijd');

      expect(editForm.find('select').element.value).toBe('number');
    });

    it('emits save with updated input preserving source parameters', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 4);
      const editForm = wrapper.find('.edit-form');

      const textInputs = editForm.findAll('input[type="text"]');
      await textInputs[0].setValue('age');
      await textInputs[1].setValue('wet_brp_v2');

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('input');
      expect(events[0][0].index).toBe(0);
      expect(events[0][0].data.name).toBe('age');
      expect(events[0][0].data.source.regulation).toBe('wet_brp_v2');
      // Preserves parameters from original
      expect(events[0][0].data.source.parameters).toEqual({ bsn: '$bsn' });
    });
  });

  describe('output editing', () => {
    it('opens edit form with name and type', async () => {
      const wrapper = mountEditable();
      // Outputs Bewerk: after 3 defs + 1 param + 2 inputs → index 6
      await clickBewerk(wrapper, 6);
      const editForm = wrapper.find('.edit-form');
      expect(editForm.exists()).toBe(true);

      expect(editForm.find('input[type="text"]').element.value).toBe('heeft_recht');
      expect(editForm.find('select').element.value).toBe('boolean');
    });

    it('emits save with updated output preserving type_spec', async () => {
      const wrapper = mountEditable();
      // Second output (hoogte, amount, has type_spec) → index 7
      await clickBewerk(wrapper, 7);
      const editForm = wrapper.find('.edit-form');

      await editForm.find('input[type="text"]').setValue('bedrag');
      await editForm.find('select').setValue('number');

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0]).toEqual({
        section: 'output',
        index: 1,
        data: { name: 'bedrag', type: 'number', type_spec: { unit: 'eurocent' } },
      });
    });
  });

  describe('actions', () => {
    it('emits open-action on Bewerk click', async () => {
      const wrapper = mountEditable();
      // Action Bewerk: last Bewerk button → index 8
      await clickBewerk(wrapper, 8);
      const events = wrapper.emitted('open-action');
      expect(events).toHaveLength(1);
      expect(events[0][0].output).toBe('hoogte');
    });
  });

  describe('adding new items', () => {
    // Helper: find add button by text
    function findAddButton(wrapper, text) {
      return wrapper.findAll('.add-button').find((b) => b.text().includes(text));
    }

    it('shows add buttons for all sections when editable', () => {
      const wrapper = mountEditable();
      expect(findAddButton(wrapper, 'definitie').exists()).toBe(true);
      expect(findAddButton(wrapper, 'parameter').exists()).toBe(true);
      expect(findAddButton(wrapper, 'input').exists()).toBe(true);
      expect(findAddButton(wrapper, 'output').exists()).toBe(true);
    });

    it('does not show add buttons when not editable', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      expect(wrapper.findAll('.add-button').length).toBe(0);
    });

    it('opens add definition form and emits add-definition on save', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'definitie').trigger('click');

      const form = wrapper.find('.edit-form');
      expect(form.exists()).toBe(true);

      await form.find('input[type="text"]').setValue('nieuwe_waarde');
      await form.find('input[type="number"]').setValue(42);

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events).toHaveLength(1);
      expect(events[0][0].section).toBe('add-definition');
      expect(events[0][0].key).toBe('nieuwe_waarde');
      expect(events[0][0].data.value).toBe(42);
    });

    it('opens add parameter form and emits add-parameter on save', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'parameter').trigger('click');

      const form = wrapper.find('.edit-form');
      await form.find('input[type="text"]').setValue('nieuw_param');
      await form.find('select').setValue('boolean');
      await form.find('input[type="checkbox"]').setValue(true);

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('add-parameter');
      expect(events[0][0].data).toEqual({ name: 'nieuw_param', type: 'boolean', required: true });
    });

    it('opens add input form and emits add-input on save', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'input').trigger('click');

      const form = wrapper.find('.edit-form');
      const textInputs = form.findAll('input[type="text"]');
      await textInputs[0].setValue('nieuwe_input');
      await textInputs[1].setValue('bron_wet');
      await textInputs[2].setValue('bron_output');
      await form.find('select').setValue('number');

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('add-input');
      expect(events[0][0].data.name).toBe('nieuwe_input');
      expect(events[0][0].data.source.regulation).toBe('bron_wet');
      expect(events[0][0].data.source.output).toBe('bron_output');
    });

    it('opens add output form and emits add-output on save', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'output').trigger('click');

      const form = wrapper.find('.edit-form');
      await form.find('input[type="text"]').setValue('nieuw_resultaat');
      await form.find('select').setValue('amount');

      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      const events = wrapper.emitted('save');
      expect(events[0][0].section).toBe('add-output');
      expect(events[0][0].data).toEqual({ name: 'nieuw_resultaat', type: 'amount' });
    });

    it('does not save when name is empty', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'definitie').trigger('click');

      // Don't fill name, just click save
      const opslaan = wrapper.findAll('rr-button').find((b) => b.text() === 'Opslaan');
      await opslaan.trigger('click');

      expect(wrapper.emitted('save')).toBeUndefined();
      // Form stays open
      expect(wrapper.find('.edit-form').exists()).toBe(true);
    });
  });

  describe('non-editable mode', () => {
    it('does not open edit forms when editable is false', async () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const buttons = findBewerkButtons(wrapper);
      await buttons[0].trigger('click');
      expect(wrapper.find('.edit-form').exists()).toBe(false);
    });
  });
});
