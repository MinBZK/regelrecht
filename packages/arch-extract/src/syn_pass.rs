//! Tier-1b extraction: source-level structure via `syn`.
//!
//! For every workspace crate we walk `src/**.rs`, derive each file's module
//! path from its location, and parse it with `syn` (pinned stable — no nightly
//! rustdoc-JSON needed). We emit module / struct / enum / trait / method / fn
//! nodes (with the first line of their doc-comment) plus `impl` and `uses`
//! edges, all scoped inside one crate.
//!
//! This is deliberately a *structural* pass, not a name-resolution pass: syn
//! sees one file at a time with no type inference, so cross-crate and macro-
//! generated items are invisible and `impl`/`uses` edges are resolved
//! best-effort by matching a type's leaf identifier against the crate's own
//! type nodes. That is plenty for an architecture map at crate/module/type/
//! method granularity, and it is why the README documents this over rustdoc.

use std::collections::HashMap;
use std::path::Path;

use syn::visit::Visit;

use crate::crate_graph::{rel, CrateInfo};
use crate::model::{Edge, EdgeKind, Kind, Level, Node};

/// Everything harvested from a single crate before edge resolution.
struct CrateItems {
    nodes: Vec<Node>,
    /// Pending `impl Trait for Type`: (type leaf, trait leaf).
    trait_impls: Vec<(String, String)>,
    /// Pending `uses`: (owner type node id, referenced type leaf).
    type_refs: Vec<(String, String)>,
    /// Type leaf name -> node id, for same-crate edge resolution. When a name
    /// is ambiguous (defined twice) it is dropped to avoid guessing wrong.
    type_by_name: HashMap<String, Option<String>>,
}

/// Extracts nodes and edges for one crate, appending onto `nodes`/`edges`.
pub fn extract_crate(
    repo_root: &Path,
    krate: &CrateInfo,
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) {
    let src = krate.dir.join("src");
    if !src.is_dir() {
        return;
    }

    let mut items = CrateItems {
        nodes: Vec::new(),
        trait_impls: Vec::new(),
        type_refs: Vec::new(),
        type_by_name: HashMap::new(),
    };

    // Ensure every ancestor module node exists exactly once (id → node index).
    let mut seen_modules: HashMap<String, usize> = HashMap::new();

    let mut files: Vec<_> = walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        .map(walkdir::DirEntry::into_path)
        .collect();
    files.sort();

    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(ast) = syn::parse_file(&source) else {
            eprintln!("arch-extract: skipping unparseable file {}", file.display());
            continue;
        };

        let Some(base) = base_module(&src, file) else {
            continue;
        };

        // The container the file's top-level items hang from, and the module
        // path segments that prefix their ids.
        let (parent_id, mod_path) = match &base {
            FileRole::CrateRoot => (krate.node_id.clone(), Vec::new()),
            FileRole::Binary(name) => {
                let id = format!("bin:{}::{name}", krate.short);
                items.nodes.push(Node {
                    id: id.clone(),
                    level: Level::Container,
                    kind: Kind::Binary,
                    lang: "rust".to_string(),
                    name: name.clone(),
                    path: rel(repo_root, file),
                    parent: Some(krate.node_id.clone()),
                    doc: first_doc(&ast.attrs),
                });
                (id, Vec::new())
            }
            FileRole::Module(segs) => {
                let id = ensure_module_chain(
                    krate,
                    repo_root,
                    file,
                    segs,
                    first_doc(&ast.attrs),
                    &mut items.nodes,
                    &mut seen_modules,
                );
                (id, segs.clone())
            }
        };

        let mut visitor = ItemVisitor {
            krate,
            repo_root,
            file,
            mod_path,
            parent_id,
            items: &mut items,
            seen_modules: &mut seen_modules,
        };
        for item in &ast.items {
            visitor.visit_item(item);
        }
    }

    // Reconcile method parents: a method's parent id is built from the impl's
    // own module prefix, but the type may live in another file of the crate.
    // Repoint to the real type node when its leaf name resolves unambiguously.
    let type_ids: std::collections::HashSet<String> = items
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, Kind::Struct | Kind::Enum | Kind::Trait))
        .map(|n| n.id.clone())
        .collect();
    for n in &mut items.nodes {
        if n.kind != Kind::Method {
            continue;
        }
        let Some(parent) = &n.parent else { continue };
        if type_ids.contains(parent) {
            continue;
        }
        if let Some(leaf) = parent.rsplit("::").next() {
            if let Some(Some(real)) = items.type_by_name.get(leaf) {
                n.parent = Some(real.clone());
            }
        }
    }

    // Resolve best-effort edges now that every type node in the crate is known.
    for (ty_leaf, tr_leaf) in &items.trait_impls {
        if let (Some(Some(from)), Some(Some(to))) = (
            items.type_by_name.get(ty_leaf),
            items.type_by_name.get(tr_leaf),
        ) {
            edges.push(Edge {
                from: from.clone(),
                to: to.clone(),
                kind: EdgeKind::Impl,
            });
        }
    }
    for (owner_id, ref_leaf) in &items.type_refs {
        if let Some(Some(to)) = items.type_by_name.get(ref_leaf) {
            if to != owner_id {
                edges.push(Edge {
                    from: owner_id.clone(),
                    to: to.clone(),
                    kind: EdgeKind::Uses,
                });
            }
        }
    }

    nodes.append(&mut items.nodes);
}

