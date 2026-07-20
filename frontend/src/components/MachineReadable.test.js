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

// "Bewerk" moved from an inline nldd-button to a RowActionsMenu menu-item
// (the more-menu). In mount() custom elements aren't upgraded, so the
// menu-item is just a DOM node and triggering click still fires the
// component's @click → emit('edit'). DOM order per row is unchanged, so
// the index mapping used by the tests is preserved.
function findBewerkButtons(wrapper) {
  return wrapper.findAll('nldd-menu-item[text="Bewerk"]');
}

function findBekijkButtons(wrapper) {
  return wrapper.findAll('nldd-button[text="Bekijk"]');
}

async function clickBewerk(wrapper, index) {
  const buttons = findBewerkButtons(wrapper);
  await buttons[index].trigger('click');
}

// Add buttons all carry a stable data-testid (`add-{section}-btn`); using
// it sidesteps copy churn — we changed labels from "Nieuwe X" to
// "X toevoegen" and don't want the test to break on the next reword.
const ADD_BTN_TESTID = {
  definitie: 'add-def-btn',
  parameter: 'add-param-btn',
  input: 'add-input-btn',
  output: 'add-output-btn',
  action: 'add-action-btn',
};
function findAddButton(wrapper, section) {
  return wrapper.find(`[data-testid="${ADD_BTN_TESTID[section]}"]`);
}

// The destructive button in the confirm modal that actually emits the
// delete event. There's one per pendingDelete (the modal renders only
// when something is staged) so a single global lookup is fine.
function findConfirmDeleteButton(wrapper) {
  return wrapper.find('nldd-button[variant="destructive"][text="Verwijder"]');
}

describe('MachineReadable', () => {
  describe('display mode', () => {
    it('renders all sections', () => {
      const wrapper = mountEditable();
      const headings = wrapper.findAll('nldd-title');
      const titles = headings.map((h) => h.text());
      expect(titles).toContain('Definities');
      expect(titles).toContain('Parameters');
      expect(titles).toContain('Inputs');
      expect(titles).toContain('Outputs');
      expect(titles).toContain('Acties');
    });

    it('shows produces metadata as editable dropdowns', () => {
      const wrapper = mountEditable();
      expect(wrapper.find('select[aria-label="Juridische basis"]').exists()).toBe(true);
      expect(wrapper.find('select[aria-label="Besluit-type"]').exists()).toBe(true);
      const opts = wrapper.findAll('option').map((o) => o.text());
      expect(opts).toContain('beschikking');
      expect(opts).toContain('toekenning');
    });

    it('shows empty state when no machine_readable', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: { number: '1' }, editable: true },
      });
      expect(wrapper.find('nldd-inline-dialog').attributes('text')).toContain('Geen machine-leesbare gegevens');
    });

    it('formats percentage values (0 < v < 1)', () => {
      const wrapper = mountEditable();
      // The value sits in the `text` attribute (read-only cells) or as
      // slot text next to BreakableName (editable definition rows).
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
      expect(cells.some(t => /1,896\s*%/.test(t))).toBe(true);
    });

    it('formats eurocent values as currency', () => {
      const wrapper = mountEditable();
      // The value sits in the `text` attribute (read-only cells) or as
      // slot text next to BreakableName (editable definition rows).
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
      expect(cells.some(t => /1\.500,00/.test(t))).toBe(true);
    });

    it('shows plain number when no unit', () => {
      const wrapper = mountEditable();
      // The value sits in the `text` attribute (read-only cells) or as
      // slot text next to BreakableName (editable definition rows).
      const cells = wrapper.findAll('nldd-text-cell').map(c => `${c.attributes('text') || ''} ${c.text()}`);
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
      // No add-X-btn data-testid should be present in read-only mode.
      const addButtons = wrapper.findAll('[data-testid^="add-"][data-testid$="-btn"]');
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
    it('renders action rows as button-type list items with a chevron', () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      // No Bewerk/Bekijk buttons in read-only mode; list-item itself is the trigger.
      expect(findBekijkButtons(wrapper).length).toBe(0);
      const actionItems = wrapper.findAll('nldd-list-item[button]');
      expect(actionItems.length).toBeGreaterThan(0);
    });

    it('clicking an action row emits open-action', async () => {
      const wrapper = mount(MachineReadable, {
        props: { article: createArticle(), editable: false },
      });
      const actionItems = wrapper.findAll('nldd-list-item[button]');
      await actionItems[0].trigger('click');
      expect(wrapper.emitted('open-action')).toHaveLength(1);
      expect(wrapper.emitted('open-edit')).toBeUndefined();
    });
  });

  describe('save error dialog', () => {
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

    // The minus icon now stages a confirmation in `pendingDelete` and the
    // destructive button in the modal-dialog is what actually emits the
    // delete event. Each test exercises that two-step flow so we cover the
    // user-facing contract, not the internal staging state.
    async function clickDeleteAndConfirm(wrapper, testid) {
      await findDeleteByTestId(wrapper, testid).trigger('click');
      await findConfirmDeleteButton(wrapper).trigger('click');
    }

    it('emits delete with definition payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'def-drempelinkomen-delete-btn');
      const events = wrapper.emitted('delete');
      expect(events).toHaveLength(1);
      expect(events[0][0]).toEqual({ section: 'definition', key: 'drempelinkomen' });
    });

    it('does not emit delete on the minus click alone (waits for confirmation)', async () => {
      const wrapper = mountEditable();
      await findDeleteByTestId(wrapper, 'def-drempelinkomen-delete-btn').trigger('click');
      expect(wrapper.emitted('delete')).toBeUndefined();
    });

    it('emits delete with parameter index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'param-bsn-delete-btn');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'parameter', index: 0 });
    });

    it('emits delete with input index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'input-leeftijd-delete-btn');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'input', index: 0 });
    });

    it('emits delete with output index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'output-heeft_recht-delete-btn');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'output', index: 0 });
    });

    it('emits delete with action index payload after confirming', async () => {
      const wrapper = mountEditable();
      await clickDeleteAndConfirm(wrapper, 'action-hoogte-delete-btn');
      const events = wrapper.emitted('delete');
      expect(events[0][0]).toEqual({ section: 'action', index: 0 });
    });
  });
});
