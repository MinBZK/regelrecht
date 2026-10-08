import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { fetchCalls, fetchCrate, fetchFunction, fetchSource, fetchStatus, fetchType, fetchWorkspace } from '../lib/api.js';
import {
  defaultDepth,
  focusOf,
  itemKind,
  layoutOptions,
  levelFor,
  levelsFor,
  lift,
  neighbourhood,
} from '../lib/callgraph.js';
import { findModule, formatHash, isType, parseHash, search, sourceRange, widen } from '../lib/guide.js';

/**
 * A computed value that is worked out again only when one of `inputs()` is
 * different (by identity) from last time; otherwise it returns the same object,
 * so nothing that depends on it is told it changed.
 */
function remember(inputs, compute) {
  let last = null;
  let value;
  return computed(() => {
    const now = inputs();
    if (last && now.length === last.length && now.every((x, i) => x === last[i])) return value;
    last = now;
    value = compute();
    return value;
  });
}

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
  // How the graph is drawn (direction, layout, spacing) follows the reader
  // everywhere; what it shows (level, depth) has a default per page, so it is
  // kept only while the same item stays open (another method of the same type).
  const hrefFor = (sel = {}) => {
    const r = route.value;
    const target = { crate: r.crate, module: null, item: null, ...sel };
    const samePage = target.crate === r.crate && target.module === r.module && target.item === r.item;
    return formatHash({
      ...r,
      ...(samePage ? {} : { level: null, depth: null }),
      method: null,
      ...target,
    });
  };
  /** The address of what is open now, with `overrides` applied. */
  const hrefHere = (overrides = {}) => formatHash({ ...route.value, ...overrides });

  // The call graph: every function and call, fetched once the graph is opened
  // and again when the server has rebuilt its model (its `generation` moved):
  // node numbers from an older build mean other functions.
  const calls = ref(null);
  const callsError = ref(null);
  let callsGeneration;
  let callsToken = 0;
  async function loadCalls() {
    // Without a status yet, wait for it rather than fetch twice.
    if (!status.value) return;
    const generation = status.value.generation ?? null;
    if (calls.value && generation === callsGeneration) return;
    const token = ++callsToken;
    try {
      const data = await fetchCalls();
      if (token !== callsToken) return;
      calls.value = data;
      callsGeneration = generation;
      callsError.value = null;
    } catch (e) {
      // A failed refetch keeps the graph that is there; the next status poll
      // tries again, because the generation still differs.
      if (token === callsToken && !calls.value) callsError.value = e.message;
    }
  }
  // On an item address, whether it is a type or a function is read from the
  // calls, so the graph does not wait for (or trust a stale) crate view.
  const graphPage = computed(() =>
    (page.value === 'type' || page.value === 'function') && calls.value
      ? itemKind(calls.value, route.value)
      : page.value,
  );
  const levels = computed(() => levelsFor(graphPage.value));
  const level = computed(() => levelFor(graphPage.value, route.value.level));
  // One selected method is centred on like one function: it reaches as far.
  const depth = computed(() =>
    route.value.depth === null
      ? defaultDepth(level.value, route.value.method ? 'function' : graphPage.value)
      : Number(route.value.depth),
  );
  const lifted = computed(() => (calls.value ? lift(calls.value, level.value) : null));
  // What is drawn depends on the item, the level, the direction and the depth,
  // not on the layout or spacing: those come from the same address, so without
  // this every layout change would make a new graph, and the graph pane would
  // drop its selection, highlights and added functions. A computed that returns
  // the same object does not notify what depends on it.
  const graphKey = computed(() => {
    const r = route.value;
    return [r.crate, r.module, r.item, r.method, graphPage.value, level.value, r.calls, depth.value].join('|');
  });
  const focus = remember(
    () => [lifted.value, graphKey.value],
    () => (lifted.value ? focusOf(lifted.value, route.value, graphPage.value) : new Set()),
  );
  const part = remember(
    () => [lifted.value, graphKey.value],
    () => (lifted.value ? neighbourhood(lifted.value, focus.value, { calls: route.value.calls, depth: depth.value }) : null),
  );
  const crateOrder = computed(() => [...new Set((calls.value?.nodes ?? []).map((n) => n.crate))].sort());
  // The functions, for opening a module or crate node up into them.
  const fnGraph = computed(() =>
    !calls.value ? null : level.value === 'functions' ? lifted.value : lift(calls.value, 'functions'),
  );
  const layout = computed(() => layoutOptions(route.value.layout, route.value.spacing));
  watch(
    [() => route.value.view, status],
    () => {
      if (route.value.view === 'graph') loadCalls();
    },
    { immediate: true },
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

  /** `keep`: leave the crate on screen while it loads again (a new model). */
  async function loadCrate(name, { keep = false } = {}) {
    const token = ++tokens.crate;
    if (!keep) crateView.value = null;
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

  let typeGeneration = null;
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
    // The open item is kept while its own method changes: no refetch, unless
    // the server's model was rebuilt since it was loaded.
    const generation = status.value?.generation ?? null;
    if (
      kind === 'type' &&
      typeGeneration === generation &&
      typeView.value?.name === r.item &&
      typeView.value?.module === r.module &&
      typeView.value?.crate === r.crate
    ) {
      return;
    }
    try {
      const data = kind === 'type' ? await fetchType(args) : await fetchFunction(args);
      if (token !== tokens.item) return;
      if (kind === 'type') {
        typeView.value = data;
        typeGeneration = generation;
      } else fnView.value = data;
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

  // A new model on the server (an index built or rebuilt while the page is
  // open): every view is loaded again, and an error from before, such as "no
  // index yet", goes. The first status only says what the page already loads.
  watch(
    () => status.value?.generation,
    (generation, before) => {
      if (generation === undefined || generation === null || before === undefined || generation === before) return;
      loadError.value = null;
      itemError.value = null;
      loadWorkspace();
      if (route.value.crate) loadCrate(route.value.crate, { keep: true });
      else loadItem();
    },
  );

  watch(
    () => route.value.crate,
    (name) => {
      query.value = '';
      loadCrate(name);
    },
    { immediate: true },
  );
  watch([crateView, () => route.value.module, () => route.value.item], () => loadItem());

  // What the source viewer shows for the page itself.
  const pageRange = () => {
    const p = page.value;
    return p === 'type'
      ? sourceRange({ type: typeView.value, method: method.value })
      : p === 'function'
        ? sourceRange({ fn: fnView.value })
        : p === 'module'
          ? sourceRange({ module: module.value })
          : null;
  };
  // A node selected in the graph shows its own source instead; `sourceTitle`
  // names it, since the page's title no longer does.
  const sourceTitle = ref(null);
  function showInSource(node) {
    sourceTitle.value = node?.source ? node.label : null;
    range.value = node?.source ? { ...node.source } : pageRange();
  }
  // A new page resets the viewer to that page's own range.
  watch(
    [module, typeView, fnView, method, page],
    () => {
      sourceTitle.value = null;
      range.value = pageRange();
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
    graphPage,
    graphKey,
    calls,
    callsError,
    levels,
    level,
    depth,
    lifted,
    focus,
    part,
    fnGraph,
    crateOrder,
    layout,
    range,
    source,
    sourceError,
    sourceLoading,
    showMoreContext,
    showInSource,
    sourceTitle,
    canShowMoreAbove,
    canShowMoreBelow,
  };
}
