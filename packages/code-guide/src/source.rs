//! What the rust-analyzer index does not say, read from the source with `syn`:
//!
//! - the exact extent of every function, from its doc comment through its
//!   closing brace. The index has extents too, but inside macro-generated code
//!   (`#[wasm_bindgen] impl`) it gives every method the whole impl block, so a
//!   call there could not be placed in the right function without guessing;
//! - which code is test code: files under `tests/`, `benches/` and `examples/`,
//!   items compiled only under `cfg(test)` (and modules declared that way), and
//!   `#[test]` functions. Test code is left out of the guide entirely.
//!
//! Everything else (names, kinds, docs, signatures, visibility, and what a call
//! resolves to) comes from the index.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use quote::ToTokens;
use walkdir::WalkDir;

/// A function or method in the source, by where its name and body are.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FnSite {
    pub name: String,
    /// 1-based line of the function's name, where the index has its definition.
    pub name_line: usize,
    /// 1-based, inclusive: doc comment through closing brace.
    pub line: usize,
    pub end_line: usize,
    /// Compiled only for tests.
    pub test: bool,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    /// The whole file is test code.
    pub test: bool,
    pub fns: Vec<FnSite>,
    /// 1-based inclusive line ranges of items compiled only for tests, in a file
    /// that is not test code as a whole: a `#[cfg(test)] mod tests { .. }`, a
    /// test-only impl, a `#[test]` function. A type or function defined there is
    /// test code too.
    pub test_ranges: Vec<(usize, usize)>,
    /// 1-based inclusive line ranges of `use` items, at any depth (a `use`
    /// inside a function body included). A reference there imports a name; it
    /// does not call anything.
    pub use_ranges: Vec<(usize, usize)>,
}

/// The functions of every scanned file, by workspace-relative path (`/`).
///
/// The scan is taken when the index is built and stored next to it, so the two
/// always describe the same version of the code: the index's line numbers are
/// never read against an edited file.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Sources {
    files: HashMap<String, FileInfo>,
}

/// Directories of a crate whose files are test, benchmark or example code.
const TEST_DIRS: [&str; 3] = ["tests", "benches", "examples"];

impl Sources {
    /// Scans the crates in `crate_dirs` (relative to `workspace`).
    pub fn scan(workspace: &Path, crate_dirs: &[String]) -> Sources {
        let mut sources = Sources::default();
        for dir in crate_dirs {
            sources.scan_crate(workspace, dir);
        }
        sources
    }

    fn scan_crate(&mut self, workspace: &Path, crate_dir: &str) {
        let root = workspace.join(crate_dir);
        let mut parsed: Vec<(String, Vec<String>, bool, syn::File)> = Vec::new();
        for sub in ["src", "tests", "benches", "examples"] {
            let dir = root.join(sub);
            if !dir.is_dir() {
                continue;
            }
            let mut files: Vec<_> = WalkDir::new(&dir)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("rs"))
                .map(walkdir::DirEntry::into_path)
                .collect();
            files.sort();
            for file in files {
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                let Ok(ast) = syn::parse_file(&text) else {
                    continue;
                };
                let rel = file
                    .strip_prefix(workspace)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                let in_test_dir = TEST_DIRS.contains(&sub);
                let module = if sub == "src" {
                    module_path(&dir, &file)
                } else {
                    Vec::new()
                };
                parsed.push((rel, module, in_test_dir, ast));
            }
        }

        // Modules declared `#[cfg(test)] mod x;` make their own file test code,
        // and every file below it. Collect those declarations first.
        let mut gated: BTreeSet<Vec<String>> = BTreeSet::new();
        for (_, module, in_test_dir, ast) in &parsed {
            // A file under tests/ has no module path in the library; its
            // `#[cfg(test)] mod x;` must not mark the library's `x` as test code.
            if !in_test_dir {
                collect_gated_decls(&ast.items, module, &mut gated);
            }
        }

