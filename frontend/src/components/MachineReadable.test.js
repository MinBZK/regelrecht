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
  return wrapper.findAll('ndd-button[text="Bewerk"]');
}

function findBekijkButtons(wrapper) {
  return wrapper.findAll('ndd-button[text="Bekijk"]');
}

async function clickBewerk(wrapper, index) {
  const buttons = findBewerkButtons(wrapper);
  await buttons[index].trigger('click');
}

function findAddButton(wrapper, text) {
  return wrapper.findAll('ndd-button').find((b) => b.attributes('text')?.includes(`Nieuwe ${text}`));
}

describe('MachineReadable', () => {
  describe('display mode', () => {
    it('renders all sections', () => {
      const wrapper = mountEditable();
      const headings = wrapper.findAll('ndd-title');
      const titles = headings.map((h) => h.text());
      expect(titles).toContain('Definities');
      expect(titles).toContain('Parameters');
      expect(titles).toContain('Inputs');
      expect(titles).toContain('Outputs');
      expect(titles).toContain('Acties');
    });

    it('shows produces metadata', () => {
      const wrapper = mountEditable();
      expect(wrapper.find('ndd-button[text="BESCHIKKING"]').exists()).toBe(true);
      expect(wrapper.find('ndd-button[text="TOEKENNING"]').exists()).toBe(true);
    });

    it('shows empty state when no machine_readable', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: { number: '1' }, editable: true },
      });
      expect(wrapper.find('ndd-inline-dialog').attributes('text')).toContain('Geen machine-leesbare gegevens');
    });

    it('formats percentage values (0 < v < 1)', () => {
      const wrapper = mountEditable();
      const cells = wrapper.findAll('ndd-text-cell').map(c => c.attributes('text') || '');
      expect(cells.some(t => /1,896\s*%/.test(t))).toBe(true);
    });

    it('formats eurocent values as currency', () => {
      const wrapper = mountEditable();
      const cells = wrapper.findAll('ndd-text-cell').map(c => c.attributes('text') || '');
      expect(cells.some(t => /1\.500,00/.test(t))).toBe(true);
    });

    it('shows plain number when no unit', () => {
      const wrapper = mountEditable();
      const cells = wrapper.findAll('ndd-text-cell').map(c => c.attributes('text') || '');
      expect(cells.some(t => t.includes('3971900'))).toBe(true);
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
      const addButtons = wrapper.findAll('ndd-button').filter((b) => b.attributes('text')?.includes('Nieuwe'));
      expect(addButtons.length).toBe(0);
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
    it('only shows Bekijk button for actions when editable is false', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const buttons = findBekijkButtons(wrapper);
      // Only the action Bekijk button is rendered
      expect(buttons.length).toBe(1);
    });

    it('action Bekijk emits open-action even when not editable', async () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const buttons = findBekijkButtons(wrapper);
      await buttons[0].trigger('click');
      expect(wrapper.emitted('open-action')).toHaveLength(1);
      expect(wrapper.emitted('open-edit')).toBeUndefined();
    });
  });

  describe('save bar', () => {
    function findSaveButton(wrapper) {
      return wrapper.find('[data-testid="save-mr-btn"]');
    }

    it('is hidden when editable is false', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      expect(findSaveButton(wrapper).exists()).toBe(false);
    });

    it('shows "Opgeslagen" and is disabled when not dirty', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: true, dirty: false },
      });
      const btn = findSaveButton(wrapper);
      expect(btn.exists()).toBe(true);
      expect(btn.attributes('text')).toBe('Opgeslagen');
      expect(btn.attributes('disabled')).toBe('true');
    });

    it('shows "Opslaan" and reports not-disabled when dirty', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: true, dirty: true },
      });
      const btn = findSaveButton(wrapper);
      expect(btn.attributes('text')).toBe('Opslaan');
      // Vue serializes a reactive bool binding as "true"/"false" on a custom
      // element; the string "false" means the button is enabled.
      expect(btn.attributes('disabled')).toBe('false');
    });

    it('shows "Opslaan…" while saving regardless of dirty state', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: true, dirty: true, saving: true },
      });
      const btn = findSaveButton(wrapper);
      expect(btn.attributes('text')).toBe('Opslaan\u2026');
      expect(btn.attributes('disabled')).toBe('true');
    });

    it('emits save on click when dirty', async () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: true, dirty: true },
      });
      await findSaveButton(wrapper).trigger('click');
      expect(wrapper.emitted('save')).toHaveLength(1);
    });

    it('renders save error dialog when saveError is set', () => {
      const err = new Error('Forbidden: read-only backend');
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: true, saveError: err },
      });
      const dialog = wrapper.find('[data-testid="save-mr-error"]');
      expect(dialog.exists()).toBe(true);
      expect(dialog.attributes('supporting-text')).toBe('Forbidden: read-only backend');
    });

    it('does not render save error dialog when saveError is null', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: true, saveError: null },
      });
      expect(wrapper.find('[data-testid="save-mr-error"]').exists()).toBe(false);
    });
  });

  describe('delete row buttons', () => {
    function findDeleteByTestId(wrapper, testid) {
      return wrapper.find(`[data-testid="${testid}"]`);
    }

    it('renders a minus icon button next to every editable row when editable', () => {
      const wrapper = mountEditable();
      // 3 definitions + 1 parameter + 2 inputs + 2 outputs + 1 action = 9
      expect(wrapper.findAll('[data-testid$="-delete-btn"]')).toHaveLength(9);
    });

    it('does not render delete buttons in read-only mode', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      expect(wrapper.findAll('[data-testid$="-delete-btn"]')).toHaveLength(0);
    });

    it('emits delete with definition payload when the def minus is clicked', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'def-drempelinkomen-delete-btn').trigger('click');
      const events = wrapper.emitted('delete');
      expect(events).toHaveLength(1);
      expect(events[0][0]).toEqual({ section: 'definition', key: 'drempelinkomen' });
    });

    it('emits delete with parameter index payload', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'param-bsn-delete-btn').trigger('click');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'parameter', index: 0 });
    });

    it('emits delete with input index payload', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'input-leeftijd-delete-btn').trigger('click');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'input', index: 0 });
    });

    it('emits delete with output index payload', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'output-heeft_recht-delete-btn').trigger('click');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'output', index: 0 });
    });

    it('emits delete with action index payload', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'action-hoogte-delete-btn').trigger('click');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'action', index: 0 });
    });
  });
});
