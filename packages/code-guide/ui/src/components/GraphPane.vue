<script setup>
import { computed, ref, watch } from 'vue';
import { GRAPH_OPTIONS } from '../lib/guide.js';
import { LAYOUTS, boundaryOf, expand, highlight, legendOf, toElements, unitsOf } from '../lib/callgraph.js';
import CallGraph from './CallGraph.vue';
import LegendGraph from './LegendGraph.vue';

// The call graph around the open page, and the controls for it. What it shows
// (level, callers or callees, depth) and how (layout, spacing) is in the
// address; what is selected and highlighted is not, it is a way of reading.
const props = defineProps({ g: { type: Object, required: true } });

const LEVEL_TEXT = { functions: 'Functions', modules: 'Modules', crates: 'Crates' };
const NOUN = { functions: ['function', 'functions'], modules: ['module', 'modules'], crates: ['crate', 'crates'] };
const CALLS_TEXT = { both: 'Both ways', in: 'Callers', out: 'Callees' };
const SPACING_TEXT = { compact: 'Compact', normal: 'Normal', roomy: 'Roomy' };
const MODE_TEXT = { paths: 'Paths', in: 'All callers', out: 'All callees' };
// The toolbar's own words default to Dutch; this page is in English.
const TOOLBAR_TEXT = { 'components.toolbar.overflow-action': 'More', 'components.toolbar.opens-in-new-tab-label': 'Opens in a new tab' };
const DEPTHS = GRAPH_OPTIONS.depth.filter((d) => d !== null).map(Number);

const canvas = ref(null);
const selected = ref([]);
const mode = ref('paths');
const hideRest = ref(false);
const hovered = ref(null);

const r = computed(() => props.g.route);
/** A node by id: a module or crate at those levels, or a function in an opened box. */
const node = (id) => props.g.lifted?.nodes.get(id) ?? props.g.fnGraph?.nodes.get(id) ?? null;
const isFunction = (id) => /^f\d+$/.test(id);

// Functions added to a module or crate graph, per node (see `expand`).
const picked = ref(new Map());
const lastOpened = ref(null);
const part = computed(() =>
  props.g.part && props.g.calls ? expand(props.g.calls, props.g.lifted, props.g.part, picked.value) : null,
);
const legend = computed(() => (part.value ? legendOf(node, part.value, props.g.crateOrder) : []));
const legendText = computed(
  () =>
    `Each colour is a crate. A thick blue border marks ${focusName.value}, a dashed border a private function and a thick ` +
    'dark border what is selected' +
    (canOpen.value ? `; a box is a ${noun(1)} with functions added to it. ` : '. ') +
    'Arrows run from caller to callee and are thicker for more calls (×n). Hovering a node turns the arrows into ' +
    'it orange and the arrows out of it green; a highlighted path is blue.',
);
const pickedCount = computed(() => [...picked.value.values()].reduce((t, s) => t + s.size, 0));
const elements = computed(() =>
  part.value ? toElements(node, part.value, props.g.focus, props.g.crateOrder) : [],
);
const canOpen = computed(() => props.g.level !== 'functions');
const noun = (n) => NOUN[props.g.level][n === 1 ? 0 : 1];
// At the level of all crates the focus is everything, so direction and depth
// change nothing.
const everything = computed(() => props.g.graphPage === 'workspace');

// A new graph starts without a selection, and with everything closed.
watch(
  () => props.g.part,
  () => {
    selected.value = [];
    hovered.value = null;
    picked.value = new Map();
    lastOpened.value = null;
  },
);
// The source viewer follows the selection: the node selected last, or the
// page's own source again once nothing is selected.
watch(
  () => selected.value.at(-1),
  (id) => props.g.showInSource(id ? node(id) : null),
);

const hl = computed(() =>
  part.value ? highlight(part.value, { mode: mode.value, selected: selected.value, focus: props.g.focus }) : null,
);

const go = (overrides) => {
  globalThis.location.hash = props.g.hrefHere(overrides);
};
// `nldd-dropdown` fires the select's own change and then one of its own without
// a value; reading the select makes both say the same.
const selectValue = (event) => event.currentTarget.querySelector('select')?.value;
const onLevel = (event) => go({ level: event.target.value });
const onCalls = (event) => go({ calls: event.target.value });
const onDepth = (event) => {
  const v = selectValue(event);
  if (v !== undefined && Number(v) !== props.g.depth) go({ depth: v });
};
const onLayout = (event) => {
  const v = selectValue(event);
  if (v && v !== r.value.layout) go({ layout: v });
};
const onSpacing = (event) => {
  const v = selectValue(event);
  if (v && v !== r.value.spacing) go({ spacing: v });
};
const onMode = (event) => {
  mode.value = event.target.value;
};
const onHide = (event) => {
  hideRest.value = event.target.checked;
};

