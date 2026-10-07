//! What each page of the app gets, built from the [`Model`].
//!
//! Every relation here is a call, as rust-analyzer resolved it: a crate builds
//! on another when its functions call into it, a module likewise, and a method
//! on another when it calls it. Layers (the reading order) are computed from
//! those calls at each level.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use crate::layers::{layer_of, layers};
use crate::model::{crate_calls, deps_of, module_calls, Function, Model, Place, TypeKind, Vis};

/// The model plus the lookups every view needs.
pub struct Views<'m> {
    pub m: &'m Model,
    /// For each function, the type it belongs to (an index into `m.types`).
    owner_type: Vec<Option<usize>>,
    callers: Vec<Vec<(usize, usize)>>,
    callees: Vec<Vec<(usize, usize)>>,
    module_edges: BTreeMap<(String, String), BTreeMap<(String, String), usize>>,
}

/// A link to a function, enough for the app to address it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FnRef {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    /// The type the function belongs to, when the guide knows it.
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    /// The method key within its type (`fmt@Display`), or the function name.
    pub key: String,
    pub name: String,
    pub sites: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrateRow {
    pub name: String,
    pub description: Option<String>,
    pub layer: usize,
    pub cycle_with: Vec<String>,
    pub modules: usize,
    pub types: usize,
    pub functions: usize,
}

