import { describe, it, expect, beforeAll } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import OperationSettings from './OperationSettings.vue';

// `ndd-*` tags are configured as custom elements in vite.config.js, so
// Vue Test Utils' stub system doesn't replace them. None of the tests
// here interact with the rendered NDD elements (we drive the component
// via wrapper.vm), so we leave them as raw HTMLElement and only stub the
// methods that the watcher / lifecycle would otherwise complain about.
beforeAll(() => {
  // No custom element registration needed for OperationSettings — it
  // doesn't call show()/hide() on any host the way EditSheet does.
});

/**
 * The component takes an `operation` prop in the shape produced by
 * buildOperationTree(): `{ number, title, subtitle, operation, values, node }`.
 * For these tests we only care about `operation` (the type) and `node` (the
 * mutable underlying object that operationValues / mutations read and
 * write). Helper to build a minimal valid prop.
 */
function makeOpProp(node) {
  return {
    number: '1',
    title: 'test op',
    subtitle: '',
    operation: node.operation,
    values: node.values || [],
    node,
  };
}

function mountOp(node, { editable = true } = {}) {
  return mount(OperationSettings, {
    props: {
      operation: makeOpProp(node),
      article: { machine_readable: { execution: { input: [] } } },
      editable,
    },
  });
}

describe('OperationSettings — AGE op', () => {
  describe('operationValues', () => {
    it('returns date_of_birth and reference_date rows for an AGE node', () => {
      const node = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);
      const rows = wrapper.vm.operationValues;
      expect(rows).toHaveLength(2);
      expect(rows[0]).toMatchObject({
        _label: 'Geboortedatum',
        _value: '$geboortedatum',
        _kind: 'date_of_birth',
      });
      expect(rows[1]).toMatchObject({
        _label: 'Peildatum',
        _value: '2025-01-01',
        _kind: 'reference_date',
      });
    });

    it('falls back to empty strings for missing AGE fields', () => {
      const node = { operation: 'AGE' };
      const wrapper = mountOp(node);
      const rows = wrapper.vm.operationValues;
      expect(rows[0]._value).toBe('');
      expect(rows[1]._value).toBe('');
    });
  });

  describe('changeOperationType to AGE', () => {
    it('seeds date_of_birth and reference_date as empty strings', async () => {
      const node = {
        operation: 'EQUALS',
        subject: '$foo',
        value: 42,
      };
      const wrapper = mountOp(node);

      // Simulate the dropdown change. The handler reads event.target.value.
      wrapper.vm.changeOperationType({ target: { value: 'AGE' } });
      await nextTick();

      expect(node.operation).toBe('AGE');
      expect(node.date_of_birth).toBe('');
      expect(node.reference_date).toBe('');
      // The old comparison-shape fields are stripped.
      expect(node.subject).toBeUndefined();
      expect(node.value).toBeUndefined();
    });

    it('preserves existing AGE fields when re-entering AGE', async () => {
      const node = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);

      // Switching to the same op type is a no-op (early return).
      wrapper.vm.changeOperationType({ target: { value: 'AGE' } });
      await nextTick();

      expect(node.date_of_birth).toBe('$geboortedatum');
      expect(node.reference_date).toBe('2025-01-01');
    });

    it('strips AGE fields when switching back to a comparison op', async () => {
      const node = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType({ target: { value: 'EQUALS' } });
      await nextTick();

      expect(node.operation).toBe('EQUALS');
      expect(node.subject).toBe('');
      expect(node.value).toBe('');
      // The schema sets `additionalProperties: false` on every operation
      // type, so any leaked AGE field would fail validation on save.
      // Both must be removed by the EQUALS branch.
      expect(node.date_of_birth).toBeUndefined();
      expect(node.reference_date).toBeUndefined();
    });

    it.each([
      ['AND', { conditions: [] }],
      ['IF', { cases: expect.any(Array), default: 0 }],
      ['NOT', { value: '' }],
      ['SWITCH', { cases: expect.any(Array), default: 0 }],
      ['ADD', { values: [] }],
    ])('strips AGE fields when switching to %s', async (newType, _expected) => {
      const node = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);

      wrapper.vm.changeOperationType({ target: { value: newType } });
      await nextTick();

      expect(node.operation).toBe(newType);
      expect(node.date_of_birth).toBeUndefined();
      expect(node.reference_date).toBeUndefined();
    });
  });

  describe('applyValueMutation', () => {
    it('writes date_of_birth on the node', () => {
      const node = { operation: 'AGE', date_of_birth: '', reference_date: '' };
      const wrapper = mountOp(node);
      wrapper.vm.applyValueMutation({ _kind: 'date_of_birth' }, '$geboortedatum');
      expect(node.date_of_birth).toBe('$geboortedatum');
    });

    it('writes reference_date on the node', () => {
      const node = { operation: 'AGE', date_of_birth: '', reference_date: '' };
      const wrapper = mountOp(node);
      wrapper.vm.applyValueMutation({ _kind: 'reference_date' }, '2025-01-01');
      expect(node.reference_date).toBe('2025-01-01');
    });
  });

  describe('canRemoveValue', () => {
    it('blocks removal of both AGE structural fields', () => {
      const node = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);
      expect(wrapper.vm.canRemoveValue({ _kind: 'date_of_birth' })).toBe(false);
      expect(wrapper.vm.canRemoveValue({ _kind: 'reference_date' })).toBe(false);
    });
  });

  describe('canAddValue / canAddNestedOperation', () => {
    it('disables both add buttons for AGE', () => {
      const node = {
        operation: 'AGE',
        date_of_birth: '$geboortedatum',
        reference_date: '2025-01-01',
      };
      const wrapper = mountOp(node);
      expect(wrapper.vm.canAddValue).toBe(false);
      expect(wrapper.vm.canAddNestedOperation).toBe(false);
    });
  });
});
