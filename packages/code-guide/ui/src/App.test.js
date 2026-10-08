import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import App from './App.vue';

// Cytoscape needs a canvas, which happy-dom does not have. The canvas is
// replaced by a stub that shows what it was given and can raise its events;
// the canvas itself is tested in lib/callgraph.test.js (headless) and in the
// browser.
vi.mock('./components/LegendGraph.vue', async () => {
  const { defineComponent, h } = await import('vue');
  return {
    default: defineComponent({
      name: 'LegendGraph',
      props: ['boxes', 'boxNoun', 'focusName', 'description'],
      setup(props) {
        return () =>
          h('div', {
            class: 'legend-stub',
            'data-boxes': String(props.boxes),
            'data-focus': props.focusName,
            'aria-label': props.description,
          });
      },
    }),
  };
});
vi.mock('./components/CallGraph.vue', async () => {
  const { defineComponent, h } = await import('vue');
  return {
    default: defineComponent({
      name: 'CallGraph',
      props: ['elements', 'layout', 'highlight', 'hideRest', 'selected'],
      emits: ['select', 'clear', 'open', 'hover'],
      setup(props) {
        return () =>
          h('div', {
            class: 'call-graph-stub',
            'data-nodes': props.elements
              .filter((e) => e.group === 'nodes')
              .map((e) => e.data.id)
              .join(' '),
            'data-edges': props.elements
              .filter((e) => e.group === 'edges')
              .map((e) => e.data.id)
              .join(' '),
            'data-layout': `${props.layout.name}:${props.layout.rankDir ?? ''}`,
            'data-highlight': props.highlight ? [...props.highlight.nodes].sort().join(' ') : '',
          });
      },
    }),
  };
});

// The design system's elements are not defined under happy-dom, so they render
// as plain unknown elements with the attributes and text the app gives them:
// enough to test what the app decides (links, requests, what it shows when).

const place = (file, line) => ({ file, line });
const fnRef = (crate, module, type, key, sites = 1) => ({ crate, module, type, key, name: key.split('@')[0], sites });

const workspace = {
  crates: [
    { name: 'engine', description: 'The engine', layer: 1, cycleWith: [], modules: 2, types: 1, functions: 3 },
    { name: 'model', description: 'The model', layer: 0, cycleWith: [], modules: 1, types: 0, functions: 1 },
  ],
  edges: [{ from: 'engine', to: 'model', calls: 4 }],
  layers: [[['model']], [['engine']]],
};

const crateView = {
  name: 'engine',
  dir: 'engine',
  description: 'The engine',
  docs: 'The engine crate.',
  callsInto: [{ crate: 'model', module: 'law', calls: 4 }],
  calledFrom: [],
  layers: [[['types']], [['service']]],
  modules: [
    {
      path: 'types',
      doc: 'Types.',
      docs: 'Types.',
      vis: 'pub',
      file: 'engine/src/types.rs',
      layer: 0,
      cycleWith: [],
      callsInto: [],
      calledFrom: [{ crate: 'engine', module: 'service', calls: 2 }],
      types: [],
      functions: [{ key: 'parse', name: 'parse', vis: 'pub', doc: 'Parses.', callers: 1, stale: false }],
    },
    {
      path: 'service',
      doc: 'Service layer.',
      docs: 'Service layer.\n\nMore.',
      vis: 'pub',
      file: 'engine/src/service.rs',
      layer: 1,
      cycleWith: [],
      callsInto: [
        { crate: 'engine', module: 'types', calls: 2 },
        { crate: 'model', module: 'law', calls: 4 },
      ],
      calledFrom: [{ crate: 'app', module: 'main', calls: 3 }],
      types: [{ name: 'Service', kind: 'struct', vis: 'pub', doc: 'Runs laws.', methods: 2, callers: 1 }],
      functions: [{ key: 'helper', name: 'helper', vis: 'private', doc: null, callers: 0, stale: false }],
    },
  ],
  search: [
    { kind: 'struct', module: 'service', type: null, key: null, name: 'Service', doc: 'Runs laws.' },
    { kind: 'method', module: 'service', type: 'Service', key: 'run', name: 'Service::run', doc: 'Run.' },
    { kind: 'fn', module: 'types', type: null, key: 'parse', name: 'parse', doc: 'Parses.' },
  ],
};

