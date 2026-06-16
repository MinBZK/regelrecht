import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import DocumentList from './DocumentList.vue';

// DS elements (nldd-list, nldd-list-item, nldd-text-cell, …) compile to raw
// custom elements under happy-dom; we assert against attributes/markup rather
// than rendered shadow DOM.
const DOCS = [{ path: 'beleid.md' }, { path: 'nota/bijlage.txt' }];

function mountList(props = {}) {
  return mount(DocumentList, {
    props: { documents: DOCS, ...props },
  });
}

describe('DocumentList', () => {
  it('renders one row per document plus the "Nieuw document" row', () => {
    const wrapper = mountList();
    const items = wrapper.findAll('nldd-list-item');
    expect(items).toHaveLength(DOCS.length + 1);
  });

  it('hides the .md extension but keeps .txt visible', () => {
    const wrapper = mountList();
    const texts = wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
    expect(texts).toContain('beleid');
    expect(texts).toContain('nota/bijlage.txt');
  });

  it('emits select with the document path when a row is clicked', async () => {
    const wrapper = mountList();
    await wrapper.findAll('nldd-list-item')[0].trigger('click');
    expect(wrapper.emitted('select')).toBeTruthy();
    expect(wrapper.emitted('select')[0]).toEqual(['beleid.md']);
  });

  it('emits new when the last row is clicked', async () => {
    const wrapper = mountList();
    const items = wrapper.findAll('nldd-list-item');
    await items[items.length - 1].trigger('click');
    expect(wrapper.emitted('new')).toBeTruthy();
  });

  it('marks the selected document row', () => {
    const wrapper = mountList({ selectedPath: 'beleid.md' });
    const selected = wrapper.findAll('nldd-list-item').filter((i) => i.attributes('selected') !== undefined);
    expect(selected).toHaveLength(1);
  });

  it('shows a chevron trailing icon by default (in-place select)', () => {
    const wrapper = mountList();
    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('name'));
    expect(icons).toContain('chevron-right');
    expect(icons).not.toContain('open-new-page');
  });

  it('renders rows as new-tab links with the open-new-page icon when hrefFor is set', () => {
    const wrapper = mountList({ hrefFor: (p) => `/werkdocumenten/t-abc12345/${p}` });
    const icons = wrapper.findAll('nldd-icon').map((i) => i.attributes('name'));
    expect(icons).toContain('open-new-page');
    expect(icons).not.toContain('chevron-right');

    const first = wrapper.findAll('nldd-list-item')[0];
    expect(first.attributes('href')).toBe('/werkdocumenten/t-abc12345/beleid.md');
    expect(first.attributes('target')).toBe('_blank');
    expect(first.attributes('rel')).toBe('noopener');
  });

  it('does not emit select for link rows (native navigation handles it)', async () => {
    const wrapper = mountList({ hrefFor: (p) => `/x/${p}` });
    await wrapper.findAll('nldd-list-item')[0].trigger('click');
    expect(wrapper.emitted('select')).toBeFalsy();
  });
});