        for (rel, module, in_test_dir, ast) in parsed {
            let gated_file = (1..=module.len()).any(|n| gated.contains(&module[..n]));
            // `#![cfg(test)]` at the top of a file makes the whole file test code.
            let test = in_test_dir || gated_file || is_test_only(&ast.attrs);
            let mut info = FileInfo {
                test,
                fns: Vec::new(),
                test_ranges: Vec::new(),
                use_ranges: Vec::new(),
            };
            collect_fns(&ast.items, test, &mut info.fns);
            collect_test_ranges(&ast.items, &mut info.test_ranges);
            let mut uses = UseRanges(&mut info.use_ranges);
            syn::visit::Visit::visit_file(&mut uses, &ast);
            self.files.insert(rel, info);
        }
    }

    /// Whether `file` is entirely test code.
    pub fn is_test_file(&self, file: &str) -> bool {
        self.files.get(file).is_some_and(|f| f.test)
    }

    /// Whether line `line` (1-based) of `file` is test code.
    pub fn is_test_at(&self, file: &str, line: usize) -> bool {
        self.files
            .get(file)
            .is_some_and(|f| f.test || f.test_ranges.iter().any(|(a, b)| *a <= line && line <= *b))
    }

    /// Whether line `line` (1-based) of `file` is inside a `use` item.
    pub fn is_import_at(&self, file: &str, line: usize) -> bool {
        self.files
            .get(file)
            .is_some_and(|f| f.use_ranges.iter().any(|(a, b)| *a <= line && line <= *b))
    }

    /// Whether `file` was scanned at all (it belongs to a workspace crate).
    pub fn knows(&self, file: &str) -> bool {
        self.files.contains_key(file)
    }

    /// The function whose name is at `name_line` (1-based) in `file`.
    pub fn fn_named_at(&self, file: &str, name_line: usize, name: &str) -> Option<&FnSite> {
        self.files
            .get(file)?
            .fns
            .iter()
            .find(|f| f.name_line == name_line && f.name == name)
    }

    /// The innermost function whose extent contains `line` (1-based) of `file`.
    pub fn enclosing_fn(&self, file: &str, line: usize) -> Option<&FnSite> {
        self.files
            .get(file)?
            .fns
            .iter()
            .filter(|f| f.line <= line && line <= f.end_line)
            .min_by_key(|f| f.end_line - f.line)
    }
}

/// The module path a file under a crate's `src/` stands for: `src/lib.rs` and
/// `src/main.rs` are the root, `src/a/b.rs` and `src/a/b/mod.rs` are `a::b`. A
/// binary under `src/bin/` is a root of its own, prefixed `bin:<name>` so its
/// modules never collide with the library's.
fn module_path(src: &Path, file: &Path) -> Vec<String> {
    let Ok(rel) = file.strip_prefix(src) else {
        return Vec::new();
    };
    let parts: Vec<String> = rel
        .iter()
        .map(|p| p.to_string_lossy().trim_end_matches(".rs").to_string())
        .collect();
    let mut segs: Vec<String> = match parts.first().map(String::as_str) {
        Some("bin") => {
            let mut v = vec![format!("bin:{}", parts.get(1).cloned().unwrap_or_default())];
            v.extend(parts.iter().skip(2).cloned());
            v
        }
        _ => parts,
    };
    if segs
        .last()
        .is_some_and(|s| s == "mod" || s == "lib" || s == "main")
    {
        segs.pop();
    }
    segs
}

fn collect_gated_decls(items: &[syn::Item], module: &[String], out: &mut BTreeSet<Vec<String>>) {
    for item in items {
        if let syn::Item::Mod(m) = item {
            let mut path = module.to_vec();
            path.push(m.ident.to_string());
            if is_test_only(&m.attrs) {
                out.insert(path.clone());
            }
            if let Some((_, inner)) = &m.content {
                collect_gated_decls(inner, &path, out);
            }
        }
    }
}

/// Records every function and method in `items`, recursing into inline modules,
/// impl blocks and traits but not into function bodies: a function nested in
/// another belongs, for the guide, to the one it is nested in.
fn collect_fns(items: &[syn::Item], test: bool, out: &mut Vec<FnSite>) {
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                let t = test || is_test_only(&f.attrs) || is_test_fn(&f.attrs);
                push_fn(&f.sig, f, t, out);
            }
            syn::Item::Impl(i) => {
                let t = test || is_test_only(&i.attrs);
                for it in &i.items {
                    if let syn::ImplItem::Fn(f) = it {
                        let t = t || is_test_only(&f.attrs) || is_test_fn(&f.attrs);
                        push_fn(&f.sig, f, t, out);
                    }
                }
            }
            syn::Item::Trait(tr) => {
                let t = test || is_test_only(&tr.attrs);
                for it in &tr.items {
                    if let syn::TraitItem::Fn(f) = it {
                        push_fn(&f.sig, f, t || is_test_only(&f.attrs), out);
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_fns(inner, test || is_test_only(&m.attrs), out);
                }
            }
            _ => {}
        }
    }
}

