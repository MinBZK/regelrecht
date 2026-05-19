/**
 * Composables for the per-traject docs view.
 *
 * Two backend endpoints, both will be served by editor-api once the
 * traject_members / traject_corpus_sources tables exist (PR #632):
 *   GET /api/trajects/:trajectId/docs/tree
 *     -> { sources: [{ source_id, name, tree: [{ path, type }] }] }
 *   GET /api/trajects/:trajectId/docs/page?source=:src&path=:p
 *     -> raw markdown text/plain
 *
 * While the upstream PRs are still in review the backend returns canned
 * data — see packages/editor-api/src/traject_docs.rs. The composables here
 * stay shape-stable across the stub→real transition.
 */
import { ref, shallowRef, watch } from 'vue';

function trajectsRoot(trajectId) {
  return `/api/trajects/${encodeURIComponent(trajectId)}/docs`;
}

/**
 * Reactive ref<{ sources: Array }> for the given traject's docs tree.
 * Returns { tree, loading, error, reload }.
 */
export function useDocsTree(trajectIdRef) {
  const tree = shallowRef({ sources: [] });
  const loading = ref(false);
  const error = ref(null);

  async function load() {
    const id = unwrap(trajectIdRef);
    if (!id) {
      tree.value = { sources: [] };
      return;
    }
    loading.value = true;
    error.value = null;
    try {
      const resp = await fetch(`${trajectsRoot(id)}/tree`, {
        credentials: 'include',
      });
      if (!resp.ok) {
        // 403 surfaces as "no access to this traject" upstream; we don't
        // try to redirect — the parent view decides UX.
        throw new Error(`tree fetch failed: ${resp.status}`);
      }
      tree.value = await resp.json();
    } catch (err) {
      error.value = err;
      tree.value = { sources: [] };
    } finally {
      loading.value = false;
    }
  }

  watch(() => unwrap(trajectIdRef), load, { immediate: true });

  return { tree, loading, error, reload: load };
}

/**
 * Reactive ref<string> with the markdown source for a single page.
 * Watches all three refs; refetches on any change.
 */
export function useDocsPage(trajectIdRef, sourceIdRef, pathRef) {
  const text = ref('');
  const loading = ref(false);
  const error = ref(null);

  async function load() {
    const tid = unwrap(trajectIdRef);
    const sid = unwrap(sourceIdRef);
    const path = unwrap(pathRef);
    if (!tid || !sid || !path) {
      text.value = '';
      return;
    }
    loading.value = true;
    error.value = null;
    try {
      const url =
        `${trajectsRoot(tid)}/page` +
        `?source=${encodeURIComponent(sid)}&path=${encodeURIComponent(path)}`;
      const resp = await fetch(url, { credentials: 'include' });
      if (!resp.ok) throw new Error(`page fetch failed: ${resp.status}`);
      text.value = await resp.text();
    } catch (err) {
      error.value = err;
      text.value = '';
    } finally {
      loading.value = false;
    }
  }

  watch(
    () => [unwrap(trajectIdRef), unwrap(sourceIdRef), unwrap(pathRef)],
    load,
    { immediate: true },
  );

  return { text, loading, error, reload: load };
}

function unwrap(maybeRef) {
  return maybeRef && typeof maybeRef === 'object' && 'value' in maybeRef
    ? maybeRef.value
    : maybeRef;
}
