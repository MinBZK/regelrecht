import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import PersonalNotes from './PersonalNotes.vue';

// PersonalNotes fetches /api/user/notes/{lawId} through apiFetch, which
// calls the global fetch — stub it per test, same pattern as NoteCreator.

function jsonResponse(data, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => 'application/json' },
    json: async () => data,
    text: async () => JSON.stringify(data),
  };
}

function note(id, value, extra = {}) {
  return {
    id,
    type: 'Annotation',
    motivation: 'commenting',
    target: { source: 'regelrecht://test_wet' },
    body: { type: 'TextualBody', value, purpose: 'commenting', format: 'text/markdown' },
    created: '2026-07-06T10:00:00Z',
    modified: '2026-07-06T10:00:00Z',
    ...extra,
  };
}

// nldd-* tags are compiled as custom elements (vite isCustomElement), so
// vue-test-utils stubs don't apply to them — they render as plain unknown
// elements in happy-dom. Interactions go through attributes and DOM events;
// only the built-in Teleport is stubbed so the modal stays in the wrapper.
const stubs = { teleport: true };

async function flush() {
  // A macrotask drains the whole microtask chain of the fetch->json
  // awaits inside the composable, then let Vue re-render.
  await new Promise((r) => setTimeout(r, 0));
  await nextTick();
}

function mountNotes() {
  return mount(PersonalNotes, {
    props: { lawId: 'test_wet' },
    global: { stubs },
  });
}

beforeEach(() => {
  globalThis.fetch = vi.fn();
});

describe('PersonalNotes', () => {
  it('renders the note body as markdown', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse([note('n1', '**belangrijk**: zie art. 2')]));
    const wrapper = mountNotes();
    await flush();

    const body = wrapper.find('[data-testid="personal-note-body"]');
    expect(body.exists()).toBe(true);
    // marked renders the emphasis. (Sanitization is renderArticleHtml's
    // job — DOMPurify is a passthrough under happy-dom, so it cannot be
    // asserted here; the shared pipeline is the tested contract.)
    expect(body.element.innerHTML).toContain('<strong>belangrijk</strong>');
  });

  it('hides itself entirely when personal notes are unavailable (401)', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse({ error: 'unauthorized' }, 401));
    const wrapper = mountNotes();
    await flush();

    expect(wrapper.find('[data-testid="personal-notes"]').exists()).toBe(false);
  });

  it('adds a note via the composer', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([]));
    const wrapper = mountNotes();
    await flush();

    const field = wrapper.find('[data-testid="personal-note-draft-field"]');
    field.element.value = 'nieuwe *notitie*';
    await field.trigger('input');

    globalThis.fetch.mockResolvedValueOnce(jsonResponse(note('n1', 'nieuwe *notitie*'), 201));
    await wrapper.find('[data-testid="personal-note-add"]').trigger('click');
    await flush();

    const [url, init] = globalThis.fetch.mock.calls.at(-1);
    expect(url).toBe('/api/user/notes/test_wet');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body)).toEqual({ value: 'nieuwe *notitie*' });
    expect(wrapper.find('[data-testid="personal-note-body"]').exists()).toBe(true);
    // Composer is cleared after a successful save. Once the test has set
    // el.value, Vue patches the property (not the attribute) on custom
    // elements — assert the property from here on.
    expect(wrapper.find('[data-testid="personal-note-draft-field"]').element.value).toBe('');
  });

  it('edits a note in place', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([note('n1', 'oud')]));
    const wrapper = mountNotes();
    await flush();

    await wrapper.find('[data-testid="personal-note-edit"]').trigger('click');
    const field = wrapper.find('[data-testid="personal-note-edit-field"]');
    // Custom element: the bound value lands as an attribute.
    expect(field.attributes('value')).toBe('oud');
    field.element.value = 'nieuw';
    await field.trigger('input');

    globalThis.fetch.mockResolvedValueOnce(jsonResponse(note('n1', 'nieuw')));
    await wrapper.find('[data-testid="personal-note-save-edit"]').trigger('click');
    await flush();

    const [url, init] = globalThis.fetch.mock.calls.at(-1);
    expect(url).toBe('/api/user/notes/test_wet/n1');
    expect(init.method).toBe('PUT');
    expect(wrapper.find('[data-testid="personal-note-body"]').text()).toContain('nieuw');
  });

  it('deletes a note after confirmation', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([note('n1', 'weg ermee')]));
    const wrapper = mountNotes();
    await flush();

    await wrapper.find('[data-testid="personal-note-delete"]').trigger('click');

    globalThis.fetch.mockResolvedValueOnce({ ok: true, status: 204, headers: { get: () => '' } });
    await wrapper.find('[data-testid="personal-note-confirm-delete"]').trigger('click');
    await flush();

    const [url, init] = globalThis.fetch.mock.calls.at(-1);
    expect(url).toBe('/api/user/notes/test_wet/n1');
    expect(init.method).toBe('DELETE');
    expect(wrapper.find('[data-testid="personal-note-body"]').exists()).toBe(false);
  });

  it('shows an action error and keeps the draft when saving fails', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([]));
    const wrapper = mountNotes();
    await flush();

    const field = wrapper.find('[data-testid="personal-note-draft-field"]');
    field.element.value = 'blijft staan';
    await field.trigger('input');

    globalThis.fetch.mockResolvedValueOnce(jsonResponse({ error: 'boom' }, 500));
    await wrapper.find('[data-testid="personal-note-add"]').trigger('click');
    await flush();

    expect(wrapper.find('[data-testid="personal-notes-error"]').exists()).toBe(true);
    // Same property-vs-attribute note as the add test: the draft ref is
    // unchanged, so the value property still holds the typed text.
    expect(wrapper.find('[data-testid="personal-note-draft-field"]').element.value).toBe(
      'blijft staan',
    );
  });
});
