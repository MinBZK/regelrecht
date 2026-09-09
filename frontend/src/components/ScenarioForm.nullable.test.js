import { mount } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import ScenarioForm from './ScenarioForm.vue';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';
import { NOT_NULLABLE_MESSAGE } from '../utils/nullability.js';

// RFC-036 / schema v0.5.8: a scenario parameter takes the word `null` (a
// stated absence) only when the law declares the parameter `nullable: true`.
// The declaration reaches the form through the typeMap built from the law's
// articles (buildTypeMap); data-source columns get theirs through the
// externalFieldTypeMap and are checked in DataSourceTable.

const setupWith = (parameters) => ({
  calculationDate: '2025-01-01',
  parameters,
  dataSources: [
    { sourceName: 'insurance', keyField: 'bsn', headers: ['bsn', 'huur', 'spaargeld'], rows: [['1', '', '']] },
  ],
});

const typeMap = new Map([
  ['huur', { type: 'string', unit: null, nullable: true }],
  ['inkomen', { type: 'string', unit: null, nullable: false }],
]);

const mountForm = (parameters, externalFieldTypeMap = null) =>
  mount(ScenarioForm, {
    props: { scenario: { assertions: [] }, setup: setupWith(parameters), lawId: 'l', typeMap, externalFieldTypeMap },
  });

const inputFor = (w, name) =>
  w.findAllComponents(ScenarioParameterInput).find((c) => c.props('name') === name);
const errorTexts = (w) => w.findAll('nldd-form-field-error-text').map((e) => e.text());
const values = (w) => w.vm.getFormValues().parameterValues;

describe('ScenarioForm parameter nullability', () => {
  it('accepts null for a nullable parameter', async () => {
    const w = mountForm([{ name: 'huur', value: '500' }]);
    inputFor(w, 'huur').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(values(w).huur).toBe('null');
    expect(inputFor(w, 'huur').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
    expect(w.emitted('change')).toHaveLength(1);
  });

  it('refuses null for a non-nullable parameter: blank stored, field invalid with the message', async () => {
    const w = mountForm([{ name: 'inkomen', value: '500' }]);
    inputFor(w, 'inkomen').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(values(w).inkomen).toBe('');
    const field = inputFor(w, 'inkomen');
    expect(field.props('invalid')).toBe(true);
    expect(field.props('errorMessageIds')).toBeTruthy();
    expect(errorTexts(w)).toEqual([NOT_NULLABLE_MESSAGE]);
    expect(w.find('nldd-form-field-error-text').attributes('id')).toBe(field.props('errorMessageIds'));

    // Typing on clears the refusal.
    field.vm.$emit('update', '600');
    await w.vm.$nextTick();
    expect(values(w).inkomen).toBe('600');
    expect(inputFor(w, 'inkomen').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
  });

  it('accepts null for a parameter the law does not declare (no claim)', async () => {
    const w = mountForm([{ name: 'onbekend', value: '' }]);
    inputFor(w, 'onbekend').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(values(w).onbekend).toBe('null');
    expect(errorTexts(w)).toEqual([]);
  });

  it('marks a null read from the file for a non-nullable parameter, and keeps it', () => {
    const w = mountForm([{ name: 'inkomen', value: 'null' }]);
    expect(values(w).inkomen).toBe('null');
    expect(inputFor(w, 'inkomen').props('invalid')).toBe(true);
    expect(errorTexts(w)).toEqual([NOT_NULLABLE_MESSAGE]);
  });

  it('forgets a refusal when the edits are discarded', async () => {
    const w = mountForm([{ name: 'inkomen', value: '500' }]);
    inputFor(w, 'inkomen').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(errorTexts(w)).toHaveLength(1);
    w.vm.discardEdits();
    await w.vm.$nextTick();
    expect(values(w).inkomen).toBe('500');
    expect(errorTexts(w)).toEqual([]);
  });

  it('passes each data-source column its declared nullability, unknown when the law never names it', async () => {
    const w = mountForm([], new Map([
      ['huur', { type: 'amount', unit: 'eurocent', nullable: true }],
      ['spaargeld', { type: 'amount', unit: 'eurocent', nullable: false }],
    ]));
    await w.find('[data-testid="ds-row-0"]').trigger('click');
    const fields = w.findComponent(DataSourceTable).props('fields');
    expect(fields.find((c) => c.name === 'huur').nullable).toBe(true);
    expect(fields.find((c) => c.name === 'spaargeld').nullable).toBe(false);

    // Re-typing in place when the map arrives later carries nullability too.
    await w.setProps({ externalFieldTypeMap: new Map([['spaargeld', { type: 'amount', unit: 'eurocent', nullable: true }]]) });
    const retyped = w.findComponent(DataSourceTable).props('fields');
    expect(retyped.find((c) => c.name === 'spaargeld').nullable).toBe(true);
    expect(retyped.find((c) => c.name === 'huur').nullable).toBeUndefined();
  });
});