const method = (key, extra = {}) => ({
  key,
  name: key,
  trait: null,
  vis: 'pub',
  signature: `pub fn ${key}(&self)`,
  doc: `${key} doc.`,
  docs: `${key} doc.`,
  place: place('engine/src/service.rs', 20),
  extent: [18, 30],
  stale: false,
  callers: [],
  callees: [],
  ...extra,
});

const typeView = {
  crate: 'engine',
  module: 'service',
  name: 'Service',
  kind: 'struct',
  vis: 'pub',
  signature: 'pub struct Service',
  doc: 'Runs laws.',
  docs: 'Runs laws.',
  place: place('engine/src/service.rs', 5),
  extent: [3, 10],
  methods: [
    method('run', {
      callers: [fnRef('app', 'main', null, 'main', 3)],
      callees: [fnRef('engine', 'service', 'Service', 'step'), fnRef('engine', 'types', null, 'parse')],
    }),
    method('step', { callers: [fnRef('engine', 'service', 'Service', 'run')], extent: [32, 40] }),
  ],
};

// The call graph: app::main -> Service::run -> Service::step -> parse, and
// Service::run -> parse; model::law::load is called by parse.
const callGraph = {
  nodes: [
    { crate: 'app', module: 'main', type: null, key: 'main', name: 'main', vis: 'pub', doc: null, stale: false },
    { crate: 'engine', module: 'service', type: 'Service', key: 'run', name: 'run', vis: 'pub', doc: 'Run.', stale: false },
    {
      crate: 'engine',
      module: 'service',
      type: 'Service',
      key: 'step',
      name: 'step',
      vis: 'private',
      doc: null,
      stale: false,
      place: place('engine/src/service.rs', 33),
      extent: [32, 40],
    },
    {
      crate: 'engine',
      module: 'types',
      type: null,
      key: 'parse',
      name: 'parse',
      vis: 'pub',
      doc: 'Parses.',
      stale: false,
      place: place('engine/src/types.rs', 3),
      extent: [2, 6],
    },
    { crate: 'model', module: 'law', type: null, key: 'load', name: 'load', vis: 'pub', doc: null, stale: false },
  ],
  edges: [
    [0, 1, 3],
    [1, 2, 1],
    [2, 3, 1],
    [1, 3, 1],
    [3, 4, 4],
  ],
};

const fnView = {
  crate: 'engine',
  module: 'types',
  ...method('parse', {
    place: place('engine/src/types.rs', 3),
    extent: [2, 6],
    callers: [fnRef('engine', 'service', 'Service', 'run')],
  }),
};

const json = (data) => ({ ok: true, status: 200, statusText: 'OK', json: async () => data, text: async () => '' });
const failure = (status, text) => ({ ok: false, status, statusText: 'Err', json: async () => ({}), text: async () => text });

let fetchMock;
const calls = () => fetchMock.mock.calls.map(([u]) => String(u));
const params = (url) => Object.fromEntries(new URL(url, 'http://x/').searchParams);
const sourceCalls = () => calls().filter((u) => u.startsWith('api/source'));

function install(overrides = {}) {
  fetchMock = vi.fn(async (url) => {
    const u = String(url);
    const route = u.split('?')[0];
    if (overrides[route]) return overrides[route](u);
    if (route === 'api/status') return json({ index: { state: 'fresh' }, command: 'just code-guide-index', counts: { staleFiles: [] } });
    if (route === 'api/workspace') return json(workspace);
    if (route === 'api/crate') return params(u).name === 'engine' ? json(crateView) : failure(404, 'no crate');
    if (route === 'api/type') return json(typeView);
    if (route === 'api/function') return json(fnView);
    if (route === 'api/calls') return json(callGraph);
    if (route === 'api/source') {
      const p = params(u);
      return json({ path: p.path, from: Number(p.from), to: Number(p.to), total: 500, text: `// ${p.from}-${p.to}` });
    }
    return failure(404, 'no route');
  });
  vi.stubGlobal('fetch', fetchMock);
}

