import { describe, it, expect, beforeAll, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import TrajectInfoDialog from './TrajectInfoDialog.vue';

// nldd-* tags compile to raw HTML (vite.config.js isCustomElement), so the
// `nldd-sheet` template ref is a real HTMLElement with no show()/hide().
// Register a no-op stub so the modelValue watcher doesn't throw on mount.
// (Same approach as EditSheet.test.js.)
beforeAll(() => {
  if (typeof customElements !== 'undefined' && !customElements.get('nldd-sheet')) {
    class NddSheetTestStub extends HTMLElement {
      show() {}
      hide() {}
    }
    customElements.define('nldd-sheet', NddSheetTestStub);
  }
});

const OWN_SOURCE = {
  source_id: 'own',
  source_type: 'github',
  gh_owner: 'MinBZK',
  gh_repo: 'regelrecht-corpus',
  gh_branch: 'traject/tariefswijziging-2026',
  gh_base_branch: 'development',
  gh_path: 'regulation/nl',
  is_writable_own: true,
};

const DETAIL = {
  id: 'abc',
  name: 'Tariefswijziging 2026',
  description: 'Waarom dit traject',
  scope: 'zorgtoeslag',
  status: 'bezig',
  role: 'owner',
  members: [],
  pending_invites: [],
  sources: [OWN_SOURCE],
};

function res(json, ok = true, status = 200) {
  return { ok, status, async json() { return json; }, async text() { return ''; } };
}

beforeEach(() => {
  vi.restoreAllMocks();
});

function mountDialog() {
  return mount(TrajectInfoDialog, {
    attachTo: document.body,
    // Render <Teleport> content inline so the assertions can reach it via
    // wrapper.get()/wrapper.findAll(); the component teleports to <body> in
    // production to escape the toolbar's clipping (like TrajectMembersDialog).
    global: { stubs: { teleport: true } },
    props: { modelValue: false, trajectId: 'abc', trajectName: 'Tariefswijziging 2026' },
  });
}

// Values render as `text` attributes on nldd-text-cell (custom elements
// don't expand their shadow templates in this env), so assertions read
// the attribute values instead of wrapper.text().
function cellTexts(wrapper) {
  return wrapper.findAll('nldd-text-cell').map((c) => c.attributes('text'));
}

describe('TrajectInfoDialog', () => {
  it('loads detail when opened and renders the traject fields', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();

    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    // useTrajectDetail now goes through apiFetch, which calls fetch(url, init).
    expect(globalThis.fetch).toHaveBeenCalledWith('/api/trajects/abc', expect.anything());
    const texts = cellTexts(wrapper);
    expect(texts).toContain('Tariefswijziging 2026');
    expect(texts).toContain('Waarom dit traject');
    expect(texts).toContain('zorgtoeslag');
    expect(texts).toContain('bezig');
    expect(texts).toContain('owner');
  });

  it('renders the repo as a new-tab nldd-link to the traject branch', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    // nldd-link compiles to a raw custom element in the test env (vite
    // isCustomElement), so assert on its attributes — the underlying <a>,
    // slot text, and auto-rel only exist once the real Lit component
    // upgrades in the browser. We bind href/target/text/rel explicitly so
    // they are present as attributes here.
    const link = wrapper.get('nldd-link');
    expect(link.attributes('href')).toBe(
      'https://github.com/MinBZK/regelrecht-corpus/tree/traject/tariefswijziging-2026',
    );
    expect(link.attributes('target')).toBe('_blank');
    expect(link.attributes('rel')).toContain('noopener');
    expect(link.attributes('text')).toBe('MinBZK/regelrecht-corpus');
  });

  it('shows the branch, base branch and subpath', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    const texts = cellTexts(wrapper);
    expect(texts).toContain('traject/tariefswijziging-2026'); // branch
    expect(texts).toContain('development'); // base branch
    expect(texts).toContain('regulation/nl'); // subpath
  });

  it('falls back to "repo-root" when the subpath is empty', async () => {
    const detail = { ...DETAIL, sources: [{ ...OWN_SOURCE, gh_path: '' }] };
    globalThis.fetch = vi.fn().mockResolvedValue(res(detail));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    expect(cellTexts(wrapper)).toContain('repo-root');
  });

  it('shows an error message when the load fails', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(null, false, 404));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    expect(wrapper.get('nldd-inline-dialog').attributes('text')).toMatch(/niet laden|404/i);
  });

  it('emits update:modelValue=false when dismissed', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue(res(DETAIL));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    wrapper.vm.close();
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
  });

  it('shows "onbekend" for repo/branch when there is no writable source', async () => {
    const detail = { ...DETAIL, sources: [] };
    globalThis.fetch = vi.fn().mockResolvedValue(res(detail));
    const wrapper = mountDialog();
    await wrapper.setProps({ modelValue: true });
    await flushPromises();

    // No writable source → no repo link, and the value cells read "onbekend".
    // Subpath must NOT fall back to "repo-root" here (that implies a real
    // source whose path is empty); it should read "onbekend" like the others.
    expect(wrapper.find('nldd-link').exists()).toBe(false);
    expect(cellTexts(wrapper)).toContain('onbekend');
    expect(cellTexts(wrapper)).not.toContain('repo-root');
  });
});
