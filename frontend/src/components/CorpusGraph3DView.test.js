import { describe, it, expect } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import CorpusGraph3DView from './CorpusGraph3DView.vue';

/**
 * The test environment has no WebGL, which is exactly the case a user on a
 * machine with hardware acceleration disabled hits. The view must then still
 * render its chrome and say what went wrong instead of throwing during mount
 * and taking the surrounding page with it.
 */
describe('CorpusGraph3DView', () => {
  it('mounts without a WebGL context and reports the failure', async () => {
    const wrapper = mount(CorpusGraph3DView, { props: { nodes: 50, edges: 100 } });
    // The build is async now (a payload may be fetched), so the failure lands
    // a microtask later than the mount.
    await flushPromises();
    expect(wrapper.find('canvas').exists()).toBe(true);
    expect(wrapper.find('nldd-banner').exists()).toBe(true);
    wrapper.unmount();
  });

  it('renders its controls as design-system components', () => {
    const wrapper = mount(CorpusGraph3DView, { props: { nodes: 50, edges: 100 } });
    expect(wrapper.find('nldd-segmented-control').exists()).toBe(true);
    expect(wrapper.find('nldd-switch-field').exists()).toBe(true);
    expect(wrapper.find('nldd-button').exists()).toBe(true);
    // Everything on top of the canvas is an ndd-* element; the only custom
    // markup is the positioning wrapper.
    const custom = wrapper
      .findAll('.graph3d-hud > *')
      .map((el) => el.element.tagName.toLowerCase());
    expect(custom.every((tag) => tag.startsWith('nldd-'))).toBe(true);
    wrapper.unmount();
  });

  it('gives the canvas a keyboard focus stop and an accessible name', () => {
    const wrapper = mount(CorpusGraph3DView, { props: { nodes: 20, edges: 20 } });
    const canvas = wrapper.find('canvas');
    expect(canvas.attributes('tabindex')).toBe('0');
    expect(canvas.attributes('aria-label')).toContain('lijstweergave');
    wrapper.unmount();
  });
});
