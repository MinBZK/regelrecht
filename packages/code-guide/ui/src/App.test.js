import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import App from './App.vue';

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
  graph: {
    edges: [{ from: 'run', to: 'step', via: [] }],
    layers: [[['step']], [['run']]],
    external: [{ crate: 'app', module: 'main', targets: { run: 3 } }],
  },
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
const back = () => pane('main').find('nldd-top-title-bar nldd-button[slot="toolbar"]');

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

  it('offers no back button at the top', async () => {
    await open('#/');
    expect(back().exists()).toBe(false);
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
    expect(back().attributes('href')).toBe('#/');
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
    expect(back().attributes('href')).toBe('#/engine');
    expect(params(sourceCalls().at(-1))).toMatchObject({ path: 'engine/src/service.rs', from: '1', to: '80' });
  });
});

describe('a type', () => {
  it('opens its own page, a level below its module', async () => {
    await open('#/engine/service/Service');
    expect(calls().some((u) => u.startsWith('api/type') && params(u).name === 'Service')).toBe(true);
    expect(back().attributes('href')).toBe('#/engine/service');
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
  const scopes = () => pane('main').findAll('nldd-segmented-control-item').map((i) => i.attributes('value'));

  it('offers the levels down to what is open', async () => {
    await open('#/?view=graph');
    expect(scopes()).toEqual(['details', 'graph', 'workspace']);
    await goto('#/engine/service?view=graph');
    expect(scopes()).toEqual(['details', 'graph', 'workspace', 'crate', 'module']);
    await goto('#/engine/service/Service?view=graph');
    expect(scopes()).toEqual(['details', 'graph', 'workspace', 'crate', 'module', 'type']);
  });

  it('draws a type\'s methods and its outside callers, each a link that keeps the graph', async () => {
    await open('#/engine/service/Service?view=graph');
    const cards = pane('main').findAll('nldd-card');
    expect(cards.map((c) => [c.attributes('accessible-label'), c.attributes('href')]).sort()).toEqual([
      ['app::main', '#/app/main?view=graph'],
      ['run', '#/engine/service/Service/run?view=graph'],
      ['step', '#/engine/service/Service/step?view=graph'],
    ]);
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