let wrapper;
async function open(hash) {
  globalThis.location.hash = hash;
  wrapper = mount(App, { attachTo: document.body });
  await flushPromises();
  await flushPromises();
}
async function goto(hash) {
  globalThis.location.hash = hash;
  globalThis.dispatchEvent(new HashChangeEvent('hashchange'));
  await flushPromises();
  await flushPromises();
}
const pane = (slot) => wrapper.find(`nldd-split-view-pane[slot="${slot}"]`);
const hrefs = (el) => el.findAll('nldd-list-item').map((r) => r.attributes('href'));
/** The breadcrumbs as `text -> href`, the current page as `[text]`. */
const crumbs = () =>
  pane('main')
    .find('nldd-breadcrumbs')
    .findAll('nldd-breadcrumbs-item')
    .map((c) => (c.attributes('current') !== undefined ? `[${c.attributes('text')}]` : `${c.attributes('text')} -> ${c.attributes('href')}`));

beforeEach(() => install());
afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
  globalThis.location.hash = '';
});

describe('layout', () => {
  it('uses the design-system hierarchy: app view, split view, three panes with a page each', async () => {
    await open('#/engine');
    const view = wrapper.find('nldd-app-view > nldd-navigation-split-view');
    const panes = view.findAll(':scope > nldd-split-view-pane');
    expect(panes.map((p) => p.attributes('slot'))).toEqual(['primary-sidebar', 'main', 'inspector']);
    for (const p of panes) expect(p.find(':scope > nldd-page').exists()).toBe(true);
  });
});

describe('all crates', () => {
  it('lists the crates in reading order, each a link', async () => {
    await open('#/');
    expect(pane('main').findAll('nldd-title').map((t) => t.attributes('text'))).toContain('Layer 0');
    expect(hrefs(pane('main'))).toEqual(['#/model', '#/engine']);
  });

  it('shows only where you are in the breadcrumbs', async () => {
    await open('#/');
    expect(crumbs()).toEqual(['[All crates]']);
  });
});

describe('the crate switcher', () => {
  it('goes to the chosen crate, and to all crates for the empty choice', async () => {
    await open('#/model');
    const select = wrapper.find('nldd-dropdown select');
    select.element.value = 'engine';
    await select.trigger('change');
    expect(globalThis.location.hash).toBe('#/engine');
    await goto('#/engine');
    select.element.value = '';
    await select.trigger('change');
    expect(globalThis.location.hash).toBe('#/');
  });

  it('ignores the second, value-less change the dropdown itself fires', async () => {
    await open('#/engine');
    wrapper.find('nldd-dropdown').element.dispatchEvent(new Event('change', { bubbles: true }));
    await flushPromises();
    expect(globalThis.location.hash).toBe('#/engine');
  });
});

describe('a crate', () => {
  it('lists its modules as links and shows what it calls in other crates', async () => {
    await open('#/engine');
    expect(hrefs(pane('primary-sidebar'))).toEqual(['#/engine/service', '#/engine/types']);
    const into = pane('main').find('nldd-list[accessible-label="Modules of other crates this crate calls into"]');
    expect(hrefs(into)).toEqual(['#/model/law']);
    expect(crumbs()).toEqual(['All crates -> #/', '[engine]']);
  });

  it('groups modules by reading order on request', async () => {
    await open('#/engine');
    const control = pane('primary-sidebar').find('nldd-segmented-control[accessible-label="Group modules by"]');
    control.element.value = 'layers';
    await control.trigger('change');
    const lists = pane('primary-sidebar').findAll('nldd-list');
    expect(lists.map((l) => l.attributes('accessible-label'))).toEqual(['Layer 0', 'Layer 1']);
  });
});

