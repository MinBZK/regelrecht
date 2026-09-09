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
