import { mount } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';

const fields = [
  { name: 'verdragsinschrijving', type: 'boolean', unit: null },
  { name: 'spaargeld', type: 'amount', unit: 'eurocent' },
  { name: 'geboortedatum', type: 'date', unit: null },
  { name: 'land_verblijf', type: 'string', unit: null },
];

const mountTable = (rows) =>
  mount(DataSourceTable, {
    props: { title: 'box', keyField: 'bsn', fields, modelValue: rows, defaultExpanded: true, drilledIn: true },
  });

const spiFor = (w, name) =>
  w.findAllComponents(ScenarioParameterInput).find((c) => c.props('name') === name);

describe('DataSourceTable typed cells', () => {
  it('renders the control matching each column datatype', () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'false', spaargeld: '79547', geboortedatum: '2005-01-01', land_verblijf: 'NEDERLAND' },
    ]);
    // boolean -> kept tri-state dropdown (true/false/null)
    expect(w.find('select').exists()).toBe(true);
    expect(w.findAll('option').some((o) => o.attributes('value') === 'null')).toBe(true);
    // amount/date/string -> reused ScenarioParameterInput with correct type/unit
    expect(spiFor(w, 'spaargeld').props('type')).toBe('amount');
    expect(spiFor(w, 'spaargeld').props('unit')).toBe('eurocent');
    expect(spiFor(w, 'geboortedatum').props('type')).toBe('date');
    expect(spiFor(w, 'land_verblijf').props('type')).toBe('string');
    // boolean column is NOT routed through ScenarioParameterInput
    expect(spiFor(w, 'verdragsinschrijving')).toBeUndefined();
  });

  // RFC-036: a blank cell ("no value stated") and the word `null` (an
  // absence the author stated) are different cells and must stay apart.
  it('shows a null cell as the word null, in a text control whatever the column type', () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'null', spaargeld: 'null', geboortedatum: null, land_verblijf: 'null' },
    ]);
    expect(spiFor(w, 'spaargeld').props('value')).toBe('null');
    expect(spiFor(w, 'spaargeld').props('type')).toBe('string');
    expect(spiFor(w, 'geboortedatum').props('value')).toBe('null'); // a JS null from a typed record
    expect(spiFor(w, 'geboortedatum').props('type')).toBe('string');
    expect(spiFor(w, 'land_verblijf').props('value')).toBe('null');
    expect(w.find('select').element.value).toBe('null');
  });

  it('shows a blank or absent cell as empty, in the control of its column type', () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: '', spaargeld: '', land_verblijf: 'NL' },
    ]);
    expect(spiFor(w, 'spaargeld').props('value')).toBe('');
    expect(spiFor(w, 'spaargeld').props('type')).toBe('amount');
    expect(spiFor(w, 'geboortedatum').props('value')).toBe(''); // key absent from the row
    expect(spiFor(w, 'geboortedatum').props('type')).toBe('date');
    expect(w.find('select').element.value).toBe('');
    expect(w.findAll('option').map((o) => o.attributes('value'))).toEqual(['true', 'false', 'null', '']);
  });

  it('clears a field to a blank cell, not to null', async () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'true', spaargeld: '79547', geboortedatum: '2005-01-01', land_verblijf: 'NL' },
    ]);
    spiFor(w, 'spaargeld').vm.$emit('update', '');
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].spaargeld).toBe('');
    spiFor(w, 'land_verblijf').vm.$emit('update', null);
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].land_verblijf).toBe('');
  });

  it('lets the author state an absence by typing null or picking it', async () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'true', spaargeld: '79547', geboortedatum: '2005-01-01', land_verblijf: 'NL' },
    ]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].land_verblijf).toBe('null');
    const select = w.find('select');
    select.element.value = 'null';
    await select.trigger('change');
    expect(w.emitted('update:modelValue').at(-1)[0][0].verdragsinschrijving).toBe('null');
    select.element.value = '';
    await select.trigger('change');
    expect(w.emitted('update:modelValue').at(-1)[0][0].verdragsinschrijving).toBe('');
  });
});

