import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ArticleGroup from './ArticleGroup.vue';

// Stub the per-segment renderers so we test ArticleGroup's stacking/provenance
// logic without pulling in their (network-touching) internals.
const stubs = {
  ArticleText: { props: ['article'], template: '<div class="stub-text">{{ article.number }}</div>' },
  MachineReadable: { props: ['article'], template: '<div class="stub-mr">{{ article.number }}</div>' },
  YamlView: { props: ['article'], template: '<div class="stub-yaml">{{ article.number }}</div>' },
};

const seg = (number, mr = false) => ({
  number,
  text: `tekst ${number}`,
  ...(mr ? { machine_readable: { execution: {} } } : {}),
});

function mountGroup(segments, view) {
  return mount(ArticleGroup, { props: { segments, view }, global: { stubs } });
}

describe('ArticleGroup — tekst view', () => {
  it('renders one ArticleText per segment, in order, without provenance labels', () => {
    const w = mountGroup([seg('1'), seg('1.a'), seg('1.e.1°')], 'tekst');
    const texts = w.findAll('.stub-text');
    expect(texts.map((t) => t.text())).toEqual(['1', '1.a', '1.e.1°']);
    expect(w.findAll('nldd-title')).toHaveLength(0);
  });
});

describe('ArticleGroup — machine view', () => {
  it('renders only segments that carry machine_readable', () => {
    const w = mountGroup([seg('1'), seg('1.a', true), seg('1.e', true)], 'machine');
    expect(w.findAll('.stub-mr').map((m) => m.text())).toEqual(['1.a', '1.e']);
  });

  it('shows a provenance label per MR segment when the article is split', () => {
    const w = mountGroup([seg('1'), seg('1.a', true), seg('1.e', true)], 'machine');
    const labels = w.findAll('nldd-title').map((t) => t.text());
    expect(labels).toEqual(['Artikel 1.a', 'Artikel 1.e']);
  });

  it('omits the provenance label for a single-segment article', () => {
    const w = mountGroup([seg('3.1', true)], 'machine');
    expect(w.findAll('nldd-title')).toHaveLength(0);
    expect(w.findAll('.stub-mr')).toHaveLength(1);
  });

  it('shows an empty-state when no segment has machine_readable', () => {
    const w = mountGroup([seg('1'), seg('1.a')], 'machine');
    expect(w.find('nldd-inline-dialog').exists()).toBe(true);
    expect(w.findAll('.stub-mr')).toHaveLength(0);
  });
});

describe('ArticleGroup — yaml view', () => {
  it('renders YamlView per MR segment with provenance when split', () => {
    const w = mountGroup([seg('2.4.1', true), seg('2.4.1.a', true)], 'yaml');
    expect(w.findAll('.stub-yaml').map((y) => y.text())).toEqual(['2.4.1', '2.4.1.a']);
    expect(w.findAll('nldd-title').map((t) => t.text())).toEqual([
      'Artikel 2.4.1',
      'Artikel 2.4.1.a',
    ]);
  });
});
