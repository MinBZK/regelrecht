//! BDD Test Runner for RegelRecht Engine
//!
//! Runs Cucumber/Gherkin tests for the canonical, engine-agnostic BDD feature
//! language. Feature files live in two buckets:
//!
//! - **Bucket A — law validation**: `corpus/regulation/**/scenarios/*.feature`
//!   (run against the live laws, any file under a `scenarios/` directory).
//! - **Bucket B — engine conformance**: `bdd/conformance/*.feature`.
//!
//! The Rust engine supports every capability tier, so all features run except
//! scenarios tagged `@wip` (genuine, NB-documented engine gaps that assert the
//! desired-but-not-yet-produced outcome).
//!
//! # Usage
//!
//! ```bash
//! cargo test --test bdd -- --nocapture
//! ```
//!
//! Or via just:
//!
//! ```bash
//! just bdd
//! ```

// Allow panic/expect in test code - these are appropriate for test setup
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "../common/mod.rs"]
mod common;
mod dispatch;
mod helpers;
mod world;

/// Code-generated cucumber step bindings from `bdd/grammar.yaml`.
/// The generated fns reference `RegelrechtWorld` and `ArgValue`; bring them into
/// scope here. `cucumber::gherkin::Step` is referenced fully-qualified.
#[allow(unused_mut, clippy::let_and_return, clippy::vec_init_then_push)]
mod generated_steps {
    use crate::dispatch::ArgValue;
    use crate::world::RegelrechtWorld;
    include!(concat!(env!("OUT_DIR"), "/bdd_generated_steps.rs"));
}

use std::path::{Path, PathBuf};

use cucumber::feature::Ext as _;
use cucumber::{cli, parser, World as _};
use futures::stream;
use walkdir::WalkDir;

/// Parser that consumes an explicit, pre-collected list of feature file paths
/// (the two buckets) instead of recursively walking a single root. This keeps
/// the runner from picking up unrelated `.feature` files and lets us combine
/// `corpus/regulation/**/scenarios` with `bdd/conformance` in one run.
struct ExplicitPaths;

impl parser::Parser<Vec<PathBuf>> for ExplicitPaths {
    type Cli = cli::Empty;
    type Output = stream::Iter<std::vec::IntoIter<Result<gherkin::Feature, parser::Error>>>;

    fn parse(self, input: Vec<PathBuf>, _cli: Self::Cli) -> Self::Output {
        let features: Vec<Result<gherkin::Feature, parser::Error>> = input
            .into_iter()
            .map(|path| {
                gherkin::Feature::parse_path(&path, gherkin::GherkinEnv::default())
                    .map_err(parser::Error::from)
                    .and_then(|f| f.expand_examples().map_err(parser::Error::from))
            })
            .collect();
        stream::iter(features)
    }
}

/// Collect every feature file from both buckets:
/// - bucket A: any `*.feature` under a `scenarios/` directory in the corpus, and
/// - bucket B: `bdd/conformance/*.feature`.
fn collect_feature_paths(root: &Path) -> Vec<PathBuf> {
    let mut features: Vec<PathBuf> = Vec::new();

    // Bucket A — corpus scenarios.
    for entry in WalkDir::new(root.join("corpus/regulation"))
        .into_iter()
        .flatten()
    {
        let p = entry.path();
        let is_feature = p.extension().map(|e| e == "feature").unwrap_or(false);
        let under_scenarios = p.components().any(|c| c.as_os_str() == "scenarios");
        if is_feature && under_scenarios {
            features.push(p.to_path_buf());
        }
    }

    // Bucket B — engine conformance.
    for entry in WalkDir::new(root.join("bdd/conformance"))
        .into_iter()
        .flatten()
    {
        let p = entry.path();
        if p.extension().map(|e| e == "feature").unwrap_or(false) {
            features.push(p.to_path_buf());
        }
    }

    features.sort();
    features
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .init();

    // Project root = two levels up from the engine package manifest.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let root = Path::new(manifest_dir)
        .parent() // packages/
        .and_then(|p| p.parent()) // project root
        .expect("Could not resolve project root")
        .to_path_buf();

    let features = collect_feature_paths(&root);
    assert!(
        !features.is_empty(),
        "No feature files found under {} (corpus/regulation/**/scenarios or bdd/conformance)",
        root.display()
    );

    // Run cucumber over both buckets. `@wip` scenarios document genuine,
    // NB-annotated engine gaps (they assert the desired-but-not-yet-produced
    // outcome) — skip them so they neither run nor fail the suite.
    // `filter_run_and_exit` exits non-zero on test failures.
    world::RegelrechtWorld::cucumber::<&std::path::Path>()
        .with_parser::<ExplicitPaths, Vec<PathBuf>>(ExplicitPaths)
        .max_concurrent_scenarios(1) // Run scenarios sequentially for predictable state
        .with_default_cli()
        .filter_run_and_exit(features, |feature, _rule, scenario| {
            let is_wip = |tags: &[String]| tags.iter().any(|t| t == "wip");
            !is_wip(&feature.tags) && !is_wip(&scenario.tags)
        })
        .await;
}
