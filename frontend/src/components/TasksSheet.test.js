import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest';
import { mount } from '@vue/test-utils';

// Route useTasks.js's network leg through a controllable spy - same pattern
// as useTasks.test.js.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a) => apiFetch(...a) }));

// The component navigates on "Beoordelen" via vue-router; stub it so the
// sheet mounts without a real router (route-building itself is verified
// against the real router in ../lib/taskReview.test.js).
const pushMock = vi.fn();
vi.mock('vue-router', () => ({ useRouter: () => ({ push: pushMock }) }));

// nldd-sheet compiles to a raw custom element under happy-dom; stub
// show()/hide() so the watch(isOpen) handler doesn't throw.
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() {}
      hide() {}
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

describe('TasksSheet', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
    pushMock.mockReset();
  });

  // useTasks/useTasksSheet are module singletons; re-import both dynamically
  // after resetModules() so each test starts from a clean slate (mirrors
  // useTasks.test.js).
  async function mountSheet(tasks, running = []) {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks, open_count: tasks.length, running }),
    });
    const { default: TasksSheet } = await import('./TasksSheet.vue');
    const { useTasksSheet } = await import('../composables/useTasksSheet.js');
    useTasksSheet().open();
    const wrapper = mount(TasksSheet, {
      attachTo: document.body,
      global: { stubs: { teleport: true } },
    });
    // Flush the sheet's open-watcher tick and useTasks' deferred initial load.
    await wrapper.vm.$nextTick();
    await Promise.resolve();
    await wrapper.vm.$nextTick();
    return wrapper;
  }

  it('toont een lege staat zonder open taken', async () => {
    const wrapper = await mountSheet([]);
    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toBe('Geen open taken.');
  });

  it('toont de Bezig-sectie met een activity-indicator per lopende job en NIET de lege staat', async () => {
    const wrapper = await mountSheet(
      [],
      [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }]
    );
    const indicators = wrapper.findAll('nldd-activity-indicator');
    expect(indicators).toHaveLength(1);
    expect(indicators[0].attributes('text')).toBe('Verrijking loopt - test_wet');
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  it('toont zowel de Bezig-sectie als de takenlijst wanneer beide gevuld zijn', async () => {
    const wrapper = await mountSheet(
      [{ id: 't1', task_type: 'job_review', title: 'Verrijking beoordelen: andere_wet', payload: {} }],
      [{ job_id: 'j1', law_id: 'test_wet', status: 'processing' }]
    );
    expect(wrapper.findAll('nldd-activity-indicator')).toHaveLength(1);
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(2); // 1 running-rij + 1 taak-rij
    expect(wrapper.find('nldd-inline-dialog').exists()).toBe(false);
  });

  it('toont een job_failed-taak als alert-rij met titel + error als secundaire tekst en een Gezien-knop', async () => {
    const wrapper = await mountSheet([
      {
        id: 't1',
        task_type: 'job_failed',
        title: 'Verrijking mislukt: test_wet',
        payload: { error: 'boom', law_id: 'test_wet', traject_ref: 'traject-abcd1234' },
      },
    ]);
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('alert');
    expect(wrapper.get('nldd-icon-cell').attributes('color')).toBe('critical');
    const cell = wrapper.get('nldd-text-cell');
    expect(cell.attributes('color')).toBe('critical');
    expect(cell.attributes('text')).toBe('Verrijking mislukt: test_wet');
    expect(cell.attributes('supporting-text')).toBe('boom');

    apiFetch.mockResolvedValue({ status: 200, json: async () => ({ tasks: [], open_count: 0 }) });
    await wrapper.get('nldd-button').trigger('click');
    expect(apiFetch).toHaveBeenCalledWith(
      '/api/tasks/t1/resolve',
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('navigeert naar de editor-route met ?task= voor een job_review-taak en sluit de sheet', async () => {
    const wrapper = await mountSheet([
      {
        id: 't2',
        task_type: 'job_review',
        title: 'Verrijking beoordelen: test_wet',
        payload: { law_id: 'test_wet', traject_ref: 'traject-abcd1234' },
      },
    ]);
    const { useTasksSheet } = await import('../composables/useTasksSheet.js');
    const { isOpen } = useTasksSheet();
    expect(isOpen.value).toBe(true);

    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('tasks');
    const cell = wrapper.get('nldd-text-cell');
    expect(cell.attributes('color')).toBeUndefined();
    await wrapper.get('nldd-button').trigger('click');

    expect(pushMock).toHaveBeenCalledWith({
      name: 'editor-traject',
      params: { trajectRef: 'traject-abcd1234', lawId: 'test_wet' },
      query: { task: 't2' },
    });
    expect(isOpen.value).toBe(false);
  });

  it('navigeert naar de werkdocumenten-route met ?task= voor een document-review-taak, met het documents-icoon', async () => {
    const wrapper = await mountSheet([
      {
        id: 't5',
        task_type: 'job_review',
        title: 'Documentconversie beoordelen: bijv-rapport.md',
        payload: { kind: 'document', traject_ref: 'traject-abcd1234', target_path: 'bijv-rapport.md' },
      },
    ]);
    expect(wrapper.get('nldd-icon-cell').attributes('icon')).toBe('documents');
    await wrapper.get('nldd-button').trigger('click');
    expect(pushMock).toHaveBeenCalledWith({
      name: 'werkdocumenten-traject',
      params: { trajectRef: 'traject-abcd1234', docPath: 'bijv-rapport.md' },
      query: { task: 't5' },
    });
  });

  it('toont een disabled Beoordelen-knop voor een taak zonder traject_ref/law_id', async () => {
    const wrapper = await mountSheet([
      { id: 't3', task_type: 'job_review', title: 'Verrijking beoordelen: ???', payload: {} },
    ]);
    const btn = wrapper.get('nldd-button');
    expect(btn.attributes('disabled')).toBeDefined();
    await btn.trigger('click');
    expect(pushMock).not.toHaveBeenCalled();
  });
});
