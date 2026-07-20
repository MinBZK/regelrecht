/**
 * createLruMap — a Map with least-recently-used eviction.
 *
 * Consolidates the copy-pasted "Map insertion order IS the LRU order"
 * caches (law cache in `useLaw`, per-scope laws list in `useCorpusLaws`,
 * loaded-scope tracking in `useEngine`): `set` and `get` move the key to
 * the most-recent position (delete + set), and `set` evicts the oldest
 * entries past `maxSize`.
 *
 * `onEvict(key, value)` lets a cache keep companion state in sync when
 * an entry falls out (drop a paired promise, unload a law from the WASM
 * engine, …). It only fires for capacity evictions — not for explicit
 * `delete`/`clear`, where the caller is already managing the entry.
 *
 * @template K, V
 * @param {number} maxSize
 * @param {{ onEvict?: (key: K, value: V) => void }} [options]
 */
export function createLruMap(maxSize, { onEvict } = {}) {
  /** @type {Map<K, V>} */
  const map = new Map();

  function evictOverflow() {
    while (map.size > maxSize) {
      const oldestKey = map.keys().next().value;
      const oldestValue = map.get(oldestKey);
      map.delete(oldestKey);
      onEvict?.(oldestKey, oldestValue);
    }
  }

  return {
    has(key) {
      return map.has(key);
    },
    /** Read + touch: marks the key most-recently-used. */
    get(key) {
      if (!map.has(key)) return undefined;
      const value = map.get(key);
      map.delete(key);
      map.set(key, value);
      return value;
    },
    /** Read without touching the LRU order. */
    peek(key) {
      return map.get(key);
    },
    /** Insert/overwrite at the most-recent position, then evict overflow. */
    set(key, value) {
      map.delete(key);
      map.set(key, value);
      evictOverflow();
      return this;
    },
    delete(key) {
      return map.delete(key);
    },
    clear() {
      map.clear();
    },
    keys() {
      return map.keys();
    },
    get size() {
      return map.size;
    },
  };
}
