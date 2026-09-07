import { mount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import ScenarioForm from './ScenarioForm.vue';
import DataSourceTable from './DataSourceTable.vue';
import ScenarioParameterInput from './ScenarioParameterInput.vue';

// A collection-valued parameter (RFC-016) as formMapper hands it to the form:
// records plus the column list.
const setup = () => ({
  calculationDate: '2025-01-01',
  parameters: [
    { name: 'leeftijd', value: 35 },
    { name: 'medebewoners', value: [{ leeftijd: 25 }, { leeftijd: 19 }], columns: ['leeftijd'] },
  ],
  dataSources: [],
});

const fakeEngine = () => ({
  clearDataSources: vi.fn(),
  registerDataSource: vi.fn(),
  executeWithTrace: vi.fn(() => ({ outputs: { aantal: 2 }, trace_text: '' })),
});

const mountForm = (engine = null) =>
  mount(ScenarioForm, {
    props: {
      scenario: { assertions: [], execution: { outputName: 'aantal', lawId: 'participatiewet' } },
      setup: setup(),
      lawId: 'participatiewet',
      engine,
      ready: !!engine,
    },
  });

describe('ScenarioForm collection parameters', () => {
  it('shows a collection as a drill-in row, not as a scalar control', () => {
    const w = mountForm();
    const scalarNames = w.findAllComponents(ScenarioParameterInput).map((c) => c.props('name'));
    expect(scalarNames).toContain('leeftijd');
    expect(scalarNames).not.toContain('medebewoners');
    const row = w.find('[data-testid="coll-row-0"]');
    expect(row.exists()).toBe(true);
    expect(row.html()).toContain('medebewoners');
    expect(row.html()).toContain('text="2"');
  });

  it('drills into a keyless table of the elements and reports the drill', async () => {
    const w = mountForm();
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    const table = w.findComponent(DataSourceTable);
    expect(table.exists()).toBe(true);
    expect(table.props('keyField')).toBeNull();
    expect(table.props('fields').map((f) => f.name)).toEqual(['leeftijd']);
    expect(table.props('modelValue')).toEqual([{ _id: 0, leeftijd: 25 }, { _id: 1, leeftijd: 19 }]);
    expect(w.emitted('drill-change').at(-1)).toEqual(['medebewoners']);
  });

  it('passes the collection to the engine as an array of typed records', () => {
    const engine = fakeEngine();
    const w = mountForm(engine);
    w.vm.execute();
    const params = engine.executeWithTrace.mock.calls[0][2];
    expect(params.leeftijd).toBe(35);
    expect(params.medebewoners).toEqual([{ leeftijd: 25 }, { leeftijd: 19 }]);
  });

  it('passes an emptied collection as an empty array, not as a missing input', async () => {
    const engine = fakeEngine();
    const w = mountForm(engine);
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', []);
    await w.vm.$nextTick();
    w.vm.execute();
    const params = engine.executeWithTrace.mock.calls.at(-1)[2];
    expect(params.medebewoners).toEqual([]);
  });

  it('hands the edited collection back through getFormValues', async () => {
    const w = mountForm();
    await w.find('[data-testid="coll-row-0"]').trigger('click');
    w.findComponent(DataSourceTable).vm.$emit('update:modelValue', [{ _id: 0, leeftijd: '40' }]);
    await w.vm.$nextTick();
    const values = w.vm.getFormValues();
    expect(values.collections).toEqual([
      { name: 'medebewoners', columns: [{ name: 'leeftijd', type: 'string', unit: null }], rows: [{ _id: 0, leeftijd: '40' }] },
    ]);
    expect(values.parameterValues).toEqual({ leeftijd: 35 });
    expect(w.emitted('change')).toBeTruthy();
  });
});