describe('a module', () => {
  it('shows what it calls into and is called from, across crates, with counts', async () => {
    await open('#/engine/service');
    const into = pane('main').find('nldd-list[accessible-label="Modules this module calls into"]');
    expect(hrefs(into)).toEqual(['#/engine/types', '#/model/law']);
    expect(into.findAll('nldd-tag').map((t) => t.attributes('text'))).toEqual(['2 calls', '4 calls']);
    const from = pane('main').find('nldd-list[accessible-label="Modules that call this module"]');
    expect(hrefs(from)).toEqual(['#/app/main']);
  });

  it('lists its public types and functions, the rest on request', async () => {
    await open('#/engine/service');
    expect(hrefs(pane('main').find('nldd-list[accessible-label="Types"]'))).toEqual(['#/engine/service/Service']);
    expect(pane('main').find('nldd-list[accessible-label="Functions"]').exists()).toBe(false);
    const sw = wrapper.find('nldd-switch-field');
    sw.element.checked = true;
    await sw.trigger('change');
    expect(hrefs(pane('main').find('nldd-list[accessible-label="Functions"]'))).toEqual(['#/engine/service/helper']);
  });

  it('leads back to its crate and asks for the top of its file', async () => {
    await open('#/engine/service');
    expect(crumbs()).toEqual(['All crates -> #/', 'engine -> #/engine', '[service]']);
    expect(params(sourceCalls().at(-1))).toMatchObject({ path: 'engine/src/service.rs', from: '1', to: '80' });
  });
});

describe('a type', () => {
  it('opens its own page, a level below its module', async () => {
    await open('#/engine/service/Service');
    expect(calls().some((u) => u.startsWith('api/type') && params(u).name === 'Service')).toBe(true);
    expect(crumbs()).toEqual(['All crates -> #/', 'engine -> #/engine', 'service -> #/engine/service', '[Service]']);
    expect(pane('main').find('nldd-title[text="struct Service"]').exists()).toBe(true);
  });

  it('says how many places call each method and how many it calls', async () => {
    await open('#/engine/service/Service');
    const run = wrapper.find('#method-run');
    const tags = run.findAll('nldd-tag').map((t) => t.attributes('text'));
    expect(tags).toContain('called from 1');
    expect(tags).toContain('calls 2');
  });

  it('lists the selected method\'s callers and callees as links, in any crate', async () => {
    await open('#/engine/service/Service/run');
    const run = wrapper.find('#method-run');
    expect(hrefs(run.find('nldd-list[accessible-label="Functions that call run"]'))).toEqual(['#/app/main/main']);
    expect(hrefs(run.find('nldd-list[accessible-label="Functions run calls"]'))).toEqual([
      '#/engine/service/Service/step',
      '#/engine/types/parse',
    ]);
    expect(wrapper.find('#method-step nldd-list').exists()).toBe(false);
  });

  it('points the source viewer at the selected method', async () => {
    await open('#/engine/service/Service/step');
    expect(params(sourceCalls().at(-1))).toMatchObject({ path: 'engine/src/service.rs', from: '32', to: '40' });
  });
});

describe('a method that is not public', () => {
  it('is on its type\'s page when the address names it, so a link to it lands', async () => {
    const withPrivate = { ...typeView, methods: typeView.methods.map((m) => (m.key === 'step' ? { ...m, vis: 'private' } : m)) };
    install({ 'api/type': () => json(withPrivate) });
    await open('#/engine/service/Service');
    expect(pane('main').find('#method-step').exists()).toBe(false);
    await goto('#/engine/service/Service/step');
    expect(pane('main').find('#method-step').exists()).toBe(true);
  });
});

describe('a function', () => {
  it('opens as a function when the module has no type of that name', async () => {
    await open('#/engine/types/parse');
    expect(calls().some((u) => u.startsWith('api/function'))).toBe(true);
    expect(hrefs(pane('main').find('nldd-list[accessible-label="Functions that call parse"]'))).toEqual([
      '#/engine/service/Service/run',
    ]);
  });
});

describe('a failure', () => {
  it('clears once a valid item opens, and leaves nothing stale under it', async () => {
    install({ 'api/function': () => failure(404, 'no such function') });
    await open('#/engine/types/gone');
    const banner = () => pane('main').find('nldd-banner[variant="critical"]');
    expect(banner().attributes('supporting-text')).toContain('no such function');
    install();
    await goto('#/engine/types/parse');
    expect(banner().exists()).toBe(false);
    expect(pane('main').find('nldd-list[accessible-label="Functions that call parse"]').exists()).toBe(true);
  });
});