/**
 * Tap: select one node; Shift-tap: add a second, for the paths between two;
 * tapping the selected node again lets go of it. Letting go waits as long as a
 * double-click takes: it removes the sections under the graph, the page gets
 * shorter and, scrolled to its end, moves, so the second click of a
 * double-click on a selected node would land beside it.
 */
const DOUBLE_CLICK = 300;
let releasing = null;
function onSelect(id, additive) {
  clearTimeout(releasing);
  const [first] = selected.value;
  if (additive && first && first !== id) selected.value = [first, id];
  else if (!additive && selected.value.length === 1 && first === id) {
    releasing = setTimeout(() => {
      selected.value = [];
    }, DOUBLE_CLICK);
  } else selected.value = [id];
}

/** The box a function would go in: the module or crate node it belongs to. */
const ownerOf = (fid) => props.g.lifted?.idOf[Number(fid.slice(1))] ?? null;
const isPicked = (fid) => picked.value.get(ownerOf(fid))?.has(fid) ?? false;
/**
 * Adds a function to the graph inside its module or crate, and selects it so
 * its paths and its source show; or takes it out again.
 */
function togglePicked(fid) {
  const owner = ownerOf(fid);
  if (!owner) return;
  const next = new Map(picked.value);
  const fns = new Set(next.get(owner) ?? []);
  if (fns.has(fid)) {
    fns.delete(fid);
    if (selected.value.includes(fid)) selected.value = selected.value.filter((id) => id !== fid);
  } else {
    fns.add(fid);
    lastOpened.value = fid;
  }
  if (fns.size) next.set(owner, fns);
  else next.delete(owner);
  picked.value = next;
  if (fns.has(fid)) selected.value = [fid];
}
const removeAllPicked = () => {
  picked.value = new Map();
  lastOpened.value = null;
  selected.value = selected.value.filter((id) => !isFunction(id));
};

// The module or crate whose functions are listed: the one selected last, or
// the box of a function selected in it.
const listedNode = computed(() => {
  if (!canOpen.value) return null;
  const id = selected.value.at(-1);
  if (!id) return null;
  return isFunction(id) ? ownerOf(id) : id;
});
const listed = computed(() =>
  listedNode.value && props.g.calls && props.g.part
    ? boundaryOf(props.g.calls, props.g.lifted, props.g.part, listedNode.value).map((x) => ({ ...x, n: node(x.id) }))
    : [],
);
const clear = () => {
  selected.value = [];
};
// Following the graph keeps what it shows (level, depth): the reader is
// moving along it, not opening a new page.
// A function from an opened box goes to its own page at the functions level.
const hrefOf = (id, view = 'graph') => {
  const n = node(id);
  if (!n) return null;
  const keep = !(isFunction(id) && props.g.level !== 'functions');
  return props.g.hrefFor({ ...n.target, view, ...(keep ? { level: r.value.level, depth: r.value.depth } : {}) });
};
/** Double-click: centre the graph on that node (its own page, still as a graph). */
const onOpen = (id) => {
  clearTimeout(releasing);
  const href = hrefOf(id);
  if (href) globalThis.location.hash = href;
};

const hint = computed(
  () =>
    `Click a node to highlight the paths between it and ${focusName.value}, or everything that leads to it or follows ` +
    'from it; Shift-click a second node for the paths between the two. Double-click a node to centre the graph on ' +
    'it. Drag nodes to move them, scroll to zoom, drag the background to pan.' +
    (canOpen.value ? ` A selected ${noun(1)} lists the functions behind its calls, to add to the graph one by one.` : ''),
);

const shown = computed(() => hovered.value ?? selected.value.at(-1) ?? null);
const info = computed(() => {
  const id = shown.value;
  const n = id ? node(id) : null;
  if (!n || !part.value) return null;
  // An open box counts the calls crossing it.
  const inside = unitsOf(part.value, [id]);
  const callers = part.value.edges.filter((e) => inside.has(e.target) && !inside.has(e.source)).length;
  const callees = part.value.edges.filter((e) => inside.has(e.source) && !inside.has(e.target)).length;
  return { id, n, callers, callees, focus: props.g.focus.has(id) };
});

