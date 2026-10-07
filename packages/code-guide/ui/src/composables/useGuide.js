import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { fetchCrate, fetchFunction, fetchSource, fetchStatus, fetchType, fetchWorkspace } from '../lib/api.js';
import { buildGraph, defaultScope } from '../lib/graph.js';
import { findModule, formatHash, isType, parseHash, search, sourceRange, widen } from '../lib/guide.js';

/** How often the index status is checked again while the page is open, in ms. */
const STATUS_INTERVAL = 30_000;

/**
 * All state of the code guide. The address is the source of truth for what is
 * open (crate, module, item, method) and how it is shown: every link is a plain
 * `href`, and this reads the hash back, so Back and Forward work and every view
 * is a link that can be shared.
 */
export function useGuide() {
  const route = ref(parseHash(globalThis.location?.hash));

  const status = ref(null);
  const workspace = ref(null);
  const crateView = ref(null);
  const typeView = ref(null);
  const fnView = ref(null);
  const loading = ref(false);
  // Kept apart so that opening a valid item clears an item's failure without
  // hiding a failure to load the crate, and the other way round.
  const loadError = ref(null);
  const itemError = ref(null);
  const error = computed(() => loadError.value ?? itemError.value);

  const query = ref('');
  const includePrivate = ref(false);
  const view = ref('modules');

  const source = ref(null);
  const sourceError = ref(null);
  const sourceLoading = ref(false);
  const range = ref(null);

  const module = computed(() => (route.value.module === null ? null : findModule(crateView.value, route.value.module)));
  const itemIsType = computed(() => !!route.value.item && isType(module.value, route.value.item));
  const method = computed(() =>
    route.value.method ? (typeView.value?.methods.find((m) => m.key === route.value.method) ?? null) : null,
  );
  /** Which page is open: workspace, crate, module, type or function. */
  const page = computed(() => {
    const r = route.value;
    if (!r.crate) return 'workspace';
    if (r.module === null) return 'crate';
    if (!r.item) return 'module';
    return itemIsType.value ? 'type' : 'function';
  });

  const results = computed(() => search(crateView.value?.search, query.value));

  /**
   * The address of `sel`: the current crate and how things are shown are kept
   * unless `sel` says otherwise, so following a link does not undo the reader's
   * choice of view.
   */
  const hrefFor = (sel = {}) =>
    formatHash({
      crate: route.value.crate,
      module: null,
      item: null,
      method: null,
      view: route.value.view,
      scope: route.value.scope,
      wide: route.value.wide,
      ...sel,
    });
  /** The address of what is open now, with `overrides` applied. */
  const hrefHere = (overrides = {}) => formatHash({ ...route.value, ...overrides });

  const scope = computed(() => route.value.scope ?? defaultScope(route.value, page.value === 'type'));
  const graph = computed(() =>
    buildGraph({
      scope: scope.value,
      workspace: workspace.value,
      crateView: crateView.value,
      typeView: typeView.value,
      route: route.value,
      // A node keeps the graph open, but at the node's own level.
      hrefFor: (target) =>
        formatHash({
          item: null,
          method: null,
          ...target,
          view: 'graph',
          wide: route.value.wide,
          scope: null,
        }),
    }),
  );

  // Responses can arrive out of order; a token per kind of request makes a
  // late answer to an old click a no-op.
  const tokens = { crate: 0, item: 0, source: 0 };

  async function loadStatus() {
    try {
      status.value = await fetchStatus();
    } catch (e) {
      status.value = { index: { state: 'unknown' }, message: e.message };
    }
  }

  async function loadWorkspace() {
    try {
      workspace.value = await fetchWorkspace();
    } catch (e) {
      loadError.value = e.message;
    }
  }

  async function loadCrate(name) {
    const token = ++tokens.crate;
    crateView.value = null;
    if (!name) return;
    loading.value = true;
    loadError.value = null;
    try {
      const c = await fetchCrate(name);
      if (token === tokens.crate) crateView.value = c;
    } catch (e) {
      if (token === tokens.crate) loadError.value = e.message;
    } finally {
      if (token === tokens.crate) loading.value = false;
    }
  }

  async function loadItem() {
    const token = ++tokens.item;
    itemError.value = null;
    const r = route.value;
    const kind = page.value;
    if (kind !== 'type') typeView.value = null;
    if (kind !== 'function') fnView.value = null;
    if (kind !== 'type' && kind !== 'function') return;
    if (!crateView.value) return;
    const args = { crate: r.crate, module: r.module, name: r.item };
    // The open item is kept while its own method changes: no refetch.
    if (kind === 'type' && typeView.value?.name === r.item && typeView.value?.module === r.module && typeView.value?.crate === r.crate) {
      return;
    }
    try {
      const data = kind === 'type' ? await fetchType(args) : await fetchFunction(args);
      if (token !== tokens.item) return;
      if (kind === 'type') typeView.value = data;
      else fnView.value = data;
    } catch (e) {
      if (token !== tokens.item) return;
      // Never leave the previous item on screen under the failure.
      if (kind === 'type') typeView.value = null;
      else fnView.value = null;
      itemError.value = e.message;
    }
  }

  async function loadSource(r) {
    const token = ++tokens.source;
    if (!r) {
      source.value = null;
      sourceError.value = null;
      sourceLoading.value = false;
      return;
    }
    sourceLoading.value = true;
    sourceError.value = null;
    try {
      const s = await fetchSource(r);
      if (token === tokens.source) source.value = s;
    } catch (e) {
      if (token === tokens.source) {
        source.value = null;
        sourceError.value = e.message;
      }
    } finally {
      if (token === tokens.source) sourceLoading.value = false;
    }
  }

  /** Shows more lines around what the viewer shows now (what was returned). */
  function showMoreContext() {
    if (!range.value || !source.value) return;
    range.value = widen({ ...range.value, from: source.value.from, to: source.value.to }, source.value.total);
  }
  const canShowMoreAbove = computed(() => !!source.value && source.value.from > 1);
  const canShowMoreBelow = computed(() => !!source.value && source.value.to < source.value.total);

  const onHashChange = () => {
    route.value = parseHash(globalThis.location.hash);
  };
  let timer = null;
  onMounted(() => {
    globalThis.addEventListener('hashchange', onHashChange);
    loadStatus();
    loadWorkspace();
    timer = setInterval(loadStatus, STATUS_INTERVAL);
  });
  onBeforeUnmount(() => {
    globalThis.removeEventListener('hashchange', onHashChange);
    clearInterval(timer);
  });

  watch(
    () => route.value.crate,
    (name) => {
      query.value = '';
      loadCrate(name);
    },
    { immediate: true },
  );
  watch([crateView, () => route.value.module, () => route.value.item], () => loadItem());

  // A new selection resets the viewer to that selection's own range.
  watch(
    [module, typeView, fnView, method, page],
    () => {
      const p = page.value;
      range.value =
        p === 'type'
          ? sourceRange({ type: typeView.value, method: method.value })
          : p === 'function'
            ? sourceRange({ fn: fnView.value })
            : p === 'module'
              ? sourceRange({ module: module.value })
              : null;
    },
    { immediate: true },
  );
  watch(range, (r) => loadSource(r), { deep: true });

  return {
    route,
    page,
    status,
    workspace,
    crateView,
    typeView,
    fnView,
    module,
    method,
    loading,
    error,
    query,
    includePrivate,
    view,
    results,
    hrefFor,
    hrefHere,
    scope,
    graph,
    range,
    source,
    sourceError,
    sourceLoading,
    showMoreContext,
    canShowMoreAbove,
    canShowMoreBelow,
  };
}