describe('the index', () => {
  it('names the files that changed since it was built', async () => {
    install({
      'api/status': () =>
        json({ index: { state: 'stale' }, command: 'just code-guide-index', counts: { staleFiles: ['engine/src/a.rs'] } }),
    });
    await open('#/engine');
    const banner = pane('main').find('nldd-banner');
    expect(banner.attributes('supporting-text')).toContain('engine/src/a.rs');
    expect(banner.attributes('supporting-text')).toContain('just code-guide-index');
  });

  it('loads every view again once an index is built while the page is open', async () => {
    vi.useFakeTimers({ toFake: ['setInterval'] });
    try {
      const missing = { index: { state: 'missing' }, command: 'just code-guide-index', counts: null, generation: null };
      install({
        'api/status': () => json(missing),
        'api/workspace': () => failure(503, 'No index yet. Run `just code-guide-index` to build it.'),
        'api/crate': () => failure(503, 'No index yet.'),
      });
      await open('#/engine');
      expect(pane('main').find('nldd-banner[variant="critical"]').exists()).toBe(true);
      // The index is built; the next status poll reports the new model.
      install({
        'api/status': () =>
          json({ index: { state: 'fresh' }, command: 'just code-guide-index', counts: { staleFiles: [] }, generation: 1 }),
      });
      vi.advanceTimersByTime(30_000);
      await flushPromises();
      await flushPromises();
      expect(pane('main').find('nldd-banner[variant="critical"]').exists()).toBe(false);
      expect(calls()).toContain('api/workspace');
      expect(calls().some((u) => u.startsWith('api/crate'))).toBe(true);
      expect(hrefs(pane('primary-sidebar'))).toEqual(['#/engine/service', '#/engine/types']);
    } finally {
      vi.useRealTimers();
    }
  });

  it('says in the source viewer when the file shown changed after indexing', async () => {
    install({
      'api/status': () =>
        json({ index: { state: 'stale' }, command: 'just code-guide-index', counts: { staleFiles: ['engine/src/service.rs'] } }),
    });
    await open('#/engine/service/Service/run');
    expect(pane('inspector').find('nldd-banner[text="This file changed after indexing"]').exists()).toBe(true);
    await goto('#/engine/types/parse');
    expect(pane('inspector').find('nldd-banner[text="This file changed after indexing"]').exists()).toBe(false);
  });

  it('says how to build it when there is none', async () => {
    install({ 'api/status': () => json({ index: { state: 'missing' }, command: 'just code-guide-index', counts: null }) });
    await open('#/');
    expect(pane('main').find('nldd-banner').attributes('text')).toBe('There is no rust-analyzer index yet');
  });

  it('shows nothing about it while it is fresh', async () => {
    await open('#/engine');
    expect(pane('main').find('nldd-banner').exists()).toBe(false);
  });
});

describe('search', () => {
  it('lists matches as links to the thing found', async () => {
    await open('#/engine');
    const field = wrapper.find('nldd-search-field');
    field.element.value = 'run';
    await field.trigger('input');
    // The method by name first; then the type, whose doc line says "Runs laws.".
    expect(hrefs(pane('primary-sidebar'))).toEqual(['#/engine/service/Service/run', '#/engine/service/Service']);
  });
});

