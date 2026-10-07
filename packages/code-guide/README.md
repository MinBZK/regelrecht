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

On a type, the graph shows the methods that can be called from outside it:
`pub`, `pub(crate)` and the like, and trait implementations. Private methods
are helpers and are folded away: `a` calling private `h` calling `b` is the
edge `a -> b`, drawn dashed and labelled with the helpers (the shortest chain).
The modules outside the type that call its methods are drawn above it.

## The app

`ui/` is a standalone Vite + Vue 3 app (its own `package.json`, not part of the
root npm workspace), built from the NLDD design system like the other
frontends. Addresses hold everything that is open: `#/` (all crates),
`#/engine`, `#/engine/service`, `#/engine/service/LawExecutionService` and
`…/evaluate_law`, with `~` for the crate root and for the `:` of a binary root
(`bin~evaluate`). After `?`: `view=graph`, `scope=…` and `source=wide`.

- **Sidebar:** the crate switcher, search over the open crate, and its modules,
  as a tree or in reading order.
- **Middle:** the page for what is open (all crates, a crate, a module, a type,
  a function), or its graph. A button in the title bar leads one level up.
- **Right:** the source of what is selected, read from the working tree;
  "Widen" gives it the middle pane.

The server (`src/serve.rs`) answers `/api/status`, `/api/workspace`,
`/api/crate`, `/api/type`, `/api/function` and `/api/source`. The model is built
from the index on the first request and rebuilt when the index is rebuilt or a
source file is changed, added, removed or renamed. `/api/source` takes a path from the URL, so it reads only `.rs` files
inside the workspace (not under `target/`), refuses `..`, absolute and
drive-prefixed paths, and checks again after symlinks are resolved.

**What is not the design system.** The graph canvas is Vue Flow; its nodes are
`nldd-card`s with an `href`, so a click, the keyboard and "open in a new tab"
work as ordinary links. `src/graph.css` imports Vue Flow's stylesheets, gives the
canvas an explicit height (a section's height is not definite, and Vue Flow
draws nothing into a box without one) and switches off Vue Flow's own node box.
Node width is set inline: enough for the longest label, between 180 and 360px.
A call through helpers gets an inline `stroke-dasharray`. Each method entry has
an inline `scroll-margin-top`, so one that is scrolled to is not hidden under
the sticky header. Two Vue Flow behaviours are worked around in
`GraphPane.vue`, each with a comment: its fit runs before the cards have a size,
and it turns pointer events off on nodes nobody listens to.

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
  ranges, the graph builders and layout, the Markdown rendering) and the whole
  app against a mocked API (`src/App.test.js`). The Markdown tests run under
  jsdom, because happy-dom 20 has a `NodeIterator` bug that makes DOMPurify drop
  elements.

None of these needs rust-analyzer or an index.
