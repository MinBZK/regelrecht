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

  it('displays a "null" cell as empty and clears to null', async () => {
    const w = mountTable([
      { _id: 1, bsn: '1', verdragsinschrijving: 'null', spaargeld: 'null', geboortedatum: null, land_verblijf: 'null' },
    ]);
    const spi = spiFor(w, 'spaargeld');
    expect(spi.props('value')).toBe(''); // stored "null" -> displayed empty
    spi.vm.$emit('update', ''); // user clears the field
    await w.vm.$nextTick();
    expect(w.emitted('update:modelValue').at(-1)[0][0].spaargeld).toBeNull();
  });
});