const label = (id) => node(id)?.label ?? id;
const hlText = computed(() => {
  const h = hl.value;
  if (!h) return null;
  const [a, b] = selected.value;
  const n = h.nodes.size;
  const others = n - selected.value.filter((id) => h.nodes.has(id)).length;
  if (mode.value === 'in') return `${others} ${noun(others)} in this graph lead to ${label(a)}${b ? ` or ${label(b)}` : ''}.`;
  if (mode.value === 'out') return `${label(a)}${b ? ` and ${label(b)}` : ''} lead${b ? '' : 's'} to ${others} ${noun(others)} in this graph.`;
  if (!b && props.g.focus.size === 1 && props.g.focus.has(a)) {
    return `${label(a)} is ${focusName.value}; its own ${h.edges.size} ${h.edges.size === 1 ? 'call' : 'calls'} in this graph are highlighted.`;
  }
  const between = b ? `${label(a)} and ${label(b)}` : `${label(a)} and ${focusName.value}`;
  if (!h.connected) return `No path in this graph between ${between}. A higher depth may find one.`;
  return `${n} ${noun(n)} and ${h.edges.size} ${h.edges.size === 1 ? 'call' : 'calls'} on the paths between ${between}.`;
});
/** The highlighted nodes as links, nearest the focus first: the graph, readable without the canvas. */
const hlList = computed(() => {
  if (!hl.value || !part.value) return [];
  return [...hl.value.nodes]
    .map((id) => ({ id, d: part.value.dist.get(id) ?? 0, n: node(id) }))
    .filter((x) => x.n)
    .sort((a, b) => a.d - b.d || a.n.label.localeCompare(b.n.label));
});

const summary = computed(() => {
  const p = part.value;
  if (!p) return '';
  const n = p.dist.size;
  const e = p.edges.length;
  return `${n} ${noun(n)}, ${e} ${e === 1 ? 'call' : 'calls'}.`;
});
/**
 * What the graph is centred on, named after the page: "this method", "this
 * type's methods", "this crate's modules". `focusTag` is the short form for a
 * tag beside one of those nodes.
 */
const focusName = computed(() => {
  const g = props.g;
  if (everything.value) return 'every crate';
  if (g.level === 'functions') {
    if (g.graphPage === 'function') return 'this function';
    if (g.graphPage === 'type') return r.value.method ? 'this method' : "this type's methods";
    return "this module's functions";
  }
  if (g.level === 'modules') return g.graphPage === 'crate' ? "this crate's modules" : 'this module';
  return 'this crate';
});
const focusTag = computed(() => {
  const g = props.g;
  if (everything.value) return 'crate';
  if (g.level === 'functions') {
    if (g.graphPage === 'type') return r.value.method ? 'this method' : 'this type';
    return g.graphPage === 'function' ? 'this function' : 'this module';
  }
  return g.level === 'modules' && g.graphPage !== 'crate' ? 'this module' : 'this crate';
});
/** One node is the centre, or one of several. */
const focusOne = computed(() => props.g.focus.size === 1);

const where = computed(() => {
  const g = props.g;
  if (everything.value) return 'every crate';
  const f = g.focus.size;
  if (g.level === 'functions') {
    if (g.graphPage === 'function') return 'this function';
    if (g.graphPage === 'type') return r.value.method ? 'this method' : `the ${f} ${f === 1 ? 'method' : 'methods'} of this type`;
    return `the ${f} ${f === 1 ? 'function' : 'functions'} of this module`;
  }
  if (g.level === 'modules') return g.graphPage === 'crate' ? 'the modules of this crate' : 'this module';
  return 'this crate';
});
</script>

