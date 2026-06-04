import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ScenarioParameterInput from './ScenarioParameterInput.vue';

// nldd-* tags render as raw custom elements (vite.config.js isCustomElement),
// so we drive the component by dispatching the CustomEvents the real NDD
// components emit (detail.checked / detail.value) and assert the typed value
// the component emits via `update`.
function mountInput(props) {
  return mount(ScenarioParameterInput, { attachTo: document.body, props });
}

function lastUpdate(wrapper) {
  const events = wrapper.emitted('update');
  return events[events.length - 1][0];
}

describe('ScenarioParameterInput', () => {
  describe('control selection per datatype', () => {
    const cases = [
      ['boolean', 'nldd-switch-field'],
      ['number', 'nldd-number-field'],
      ['amount', 'nldd-number-field'],
      ['date', 'nldd-text-field'],
      ['string', 'nldd-text-field'],
    ];
    for (const [type, tag] of cases) {
      it(`renders ${tag} for type=${type}`, () => {
        const wrapper = mountInput({ type, value: '' });
        expect(wrapper.find(tag).exists()).toBe(true);
      });
    }

    it('falls back to text-field for an unknown type', () => {
      const wrapper = mountInput({ type: 'mystery', value: '' });
      expect(wrapper.find('nldd-text-field').exists()).toBe(true);
    });
  });

  describe('boolean', () => {
    it('renders a string "true" value as checked', () => {
      const wrapper = mountInput({ type: 'boolean', value: 'true' });
      expect(wrapper.find('nldd-switch-field').attributes('checked')).toBeDefined();
    });

    it('renders a falsy value as unchecked', () => {
      const wrapper = mountInput({ type: 'boolean', value: 'false' });
      expect(wrapper.find('nldd-switch-field').attributes('checked')).toBeUndefined();
    });

    it('emits a real boolean on toggle', () => {
      const wrapper = mountInput({ type: 'boolean', value: 'false' });
      wrapper.find('nldd-switch-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { checked: true } }),
      );
      expect(lastUpdate(wrapper)).toBe(true);
    });
  });

  describe('number', () => {
    it('emits a real number', () => {
      const wrapper = mountInput({ type: 'number', value: '' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: '42' } }),
      );
      expect(lastUpdate(wrapper)).toBe(42);
    });

    it('emits empty string for a cleared field', () => {
      const wrapper = mountInput({ type: 'number', value: '7' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '' } }),
      );
      expect(lastUpdate(wrapper)).toBe('');
    });
  });

  describe('amount', () => {
    it('displays eurocent values in euros', () => {
      const wrapper = mountInput({ type: 'amount', unit: 'eurocent', value: 109171 });
      expect(wrapper.find('nldd-number-field').attributes('value')).toBe('1091.71');
    });

    it('emits eurocents when the unit is eurocent', () => {
      const wrapper = mountInput({ type: 'amount', unit: 'eurocent', value: '' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '1500' } }),
      );
      expect(lastUpdate(wrapper)).toBe(150000);
    });

    it('emits the raw number when no unit is declared', () => {
      const wrapper = mountInput({ type: 'amount', unit: null, value: '' });
      wrapper.find('nldd-number-field').element.dispatchEvent(
        new CustomEvent('change', { detail: { value: '1500' } }),
      );
      expect(lastUpdate(wrapper)).toBe(1500);
    });
  });

  describe('date / string', () => {
    it('emits the date string', () => {
      const wrapper = mountInput({ type: 'date', value: '' });
      wrapper.find('nldd-text-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: '2025-01-01' } }),
      );
      expect(lastUpdate(wrapper)).toBe('2025-01-01');
    });

    it('emits the string value', () => {
      const wrapper = mountInput({ type: 'string', value: '' });
      wrapper.find('nldd-text-field').element.dispatchEvent(
        new CustomEvent('input', { detail: { value: 'hello' } }),
      );
      expect(lastUpdate(wrapper)).toBe('hello');
    });
  });
});
