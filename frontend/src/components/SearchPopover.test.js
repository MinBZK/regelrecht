import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import SearchPopover from './SearchPopover.vue';

// `nldd-*` tags are registered as custom elements in vite.config.js, so they
// render as raw HTMLElements here. We drive the component via `wrapper.vm`
// and assert on the `groupedLaws` computed rather than the shadow DOM.
//
// useAuth()/useBwb* fire a `fetch` on setup; stub it so the shared
// /auth/status probe resolves quietly instead of hitting the network.
beforeEach(() => {
  vi.stubGlobal(
    'fetch',
    vi.fn(() => Promise.resolve({ ok: false, json: () => Promise.resolve({}) })),
  );
});

function law(law_id, source_id, source_name, source_priority) {
  return { law_id, source_id, source_name, source_priority };
}

// Two sources: the traject's own writable repo (priority 0) and the seeded
// central corpus (priority 5). The central laws are listed first in the prop
// to prove ordering comes from priority, not input order.
const LAWS = [
  law('kieswet', 'central', 'MinBZK Central', 5),
  law('besluit_zorgverzekering_bes', 'traject-own-abc', 'MinBZK/regelrecht-corpus-BES', 0),
  law('wet_inkomstenbelasting_bes', 'traject-own-abc', 'MinBZK/regelrecht-corpus-BES', 0),
];

function mountPopover() {
  return mount(SearchPopover, { props: { laws: LAWS } });
}

describe('SearchPopover groupedLaws', () => {
  it('groups matches by source, private repo (priority 0) before central', async () => {
    const wrapper = mountPopover();
    wrapper.vm.search = 'bes';
    await nextTick();

    const groups = wrapper.vm.groupedLaws;
    expect(groups.map((g) => g.source_name)).toEqual([
      'MinBZK/regelrecht-corpus-BES',
      // central has no 'bes' match, so it drops out entirely
    ]);
    expect(groups[0].laws.map((l) => l.law_id)).toEqual([
      'besluit_zorgverzekering_bes',
      'wet_inkomstenbelasting_bes',
    ]);
  });

  it('orders groups by source priority ascending when both match', async () => {
    const wrapper = mountPopover();
    wrapper.vm.search = 'wet';
    await nextTick();

    const groups = wrapper.vm.groupedLaws;
    expect(groups.map((g) => g.source_name)).toEqual([
      'MinBZK/regelrecht-corpus-BES', // priority 0
      'MinBZK Central', // priority 5
    ]);
  });

  it('returns no groups when nothing matches (external fallback territory)', async () => {
    const wrapper = mountPopover();
    wrapper.vm.search = 'zzzznomatch';
    await nextTick();

    expect(wrapper.vm.groupedLaws).toEqual([]);
  });
});
