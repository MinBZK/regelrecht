import { mount } from '@vue/test-utils';
import { describe, it, expect } from 'vitest';
import ScenarioForm from './ScenarioForm.vue';
import DataSourceTable from './DataSourceTable.vue';

const baseSetup = () => ({
  calculationDate: '2025-01-01',
  parameters: [],
  dataSources: [
    { sourceName: 'insurance', keyField: 'bsn', headers: ['bsn', 'verdragsinschrijving', 'onbekend'], rows: [['1', 'false', 'x']] },
  ],
});

const mountForm = (externalFieldTypeMap) =>
  mount(ScenarioForm, {
    props: { scenario: { assertions: [] }, setup: baseSetup(), lawId: 'l', externalFieldTypeMap },
  });

// The editable DataSourceTable only renders once a source is drilled into.
const drillIn = (w) => w.find('[data-testid="ds-row-0"]').trigger('click');
const fieldsOf = (w) => w.findComponent(DataSourceTable).props('fields');

describe('ScenarioForm data-source column typing', () => {
  it('types data-source columns from the map (fallback string, key excluded)', async () => {
    const w = mountForm(new Map([['verdragsinschrijving', { type: 'boolean', unit: null }]]));
    await drillIn(w);
    const f = fieldsOf(w);
    expect(f.find((c) => c.name === 'verdragsinschrijving').type).toBe('boolean');
    expect(f.find((c) => c.name === 'onbekend').type).toBe('string'); // not in map -> fallback
    expect(f.some((c) => c.name === 'bsn')).toBe(false); // key field excluded
  });

  it('re-types columns in place when the map arrives asynchronously', async () => {
    const w = mountForm(new Map()); // empty at mount -> all string
    await drillIn(w);
    expect(fieldsOf(w).find((c) => c.name === 'verdragsinschrijving').type).toBe('string');
    await w.setProps({ externalFieldTypeMap: new Map([['verdragsinschrijving', { type: 'boolean', unit: null }]]) });
    expect(fieldsOf(w).find((c) => c.name === 'verdragsinschrijving').type).toBe('boolean');
  });
});
