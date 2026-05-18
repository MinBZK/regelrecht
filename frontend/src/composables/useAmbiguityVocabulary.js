/**
 * useAmbiguityVocabulary — the controlled tag list for questioning notes.
 *
 * RFC-018 Decision 9: a questioning note over an open norm carries a tagging
 * body whose value is one of these ids. The list is deliberately *not* a JSON
 * Schema enum (it is still emerging from interpretation research); `just
 * validate-annotations` only warns on an unknown value. The same file is
 * copied to /data/annotations/_vocabulary/ by copy-laws.js so the editor's
 * picker and the CI check read one source and cannot drift.
 */
import { ref } from 'vue';
import yaml from 'js-yaml';

// Session cache: the file does not change while the editor is open.
let cached = null;

const items = ref([]);
const loaded = ref(false);

async function load() {
  if (cached) {
    items.value = cached;
    loaded.value = true;
    return cached;
  }
  try {
    const res = await fetch('/data/annotations/_vocabulary/ambiguity.yaml');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const doc = yaml.load(await res.text());
    const list = Array.isArray(doc?.ambiguity) ? doc.ambiguity : [];
    cached = list;
    items.value = list;
  } catch (e) {
    // A missing vocabulary must not block note creation: the tag field just
    // falls back to free text. Surfaced via the empty list.
    console.warn('Kon ambiguïteit-vocabulary niet laden:', e.message);
    cached = [];
    items.value = [];
  } finally {
    loaded.value = true;
  }
  return cached;
}

export function useAmbiguityVocabulary() {
  if (!loaded.value) load();
  return { items, loaded, reload: load };
}