describe('the graph', () => {
  const stub = () => pane('main').find('.call-graph-stub');
  const nodes = () => stub().attributes('data-nodes').split(' ').sort();
  const values = (label) => {
    const control = pane('main').find(`nldd-segmented-control[accessible-label="${label}"]`);
    return control.exists() ? control.findAll('nldd-segmented-control-item').map((i) => i.attributes('value')) : [];
  };

  it('loads the calls only once the graph is opened', async () => {
    await open('#/engine/service/Service');
    expect(calls().length).toBeGreaterThan(0);
    expect(fetchMock.mock.calls.some(([u]) => String(u) === 'api/calls')).toBe(false);
    await goto('#/engine/service/Service?view=graph');
    expect(fetchMock.mock.calls.filter(([u]) => String(u) === 'api/calls')).toHaveLength(1);
  });

  it('centres on the open page, at the finest level it offers', async () => {
    await open('#/?view=graph');
    expect(nodes()).toEqual(['c:app', 'c:engine', 'c:model']);
    expect(values('Show')).toEqual([]);
    await goto('#/engine?view=graph');
    expect(values('Show')).toEqual(['modules', 'crates']);
    expect(nodes()).toEqual(['m:engine|service', 'm:engine|types']);
    await goto('#/engine/service/Service/run?view=graph');
    expect(values('Show')).toEqual(['functions', 'modules', 'crates']);
    // A method: two steps either way.
    expect(nodes()).toEqual(['f0', 'f1', 'f2', 'f3', 'f4']);
  });

  it('follows only callers or callees, to the chosen depth, from the address', async () => {
    await open('#/engine/service/Service/run?view=graph&calls=out&depth=1');
    expect(nodes()).toEqual(['f1', 'f2', 'f3']);
    await goto('#/engine/service/Service/run?view=graph&calls=in&depth=1');
    expect(nodes()).toEqual(['f0', 'f1']);
    await goto('#/engine/service/Service/run?view=graph&depth=0');
    expect(nodes()).toEqual(['f1']);
  });

  it('puts every choice in the address, so a graph can be shared', async () => {
    await open('#/engine/service/Service/run?view=graph');
    const control = pane('main').find('nldd-segmented-control[accessible-label="Calls"]');
    control.element.value = 'in';
    await control.trigger('change');
    expect(globalThis.location.hash).toBe('#/engine/service/Service/run?view=graph&calls=in');
    await goto(globalThis.location.hash);
    // Left to right is the default, so it is not in the address.
    expect(stub().attributes('data-layout')).toBe('dagre:LR');
    const layout = pane('main').find('nldd-dropdown[accessible-label="Layout"]');
    layout.find('select').element.value = 'down';
    await layout.trigger('change');
    expect(globalThis.location.hash).toBe('#/engine/service/Service/run?view=graph&calls=in&layout=down');
    await goto(globalThis.location.hash);
    expect(stub().attributes('data-layout')).toBe('dagre:TB');
  });

  it('highlights the paths to a selected node and lists them as links', async () => {
    await open('#/engine/service/Service/run?view=graph');
    await wrapper.findComponent({ name: 'CallGraph' }).vm.$emit('select', 'f3', false);
    await flushPromises();
    expect(stub().attributes('data-highlight')).toBe('f1 f2 f3');
    const list = pane('main').find('nldd-list[accessible-label="Highlighted in the graph"]');
    // Nearest the focus first, then by name.
    expect(hrefs(list)).toEqual([
      '#/engine/service/Service/run?view=graph',
      '#/engine/types/parse?view=graph',
      '#/engine/service/Service/step?view=graph',
    ]);
    expect(pane('main').text()).toContain('3 functions and 3 calls on the paths between parse and this method');
    expect(pane('main').find('nldd-button[text="Open its page"]').attributes('href')).toBe('#/engine/types/parse');
    // The centre is named after the page, in the list and in the depth choice.
    expect(list.find('nldd-tag').attributes('text')).toBe('this method');
    expect(pane('main').find('nldd-dropdown[accessible-label="Depth"] option').text()).toBe('Only this method');
  });

  it('offers the highlight choices only once something is selected, and says so before', async () => {
    await open('#/engine/service/Service/run?view=graph');
    expect(pane('main').find('nldd-segmented-control[accessible-label="Highlight"]').exists()).toBe(false);
    expect(pane('main').find('nldd-banner[text="Select a node to highlight its paths"]').exists()).toBe(true);
    wrapper.findComponent({ name: 'CallGraph' }).vm.$emit('select', 'f3', false);
    await flushPromises();
    expect(pane('main').find('nldd-segmented-control[accessible-label="Highlight"]').exists()).toBe(true);
    expect(pane('main').find('nldd-banner[text="Select a node to highlight its paths"]').exists()).toBe(false);
  });

  it('shows the selected node\'s source, and the page\'s own again when cleared', async () => {
    await open('#/engine/service/Service/run?view=graph');
    const graph = wrapper.findComponent({ name: 'CallGraph' });
    graph.vm.$emit('select', 'f3', false);
    await flushPromises();
    expect(params(sourceCalls().at(-1))).toMatchObject({ path: 'engine/src/types.rs', from: '2', to: '6' });
    expect(pane('inspector').find('nldd-top-title-bar').attributes('text')).toBe('parse');
    graph.vm.$emit('clear');
    await flushPromises();
    expect(params(sourceCalls().at(-1))).toMatchObject({ path: 'engine/src/service.rs', from: '18', to: '30' });
    expect(pane('inspector').find('nldd-top-title-bar').attributes('text')).toBe('run');
  });

  it('keeps a chosen depth while following the graph, not when opening another page', async () => {
    await open('#/engine/service/Service/run?view=graph&depth=3&layout=down');
    // Another page: what is drawn how is kept, the depth is not.
    expect(crumbs()[2]).toBe('service -> #/engine/service?view=graph&layout=down');
    wrapper.findComponent({ name: 'CallGraph' }).vm.$emit('open', 'f3');
    await flushPromises();
    expect(globalThis.location.hash).toBe('#/engine/types/parse?view=graph&depth=3&layout=down');
  });

  it('lists a selected module\'s functions, and adds one to the graph only when asked', async () => {
    await open('#/engine?view=graph&depth=1');
    const before = ['m:app|main', 'm:engine|service', 'm:engine|types', 'm:model|law'];
    expect(nodes()).toEqual(before);
    const graph = wrapper.findComponent({ name: 'CallGraph' });
    graph.vm.$emit('select', 'm:engine|service', false);
    await flushPromises();
    // Selecting does not change the graph.
    expect(nodes()).toEqual(before);
    const list = pane('main').find('nldd-list[accessible-label="Functions behind the calls of engine::service"]');
    const rows = list.findAll('nldd-list-item');
    // run: 3 sites in from main, 1 out to parse; step: 1 out to parse.
    expect(rows.map((r) => r.find('nldd-text-cell').attributes('text'))).toEqual(['Service::run', 'Service::step']);
    expect(rows[0].find('nldd-icon-button[text="Open the graph of Service::run"]').attributes('href')).toBe(
      '#/engine/service/Service/run?view=graph',
    );
    await rows[1].find('nldd-icon-button[text="Add Service::step to the graph"]').trigger('click');
    await flushPromises();
    expect(nodes()).toEqual([...before, 'f2'].sort());
    expect(stub().attributes('data-edges').split(' ').sort()).toEqual([
      'f2>m:engine|types',
      'm:app|main>m:engine|service',
      'm:engine|service>m:engine|types',
      'm:engine|types>m:model|law',
    ]);
    // The added function is selected: its source shows, and its row offers to take it out.
    expect(pane('inspector').find('nldd-top-title-bar').attributes('text')).toBe('Service::step');
    const again = pane('main').findAll('nldd-list[accessible-label="Functions behind the calls of engine::service"] nldd-list-item');
    await again[1].find('nldd-icon-button[text="Remove Service::step from the graph"]').trigger('click');
    await flushPromises();
    expect(nodes()).toEqual(before);
  });

  it('removes every added function at once', async () => {
    await open('#/engine?view=graph&depth=1');
    wrapper.findComponent({ name: 'CallGraph' }).vm.$emit('select', 'm:engine|service', false);
    await flushPromises();
    for (const b of pane('main').findAll('nldd-icon-button[text^="Add "]')) await b.trigger('click');
    await flushPromises();
    expect(nodes()).toContain('f1');
    await pane('main').find('nldd-button[text="Remove all"]').trigger('click');
    await flushPromises();
    expect(nodes().some((n) => n.startsWith('f'))).toBe(false);
  });

  it('lets go of a selected node clicked again, but not when it is a double-click', async () => {
    await open('#/engine/service/Service/run?view=graph');
    const graph = wrapper.findComponent({ name: 'CallGraph' });
    graph.vm.$emit('select', 'f3', false);
    await flushPromises();
    graph.vm.$emit('select', 'f3', false);
    await flushPromises();
    // Still selected until a double-click could have happened.
    expect(stub().attributes('data-highlight')).toBe('f1 f2 f3');
    await new Promise((r) => setTimeout(r, 350));
    await flushPromises();
    expect(stub().attributes('data-highlight')).toBe('');
    graph.vm.$emit('select', 'f3', false);
    graph.vm.$emit('select', 'f3', false);
    graph.vm.$emit('open', 'f3');
    await new Promise((r) => setTimeout(r, 350));
    await flushPromises();
    expect(globalThis.location.hash).toBe('#/engine/types/parse?view=graph');
  });

  it('draws the legend with a box only where boxes exist, and describes it in words', async () => {
    await open('#/engine/service/Service/run?view=graph');
    const legend = () => pane('main').find('.legend-stub');
    expect(legend().attributes('data-boxes')).toBe('false');
    expect(legend().attributes('data-focus')).toBe('this method');
    expect(legend().attributes('aria-label')).toContain('A thick blue border marks this method');
    expect(legend().attributes('aria-label')).toContain('Each colour is a crate');
    expect(legend().attributes('aria-label')).not.toContain('box');
    await goto('#/engine?view=graph');
    expect(legend().attributes('data-boxes')).toBe('true');
    expect(legend().attributes('data-focus')).toBe("this crate's modules");
    expect(legend().attributes('aria-label')).toContain('a box is a module with functions added to it');
  });

  it('names the crates in the graph in the legend, each in its colour', async () => {
    await open('#/engine/service/Service/run?view=graph');
    const tags = pane('main').findAll('nldd-container nldd-tag[size="sm"]').filter((t) => t.attributes('color'));
    // Crates in name order get the palette in order: app, engine, model.
    expect(tags.map((t) => [t.attributes('text'), t.attributes('color')])).toEqual([
      ['app', 'hemelblauw'],
      ['engine', 'oranje'],
      ['model', 'mintgroen'],
    ]);
  });

  it('keeps the selection and what was highlighted when only the layout or spacing changes', async () => {
    await open('#/engine/service/Service/run?view=graph');
    wrapper.findComponent({ name: 'CallGraph' }).vm.$emit('select', 'f3', false);
    await flushPromises();
    expect(stub().attributes('data-highlight')).toBe('f1 f2 f3');
    await goto('#/engine/service/Service/run?view=graph&layout=down');
    await goto('#/engine/service/Service/run?view=graph&layout=down&spacing=roomy');
    expect(stub().attributes('data-highlight')).toBe('f1 f2 f3');
    // Another depth is another graph: that starts without a selection.
    await goto('#/engine/service/Service/run?view=graph&layout=down&spacing=roomy&depth=1');
    expect(stub().attributes('data-highlight')).toBe('');
  });

  it('highlights between two nodes with Shift, and clears on the background', async () => {
    await open('#/engine/service/Service/run?view=graph');
    const graph = wrapper.findComponent({ name: 'CallGraph' });
    graph.vm.$emit('select', 'f0', false);
    graph.vm.$emit('select', 'f2', true);
    await flushPromises();
    expect(stub().attributes('data-highlight')).toBe('f0 f1 f2');
    graph.vm.$emit('clear');
    await flushPromises();
    expect(stub().attributes('data-highlight')).toBe('');
  });

  it('centres the graph on a node when it is double-clicked, keeping the choices', async () => {
    await open('#/engine/service/Service/run?view=graph&calls=out');
    wrapper.findComponent({ name: 'CallGraph' }).vm.$emit('open', 'f3');
    await flushPromises();
    expect(globalThis.location.hash).toBe('#/engine/types/parse?view=graph&calls=out');
  });
});

describe('the source', () => {
  it('widens into the main pane and back', async () => {
    await open('#/engine/service/Service/run');
    expect(pane('inspector').find('nldd-button[text="Widen"]').attributes('href')).toBe('#/engine/service/Service/run?source=wide');
    await goto('#/engine/service/Service/run?source=wide');
    expect(wrapper.findAll('nldd-navigation-split-view > nldd-split-view-pane').map((p) => p.attributes('slot'))).toEqual([
      'primary-sidebar',
      'main',
    ]);
    expect(pane('main').find('nldd-code-viewer').exists()).toBe(true);
  });
});
