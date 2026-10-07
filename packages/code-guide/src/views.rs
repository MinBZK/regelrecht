//! What each page of the app gets, built from the [`Model`].
//!
//! Every relation here is a call, as rust-analyzer resolved it: a crate builds
//! on another when its functions call into it, a module likewise, and a method
//! on another when it calls it. Layers (the reading order) are computed from
//! those calls at each level.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

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

/// A call among a type's non-private methods, folded over private helpers.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CallEdge {
    pub from: String,
    pub to: String,
    pub via: Vec<String>,
}

/// A module outside the type that calls its non-private methods.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCaller {
    #[serde(rename = "crate")]
    pub krate: String,
    pub module: String,
    /// Method key -> call sites from this module.
    pub targets: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeGraph {
    pub edges: Vec<CallEdge>,
    pub layers: Vec<Vec<Vec<String>>>,
    pub external: Vec<ExternalCaller>,
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
    pub graph: TypeGraph,
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

    /// One type: its methods with their callers and callees, and the calls
    /// among its public methods.
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
            graph: self.type_graph(ti, &methods),
        })
    }

    /// The calls among a type's non-private methods (those callable from
    /// outside it), private helpers folded away
    /// (breadth first, so `via` is a shortest chain), plus the modules outside
    /// the type that call those methods.
    fn type_graph(&self, ti: usize, methods: &[usize]) -> TypeGraph {
        let m = self.m;
        let public: BTreeSet<usize> = methods
            .iter()
            .copied()
            .filter(|&i| m.functions[i].vis != Vis::Private)
            .collect();
        let own: BTreeSet<usize> = methods.iter().copied().collect();
        let key = |i: usize| method_key(&m.functions[i]);

        let mut edges = Vec::new();
        for &start in &public {
            let mut prev: BTreeMap<usize, usize> = BTreeMap::new();
            let mut queue: VecDeque<usize> = VecDeque::new();
            for &(c, _) in &self.callees[start] {
                if own.contains(&c) && !prev.contains_key(&c) {
                    prev.insert(c, start);
                    queue.push_back(c);
                }
            }
            while let Some(x) = queue.pop_front() {
                if x == start {
                    continue;
                }
                if public.contains(&x) {
                    let mut via = Vec::new();
                    let mut cur = prev[&x];
                    while cur != start {
                        via.push(key(cur));
                        cur = prev[&cur];
                    }
                    via.reverse();
                    edges.push(CallEdge {
                        from: key(start),
                        to: key(x),
                        via,
                    });
                    continue;
                }
                for &(c, _) in &self.callees[x] {
                    if own.contains(&c) && c != start && !prev.contains_key(&c) {
                        prev.insert(c, x);
                        queue.push_back(c);
                    }
                }
            }
        }
        edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

        let deps = deps_of(
            public.iter().map(|&i| key(i)),
            edges.iter().map(|e| (e.from.clone(), e.to.clone())),
        );

        let mut external: BTreeMap<(String, String), BTreeMap<String, usize>> = BTreeMap::new();
        for &target in &public {
            for &(c, n) in &self.callers[target] {
                if self.owner_type[c] == Some(ti) {
                    continue;
                }
                let f = &m.functions[c];
                *external
                    .entry((f.krate.clone(), f.module.clone()))
                    .or_default()
                    .entry(key(target))
                    .or_insert(0) += n;
            }
        }
        TypeGraph {
            edges,
            layers: layers(&deps),
            external: external
                .into_iter()
                .map(|((krate, module), targets)| ExternalCaller {
                    krate,
                    module,
                    targets,
                })
                .collect(),
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
