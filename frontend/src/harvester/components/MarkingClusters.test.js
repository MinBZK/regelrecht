import { mount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import MarkingClusters from './MarkingClusters.vue';

function cluster(overrides = {}) {
  return {
    resolution: 'operation',
    resolved_by: 'een WORKING_DAY-bewerking',
    markings: 3,
    laws: 2,
    articles: 3,
    providers: ['opencode', 'claude'],
    all_accepted: false,
    ...overrides,
  };
}

// Web components are not defined in jsdom; mounting is enough to read the
// rendered attributes off them.
const global = {
  config: { compilerOptions: { isCustomElement: (tag) => tag.startsWith('nldd-') } },
};

describe('MarkingClusters', () => {
  it('renders nothing while loading or without clusters', () => {
    const empty = mount(MarkingClusters, { props: { clusters: [] }, global });
    expect(empty.text()).toBe('');

    const loading = mount(MarkingClusters, {
      props: { clusters: [cluster()], loading: true },
      global,
    });
    expect(loading.text()).toBe('');
  });

  it('states the reach of a cluster in articles and laws', () => {
    const w = mount(MarkingClusters, { props: { clusters: [cluster()] }, global });
    expect(w.html()).toContain('3 artikelen in 2 wetten');
  });

  it('uses the singular where it applies', () => {
    const w = mount(MarkingClusters, {
      props: { clusters: [cluster({ articles: 1, laws: 1 })] },
      global,
    });
    expect(w.html()).toContain('1 artikel in 1 wet');
  });

  // The triage signal. A change only one provider ever asks for, while another
  // ran over the same corpus without complaint, points at the enricher rather
  // than at the format, and the two are indistinguishable from the marking.
  it('flags a cluster only one provider asked for', () => {
    const w = mount(MarkingClusters, {
      props: { clusters: [cluster({ providers: ['opencode'] })] },
      global,
    });
    expect(w.html()).toContain('alleen opencode');
    expect(w.html()).toContain('mogelijk de enricher');
  });

  it('does not flag a cluster several providers asked for', () => {
    const w = mount(MarkingClusters, { props: { clusters: [cluster()] }, global });
    expect(w.html()).not.toContain('mogelijk de enricher');
  });

  // Exact grouping on free text undercounts, and a reader has to be told
  // rather than left to infer it from a long tail of ones.
  it('warns about clusters of one, which may be the same need worded twice', () => {
    const w = mount(MarkingClusters, {
      props: {
        clusters: [
          cluster({ markings: 1, resolved_by: 'Een WORKING_DAY-bewerking' }),
          cluster({ markings: 1, resolved_by: 'Een bewerking voor de volgende werkdag' }),
        ],
      },
      global,
    });
    expect(w.html()).toContain('2 clusters tellen');
  });

  it('stays quiet when every cluster holds more than one marking', () => {
    const w = mount(MarkingClusters, { props: { clusters: [cluster()] }, global });
    expect(w.html()).not.toContain('clusters tellen');
    expect(w.html()).not.toContain('cluster telt');
  });

  // Whether a human has been past every marking in the cluster. Without this
  // a reviewed cluster and an untouched one render identically, and the
  // backlog is exactly where that difference decides what to pick up.
  it('says whether a cluster has been reviewed', () => {
    const open = mount(MarkingClusters, {
      props: { clusters: [cluster({ all_accepted: false })] },
      global,
    });
    expect(open.html()).toContain('nog te beoordelen');

    const done = mount(MarkingClusters, {
      props: { clusters: [cluster({ all_accepted: true })] },
      global,
    });
    expect(done.html()).toContain('beoordeeld');
    expect(done.html()).not.toContain('nog te beoordelen');
  });

  it('emits the cluster when one is picked', async () => {
    const c = cluster();
    const w = mount(MarkingClusters, { props: { clusters: [c] }, global });
    await w.find('nldd-list-item').trigger('click');
    expect(w.emitted('select')[0]).toEqual([c]);
  });

  it('says so when a cluster names no change', () => {
    const w = mount(MarkingClusters, {
      props: { clusters: [cluster({ resolved_by: null })] },
      global,
    });
    expect(w.html()).toContain('niet benoemd');
  });
});
