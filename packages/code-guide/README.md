# code-guide

A guide to the regelrecht Rust workspace for someone finding their way around
it. It shows the crates, their modules, types and functions, and how they call
each other: crates and modules in reading order (what others build on first),
each type with its methods, signatures and doc comments, and for every function
the functions that call it and the functions it calls, in any crate.

Every relation in the guide is a call, as rust-analyzer resolved it. Nothing in
it is written for the guide: names, doc comments and signatures come from the
code, and where a doc comment is missing the guide says so.

This is a developer tool, a tooling-only workspace member. It is not deployed.

```bash
just code-guide-index   # build the rust-analyzer index (about a minute)
just code-guide         # build the UI and serve it on http://localhost:7190
```

## The index

`code-guide index` runs `rust-analyzer scip` over the workspace and writes a
[SCIP](https://github.com/scip-code/scip) index to
`<target dir>/code-guide/index.scip`. Next to it go the source scan taken in
the same run (`index.sources.json`, see below) and a manifest (`index.json`): a
cache key and a hash per input file. The manifest is written last, so an
interrupted run leaves no index that looks complete. It needs the
rust-analyzer component (`rustup component add rust-analyzer`).

- **When it runs.** Only on request. A run takes about a minute and several GB
  of memory for this workspace, so the server never starts one; it says when the
  index is missing or stale and which command rebuilds it.
- **Cached.** The key is a hash of every Rust source and Cargo manifest of the
  workspace, `Cargo.lock`, the rust-analyzer version and the configuration it
  indexes with. When the key matches, `index` does nothing (`--force` rebuilds
  anyway). Two runs over the same inputs give a byte-identical index, so a
  cached one is as good as a fresh one. This crate's own sources are not part of
  the key: the guide leaves itself out, so editing it does not need a new index.
- **All features.** rust-analyzer indexes with every Cargo feature on. The
  engine's `wasm` module exists only with its feature, and without it every call
  made from there would be missing.
- **Stale sources.** The guide never reads the index's line numbers against an
  edited file: it uses the source scan stored with the index, so after an edit
  it still shows the code exactly as it was indexed. The manifest names which
  files changed since, the app lists them in a banner and marks their
  functions, and the source view (read from the working tree) may differ from
  what the guide says about them until the index is rebuilt.

## What the guide reads where

rust-analyzer is leading. From the index come the crates, modules, types and
functions, their names, kinds, doc comments, signatures and, from the signature,
their visibility; and what every reference resolves to.

The source scan (`src/source.rs`, with `syn`), taken when the index is built,
adds only what the index cannot be trusted with:

- **The exact extent of every function**, doc comment through closing brace,
  to place a call in the function that makes it. The index has extents too, but
  inside macro-generated code (`#[wasm_bindgen] impl`) it gives every method the
  whole impl block.
- **Which code is test code**: files under `tests/`, `benches/` and `examples/`,
  anything compiled only under `cfg(test)` (or `cfg(all(test, ..))`; not
  `cfg(not(test))` or `cfg(any(test, ..))`), including a file that starts with
  `#![cfg(test)]`, modules declared that way and everything below them, and
  `#[test]` functions. A module declared with `#[path]` is not followed. Test code is left out of the
  guide entirely, as caller and as callee.
- **`use` items**, at any depth: rust-analyzer does not always mark an imported
  path as an import, and a `use` is not a call.
- **Nested items.** rust-analyzer gives a function or type defined inside a
  function body a module-level symbol. The guide leaves it out: it is part of
  the function it sits in, and two nested `fn inner` in one module would
  otherwise be one ambiguous function.

## What counts as a call

A call is a reference, from production code inside one workspace function, to
another workspace function. A function passed as a value (`.map(Self::f)`)
counts too: the index does not tell the two apart, and both are uses.

- **Trait objects.** A call through `&dyn Trait` is a call to the trait's
  method, not to the implementation that runs. The guide shows what the code
  says, not what happens at run time.
- **Other targets.** Code compiled only for another platform
  (`cfg(target_arch = "wasm32")` when indexing on Linux) is not in the index.
- **Ambiguous symbols.** rust-analyzer gives two binaries' `main` the same
  symbol. A reference to such a symbol is dropped and counted, never guessed.
- **Generics.** `impl<'a> Ctx<'a>` belongs to `Ctx`. A trait keeps its
  arguments, so `From<A>` and `From<B>` give two methods (`from@From<A>`,
  `from@From<B>`). Inherent impls for different arguments of one type
  (`impl Foo<A> { fn new }`, `impl Foo<B> { fn new }`) are not told apart: the
  index names both `Foo`.
- **Which type a method belongs to.** The index names an impl's type by its
  name, not its path. A method goes to the type with that name in its own
  module, otherwise to the only one with that name in its crate.
  `impl LocalTrait for other_crate::Foo` in a crate that has a `Foo` of its own
  in another module is therefore shown under the local `Foo`.

`code-guide stats` prints what the index yields: functions, types, calls, and
how many references were left out and why.

## Layers

At every level the reading order comes from calls: a crate is above the crates
it calls into, a module above the modules of its crate it calls into, a public
method of a type above the methods it calls. Layer 0 calls nothing else at
its level. Nodes that call each other share a layer and are marked as a cycle.

## The call graph

The Graph view draws the calls around what is open, one node per function and
one edge per call, with [Cytoscape.js](https://js.cytoscape.org/). The server
sends every function and call once (`/api/calls`); the app takes the part it needs
(`src/lib/callgraph.js`), so changing what is shown needs no request.

- **Centred on what is open**: a function, a method, a type's methods or a
  module's functions. Calls can also be lifted to modules or crates
  ("Show"): a crate's page shows its modules, the page of all crates the crates.
- **Callers, callees or both, to a depth** of 0 to 6 steps. Past 400 nodes the
  farthest are left out, and the app says how many.
- **The functions behind a module's or crate's calls**: at those levels an
  edge stands for many calls. Selecting a node lists, in the panel beside the
  graph, the functions in it that make or receive the calls drawn to and from
  it, busiest first, with how many call sites come in and go out. The plus
  button ("Add X to the graph") adds one of them inside its node, which
  becomes a box: that function's own calls then run through it, while the
  node's other calls stay on the node. The added function is selected, so its
  paths and its source show; the minus button takes it out again, and "Remove
  all" takes out every added function. The graph button ("Open the graph of
  X") goes to that function's own call graph. Selecting never changes the
  graph by itself.
- **Selecting** a node shows its source in the source pane (a module or crate
  shows the top of its file); clearing the selection brings back the page's own.
- **Highlighting**: click a node for the paths between it and what is open;
  Shift-click a second node for the paths between the two. A path is every
  node and call on some route from one to the other, cycles included.
  "All callers" and "All callees" highlight all
  callers or callees of the selection instead. "Hide the rest" hides what is
  not highlighted. These choices appear in the panel beside the graph once a
  node is selected, with a hint there until then; beside it, so that the graph
  does not move between the two clicks of a double-click. The highlighted
  nodes are also listed there as links, nearest first.
- **Layout**: layered left to right (the default) or top to bottom (dagre:
  callers left of or above what they call; a rank too long to read at the opening zoom wraps
  into several rows or columns, and a node that would land inside a box it is
  not part of moves out to the right), force-directed (fcose), or rings around what is
  open; spacing compact, normal or roomy. Nodes can be dragged; "Lay out
  again" puts them back. A new graph opens with everything in view (a small one no larger than 120%); a
  function added from the list is centred at the zoom you were at. "Show
  everything" fits the graph again.
- Double-click a node to centre the graph on it. Nodes are coloured per crate
  and name their crate and module; private functions have a dashed border. A
  legend beside the graph names the crates in it, each as a tag in its colour
  (the design system's tags take the same category colours as the nodes), and
  shows the other marks as a small example graph drawn with the graph's own
  stylesheet (`LegendGraph.vue`): what is open, private, selected, a box, a
  call, a busier call, the arrows of a hovered node, a highlighted path. Its
  accessible description says the same in words.
- A method belongs to the module its `impl` is written in, as on the module
  pages, and is addressed under its type's module, as its page is.

What is shown and how (level, direction, depth, layout, spacing) is in the
address, so a graph can be shared; the selection is not. Direction, layout and
spacing stay as chosen on other pages; level and depth stay while you follow
the graph and return to the page's default when you open another page. The
app fetches the calls again when `/api/status` reports a new `generation`, a
rebuilt model, since node numbers from an older build name other functions.

## The app

`ui/` is a standalone Vite + Vue 3 app (its own `package.json`, not part of the
root npm workspace), built from the NLDD design system like the other
frontends. Addresses hold everything that is open: `#/` (all crates),
`#/engine`, `#/engine/service`, `#/engine/service/LawExecutionService` and
`…/evaluate_law`, with `~` for the crate root and for the `:` of a binary root
(`bin~evaluate`). After `?`: `view=graph` with the graph's options
(`level`, `calls`, `depth`, `layout`, `spacing`), and `source=wide`.

- **Sidebar:** the crate switcher, search over the open crate, and its modules,
  as a tree or in reading order.
- **Middle:** the page for what is open (all crates, a crate, a module, a type,
  a function), or its call graph. Breadcrumbs above the title lead back up
  through every level: the crate, each module of a nested path, the type.
- **Right:** the source of what is selected, read from the working tree;
  "Widen" gives it the middle pane.

The server (`src/serve.rs`) answers `/api/status`, `/api/workspace`,
`/api/crate`, `/api/type`, `/api/function`, `/api/calls` and `/api/source`. The model is built
from the index on the first request and rebuilt when the index is rebuilt or a
source file is changed, added, removed or renamed. `/api/source` takes a path from the URL, so it reads only `.rs` files
inside the workspace (not under `target/`), refuses `..`, absolute and
drive-prefixed paths, and checks again after symlinks are resolved.

**What is not the design system.** The graph is drawn by Cytoscape on a
canvas; every control around it is a design-system component. Cytoscape cannot
read CSS custom properties, so `src/lib/theme.js` resolves the design system's
colour tokens (per crate a category palette, plus content, divider and accent
colours) to plain `rgb()` values and builds the stylesheet from those, again
when the colour scheme changes. `src/graph.css` holds the custom CSS: the
canvas's container gets a height that fits the window, a border and a
background (Cytoscape needs a definite size); the panel beside it
(`.graph-side`) the same height with its own scrolling, since the design
system has no fixed-height scrolling column; and the legend's example graph
(`.graph-legend`) a fixed height for its rows. The toolbars
get English texts for their overflow button, which defaults to Dutch. Two
Cytoscape behaviours are worked around in `CallGraph.vue`, each with a comment:
it caches where its canvas is and never sees the design system's page scroll
(it stops looking at a shadow root), so the cache is dropped before each
pointer event; and the instance is put on its container element, so a test
in a browser (run by hand, not part of the repository) can find the nodes,
which are pixels. The canvas is not keyboard
accessible; the list of highlighted nodes and the Details view carry the same
calls as links.

Doc comments are rendered as Markdown with `marked` and sanitised with
`DOMPurify` (`src/lib/markdown.js`). A rustdoc link to a method of the same type
(`Self::x`) becomes a link in the app; any other Rust path keeps its text and
loses the link.

## Tests

`just code-guide-test` runs both sides.

- Rust (`cargo test -p regelrecht-code-guide`): the cache key and the list of
  changed files; the source scan (extents, every form of test code, `use`
  items, module paths); the model on synthetic SCIP indexes (every symbol shape,
  calls placed by extent, test code, imports and nested items left out,
  ambiguous symbols, changed files, generics); the layering (including a 20,000-node
  chain); and the source endpoint's path checks.
- UI (`vitest`): the pure logic in `src/lib/` (addresses, search, source
  ranges, the call graph's levels, neighbourhoods, paths and highlighting, the
  Markdown rendering, adding functions to a module graph, wrapping long ranks), every layout run in
  headless Cytoscape (force-directed without boxes: headless, fcose fails on a
  compound node, so that case was checked by hand in a browser), and the whole
  app against a mocked API with the canvas stubbed (`src/App.test.js`). The Markdown tests run under
  jsdom, because happy-dom 20 has a `NodeIterator` bug that makes DOMPurify drop
  elements.

None of these needs rust-analyzer or an index.
