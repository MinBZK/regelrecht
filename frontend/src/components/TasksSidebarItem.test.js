import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';

// Same mocking pattern as TasksPane.test.js - route useTasks.js's network
// leg through a controllable spy.
const apiFetch = vi.fn();
vi.mock('@regelrecht/frontend-shared', () => ({ apiFetch: (...a) => apiFetch(...a) }));

describe('TasksSidebarItem', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
  });

  async function mountItem(tasks, running = [], props = {}) {
    apiFetch.mockResolvedValue({
      status: 200,
      json: async () => ({ tasks, open_count: tasks.length, running }),
    });
    const { default: TasksSidebarItem } = await import('./TasksSidebarItem.vue');
    const wrapper = mount(TasksSidebarItem, { props });
    // Flush useTasks' deferred initial load (see comment in useTasks.js) -
    // same flush sequence as TasksPane.test.js's mountPane().
    await wrapper.vm.$nextTick();
    await Promise.resolve();
    await wrapper.vm.$nextTick();
    return wrapper;
  }

  it('toont geen badge zonder open taken en zonder lopende jobs', async () => {
    const wrapper = await mountItem([], []);
    expect(wrapper.find('nldd-badge').exists()).toBe(false);
  });

  it('toont de aantal-badge wanneer er open taken zijn, ook als er iets loopt', async () => {
    const wrapper = await mountItem(
      [{ id: 't1', task_type: 'job_review', title: 'x' }],
      [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }]
    );
    const badge = wrapper.get('nldd-badge');
    expect(badge.attributes('number')).toBe('1');
    expect(badge.attributes('color')).toBeUndefined();
  });

  it('toont een stille neutrale dot-badge zonder open taken maar met een lopende job', async () => {
    const wrapper = await mountItem([], [{ job_id: 'j1', law_id: 'test_wet', status: 'pending' }]);
    const badge = wrapper.get('nldd-badge');
    expect(badge.attributes('color')).toBe('neutral');
    expect(badge.attributes('number')).toBeUndefined();
  });

  it('markeert het item als selected op de taken-route', async () => {
    const wrapper = await mountItem([], [], { selected: true });
    expect(wrapper.get('nldd-list-item').attributes('selected')).toBeDefined();
  });
});
