import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import DocumentList from './DocumentList.vue';

// DS elements (nldd-list, nldd-list-item, nldd-text-cell, …) compile to raw
// custom elements under happy-dom; we assert against attributes/markup rather
// than rendered shadow DOM.
const DOCS = [{ path: 'beleid.md' }, { path: 'nota/bijlage.txt' }];
const TITLED_DOCS = [{ path: 'mijn-brief.md', title: 'Mijn Brief' }, { path: 'beleid.md' }];

function mountList(props = {}) {
  return mount(DocumentList, {
    props: { documents: DOCS, ...props },
  });
}

describe('DocumentList', () => {
  it('renders one row per document (no new-document row)', () => {
    const wrapper = mountList();
    expect(wrapper.findAll('nldd-list-item')).toHaveLength(DOCS.length);
  });

  it('de-slugs paths without a frontmatter title (extension hidden, folder kept)', () => {
    const wrapper = mountList();
    const texts = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(texts).toContain('Beleid');
    expect(texts).toContain('nota/Bijlage');
  });

  it('prefers the frontmatter title over the de-slugged path', () => {
    const wrapper = mountList({ documents: TITLED_DOCS });
    const texts = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(texts).toContain('Mijn Brief');
    expect(texts).toContain('Beleid');
  });

  it('emits select with the document path when a row is clicked', async () => {
    const wrapper = mountList();
    await wrapper.findAll('nldd-list-item')[0].trigger('click');
    expect(wrapper.emitted('select')).toBeTruthy();
    expect(wrapper.emitted('select')[0]).toEqual(['beleid.md']);
  });

  it('marks the selected document row', () => {
    const wrapper = mountList({ selectedPath: 'beleid.md' });
    const selected = wrapper
      .findAll('nldd-list-item')
      .filter((i) => i.attributes('selected') !== undefined);
    expect(selected).toHaveLength(1);
  });

  it('shows a chevron trailing icon (in-place select)', () => {
    const wrapper = mountList();
    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('name'));
    expect(icons).toContain('chevron-right');
  });
});