/// What a source file represents in the module tree.
enum FileRole {
    CrateRoot,
    Binary(String),
    Module(Vec<String>),
}

/// Maps a file path (relative to the crate `src/`) to its role/module path.
fn base_module(src: &Path, file: &Path) -> Option<FileRole> {
    let relp = file.strip_prefix(src).ok()?;
    let comps: Vec<String> = relp
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect();
    let (dirs, filename) = comps.split_at(comps.len().saturating_sub(1));
    let filename = filename.first()?.as_str();
    let stem = filename.strip_suffix(".rs")?;

    // `src/bin/<name>.rs` is a separate binary target.
    if dirs.first().map(String::as_str) == Some("bin") && dirs.len() == 1 {
        return Some(FileRole::Binary(stem.to_string()));
    }

    let mut segs: Vec<String> = dirs.to_vec();
    match stem {
        // Crate/module roots.
        "lib" | "main" if dirs.is_empty() => return Some(FileRole::CrateRoot),
        "mod" => { /* `foo/mod.rs` → module `foo` (dirs already hold it). */ }
        other => segs.push(other.to_string()),
    }
    if segs.is_empty() {
        Some(FileRole::CrateRoot)
    } else {
        Some(FileRole::Module(segs))
    }
}

/// Emits a module node for `segs` and any missing ancestors, returning the id
/// of the deepest one. Idempotent via `seen` (id → index into `nodes`). When a
/// module is first seen as an ancestor (empty path) and later visited as the
/// file that actually defines it, its path/doc are backfilled — so the node
/// points at the module's own source, not wherever it was first referenced.
fn ensure_module_chain(
    krate: &CrateInfo,
    repo_root: &Path,
    file: &Path,
    segs: &[String],
    doc: Option<String>,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<String, usize>,
) -> String {
    let mut parent = krate.node_id.clone();
    let mut acc: Vec<String> = Vec::new();
    let last = segs.len().saturating_sub(1);
    for (i, seg) in segs.iter().enumerate() {
        acc.push(seg.clone());
        let id = format!("mod:{}::{}", krate.short, acc.join("::"));
        // Only the deepest segment of this call is being *defined* here; the
        // ancestors are just containment and get their real path when visited.
        let defining = i == last;
        let this_path = if defining {
            rel(repo_root, file)
        } else {
            String::new()
        };
        match seen.get(&id) {
            Some(&idx) => {
                if defining && nodes[idx].path.is_empty() {
                    nodes[idx].path = this_path;
                    nodes[idx].doc = doc.clone();
                }
            }
            None => {
                let idx = nodes.len();
                nodes.push(Node {
                    id: id.clone(),
                    level: Level::Component,
                    kind: Kind::Module,
                    lang: "rust".to_string(),
                    name: seg.clone(),
                    path: this_path,
                    parent: Some(parent.clone()),
                    doc: if defining { doc.clone() } else { None },
                });
                seen.insert(id.clone(), idx);
            }
        }
        parent = id;
    }
    parent
}

