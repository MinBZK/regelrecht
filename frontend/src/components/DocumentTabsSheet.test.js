import { describe, it, expect, beforeAll, beforeEach, afterEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import DocumentTabsSheet from './DocumentTabsSheet.vue';
import {
  setEditorChrome,
  registerTabActions,
  clearEditorChrome,
} from '../composables/useAppChrome.js';

// nldd-sheet compiles to a raw custom element (vite.config.js isCustomElement),
// so the template ref has no show()/hide(). Register a no-op stub that forwards
// to module-level spies (reassigned per test). Same approach as
// TrajectInfoDialog.test.js / EditSheet.test.js.
let showSpy, hideSpy;
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() { showSpy?.(); }
      hide() { hideSpy?.(); }
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

const TABS = [
  { lawId: 'zorgtoeslagwet', articleNumber: '3' },
  { lawId: 'awir', articleNumber: '1' },
];
const NAMES = { zorgtoeslagwet: 'Zorgtoeslagwet', awir: 'Algemene wet inkomensafhankelijke regelingen' };

const tabKey = (t) => `${t.lawId}:${t.articleNumber}`;

let selectSpy, closeSpy, reorderSpy;

beforeEach(() => {
  showSpy = vi.fn();
  hideSpy = vi.fn();
  selectSpy = vi.fn();
  closeSpy = vi.fn();
  reorderSpy = vi.fn();
  registerTabActions({
    key: tabKey,
    displayName: (t) => NAMES[t.lawId] || t.lawId,
    select: selectSpy,
    close: closeSpy,
    reorder: reorderSpy,
  });
  setEditorChrome({ pr: null, tabs: TABS, activeTab: TABS[0] });
});

afterEach(() => {
  clearEditorChrome();
});

function mountSheet() {
  return mount(DocumentTabsSheet, {
    attachTo: document.body,
    // Render the Teleported sheet inline so its content is reachable.
    global: { stubs: { teleport: true } },
  });
}

describe('DocumentTabsSheet', () => {
  it('shows the active tab (article + law) on the trigger button', () => {
    const wrapper = mountSheet();
    const btn = wrapper.get('nldd-button');
    expect(btn.attributes('text')).toBe('Artikel 3');
    expect(btn.attributes('supporting-text')).toBe('Zorgtoeslagwet');
  });

  it('lists every open tab, marking the active one selected', () => {
    const wrapper = mountSheet();
    const items = wrapper.findAll('nldd-list-item');
    expect(items).toHaveLength(2);

    const cells = wrapper.findAll('nldd-text-cell');
    expect(cells[0].attributes('text')).toBe('Artikel 3');
    expect(cells[0].attributes('supporting-text')).toBe('Zorgtoeslagwet');
    expect(cells[1].attributes('text')).toBe('Artikel 1');
    expect(cells[1].attributes('supporting-text')).toBe(NAMES.awir);

    // Active tab (TABS[0]) is the selected list-item; the other is not.
    expect(items[0].attributes('selected')).toBeDefined();
    expect(items[1].attributes('selected')).toBeUndefined();
  });

  it('selects a tab and closes the sheet on row click', async () => {
    const wrapper = mountSheet();
    await wrapper.findAll('nldd-list-item')[1].trigger('click');
    expect(selectSpy).toHaveBeenCalledWith(TABS[1]);
    expect(hideSpy).toHaveBeenCalled();
  });

  it('closes a tab via the dismiss button without selecting it (@click.stop)', async () => {
    const wrapper = mountSheet();
    await wrapper.findAll('nldd-icon-button')[1].trigger('click');
    expect(closeSpy).toHaveBeenCalledWith(TABS[1]);
    // .stop keeps the row's select handler from firing for the same click.
    expect(selectSpy).not.toHaveBeenCalled();
  });

  it('opens the sheet when the trigger button is clicked', async () => {
    const wrapper = mountSheet();
    await wrapper.get('nldd-button').trigger('click');
    expect(showSpy).toHaveBeenCalled();
  });

  it('forwards nldd-reorder indices to the editor reorder action', () => {
    const wrapper = mountSheet();
    wrapper.get('nldd-list').element.dispatchEvent(
      new CustomEvent('nldd-reorder', { detail: { fromIndex: 0, toIndex: 1 }, bubbles: true }),
    );
    expect(reorderSpy).toHaveBeenCalledWith(0, 1);
  });

  it('renders a reorder drag handle per tab', () => {
    const wrapper = mountSheet();
    expect(wrapper.findAll('nldd-drag-handle-cell')).toHaveLength(TABS.length);
    expect(wrapper.get('nldd-list').attributes('reorderable')).toBeDefined();
  });

  it('closes the sheet when the viewport grows past sm', () => {
    let changeHandler = null;
    const spy = vi.spyOn(window, 'matchMedia').mockReturnValue({
      matches: false,
      addEventListener: (event, handler) => { if (event === 'change') changeHandler = handler; },
      removeEventListener: () => {},
    });
    try {
      mountSheet();
      // Crossing to md+ (min-width:641px now matches) hides the sheet.
      changeHandler?.({ matches: true });
      expect(hideSpy).toHaveBeenCalled();
    } finally {
      spy.mockRestore();
    }
  });
});