#[derive(Debug, Serialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub calls: usize,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceView {
    pub crates: Vec<CrateRow>,
    pub edges: Vec<Edge>,
    pub layers: Vec<Vec<Vec<String>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleLink {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    pub calls: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRow {
    pub name: String,
    pub kind: TypeKind,
    pub vis: Vis,
    pub doc: Option<String>,
    pub methods: usize,
    /// Distinct functions outside the type that call one of its methods.
    pub callers: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FnRow {
    pub key: String,
    pub name: String,
    pub vis: Vis,
    pub doc: Option<String>,
    pub callers: usize,
    pub stale: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRow {
    pub path: String,
    pub doc: Option<String>,
    pub docs: Option<String>,
    pub vis: Vis,
    pub file: Option<String>,
    pub layer: usize,
    pub cycle_with: Vec<String>,
    pub calls_into: Vec<ModuleLink>,
    pub called_from: Vec<ModuleLink>,
    pub types: Vec<TypeRow>,
    pub functions: Vec<FnRow>,
}

/// One searchable thing in a crate.
#[derive(Debug, Serialize)]
pub struct SearchEntry {
    pub kind: &'static str,
    pub module: String,
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub key: Option<String>,
    pub name: String,
    pub doc: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrateView {
    pub name: String,
    pub dir: String,
    pub description: Option<String>,
    pub docs: Option<String>,
    pub calls_into: Vec<ModuleLink>,
    pub called_from: Vec<ModuleLink>,
    pub layers: Vec<Vec<Vec<String>>>,
    pub modules: Vec<ModuleRow>,
    pub search: Vec<SearchEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FnDetail {
    pub key: String,
    pub name: String,
    #[serde(rename = "trait")]
    pub trait_name: Option<String>,
    pub vis: Vis,
    pub signature: String,
    pub doc: Option<String>,
    pub docs: Option<String>,
    pub place: Place,
    pub extent: Option<(usize, usize)>,
    pub stale: bool,
    pub callers: Vec<FnRef>,
    pub callees: Vec<FnRef>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeView {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    pub name: String,
    pub kind: TypeKind,
    pub vis: Vis,
    pub signature: String,
    pub doc: Option<String>,
    pub docs: Option<String>,
    pub place: Place,
    pub methods: Vec<FnDetail>,
}

/// One function in the call graph: where the app finds its page, and what to
/// show on its node.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CallNode {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    /// The type it belongs to, when the guide knows it.
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    /// Its address within the module or type (see [`FnRef::key`]).
    pub key: String,
    /// The module whose source defines it. For a method this can differ from
    /// `module` (its type's module, part of the address): an `impl` may sit in
    /// another file. Calls between modules are counted by this one.
    pub impl_module: String,
    pub name: String,
    pub vis: Vis,
    pub doc: Option<String>,
    pub stale: bool,
    /// Where it is defined, and its lines (doc comment through closing brace)
    /// when the source scan has them: what the source viewer shows for it.
    pub place: Place,
    pub extent: Option<(usize, usize)>,
}

/// The file a module's source starts in, for the source viewer.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ModuleFile {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    pub file: String,
}

/// Every function of the workspace and every call between them. The app picks
/// the part around what is open itself, so following callers and callees,
/// changing the depth or highlighting a path needs no further request.
#[derive(Debug, Serialize)]
pub struct CallGraph {
    pub nodes: Vec<CallNode>,
    /// `[caller, callee, call sites]`, indexes into `nodes`.
    pub edges: Vec<(usize, usize, usize)>,
    /// Modules with a file of their own (an inline `mod x { .. }` has none).
    pub modules: Vec<ModuleFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionView {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    #[serde(flatten)]
    pub detail: FnDetail,
}

fn first_line(docs: &Option<String>) -> Option<String> {
    docs.as_deref()?
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

/// The key a method is addressed by within its type: its name, with `@Trait`
/// for a trait implementation's method, so `fmt@Display` and `fmt@Debug` differ.
pub fn method_key(f: &Function) -> String {
    match &f.trait_name {
        Some(t) => format!("{}@{t}", f.name),
        None => f.name.clone(),
    }
}

impl<'m> Views<'m> {
    pub fn new(m: &'m Model) -> Self {
        // A method belongs to the type of its crate with the owner's name: the
        // one in its own module if there is one there, else the only one.
        let mut by_name: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
        for (i, t) in m.types.iter().enumerate() {
            by_name
                .entry((t.krate.as_str(), t.name.as_str()))
                .or_default()
                .push(i);
        }
        let owner_type = m
            .functions
            .iter()
            .map(|f| {
                let owner = f.owner.as_deref()?;
                let candidates = by_name.get(&(f.krate.as_str(), owner))?;
                candidates
                    .iter()
                    .copied()
                    .find(|&i| m.types[i].module == f.module)
                    .or_else(|| (candidates.len() == 1).then(|| candidates[0]))
            })
            .collect();
        let mut callers = vec![Vec::new(); m.functions.len()];
        let mut callees = vec![Vec::new(); m.functions.len()];
        for (&(a, b), &n) in &m.calls {
            callees[a].push((b, n));
            callers[b].push((a, n));
        }
        Views {
            m,
            owner_type,
            callers,
            callees,
            module_edges: module_calls(m),
        }
    }

    fn fn_ref(&self, i: usize, sites: usize) -> FnRef {
        let f = &self.m.functions[i];
        match self.owner_type[i] {
            Some(t) => {
                let ty = &self.m.types[t];
                FnRef {
                    krate: ty.krate.clone(),
                    module: ty.module.clone(),
                    type_name: Some(ty.name.clone()),
                    key: method_key(f),
                    name: f.name.clone(),
                    sites,
                }
            }
            None => FnRef {
                krate: f.krate.clone(),
                module: f.module.clone(),
                type_name: None,
                key: free_key(f),
                name: free_key(f),
                sites,
            },
        }
    }

    fn detail(&self, i: usize) -> FnDetail {
        let f = &self.m.functions[i];
        let refs = |list: &[(usize, usize)]| {
            let mut v: Vec<FnRef> = list.iter().map(|&(j, n)| self.fn_ref(j, n)).collect();
            v.sort();
            v
        };
        FnDetail {
            key: if self.owner_type[i].is_some() {
                method_key(f)
            } else {
                free_key(f)
            },
            name: f.name.clone(),
            trait_name: f.trait_name.clone(),
            vis: f.vis,
            signature: f.signature.clone(),
            doc: first_line(&f.docs),
            docs: f.docs.clone(),
            place: f.place.clone(),
            extent: f.extent,
            stale: f.stale,
            callers: refs(&self.callers[i]),
            callees: refs(&self.callees[i]),
        }
    }

    /// The crates, layered by which crate calls into which.
    pub fn workspace(&self) -> WorkspaceView {
        let edges_map = crate_calls(self.m);
        let names: Vec<String> = self.m.crates.iter().map(|c| c.name.clone()).collect();
        let deps = deps_of(
            names.clone(),
            edges_map
                .iter()
                .flat_map(|(a, tos)| tos.keys().map(move |b| (a.clone(), b.clone()))),
        );
        let ls = layers(&deps);
        let layer = layer_of(&ls);
        let cycle = cycles(&ls);
        let crates = self
            .m
            .crates
            .iter()
            .map(|c| CrateRow {
                name: c.name.clone(),
                description: c.description.clone(),
                layer: layer.get(&c.name).copied().unwrap_or(0),
                cycle_with: cycle.get(&c.name).cloned().unwrap_or_default(),
                modules: self.m.modules.keys().filter(|(k, _)| *k == c.name).count(),
                types: self.m.types.iter().filter(|t| t.krate == c.name).count(),
                functions: self
                    .m
                    .functions
                    .iter()
                    .filter(|f| f.krate == c.name)
                    .count(),
            })
            .collect();
        let edges = edges_map
            .into_iter()
            .flat_map(|(a, tos)| {
                tos.into_iter().map(move |(b, n)| Edge {
                    from: a.clone(),
                    to: b,
                    calls: n,
                })
            })
            .collect();
        WorkspaceView {
            crates,
            edges,
            layers: ls,
        }
    }

    /// One crate: its modules layered by the calls between them.
    pub fn krate(&self, name: &str) -> Option<CrateView> {
        let c = self.m.crates.iter().find(|c| c.name == name)?;
        let paths: Vec<String> = self
            .m
            .modules
            .keys()
            .filter(|(k, _)| k == name)
            .map(|(_, p)| p.clone())
            .collect();
        let deps = deps_of(
            paths.clone(),
            self.module_edges.iter().flat_map(|((ka, a), tos)| {
                tos.keys()
                    .filter(move |(kb, _)| ka == name && kb == name)
                    .map(move |(_, b)| (a.clone(), b.clone()))
            }),
        );
        let ls = layers(&deps);
        let layer = layer_of(&ls);
        let cycle = cycles(&ls);

        let mut modules = Vec::new();
        let mut search = Vec::new();
        for path in &paths {
            let Some(module) = self.m.modules.get(&(name.to_string(), path.clone())) else {
                continue;
            };
            let key = (name.to_string(), path.clone());
            let calls_into = self.links(self.module_edges.get(&key));
            let called_from = self.links_in(&key);
            let types: Vec<TypeRow> = self
                .m
                .types
                .iter()
                .enumerate()
                .filter(|(_, t)| t.krate == name && &t.module == path)
                .map(|(ti, t)| {
                    let methods: Vec<usize> = self.methods_of(ti);
                    let outside: BTreeSet<usize> = methods
                        .iter()
                        .flat_map(|&mi| self.callers[mi].iter().map(|&(c, _)| c))
                        .filter(|&c| self.owner_type[c] != Some(ti))
                        .collect();
                    search.push(SearchEntry {
                        kind: kind_name(t.kind),
                        module: path.clone(),
                        type_name: None,
                        key: None,
                        name: t.name.clone(),
                        doc: first_line(&t.docs),
                    });
                    for &mi in &methods {
                        let f = &self.m.functions[mi];
                        search.push(SearchEntry {
                            kind: "method",
                            module: path.clone(),
                            type_name: Some(t.name.clone()),
                            key: Some(method_key(f)),
                            name: format!("{}::{}", t.name, f.name),
                            doc: first_line(&f.docs),
                        });
                    }
                    TypeRow {
                        name: t.name.clone(),
                        kind: t.kind,
                        vis: t.vis,
                        doc: first_line(&t.docs),
                        methods: methods.len(),
                        callers: outside.len(),
                    }
                })
                .collect();
            let functions: Vec<FnRow> = self
                .m
                .functions
                .iter()
                .enumerate()
                .filter(|(i, f)| {
                    f.krate == name && &f.module == path && self.owner_type[*i].is_none()
                })
                .map(|(i, f)| {
                    search.push(SearchEntry {
                        kind: "fn",
                        module: path.clone(),
                        type_name: None,
                        key: Some(free_key(f)),
                        name: free_key(f),
                        doc: first_line(&f.docs),
                    });
                    FnRow {
                        key: free_key(f),
                        name: free_key(f),
                        vis: f.vis,
                        doc: first_line(&f.docs),
                        callers: self.callers[i].len(),
                        stale: f.stale,
                    }
                })
                .collect();
            search.push(SearchEntry {
                kind: "module",
                module: path.clone(),
                type_name: None,
                key: None,
                name: if path.is_empty() {
                    format!("{name} (crate root)")
                } else {
                    path.clone()
                },
                doc: first_line(&module.docs),
            });
            modules.push(ModuleRow {
                path: path.clone(),
                doc: first_line(&module.docs),
                docs: module.docs.clone(),
                vis: module.vis,
                file: module.file.clone(),
                layer: layer.get(path).copied().unwrap_or(0),
                cycle_with: cycle.get(path).cloned().unwrap_or_default(),
                calls_into,
                called_from,
                types,
                functions,
            });
        }

        // The crate's own calls into, and from, other crates.
        let mut into: BTreeMap<(String, String), usize> = BTreeMap::new();
        let mut from: BTreeMap<(String, String), usize> = BTreeMap::new();
        for ((ka, a), tos) in &self.module_edges {
            for ((kb, b), n) in tos {
                if ka == name && kb != name {
                    *into.entry((kb.clone(), b.clone())).or_insert(0) += n;
                } else if kb == name && ka != name {
                    *from.entry((ka.clone(), a.clone())).or_insert(0) += n;
                }
            }
        }
        Some(CrateView {
            name: c.name.clone(),
            dir: c.dir.clone(),
            description: c.description.clone(),
            docs: self.m.crate_docs.get(name).cloned(),
            calls_into: self.links(Some(&into)),
            called_from: self.links(Some(&from)),
            layers: ls,
            modules,
            search,
        })
    }

    fn links(&self, m: Option<&BTreeMap<(String, String), usize>>) -> Vec<ModuleLink> {
        let mut v: Vec<ModuleLink> = m
            .into_iter()
            .flatten()
            .map(|((k, p), &n)| ModuleLink {
                krate: k.clone(),
                module: p.clone(),
                calls: n,
            })
            .collect();
        v.sort_by(|a, b| {
            b.calls
                .cmp(&a.calls)
                .then((&a.krate, &a.module).cmp(&(&b.krate, &b.module)))
        });
        v
    }

    fn links_in(&self, key: &(String, String)) -> Vec<ModuleLink> {
        let incoming: BTreeMap<(String, String), usize> = self
            .module_edges
            .iter()
            .filter_map(|(from, tos)| tos.get(key).map(|&n| (from.clone(), n)))
            .collect();
        self.links(Some(&incoming))
    }

    /// The functions that belong to type `ti`, by name.
    fn methods_of(&self, ti: usize) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.m.functions.len())
            .filter(|&i| self.owner_type[i] == Some(ti))
            .collect();
        v.sort_by(|&a, &b| method_key(&self.m.functions[a]).cmp(&method_key(&self.m.functions[b])));
        v
    }

    /// One type: its methods with their callers and callees.
    pub fn type_view(&self, krate: &str, module: &str, name: &str) -> Option<TypeView> {
        let ti = self
            .m
            .types
            .iter()
            .position(|t| t.krate == krate && t.module == module && t.name == name)?;
        let t = &self.m.types[ti];
        let methods = self.methods_of(ti);
        Some(TypeView {
            krate: t.krate.clone(),
            module: t.module.clone(),
            name: t.name.clone(),
            kind: t.kind,
            vis: t.vis,
            signature: t.signature.clone(),
            doc: first_line(&t.docs),
            docs: t.docs.clone(),
            place: t.place.clone(),
            methods: methods.iter().map(|&i| self.detail(i)).collect(),
        })
    }

    /// The whole call graph, in the model's order: node `i` is function `i`.
    pub fn call_graph(&self) -> CallGraph {
        let nodes = (0..self.m.functions.len())
            .map(|i| {
                let f = &self.m.functions[i];
                let r = self.fn_ref(i, 0);
                CallNode {
                    krate: r.krate,
                    module: r.module,
                    type_name: r.type_name,
                    key: r.key,
                    impl_module: f.module.clone(),
                    name: f.name.clone(),
                    vis: f.vis,
                    doc: first_line(&f.docs),
                    stale: f.stale,
                    place: f.place.clone(),
                    extent: f.extent,
                }
            })
            .collect();
        let edges = self.m.calls.iter().map(|(&(a, b), &n)| (a, b, n)).collect();
        let modules = self
            .m
            .modules
            .values()
            .filter_map(|md| {
                Some(ModuleFile {
                    krate: md.krate.clone(),
                    module: md.path.clone(),
                    file: md.file.clone()?,
                })
            })
            .collect();
        CallGraph {
            nodes,
            edges,
            modules,
        }
    }

    /// One free function (or a method whose type the guide does not know).
    pub fn function_view(&self, krate: &str, module: &str, key: &str) -> Option<FunctionView> {
        let i = (0..self.m.functions.len()).find(|&i| {
            let f = &self.m.functions[i];
            f.krate == krate
                && f.module == module
                && self.owner_type[i].is_none()
                && free_key(f) == key
        })?;
        Some(FunctionView {
            krate: krate.to_string(),
            module: module.to_string(),
            detail: self.detail(i),
        })
    }
}

/// How a function without a known type is addressed: its name, or
/// `Owner::name` for a method on a type the guide does not have (an impl on a
/// type from another crate).
fn free_key(f: &Function) -> String {
    match (&f.owner, &f.trait_name) {
        (Some(o), Some(t)) => format!("{o}::{}@{t}", f.name),
        (Some(o), None) => format!("{o}::{}", f.name),
        _ => f.name.clone(),
    }
}

fn kind_name(k: TypeKind) -> &'static str {
    match k {
        TypeKind::Struct => "struct",
        TypeKind::Enum => "enum",
        TypeKind::Trait => "trait",
        TypeKind::Union => "union",
        TypeKind::Alias => "type",
    }
}

/// For each node in a cycle, the others in it.
fn cycles(ls: &[Vec<Vec<String>>]) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for comp in ls.iter().flatten().filter(|c| c.len() > 1) {
        for n in comp {
            out.insert(
                n.clone(),
                comp.iter().filter(|o| *o != n).cloned().collect(),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Function, TypeItem};

    fn place() -> Place {
        Place {
            file: "x/src/lib.rs".to_string(),
            line: 1,
        }
    }

    fn function(owner: Option<&str>, name: &str, vis: Vis, docs: Option<&str>) -> Function {
        Function {
            id: format!("x::m::{name}"),
            krate: "x".to_string(),
            module: "m".to_string(),
            owner: owner.map(str::to_string),
            trait_name: None,
            name: name.to_string(),
            signature: format!("fn {name}()"),
            docs: docs.map(str::to_string),
            vis,
            place: place(),
            stale: false,
            extent: None,
        }
    }

    #[test]
    fn a_method_implemented_in_another_module_keeps_both_modules() {
        let mut impl_elsewhere = function(Some("S"), "extra", Vis::Pub, None);
        impl_elsewhere.module = "m::more".to_string();
        let m = Model {
            types: vec![TypeItem {
                krate: "x".to_string(),
                module: "m".to_string(),
                name: "S".to_string(),
                kind: TypeKind::Struct,
                signature: "pub struct S".to_string(),
                docs: None,
                vis: Vis::Pub,
                place: place(),
                extent: None,
            }],
            functions: vec![impl_elsewhere],
            ..Default::default()
        };
        let n = &Views::new(&m).call_graph().nodes[0];
        assert_eq!(
            (n.module.as_str(), n.impl_module.as_str()),
            ("m", "m::more")
        );
    }

    #[test]
    fn the_call_graph_has_every_function_addressed_like_its_page_and_every_call() {
        let m = Model {
            types: vec![TypeItem {
                krate: "x".to_string(),
                module: "m".to_string(),
                name: "S".to_string(),
                kind: TypeKind::Struct,
                signature: "pub struct S".to_string(),
                docs: None,
                vis: Vis::Pub,
                place: place(),
                extent: None,
            }],
            functions: vec![
                function(Some("S"), "run", Vis::Pub, Some("Runs it.\n\nMore.")),
                function(None, "helper", Vis::Private, None),
                // A method on a type the guide does not have.
                function(Some("Foreign"), "fmt", Vis::Trait, None),
            ],
            calls: [((0, 1), 2), ((1, 2), 1)].into(),
            ..Default::default()
        };
        let g = Views::new(&m).call_graph();
        let keys: Vec<(Option<&str>, &str)> = g
            .nodes
            .iter()
            .map(|n| (n.type_name.as_deref(), n.key.as_str()))
            .collect();
        assert_eq!(
            keys,
            [(Some("S"), "run"), (None, "helper"), (None, "Foreign::fmt")]
        );
        assert_eq!(g.nodes[0].doc.as_deref(), Some("Runs it."));
        assert_eq!(g.nodes[1].vis, Vis::Private);
        assert_eq!(g.edges, [(0, 1, 2), (1, 2, 1)]);
        assert_eq!(g.nodes[0].place, place());
        assert!(g.modules.is_empty());
    }
}