// RFC-036 / schema v0.5.8: `null` is a value of a field only where the law
// declares it `nullable: true`. The column carries that declaration; a
// column without one (collection elements, a column the law never names)
// makes no claim and behaves as before.
describe('DataSourceTable nullability', () => {
  const nullableFields = [
    { name: 'verdragsinschrijving', type: 'boolean', unit: null, nullable: true },
    { name: 'land_verblijf', type: 'string', unit: null, nullable: true },
  ];
  const strictFields = [
    { name: 'verdragsinschrijving', type: 'boolean', unit: null, nullable: false },
    { name: 'land_verblijf', type: 'string', unit: null, nullable: false },
  ];
  const mountWith = (fields, rows) =>
    mount(DataSourceTable, {
      props: { title: 'box', keyField: 'bsn', fields, modelValue: rows, defaultExpanded: true, drilledIn: true },
    });
  const optionValues = (w) => w.findAll('option').map((o) => o.attributes('value'));
  const errorTexts = (w) => w.findAll('nldd-form-field-error-text').map((e) => e.text());

  it('offers the null option in a boolean column only when the field is nullable', () => {
    const nullable = mountWith(nullableFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    expect(optionValues(nullable)).toEqual(['true', 'false', 'null', '']);
    const strict = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    expect(optionValues(strict)).toEqual(['true', 'false', '']); // (leeg) stays
    expect(errorTexts(strict)).toEqual([]);
  });

  it('accepts a typed null into a nullable text column', async () => {
    const w = mountWith(nullableFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].land_verblijf).toBe('null');
    expect(spiFor(w, 'land_verblijf').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
  });

  it('refuses a typed null in a non-nullable text column: blank in the record, field invalid with the message', async () => {
    const w = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'true', land_verblijf: 'NL' }]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].land_verblijf).toBe('');
    await w.setProps({ modelValue: w.emitted('update:modelValue').at(-1)[0] });
    const field = spiFor(w, 'land_verblijf');
    expect(field.props('invalid')).toBe(true);
    expect(field.props('errorMessageIds')).toBeTruthy();
    expect(errorTexts(w)).toEqual(['Dit gegeven kan niet afwezig zijn (niet nullable)']);
    expect(w.find('nldd-form-field-error-text').attributes('id')).toBe(field.props('errorMessageIds'));

    // Typing on clears the refusal.
    field.vm.$emit('update', 'nul');
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].land_verblijf).toBe('nul');
    await w.setProps({ modelValue: w.emitted('update:modelValue').at(-1)[0] });
    expect(spiFor(w, 'land_verblijf').props('invalid')).toBe(false);
    expect(errorTexts(w)).toEqual([]);
  });

  it('still accepts null in a column without a known declaration', async () => {
    const w = mountWith([{ name: 'land_verblijf', type: 'string', unit: null }], [{ _id: 1, bsn: '1', land_verblijf: 'NL' }]);
    spiFor(w, 'land_verblijf').vm.$emit('update', 'null');
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].land_verblijf).toBe('null');
    expect(errorTexts(w)).toEqual([]);
  });

  it('marks a null already in a non-nullable column as invalid but keeps the value', () => {
    // A null read from the feature file, or a column re-typed as non-nullable
    // once the law's declaration arrived: shown, flagged, not altered.
    const w = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'null', land_verblijf: 'null' }]);
    const field = spiFor(w, 'land_verblijf');
    expect(field.props('value')).toBe('null');
    expect(field.props('invalid')).toBe(true);
    // The boolean dropdown keeps the null option while the cell holds one, so
    // the state is visible, and marks the dropdown invalid.
    expect(optionValues(w)).toEqual(['true', 'false', 'null', '']);
    expect(w.find('select').element.value).toBe('null');
    expect(w.find('nldd-dropdown').attributes('invalid')).toBeDefined();
    expect(errorTexts(w)).toHaveLength(2);
    expect(w.emitted('update:modelValue')).toBeUndefined();
  });

  it('drops the invalid mark once the boolean cell is set to a value', async () => {
    const w = mountWith(strictFields, [{ _id: 1, bsn: '1', verdragsinschrijving: 'null', land_verblijf: 'NL' }]);
    const select = w.find('select');
    select.element.value = 'false';
    await select.trigger('change');
    await w.setProps({ modelValue: w.emitted('update:modelValue').at(-1)[0] });
    expect(optionValues(w)).toEqual(['true', 'false', '']);
    expect(w.find('nldd-dropdown').attributes('invalid')).toBeUndefined();
    expect(errorTexts(w)).toEqual([]);
  });
});
