//! Step definitions for note resolution (RFC-005, RFC-018).
//!
//! Scenarios set up article text inline (so they are self-contained and do not
//! depend on the corpus), build a TextQuoteSelector, resolve it, and assert on
//! the resulting [`MatchResult`].

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use regelrecht_engine::{annotation, Article, SelectorHint, TextQuoteSelector};

use crate::world::RegelrechtWorld;

#[given(expr = "a law with the following articles:")]
fn set_note_articles(world: &mut RegelrechtWorld, step: &Step) {
    let table = step.table.as_ref().expect("articles table required");
    world.note_articles.clear();
    // First row is the header (number | text).
    for row in table.rows.iter().skip(1) {
        world.note_articles.push(Article {
            number: row[0].trim().to_string(),
            text: row[1].trim().to_string(),
            url: None,
            machine_readable: None,
        });
    }
}

#[given(expr = "a note selecting {string}")]
fn set_note_selector_exact(world: &mut RegelrechtWorld, exact: String) {
    world.note_selector = Some(TextQuoteSelector {
        exact,
        prefix: String::new(),
        suffix: String::new(),
        hint: None,
    });
}

#[given(expr = "a note selecting {string} with prefix {string} and suffix {string}")]
fn set_note_selector_context(
    world: &mut RegelrechtWorld,
    exact: String,
    prefix: String,
    suffix: String,
) {
    world.note_selector = Some(TextQuoteSelector {
        exact,
        prefix,
        suffix,
        hint: None,
    });
}

#[given(expr = "the note hints article {string}")]
fn set_note_hint_article(world: &mut RegelrechtWorld, article: String) {
    let selector = world
        .note_selector
        .as_mut()
        .expect("selector must be set before adding a hint");
    selector.hint = Some(SelectorHint {
        article_number: article,
        start: None,
        end: None,
    });
}

#[given(expr = "the note hints article {string} at position {int} to {int}")]
fn set_note_hint_position(world: &mut RegelrechtWorld, article: String, start: usize, end: usize) {
    let selector = world
        .note_selector
        .as_mut()
        .expect("selector must be set before adding a hint");
    selector.hint = Some(SelectorHint {
        article_number: article,
        start: Some(start),
        end: Some(end),
    });
}

#[when(expr = "the note is resolved")]
fn resolve_note(world: &mut RegelrechtWorld) {
    let selector = world
        .note_selector
        .as_ref()
        .expect("selector must be set before resolving");
    world.note_result = Some(annotation::resolve(selector, &world.note_articles));
}

#[then(expr = "the note resolves to article {string}")]
fn assert_resolves_to_article(world: &mut RegelrechtWorld, article: String) {
    let result = world.note_result.as_ref().expect("note must be resolved");
    assert!(
        result.is_found(),
        "expected Found, got {:?} ({} matches)",
        result.status,
        result.matches.len()
    );
    assert_eq!(
        result.single().expect("single match").article_number,
        article
    );
}

#[then(expr = "the note is an exact match")]
fn assert_exact_match(world: &mut RegelrechtWorld) {
    let result = world.note_result.as_ref().expect("note must be resolved");
    let m = result.single().expect("expected a single match");
    assert_eq!(m.confidence, 1.0, "expected exact (confidence 1.0)");
}

#[then(expr = "the note is a fuzzy match")]
fn assert_fuzzy_match(world: &mut RegelrechtWorld) {
    let result = world.note_result.as_ref().expect("note must be resolved");
    let m = result.single().expect("expected a single match");
    assert!(
        m.confidence < 1.0,
        "expected fuzzy (confidence < 1.0), got {}",
        m.confidence
    );
}

#[then(expr = "the note is orphaned")]
fn assert_orphaned(world: &mut RegelrechtWorld) {
    let result = world.note_result.as_ref().expect("note must be resolved");
    assert!(
        result.is_orphaned(),
        "expected Orphaned, got {:?}",
        result.status
    );
}

#[then(expr = "the note is ambiguous")]
fn assert_ambiguous(world: &mut RegelrechtWorld) {
    let result = world.note_result.as_ref().expect("note must be resolved");
    assert!(
        result.is_ambiguous(),
        "expected Ambiguous, got {:?}",
        result.status
    );
}