/// The line ranges of test-only items at any depth of non-test modules.
fn collect_test_ranges(items: &[syn::Item], out: &mut Vec<(usize, usize)>) {
    for item in items {
        let attrs: &[syn::Attribute] = match item {
            syn::Item::Fn(f) => &f.attrs,
            syn::Item::Impl(i) => &i.attrs,
            syn::Item::Trait(t) => &t.attrs,
            syn::Item::Struct(s) => &s.attrs,
            syn::Item::Enum(e) => &e.attrs,
            syn::Item::Mod(m) => &m.attrs,
            syn::Item::Const(c) => &c.attrs,
            syn::Item::Static(s) => &s.attrs,
            syn::Item::Type(t) => &t.attrs,
            syn::Item::Union(u) => &u.attrs,
            _ => &[],
        };
        let test = is_test_only(attrs) || (matches!(item, syn::Item::Fn(_)) && is_test_fn(attrs));
        if test {
            if let Some(range) = extent(item) {
                out.push(range);
            }
        } else if let syn::Item::Mod(m) = item {
            if let Some((_, inner)) = &m.content {
                collect_test_ranges(inner, out);
            }
        } else if let syn::Item::Trait(t) = item {
            for it in &t.items {
                if let syn::TraitItem::Fn(f) = it {
                    if is_test_only(&f.attrs) {
                        if let Some(range) = extent(f) {
                            out.push(range);
                        }
                    }
                }
            }
        } else if let syn::Item::Impl(i) = item {
            for it in &i.items {
                if let syn::ImplItem::Fn(f) = it {
                    if is_test_only(&f.attrs) || is_test_fn(&f.attrs) {
                        if let Some(range) = extent(f) {
                            out.push(range);
                        }
                    }
                }
            }
        }
    }
}

/// Collects the extent of every `use` item in a file, however deeply nested.
struct UseRanges<'a>(&'a mut Vec<(usize, usize)>);

impl<'ast> syn::visit::Visit<'ast> for UseRanges<'_> {
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        if let Some(range) = extent(node) {
            self.0.push(range);
        }
    }
}

/// First and last line of an item, 1-based.
fn extent<T: ToTokens>(item: &T) -> Option<(usize, usize)> {
    let tokens: Vec<proc_macro2::TokenTree> = item.to_token_stream().into_iter().collect();
    let (first, last) = (tokens.first()?, tokens.last()?);
    let (a, b) = (first.span().start().line, last.span().end().line);
    (a > 0).then_some((a, b))
}

fn push_fn<T: ToTokens>(sig: &syn::Signature, item: &T, test: bool, out: &mut Vec<FnSite>) {
    let Some((line, end_line)) = extent(item) else {
        return;
    };
    out.push(FnSite {
        name: sig.ident.to_string(),
        name_line: sig.ident.span().start().line,
        line,
        end_line,
        test,
    });
}

/// `#[test]`, `#[tokio::test]` and the like.
fn is_test_fn(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().segments.last().is_some_and(|s| s.ident == "test"))
}

/// Whether an item is compiled only for tests: a `#[cfg(..)]` whose predicate
/// can only hold with `test` set. `cfg(test)` and `cfg(all(test, ..))` are;
/// `cfg(not(test))` and `cfg(any(test, ..))` are not.
fn is_test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("cfg") {
            return false;
        }
        a.parse_args::<syn::Meta>()
            .map(|m| requires_test(&m))
            .unwrap_or(false)
    })
}

