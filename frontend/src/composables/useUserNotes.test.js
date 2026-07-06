import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref, nextTick } from 'vue';
import { useUserNotes } from './useUserNotes.js';

// The composable talks to /api/user/notes via apiFetch, which calls the
// global fetch at call time — stub that, same pattern as useDraftNotes.

function jsonResponse(data, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => 'application/json' },
    json: async () => data,
    text: async () => JSON.stringify(data),
  };
}

function note(id, value) {
  return {
    id,
    type: 'Annotation',
    motivation: 'commenting',
    target: { source: 'regelrecht://test_wet' },
    body: { type: 'TextualBody', value, purpose: 'commenting', format: 'text/markdown' },
    created: '2026-07-06T10:00:00Z',
    modified: '2026-07-06T10:00:00Z',
  };
}

async function flush() {
  // A macrotask drains the whole microtask chain of the fetch->json
  // awaits inside the composable, then let Vue re-render.
  await new Promise((r) => setTimeout(r, 0));
  await nextTick();
}

beforeEach(() => {
  globalThis.fetch = vi.fn();
});

describe('useUserNotes', () => {
  it('loads notes for the law on init', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse([note('a', 'eerste')]));
    const lawId = ref('test_wet');
    const { notes, loading, available } = useUserNotes(lawId);
    await flush();

    expect(globalThis.fetch).toHaveBeenCalledWith('/api/user/notes/test_wet', expect.anything());
    expect(notes.value).toHaveLength(1);
    expect(notes.value[0].body.value).toBe('eerste');
    expect(loading.value).toBe(false);
    expect(available.value).toBe(true);
  });

  it('reloads when the law changes', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse([]));
    const lawId = ref('wet_a');
    useUserNotes(lawId);
    await flush();

    lawId.value = 'wet_b';
    await flush();

    const urls = globalThis.fetch.mock.calls.map(([url]) => url);
    expect(urls).toContain('/api/user/notes/wet_a');
    expect(urls).toContain('/api/user/notes/wet_b');
  });

  it('flips available off on 401 instead of erroring', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse({ error: 'unauthorized' }, 401));
    const { notes, error, available } = useUserNotes(ref('test_wet'));
    await flush();

    expect(available.value).toBe(false);
    expect(error.value).toBeNull();
    expect(notes.value).toEqual([]);
  });

  it('surfaces other failures as error but stays available', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse({ error: 'boom' }, 500));
    const { error, available } = useUserNotes(ref('test_wet'));
    await flush();

    expect(available.value).toBe(true);
    expect(error.value).toBeTruthy();
  });

  it('addNote POSTs markdown and appends the created note', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([]));
    const { notes, addNote } = useUserNotes(ref('test_wet'));
    await flush();

    globalThis.fetch.mockResolvedValueOnce(jsonResponse(note('n1', '**vet**'), 201));
    await addNote('**vet**');

    const [url, init] = globalThis.fetch.mock.calls.at(-1);
    expect(url).toBe('/api/user/notes/test_wet');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body)).toEqual({ value: '**vet**' });
    expect(notes.value).toHaveLength(1);
    expect(notes.value[0].id).toBe('n1');
  });

  it('updateNote PUTs and replaces the note in place', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([note('n1', 'oud'), note('n2', 'ander')]));
    const { notes, updateNote } = useUserNotes(ref('test_wet'));
    await flush();

    globalThis.fetch.mockResolvedValueOnce(jsonResponse(note('n1', 'nieuw')));
    await updateNote('n1', 'nieuw');

    const [url, init] = globalThis.fetch.mock.calls.at(-1);
    expect(url).toBe('/api/user/notes/test_wet/n1');
    expect(init.method).toBe('PUT');
    expect(notes.value.map((n) => n.body.value)).toEqual(['nieuw', 'ander']);
  });

  it('removeNote DELETEs and drops the note', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([note('n1', 'weg'), note('n2', 'blijft')]));
    const { notes, removeNote } = useUserNotes(ref('test_wet'));
    await flush();

    globalThis.fetch.mockResolvedValueOnce({ ok: true, status: 204, headers: { get: () => '' } });
    await removeNote('n1');

    const [url, init] = globalThis.fetch.mock.calls.at(-1);
    expect(url).toBe('/api/user/notes/test_wet/n1');
    expect(init.method).toBe('DELETE');
    expect(notes.value.map((n) => n.id)).toEqual(['n2']);
  });

  it('a mutation resolving after a law switch does not leak into the new list', async () => {
    globalThis.fetch.mockResolvedValue(jsonResponse([]));
    const lawId = ref('wet_a');
    const { notes, addNote } = useUserNotes(lawId);
    await flush();

    // POST for wet_a resolves only after the user switched to wet_b.
    let resolvePost;
    globalThis.fetch.mockImplementationOnce(
      () => new Promise((r) => { resolvePost = r; }),
    );
    const pending = addNote('trage notitie');
    lawId.value = 'wet_b';
    globalThis.fetch.mockResolvedValue(jsonResponse([]));
    await flush();

    resolvePost(jsonResponse(note('n1', 'trage notitie'), 201));
    await pending;
    expect(notes.value).toEqual([]);
  });

  it('a failed mutation propagates and leaves the list untouched', async () => {
    globalThis.fetch.mockResolvedValueOnce(jsonResponse([note('n1', 'blijft')]));
    const { notes, addNote } = useUserNotes(ref('test_wet'));
    await flush();

    globalThis.fetch.mockResolvedValueOnce(jsonResponse({ error: 'vol' }, 400));
    await expect(addNote('x')).rejects.toThrow();
    expect(notes.value.map((n) => n.id)).toEqual(['n1']);
  });
});