/// Walks the items of one file, tracking the current module path so nested
/// `mod x { .. }` blocks and their contents get correct ids and parents.
struct ItemVisitor<'a> {
    krate: &'a CrateInfo,
    repo_root: &'a Path,
    file: &'a Path,
    mod_path: Vec<String>,
    parent_id: String,
    items: &'a mut CrateItems,
    seen_modules: &'a mut HashMap<String, usize>,
}

impl ItemVisitor<'_> {
    /// `crate::a::b` label for the current module, used to build ids.
    fn path_prefix(&self) -> String {
        if self.mod_path.is_empty() {
            self.krate.short.clone()
        } else {
            format!("{}::{}", self.krate.short, self.mod_path.join("::"))
        }
    }

    fn record_type(&mut self, name: &str, id: &str) {
        self.items
            .type_by_name
            .entry(name.to_string())
            .and_modify(|slot| *slot = None) // ambiguous → don't resolve edges to it
            .or_insert_with(|| Some(id.to_string()));
    }
}

impl<'ast> Visit<'ast> for ItemVisitor<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if is_test_gated(&node.attrs) {
            return;
        }
        // A bodyless `mod foo;` is only a declaration; the file `foo.rs` /
        // `foo/mod.rs` is walked separately and defines the node with its real
        // path. Skip the declaration so the node points at the definition.
        let Some((_, items)) = &node.content else {
            return;
        };

        let name = node.ident.to_string();
        let mut child_path = self.mod_path.clone();
        child_path.push(name.clone());

        let id = ensure_module_chain(
            self.krate,
            self.repo_root,
            self.file,
            &child_path,
            first_doc(&node.attrs),
            &mut self.items.nodes,
            self.seen_modules,
        );

        let saved_path = std::mem::replace(&mut self.mod_path, child_path);
        let saved_parent = std::mem::replace(&mut self.parent_id, id);
        for item in items {
            self.visit_item(item);
        }
        self.mod_path = saved_path;
        self.parent_id = saved_parent;
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.ident.to_string();
        let id = format!("type:{}::{name}", self.path_prefix());
        self.record_type(&name, &id);
        self.items.nodes.push(Node {
            id: id.clone(),
            level: Level::Component,
            kind: Kind::Struct,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
        // `uses`: leaf type of each field, resolved to same-crate types later.
        for field in &node.fields {
            for leaf in type_leaves(&field.ty) {
                self.items.type_refs.push((id.clone(), leaf));
            }
        }
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.ident.to_string();
        let id = format!("type:{}::{name}", self.path_prefix());
        self.record_type(&name, &id);
        self.items.nodes.push(Node {
            id,
            level: Level::Component,
            kind: Kind::Enum,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.ident.to_string();
        let id = format!("type:{}::{name}", self.path_prefix());
        self.record_type(&name, &id);
        self.items.nodes.push(Node {
            id: id.clone(),
            level: Level::Component,
            kind: Kind::Trait,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
        // Trait methods with a default or a signature become code-level nodes.
        for item in &node.items {
            if let syn::TraitItem::Fn(f) = item {
                let m = f.sig.ident.to_string();
                self.items.nodes.push(Node {
                    id: format!("fn:{}::{m}", trim_type_prefix(&id)),
                    level: Level::Code,
                    kind: Kind::Method,
                    lang: "rust".to_string(),
                    name: m,
                    path: rel(self.repo_root, self.file),
                    parent: Some(id.clone()),
                    doc: first_doc(&f.attrs),
                });
            }
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let name = node.sig.ident.to_string();
        self.items.nodes.push(Node {
            id: format!("fn:{}::{name}", self.path_prefix()),
            level: Level::Code,
            kind: Kind::Fn,
            lang: "rust".to_string(),
            name,
            path: rel(self.repo_root, self.file),
            parent: Some(self.parent_id.clone()),
            doc: first_doc(&node.attrs),
        });
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if is_test_gated(&node.attrs) {
            return;
        }
        let Some(ty_leaf) = type_leaves(&node.self_ty).into_iter().next() else {
            return;
        };
        // `impl Trait for Type` → pending impl edge.
        if let Some((_, path, _)) = &node.trait_ {
            if let Some(tr_leaf) = path_leaf(path) {
                self.items.trait_impls.push((ty_leaf.clone(), tr_leaf));
            }
        }
        // Methods hang off the type node id (resolved lazily by consumers via
        // the parent chain; we build the id from the impl's own type label so
        // it is stable even if the type lives in another file of this crate).
        let type_id = format!("type:{}::{ty_leaf}", self.path_prefix());
        for item in &node.items {
            if let syn::ImplItem::Fn(f) = item {
                let m = f.sig.ident.to_string();
                self.items.nodes.push(Node {
                    id: format!("fn:{}::{ty_leaf}::{m}", self.path_prefix()),
                    level: Level::Code,
                    kind: Kind::Method,
                    lang: "rust".to_string(),
                    name: m,
                    path: rel(self.repo_root, self.file),
                    parent: Some(type_id.clone()),
                    doc: first_doc(&f.attrs),
                });
            }
        }
    }
}

/// `type:a::b::Foo` → `a::b::Foo`, for composing method ids.
fn trim_type_prefix(type_id: &str) -> String {
    type_id.strip_prefix("type:").unwrap_or(type_id).to_string()
}

/// Leaf identifier of a path (its last segment), e.g. `foo::Bar` → `Bar`.
fn path_leaf(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|s| s.ident.to_string())
}

/// Collects the leaf identifiers referenced by a type, recursing through
/// generic arguments (so `Option<Vec<Foo>>` yields `Option`, `Vec`, `Foo`).
/// Best-effort and intentionally shallow — enough for same-crate `uses` edges.
fn type_leaves(ty: &syn::Type) -> Vec<String> {
    let mut out = Vec::new();
    collect_type_leaves(ty, &mut out);
    out
}

fn collect_type_leaves(ty: &syn::Type, out: &mut Vec<String>) {
    match ty {
        syn::Type::Path(tp) => {
            if let Some(seg) = tp.path.segments.last() {
                out.push(seg.ident.to_string());
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            collect_type_leaves(inner, out);
                        }
                    }
                }
            }
        }
        syn::Type::Reference(r) => collect_type_leaves(&r.elem, out),
        syn::Type::Slice(s) => collect_type_leaves(&s.elem, out),
        syn::Type::Array(a) => collect_type_leaves(&a.elem, out),
        syn::Type::Paren(p) => collect_type_leaves(&p.elem, out),
        syn::Type::Group(g) => collect_type_leaves(&g.elem, out),
        syn::Type::Tuple(t) => {
            for e in &t.elems {
                collect_type_leaves(e, out);
            }
        }
        _ => {}
    }
}

/// True for items gated to the test build (`#[cfg(test)]`) or test functions
/// (`#[test]`). Test scaffolding is not architecture, so it is skipped.
fn is_test_gated(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("test") {
            return true;
        }
        if attr.path().is_ident("cfg") {
            if let syn::Meta::List(list) = &attr.meta {
                if tokens_contain_test(list.tokens.clone()) {
                    return true;
                }
            }
        }
    }
    false
}

/// Recursively checks a token stream for a standalone `test` identifier, so
/// `cfg(test)` and `cfg(all(test, …))` match but `cfg(feature = "test-utils")`
/// does not.
fn tokens_contain_test(tokens: proc_macro2::TokenStream) -> bool {
    for tok in tokens {
        match tok {
            proc_macro2::TokenTree::Ident(id) if id == "test" => return true,
            proc_macro2::TokenTree::Group(g) if tokens_contain_test(g.stream()) => return true,
            _ => {}
        }
    }
    false
}

/// First non-empty line of an item's `///` / `//!` doc-comment, trimmed. This
/// is the seed for the prose layer and keeps `model.json` compact.
fn first_doc(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &attr.meta {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            {
                let line = s.value().trim().to_string();
                if !line.is_empty() {
                    return Some(line);
                }
            }
        }
    }
    None
}