fn requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(p) => p.is_ident("test"),
        syn::Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .map(|args| args.iter().any(requires_test))
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::testutil::Dir;

    fn scan(files: &[(&str, &str)]) -> (Dir, Sources) {
        let d = Dir::new();
        for (path, text) in files {
            d.write(path, text);
        }
        let s = Sources::scan(d.path(), &["c".to_string()]);
        (d, s)
    }

    #[test]
    fn a_function_gets_its_extent_from_doc_comment_to_closing_brace() {
        let (_d, s) = scan(&[("c/src/lib.rs", "\n/// Doc.\npub fn f() {\n    g();\n}\n")]);
        let f = s.fn_named_at("c/src/lib.rs", 3, "f").expect("f");
        assert_eq!((f.line, f.end_line, f.test), (2, 5, false));
        assert_eq!(
            s.enclosing_fn("c/src/lib.rs", 4).map(|f| f.name.as_str()),
            Some("f")
        );
        assert!(s.enclosing_fn("c/src/lib.rs", 1).is_none());
    }

    #[test]
    fn methods_in_impls_and_trait_default_bodies_are_functions() {
        let (_d, s) = scan(&[(
            "c/src/lib.rs",
            "pub struct T;\nimpl T {\n    pub fn a(&self) {}\n}\npub trait Tr {\n    fn b(&self) { }\n}\n",
        )]);
        assert!(s.fn_named_at("c/src/lib.rs", 3, "a").is_some());
        assert!(s.fn_named_at("c/src/lib.rs", 6, "b").is_some());
    }

    #[test]
    fn a_call_in_a_nested_function_belongs_to_the_outer_one() {
        let (_d, s) = scan(&[(
            "c/src/lib.rs",
            "fn outer() {\n    fn inner() {\n        x();\n    }\n}\n",
        )]);
        assert_eq!(
            s.enclosing_fn("c/src/lib.rs", 3).map(|f| f.name.as_str()),
            Some("outer")
        );
    }

    #[test]
    fn test_code_is_recognised_in_every_form() {
        let (_d, s) = scan(&[
            (
                "c/src/lib.rs",
                "#[cfg(test)]\nmod tests {\n    fn helper() {}\n}\n\
                 #[cfg(test)]\nmod declared;\n\
                 #[test]\nfn loose() {}\n\
                 #[cfg(all(test, feature = \"x\"))]\nfn both() {}\n\
                 #[cfg(not(test))]\nfn production() {}\n\
                 #[cfg(any(test, feature = \"x\"))]\nfn either() {}\n",
            ),
            ("c/src/declared.rs", "fn in_declared() {}\n"),
            ("c/src/declared/deeper.rs", "fn below_declared() {}\n"),
            ("c/tests/it.rs", "fn integration() {}\n"),
            ("c/benches/b.rs", "fn bench() {}\n"),
        ]);
        let test = |file: &str, name: &str| {
            let f = s.files[file]
                .fns
                .iter()
                .find(|f| f.name == name)
                .expect(name);
            f.test || s.is_test_file(file)
        };
        assert!(test("c/src/lib.rs", "helper"), "inline cfg(test) module");
        assert!(
            test("c/src/declared.rs", "in_declared"),
            "declared cfg(test) module"
        );
        assert!(
            test("c/src/declared/deeper.rs", "below_declared"),
            "below a test module"
        );
        assert!(test("c/src/lib.rs", "loose"), "#[test]");
        assert!(test("c/src/lib.rs", "both"), "cfg(all(test, ..))");
        assert!(test("c/tests/it.rs", "integration"), "tests/");
        assert!(test("c/benches/b.rs", "bench"), "benches/");
        assert!(
            !test("c/src/lib.rs", "production"),
            "cfg(not(test)) is production code"
        );
        assert!(
            !test("c/src/lib.rs", "either"),
            "cfg(any(test, ..)) also builds without tests"
        );
    }

    #[test]
    fn a_type_inside_a_test_module_is_test_code_but_its_neighbours_are_not() {
        let (_d, s) = scan(&[(
            "c/src/lib.rs",
            "pub struct Real;\n#[cfg(test)]\nmod tests {\n    struct Fixture;\n}\npub struct AlsoReal;\n",
        )]);
        assert!(!s.is_test_at("c/src/lib.rs", 1));
        assert!(s.is_test_at("c/src/lib.rs", 4));
        assert!(!s.is_test_at("c/src/lib.rs", 6));
    }

    #[test]
    fn use_items_are_imports_wherever_they_are() {
        let (_d, s) = scan(&[(
            "c/src/lib.rs",
            "use crate::a::{\n    b,\n};\nfn f() {\n    use crate::c::d;\n    d();\n}\n",
        )]);
        assert!(s.is_import_at("c/src/lib.rs", 2));
        assert!(
            s.is_import_at("c/src/lib.rs", 5),
            "a use inside a function body"
        );
        assert!(!s.is_import_at("c/src/lib.rs", 6), "the call itself");
    }

    #[test]
    fn a_test_only_declaration_under_tests_does_not_touch_the_library() {
        let (_d, s) = scan(&[
            ("c/src/lib.rs", "pub mod x;\n"),
            ("c/src/x.rs", "pub fn real() {}\n"),
            ("c/tests/common/mod.rs", "#[cfg(test)]\nmod x;\n"),
        ]);
        assert!(!s.is_test_file("c/src/x.rs"));
    }

    #[test]
    fn an_inner_cfg_test_makes_the_whole_file_test_code() {
        let (_d, s) = scan(&[("c/src/lib.rs", "#![cfg(test)]\nfn f() {}\n")]);
        assert!(s.is_test_file("c/src/lib.rs"));
    }

    #[test]
    fn a_test_only_trait_method_is_test_code() {
        let (_d, s) = scan(&[(
            "c/src/lib.rs",
            "pub trait T {\n    #[cfg(test)]\n    fn probe(&self) {}\n    fn real(&self);\n}\n",
        )]);
        assert!(s.is_test_at("c/src/lib.rs", 3));
        assert!(!s.is_test_at("c/src/lib.rs", 4));
    }

    #[test]
    fn module_paths_follow_the_file_layout() {
        let src = Path::new("/w/c/src");
        let m = |f: &str| module_path(src, &src.join(f)).join("::");
        assert_eq!(m("lib.rs"), "");
        assert_eq!(m("main.rs"), "");
        assert_eq!(m("a.rs"), "a");
        assert_eq!(m("a/mod.rs"), "a");
        assert_eq!(m("a/b.rs"), "a::b");
        assert_eq!(m("bin/tool.rs"), "bin:tool");
        assert_eq!(m("bin/tool/main.rs"), "bin:tool");
    }
}
