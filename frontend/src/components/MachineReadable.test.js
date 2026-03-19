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

function findBewerkButtons(wrapper) {
  return wrapper.findAll('rr-button').filter((b) => b.text() === 'Bewerk');
}

async function clickBewerk(wrapper, index) {
  const buttons = findBewerkButtons(wrapper);
  await buttons[index].trigger('click');
}

function findAddButton(wrapper, text) {
  return wrapper.findAll('.add-button').find((b) => b.text().includes(text));
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
      expect(wrapper.text()).toMatch(/1,896\s*%/);
    });

    it('formats eurocent values as currency', () => {
      const wrapper = mountEditable();
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

  describe('definition editing', () => {
    it('emits open-edit with definition data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 0);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(events[0][0].section).toBe('definition');
      expect(events[0][0].key).toBe('drempelinkomen');
      expect(events[0][0].rawDef).toEqual({ value: 3971900 });
    });

    it('emits open-edit for eurocent definition', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 2);
      const events = wrapper.emitted('open-edit');
      expect(events[0][0].key).toBe('standaard_bedrag');
      expect(events[0][0].rawDef).toEqual({ value: 150000, type_spec: { unit: 'eurocent' } });
    });
  });

  describe('parameter editing', () => {
    it('emits open-edit with parameter data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 3);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(events[0][0]).toEqual({
        section: 'parameter',
        index: 0,
        data: { name: 'bsn', type: 'string', required: true },
      });
    });
  });

  describe('input editing', () => {
    it('emits open-edit with input data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 4);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(events[0][0].section).toBe('input');
      expect(events[0][0].index).toBe(0);
      expect(events[0][0].data.name).toBe('leeftijd');
      expect(events[0][0].data.source.regulation).toBe('wet_brp');
    });
  });

  describe('output editing', () => {
    it('emits open-edit with output data on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 6);
      const events = wrapper.emitted('open-edit');
      expect(events).toHaveLength(1);
      expect(events[0][0]).toEqual({
        section: 'output',
        index: 0,
        data: { name: 'heeft_recht', type: 'boolean' },
      });
    });

    it('emits open-edit preserving type_spec', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 7);
      const events = wrapper.emitted('open-edit');
      expect(events[0][0].data.type_spec).toEqual({ unit: 'eurocent' });
    });
  });

  describe('actions', () => {
    it('emits open-action on Bewerk click', async () => {
      const wrapper = mountEditable();
      await clickBewerk(wrapper, 8);
      const events = wrapper.emitted('open-action');
      expect(events).toHaveLength(1);
      expect(events[0][0].output).toBe('hoogte');
    });
  });

  describe('adding new items', () => {
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

    it('emits open-edit for new definition', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'definitie').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(events[0][0].section).toBe('add-definition');
      expect(events[0][0].isNew).toBe(true);
    });

    it('emits open-edit for new parameter', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'parameter').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(events[0][0].section).toBe('add-parameter');
      expect(events[0][0].isNew).toBe(true);
    });

    it('emits open-edit for new input', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'input').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(events[0][0].section).toBe('add-input');
      expect(events[0][0].isNew).toBe(true);
    });

    it('emits open-edit for new output', async () => {
      const wrapper = mountEditable();
      await findAddButton(wrapper, 'output').trigger('click');
      const events = wrapper.emitted('open-edit');
      expect(events[0][0].section).toBe('add-output');
      expect(events[0][0].isNew).toBe(true);
    });
  });

  describe('non-editable mode', () => {
    it('only shows Bewerk button for actions when editable is false', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const buttons = findBewerkButtons(wrapper);
      // Only the action Bewerk button is rendered
      expect(buttons.length).toBe(1);
    });

    it('action Bewerk emits open-action even when not editable', async () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const buttons = findBewerkButtons(wrapper);
      await buttons[0].trigger('click');
      expect(wrapper.emitted('open-action')).toHaveLength(1);
      expect(wrapper.emitted('open-edit')).toBeUndefined();
    });
  });
});