<template>
  <!-- Full width: the graph and its controls use the whole pane on a wide screen. -->
  <nldd-simple-section width="full" padding-block="16">
    <nldd-toolbar label="What the graph shows" show-item-labels :translations="TOOLBAR_TEXT">
      <nldd-toolbar-item v-if="g.levels.length > 1" slot="start" label="Show">
        <nldd-segmented-control accessible-label="Show" :value="g.level" @change="onLevel">
          <nldd-segmented-control-item
            v-for="l in g.levels"
            :key="l"
            :value="l"
            :text="LEVEL_TEXT[l]"
            :selected="g.level === l"
          />
        </nldd-segmented-control>
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="!everything" slot="start" label="Calls" max-width="340px">
        <nldd-segmented-control accessible-label="Calls" :value="g.route.calls" @change="onCalls">
          <nldd-segmented-control-item
            v-for="(text, c) in CALLS_TEXT"
            :key="c"
            :value="c"
            :text="text"
            :selected="g.route.calls === c"
          />
        </nldd-segmented-control>
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="!everything" slot="start" label="Depth" min-width="180px">
        <nldd-dropdown accessible-label="Depth" width="180px" @change="onDepth">
          <select>
            <option v-for="d in DEPTHS" :key="d" :value="d" :selected="g.depth === d">
              {{ d === 0 ? `Only ${focusName}` : `${d} ${d === 1 ? 'step' : 'steps'}` }}
            </option>
          </select>
        </nldd-dropdown>
      </nldd-toolbar-item>
    </nldd-toolbar>
    <nldd-spacer size="8" />
    <nldd-toolbar label="How the graph is drawn" show-item-labels :translations="TOOLBAR_TEXT">
      <nldd-toolbar-item slot="start" label="Layout" min-width="250px">
        <nldd-dropdown accessible-label="Layout" width="250px" @change="onLayout">
          <select>
            <option v-for="(text, l) in LAYOUTS" :key="l" :value="l" :selected="g.route.layout === l">{{ text }}</option>
          </select>
        </nldd-dropdown>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Spacing" min-width="140px">
        <nldd-dropdown accessible-label="Spacing" width="140px" @change="onSpacing">
          <select>
            <option v-for="(text, s) in SPACING_TEXT" :key="s" :value="s" :selected="g.route.spacing === s">{{ text }}</option>
          </select>
        </nldd-dropdown>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="View" min-width="150px">
        <nldd-button-group orientation="horizontal">
          <nldd-icon-button icon="minus" text="Zoom out" @click="canvas?.zoom(0.8)" />
          <nldd-icon-button icon="plus" text="Zoom in" @click="canvas?.zoom(1.25)" />
          <nldd-icon-button icon="fit-to-view" text="Show everything" @click="canvas?.fit()" />
        </nldd-button-group>
      </nldd-toolbar-item>
      <nldd-toolbar-item slot="start" label="Positions">
        <nldd-button start-icon="refresh" text="Lay out again" @click="canvas?.relayout()" />
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="canOpen" slot="start" label="Added functions">
        <nldd-button :disabled="!pickedCount" text="Remove all" @click="removeAllPicked" />
      </nldd-toolbar-item>
    </nldd-toolbar>
    <nldd-spacer size="8" />
    <nldd-text>Centred on {{ where }}. {{ summary }} Arrows point from a caller to what it calls.</nldd-text>
  </nldd-simple-section>

  <nldd-simple-section width="full" padding-block="8" v-if="g.callsError">
    <nldd-banner variant="critical" text="Could not load the calls" :supporting-text="g.callsError" />
  </nldd-simple-section>
  <nldd-simple-section width="full" padding-block="8" v-else-if="!g.part">
    <nldd-text>Loading…</nldd-text>
  </nldd-simple-section>
  <template v-else>
    <nldd-simple-section width="full" padding-block="8" v-if="g.part.omitted">
      <nldd-banner
        variant="warning"
        :text="`${g.part.omitted} more ${noun(g.part.omitted)} left out`"
        supporting-text="The graph shows the nearest ones. Lower the depth or show only callers or callees to see all."
      />
    </nldd-simple-section>
    <nldd-simple-section width="full" padding-block="8" v-if="!g.focus.size">
      <nldd-text>There is nothing here the index has calls for.</nldd-text>
    </nldd-simple-section>
    <!-- The graph and, beside it, everything for working with a selection, so
         neither needs the page to scroll; on a narrow pane the panel goes below. -->
    <nldd-two-thirds-one-third-section width="full" padding-top="0">
      <div slot="left">
        <CallGraph
          ref="canvas"
          :elements="elements"
          :centre="lastOpened"
          :layout="g.layout"
          :highlight="hl"
          :hide-rest="hideRest"
          :selected="selected"
          @select="onSelect"
          @clear="clear"
          @open="onOpen"
          @hover="(id) => (hovered = id)"
        />
      </div>
      <div slot="right" class="graph-side">
        <nldd-container layout="stack" gap="24">
          <!-- The colours and marks the canvas uses: the crates as tags in the
               same design-system colours as their nodes, then an example graph. -->
          <nldd-container layout="stack" gap="8">
            <nldd-title :size="6" text="Legend" :heading-level="3" />
            <nldd-container layout="wrap" gap="4">
              <nldd-tag v-for="c in legend" :key="c.crate" size="sm" :color="c.color" :text="c.crate" />
            </nldd-container>
            <LegendGraph :boxes="canOpen" :box-noun="noun(1)" :focus-name="focusName" :description="legendText" />
          </nldd-container>

          <!-- What to highlight only means something once a node is selected. It
               is beside the graph: above it, appearing would move the canvas
               between the two clicks of a double-click. -->
          <!-- Stacked, not a toolbar: in a column this narrow a toolbar would fold
               every control into its overflow menu. -->
          <nldd-container v-if="selected.length" layout="stack" gap="8">
            <nldd-segmented-control accessible-label="Highlight" width="100%" :value="mode" @change="onMode">
              <nldd-segmented-control-item
                v-for="(text, m) in MODE_TEXT"
                :key="m"
                :value="m"
                :text="text"
                :selected="mode === m"
              />
            </nldd-segmented-control>
            <nldd-container layout="row" gap="16" vertical-alignment="center">
              <nldd-switch-field label="Hide the rest" :checked="hideRest" @change="onHide" />
              <nldd-button text="Clear the selection" @click="clear" />
            </nldd-container>
          </nldd-container>
          <nldd-banner v-else variant="accent" text="Select a node to highlight its paths" :supporting-text="hint" />

          <nldd-container v-if="info" layout="stack" gap="8">
            <nldd-title :size="5" :text="info.n.label" :heading-level="3" />
            <nldd-text v-if="info.n.sub">{{ info.n.sub }}</nldd-text>
            <nldd-text v-if="info.n.doc">{{ info.n.doc }}</nldd-text>
            <nldd-text>
              In this graph: called from {{ info.callers }}, calls {{ info.callees }}.{{ info.focus && !everything ? (focusOne ? ` This is ${focusName}.` : ` This is one of ${focusName}.`) : '' }}
            </nldd-text>
            <nldd-button-group>
              <nldd-button :href="hrefOf(info.id, 'details')" text="Open its page" />
              <nldd-button v-if="!info.focus" :href="hrefOf(info.id)" text="Centre the graph on it" />
            </nldd-button-group>
          </nldd-container>

          <nldd-container v-if="listedNode" layout="stack" gap="8">
            <nldd-title :size="5" :text="`Functions behind the calls of ${label(listedNode)}`" :heading-level="3" />
            <nldd-text v-if="!listed.length">None of its calls cross to another {{ noun(1) }} in this graph.</nldd-text>
            <nldd-text v-else>
              {{ listed.length }} {{ listed.length === 1 ? 'function makes or receives' : 'functions make or receive' }} the
              calls drawn to and from it, busiest first. Add one to see its own calls run through it.
            </nldd-text>
            <nldd-list v-if="listed.length" type="list" :accessible-label="`Functions behind the calls of ${label(listedNode)}`">
              <nldd-list-item v-for="x in listed" :key="x.id" size="sm">
                <nldd-text-cell size="sm" :text="x.n.label" :supporting-text="`${x.in} in · ${x.out} out`" />
                <nldd-cell>
                  <nldd-button-group orientation="horizontal">
                    <nldd-icon-button
                      size="sm"
                      :icon="isPicked(x.id) ? 'minus' : 'plus'"
                      :text="isPicked(x.id) ? `Remove ${x.n.label} from the graph` : `Add ${x.n.label} to the graph`"
                      @click="togglePicked(x.id)"
                    />
                    <nldd-icon-button size="sm" icon="graph" :href="hrefOf(x.id)" :text="`Open the graph of ${x.n.label}`" />
                  </nldd-button-group>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>

          <nldd-container v-if="hlText" layout="stack" gap="8">
            <nldd-title :size="5" text="Highlighted" :heading-level="3" />
            <nldd-text>{{ hlText }}</nldd-text>
            <nldd-list type="list" accessible-label="Highlighted in the graph">
              <nldd-list-item v-for="x in hlList" :key="x.id" size="sm" :href="hrefOf(x.id)">
                <nldd-text-cell size="sm" :text="x.n.label" :supporting-text="x.n.sub ?? undefined" />
                <nldd-cell v-if="g.focus.has(x.id)"><nldd-tag size="sm" :text="focusTag" /></nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-container>
      </div>
    </nldd-two-thirds-one-third-section>
  </template>
</template>
