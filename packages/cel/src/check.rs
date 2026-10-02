//! The checks on a cell at startup. The runtime refuses to start if one
//! fails, with a message that names the field or the parameter.
//!
//! 1. Stream, lexostatus definitions and cell definition validate against
//!    their schema (at load time, see [`crate::stream::parse`],
//!    [`crate::reduction::parse`] and [`crate::config::CellDefinition::parse`]).
//! 2. Every derivation points at something that exists: a parameter of an
//!    article from the legal basis of an event its filter selects (or from
//!    the derivation's own `legal_basis`), and field paths of that event.
//!    Every article from the legal basis of a derivation is loaded and has
//!    the paragraph it names, just as for an event. A
//!    table derivation points at a table field and reads only columns the
//!    stream declares for that field. A derivation on the chosen gram
//!    requires a lexostatus that picks a gram (`pick`).
//! 3. No orphan field: every field of an event is read by a derivation or a
//!    filter, or is listed with a reason in `not_reduced`. A field the law
//!    declares (an event with `establishes`) is recorded because the law says
//!    so: unread, it is a warning, not an error.
//! 4. No name collision: a parameter gets only one derivation.
//! 5. (Dropped with chronolex v0.2.0: every gram has a root, so a filter on
//!    `root` or `group_by: root` can select any event.)
//! 6. A list lexostatus (`group_by`) has columns, not parameters: its
//!    derivations need not be a parameter of an article and do not collide
//!    with those of other lexostatuses. Its `without` selects an event in its
//!    chronicle, otherwise it would never leave anything out.
//! 7. No lexostatus is named [`crate::reduction::CASE_STATE`]: the runtime
//!    offers that one.
//!
//! A process with a portal points at an existing event of its cell, an
//! existing lexostatus and an existing output, of an article from the legal
//! basis of that event ([`portal`], called from [`crate::process`]). The
//! checks on the synthesis live in [`crate::synthesis`], those on the actions
//! in [`crate::action`].

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::LawExecutionService;

use crate::config::{Offer, Portal};
use crate::reduction::{self, Filter, LexostatusDefinition, Lexostatuses, Period};
use crate::regulations;
use crate::stream::{Binding, Event, EventAttribute, Stream};

/// An event with the stream it belongs to.
pub type StreamEvent<'a> = (&'a Stream, &'a Event);

/// Whether an event can pass a filter: equal on every fixed value of a key
/// of the gram itself. A field path or a value `$x` depends on the gram or
/// the query and counts as a match here.
fn event_fits(filter: &Filter, stream: &Stream, event: &Event) -> bool {
    filter.iter().all(|(key, value)| {
        if value.starts_with('$') {
            return true;
        }
        // Only what is fixed in the stream selects events here. An
        // attribute an event never has (a case attribute without a case) is
        // reported by a check of its own, which therefore must see the event.
        match event.attribute(stream, key) {
            Some(EventAttribute::Fixed(w)) => w == Some(value.as_str()),
            Some(EventAttribute::Free | EventAttribute::Never) | None => true,
        }
    })
}

/// The events a definition can select: in the chronicle of the reduction,
/// through the filter of the lexostatus and, for a derivation over a
/// collection, also through its own filter.
pub fn events_for<'a>(
    def: &LexostatusDefinition,
    derivation_filter: Option<&Filter>,
    streams: &'a [Stream],
) -> Vec<StreamEvent<'a>> {
    let mut out = Vec::new();
    for stream in streams
        .iter()
        .filter(|s| s.chronicle == def.reduction.chronicle)
    {
        for event in &stream.events {
            if event_fits(&def.reduction.filter, stream, event)
                && derivation_filter.is_none_or(|f| event_fits(f, stream, event))
            {
                out.push((stream, event));
            }
        }
    }
    out
}

/// The derivations of a definition that read `<stream>/<event>`: per
/// derivation its own filter on top of that of the lexostatus; a derivation
/// on the chosen gram reads what the lexostatus selects.
pub fn derivations_reading(
    def: &LexostatusDefinition,
    streams: &[Stream],
    stream: &str,
    event: &str,
) -> Vec<String> {
    def.all_derivations()
        .filter(|(_, d)| {
            events_for(def, d.derivation.filter(), streams)
                .iter()
                .any(|(s, e)| s.id == stream && e.name == event)
        })
        .map(|(n, _)| n.clone())
        .collect()
}

/// The events in a chronicle that can pass a filter, apart from the filter
/// of a lexostatus (for `without`).
pub fn events_in<'a>(
    chronicle: &str,
    filter: &Filter,
    streams: &'a [Stream],
) -> Vec<StreamEvent<'a>> {
    streams
        .iter()
        .filter(|s| s.chronicle == chronicle)
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(s, e)| event_fits(filter, s, e))
        .collect()
}

/// Sets the period of every period derivation without `period` from the
/// regulation: the `temporal.period_type` (RFC-001) of the parameter named
/// after the derivation, in an article from the legal basis of the derivation
/// or of an event it reads. That way the law says which period a window is (a
/// month, a year), not the cell configuration. None or two different periods
/// is an error: name the period in the derivation then.
pub fn periods(
    streams: &[Stream],
    lexostatuses: &mut Lexostatuses,
    service: &LawExecutionService,
) -> Vec<String> {
    let mut errors = Vec::new();
    for def in &mut lexostatuses.lexostatus_definitions {
        let copy = def.clone();
        let derivations = def
            .reduction
            .derivations
            .iter_mut()
            .chain(def.reduction.extra_fields.iter_mut());
        for (name, a) in derivations {
            if a.derivation.period_mut().is_none_or(|p| p.is_some()) {
                continue;
            }
            let events = events_for(&copy, a.derivation.filter(), streams);
            let legal_basis: Vec<String> = a
                .legal_basis
                .iter()
                .chain(events.iter().flat_map(|(_, e)| e.legal_basis.iter()))
                .cloned()
                .collect();
            let Some(period) = a.derivation.period_mut().filter(|p| p.is_none()) else {
                continue;
            };
            let found: BTreeSet<String> = legal_basis
                .iter()
                .filter_map(|g| regulations::article(service, g).ok())
                .flat_map(|art| art.get_parameters().iter())
                .filter(|p| &p.name == name)
                .filter_map(|p| p.temporal.as_ref()?.period_type.clone())
                .collect();
            let periods: Vec<Period> = found
                .iter()
                .filter_map(|t| Period::from_period_type(t))
                .collect();
            match periods.as_slice() {
                [p] if found.len() == 1 => *period = Some(*p),
                _ => errors.push(format!(
                    "lexostatus '{}', derivation '{name}': period_of without period, and {}; name the period (year, quarter, month)",
                    copy.name,
                    if found.is_empty() {
                        format!("no parameter '{name}' in the legal basis ({}) names a temporal.period_type", legal_basis.join(", "))
                    } else {
                        format!("the legal basis names period_type {} for '{name}'", found.into_iter().collect::<Vec<_>>().join(", "))
                    }
                )),
            }
        }
    }
    errors
}

/// All checks on a cell. `Ok` if the cell may start.
pub fn check(
    streams: &[Stream],
    lexostatuses: &Lexostatuses,
    service: &LawExecutionService,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    unique(streams, lexostatuses, &mut errors);
    legal_bases(streams, service, &mut errors);
    references(streams, lexostatuses, service, &mut errors);
    orphan_fields(streams, lexostatuses, &mut errors);
    collisions(streams, lexostatuses, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn unique(streams: &[Stream], lexostatuses: &Lexostatuses, errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for s in streams {
        if !seen.insert(&s.id) {
            errors.push(format!("stream '{}' appears more than once", s.id));
        }
    }
    let mut seen = BTreeSet::new();
    for d in &lexostatuses.lexostatus_definitions {
        if d.name == reduction::CASE_STATE {
            errors.push(format!(
                "lexostatus '{}': that name belongs to the runtime, which offers it for every cell with a case",
                d.name
            ));
        }
        if !seen.insert(&d.name) {
            errors.push(format!("lexostatus '{}' appears more than once", d.name));
        }
    }
}

/// Every legal basis points at a loaded article, and a paragraph the article
/// text has (a line starting with `<n>.` or `<n> `). Also the legal basis of
/// a bound `effective_at`.
fn legal_bases(streams: &[Stream], service: &LawExecutionService, errors: &mut Vec<String>) {
    for s in streams {
        for e in &s.events {
            let of_moment = e.effective_at.iter().flat_map(|b| &b.legal_basis);
            for g in e.legal_basis.iter().chain(of_moment) {
                if let Err(f) = regulations::valid(service, g) {
                    errors.push(format!("event '{}' (stream '{}'): {f}", e.name, s.id));
                }
            }
        }
    }
}

/// The parameters of the articles behind a list of legal bases.
fn parameters_of<'s>(
    legal_basis: impl IntoIterator<Item = &'s String>,
    service: &LawExecutionService,
) -> BTreeSet<String> {
    legal_basis
        .into_iter()
        .filter_map(|g| regulations::article(service, g).ok())
        .flat_map(|a| a.get_parameters().iter().map(|p| p.name.clone()))
        .collect()
}

fn inputs_in_filter(def: &LexostatusDefinition, filter: &Filter, errors: &mut Vec<String>) {
    let inputs: BTreeSet<&str> = def.inputs.iter().map(|i| i.name.as_str()).collect();
    for (key, value) in filter {
        if let Some(input) = value.strip_prefix('$') {
            if !inputs.contains(input) {
                errors.push(format!(
                    "lexostatus '{}': filter '{key}' uses '${input}', but '{input}' is not an input",
                    def.name
                ));
            }
        }
    }
}

fn references(
    streams: &[Stream],
    lexostatuses: &Lexostatuses,
    service: &LawExecutionService,
    errors: &mut Vec<String>,
) {
    for def in &lexostatuses.lexostatus_definitions {
        inputs_in_filter(def, &def.reduction.filter, errors);
        let events = events_for(def, None, streams);
        if events.is_empty() {
            errors.push(format!(
                "lexostatus '{}': the filter selects no event in chronicle '{}'",
                def.name, def.reduction.chronicle
            ));
        }
        for (_, event) in &events {
            for path in reduction::filter_paths(&def.reduction.filter) {
                if !event.has_path(path) {
                    errors.push(format!(
                        "lexostatus '{}': filter on field path '{path}', which does not exist in event '{}'",
                        def.name, event.name
                    ));
                }
            }
        }
        if !def.reduction.without.is_empty() {
            inputs_in_filter(def, &def.reduction.without, errors);
            let without = events_in(&def.reduction.chronicle, &def.reduction.without, streams);
            if without.is_empty() {
                errors.push(format!(
                    "lexostatus '{}': without selects no event in chronicle '{}', and so would never leave out a case",
                    def.name, def.reduction.chronicle
                ));
            }
            for (_, event) in &without {
                for path in reduction::filter_paths(&def.reduction.without) {
                    if !event.has_path(path) {
                        errors.push(format!(
                            "lexostatus '{}': without filters on field path '{path}', which does not exist in event '{}'",
                            def.name, event.name
                        ));
                    }
                }
            }
        }
        if def.reduction.derivations.is_empty() && def.reduction.extra_fields.is_empty() {
            errors.push(format!(
                "lexostatus '{}': no derivation and no extra field, so it yields nothing",
                def.name
            ));
        }
        for name in def.reduction.extra_fields.keys() {
            if def.reduction.derivations.contains_key(name) {
                errors.push(format!(
                    "lexostatus '{}': '{name}' is both a derivation and an extra field",
                    def.name
                ));
            }
        }
        for (param, derivation) in def.all_derivations() {
            // A column of a list is not a parameter: it does not go to the engine.
            let is_parameter = def.reduction.derivations.contains_key(param) && !def.is_list();
            // The derivation's own legal basis: loaded, with the paragraph.
            for g in &derivation.legal_basis {
                if let Err(f) = regulations::valid(service, g) {
                    errors.push(format!(
                        "lexostatus '{}', derivation '{param}': {f}",
                        def.name
                    ));
                }
            }
            let own_legal_basis = parameters_of(&derivation.legal_basis, service);
            if derivation.at_chosen_gram() && def.reduction.pick.is_none() {
                errors.push(format!(
                    "lexostatus '{}', derivation '{param}': reads the chosen gram, but the lexostatus picks none (pick)",
                    def.name
                ));
            }
            if let Some(f) = derivation.filter() {
                inputs_in_filter(def, f, errors);
            }
            let events = events_for(def, derivation.filter(), streams);
            if events.is_empty() && derivation.filter().is_some() {
                errors.push(format!(
                    "lexostatus '{}', derivation '{param}': the filter selects no event in chronicle '{}'",
                    def.name, def.reduction.chronicle
                ));
            }
            for (_, event) in events {
                if is_parameter {
                    let params = parameters_of(&event.legal_basis, service);
                    if !params.contains(param) && !own_legal_basis.contains(param) {
                        let mut where_ = event.legal_basis.join(", ");
                        if !derivation.legal_basis.is_empty() {
                            where_ = format!(
                                "{where_}; legal basis of the derivation: {}",
                                derivation.legal_basis.join(", ")
                            );
                        }
                        errors.push(format!(
                            "lexostatus '{}', derivation '{param}': '{param}' is not a parameter of an article from the legal basis of event '{}' ({where_})",
                            def.name, event.name
                        ));
                    }
                }
                if let Some((table, read)) = derivation.table_columns() {
                    match event.columns(table) {
                        None => errors.push(format!(
                            "lexostatus '{}', derivation '{param}': field path '{table}' is not a table field of event '{}'",
                            def.name, event.name
                        )),
                        Some(columns) => {
                            for column in read.into_iter().filter(|k| !columns.iter().any(|c| c == k)) {
                                errors.push(format!(
                                    "lexostatus '{}', derivation '{param}': column '{column}' is not among the columns of table '{table}' of event '{}' ({})",
                                    def.name,
                                    event.name,
                                    columns.join(", ")
                                ));
                            }
                        }
                    }
                    continue;
                }
                for path in derivation.read_paths() {
                    if !event.has_path(path) {
                        errors.push(format!(
                            "lexostatus '{}', derivation '{param}': field path '{path}' does not exist in event '{}'",
                            def.name, event.name
                        ));
                    }
                }
            }
        }
    }
}

fn covered(path: &str, by: &str) -> bool {
    path == by || path.starts_with(&format!("{by}."))
}

fn orphan_fields(streams: &[Stream], lexostatuses: &Lexostatuses, errors: &mut Vec<String>) {
    for stream in streams {
        for event in &stream.events {
            let this_event = |(s, e): &StreamEvent<'_>| s.id == stream.id && e.name == event.name;
            let mut read: Vec<&str> = Vec::new();
            for def in &lexostatuses.lexostatus_definitions {
                if events_for(def, None, streams).iter().any(this_event) {
                    read.extend(reduction::filter_paths(&def.reduction.filter));
                }
                if events_in(&def.reduction.chronicle, &def.reduction.without, streams)
                    .iter()
                    .any(this_event)
                    && !def.reduction.without.is_empty()
                {
                    read.extend(reduction::filter_paths(&def.reduction.without));
                }
                for (_, a) in def.all_derivations() {
                    if events_for(def, a.filter(), streams).iter().any(this_event) {
                        read.extend(a.read_paths());
                    }
                }
            }
            for ng in &event.not_reduced {
                if !event.has_path(&ng.field) {
                    errors.push(format!(
                        "not_reduced names '{}', but event '{}' (stream '{}') has no such field",
                        ng.field, event.name, stream.id
                    ));
                }
            }
            for leaf in event.leaves() {
                let by_derivation = read.iter().any(|p| covered(&leaf.path, p));
                let excepted = event
                    .not_reduced
                    .iter()
                    .any(|n| covered(&leaf.path, &n.field));
                if by_derivation || excepted {
                    continue;
                }
                // A field the law declares is recorded because the law says
                // so, read or not (note "het gram uit de wet"): a warning,
                // not a reason not to start.
                if event
                    .field_defs
                    .iter()
                    .any(|d| covered(&leaf.path, &d.name))
                {
                    tracing::warn!(
                        field = %leaf.path, event = %event.name, stream = %stream.id,
                        "field with a legal basis that no derivation reads"
                    );
                    continue;
                }
                errors.push(format!(
                    "orphan field '{}' in event '{}' (stream '{}'): no derivation reads it and it is not in not_reduced",
                    leaf.path, event.name, stream.id
                ));
            }
        }
    }
}

fn collisions(streams: &[Stream], lexostatuses: &Lexostatuses, errors: &mut Vec<String>) {
    // (stream, event, parameter) -> lexostatuses that derive it
    let mut per: BTreeMap<(String, String, String), Vec<&str>> = BTreeMap::new();
    for def in lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| !d.is_list())
    {
        for (param, a) in &def.reduction.derivations {
            for (s, e) in events_for(def, a.filter(), streams) {
                let defs = per
                    .entry((s.id.clone(), e.name.clone(), param.clone()))
                    .or_default();
                if !defs.contains(&def.name.as_str()) {
                    defs.push(&def.name);
                }
            }
        }
    }
    for ((_, event, param), defs) in per {
        if defs.len() > 1 {
            errors.push(format!(
                "parameter '{param}' gets more than one derivation for event '{event}': lexostatus {}",
                defs.join(", ")
            ));
        }
    }
}

/// The checks on the portal of a process, against the streams and
/// lexostatuses of the cell it records in: the event exists, binds only
/// `$intake` paths that the portal's channels deliver (`intake_paths`, see
/// [`crate::channel`]), the assessment lexostatus reads the event and picks a
/// gram, the assessment output comes from the legal basis of the event, and
/// the offer is valid.
pub fn portal(
    streams: &[Stream],
    lexostatuses: &Lexostatuses,
    p: &Portal,
    service: &LawExecutionService,
    intake_paths: &[String],
) -> Vec<String> {
    let mut errors = Vec::new();
    portal_(streams, lexostatuses, p, service, intake_paths, &mut errors);
    errors
}

fn portal_(
    streams: &[Stream],
    lexostatuses: &Lexostatuses,
    p: &Portal,
    service: &LawExecutionService,
    intake_paths: &[String],
    errors: &mut Vec<String>,
) {
    let Some(stream) = streams.iter().find(|s| s.id == p.stream) else {
        errors.push(format!(
            "portal: stream '{}' does not exist in cell '{}'",
            p.stream, p.cell
        ));
        return;
    };
    let Some(event) = stream.event(&p.event) else {
        errors.push(format!(
            "portal: stream '{}' has no event '{}'",
            p.stream, p.event
        ));
        return;
    };
    for leaf in event.leaves() {
        if let Binding::Intake(path) = &leaf.binding {
            if !intake_paths.contains(path) {
                errors.push(format!(
                    "portal: field '{}' binds to '$intake.{path}', but the portal's channels deliver only {}",
                    leaf.path,
                    intake_paths.join(", ")
                ));
            }
        }
    }
    match lexostatuses.lexostatus(&p.assessment.lexostatus) {
        None => errors.push(format!(
            "portal: lexostatus '{}' does not exist",
            p.assessment.lexostatus
        )),
        Some(def) => {
            if !events_for(def, None, streams)
                .iter()
                .any(|(s, e)| s.id == stream.id && e.name == event.name)
            {
                errors.push(format!(
                    "portal: lexostatus '{}' does not read event '{}'",
                    def.name, event.name
                ));
            }
            if def.reduction.pick.is_none() {
                errors.push(format!(
                    "portal: lexostatus '{}' picks no gram (pick), and the assessment reduces a draft",
                    def.name
                ));
            }
            if def.is_list() {
                errors.push(format!(
                    "portal: lexostatus '{}' is a list (group_by), and a list never goes to the engine",
                    def.name
                ));
            }
            for i in def.inputs.iter().filter(|i| i.name != "root") {
                errors.push(format!(
                    "portal: lexostatus '{}' requires input '{}'; the assessment passes only 'root'",
                    def.name, i.name
                ));
            }
        }
    }
    match service.resolver().get_article_by_output(
        &p.assessment.regulation,
        &p.assessment.output,
        None,
    ) {
        None => errors.push(format!(
            "portal: regulation '{}' has no output '{}'",
            p.assessment.regulation, p.assessment.output
        )),
        // The assessment passes the lexostatus as parameters to this
        // article; those are derived for the articles from the legal basis
        // of the event. By article, even if the legal basis names a paragraph.
        Some(article) => {
            let legal_basis = format!("{}#{}", p.assessment.regulation, article.number);
            let in_legal_basis = event.legal_basis.iter().any(|g| {
                regulations::parse(g).is_ok_and(|o| {
                    o.regulation == p.assessment.regulation && o.article == article.number
                })
            });
            if !in_legal_basis {
                errors.push(format!(
                    "portal: output '{}' comes from {legal_basis}, which is not in the legal basis of event '{}' ({})",
                    p.assessment.output,
                    event.name,
                    event.legal_basis.join(", ")
                ));
            }
        }
    }
    if let Some(a) = &p.offer {
        offer_(a, service, errors);
    }
}

/// The offer: the output exists, and a deadline comes from the same
/// article. Output and deadline go in one run; a deadline from another
/// article, or one that does not exist, would make that run fail and with it
/// the verdict on the output. The windows are an output of the same
/// regulation, in a run of their own without parameters.
fn offer_(a: &Offer, service: &LawExecutionService, errors: &mut Vec<String>) {
    let resolver = service.resolver();
    if let Some(t) = &a.windows {
        match resolver.get_article_by_output(&a.regulation, t, None) {
            None => errors.push(format!(
                "portal.offer: regulation '{}' has no windows output '{t}'",
                a.regulation
            )),
            Some(art) if art.get_parameters().iter().any(|p| p.required != Some(false)) => {
                errors.push(format!(
                    "portal.offer: windows '{t}' comes from {}#{}, and that article requires a parameter; the windows are fixed before anyone fills in anything",
                    a.regulation, art.number
                ))
            }
            Some(_) => {}
        }
    }
    // The start and the opening of a window: an output of the same
    // regulation, with the window as parameter.
    for (kind, u) in [("start", &a.start), ("opening", &a.opening)] {
        let Some(u) = u else {
            continue;
        };
        if a.windows.is_none() {
            errors.push(format!("portal.offer: {kind} without windows"));
        }
        if resolver
            .get_article_by_output(&a.regulation, u, None)
            .is_none()
        {
            errors.push(format!(
                "portal.offer: regulation '{}' has no {kind} output '{u}'",
                a.regulation
            ));
        }
    }
    let Some(article) = resolver.get_article_by_output(&a.regulation, &a.output, None) else {
        errors.push(format!(
            "portal.offer: regulation '{}' has no output '{}'",
            a.regulation, a.output
        ));
        return;
    };
    let Some(t) = &a.deadline else {
        return;
    };
    match resolver.get_article_by_output(&a.regulation, t, None) {
        None => errors.push(format!(
            "portal.offer: regulation '{}' has no deadline output '{t}'",
            a.regulation
        )),
        Some(other) if other.number != article.number => errors.push(format!(
            "portal.offer: deadline '{t}' comes from {r}#{}, the output '{}' from {r}#{}; they must come from the same article",
            other.number,
            a.output,
            article.number,
            r = a.regulation,
        )),
        Some(_) => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{reduction, stream};
    use std::path::Path;

    const STREAM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");
    const CELL: &str = include_str!("../tests/fixtures/cells/instantie/lexostatuses.yaml");
    const CELL_DEF: &str = include_str!("../tests/fixtures/processes/instantie/process.yaml");
    const REG_STREAM: &str = include_str!("../tests/fixtures/chronicles/test_registers.yaml");
    const REG_CELL: &str = include_str!("../tests/fixtures/cells/register/lexostatuses.yaml");

    fn service() -> LawExecutionService {
        regulations::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation"))
            .unwrap()
            .service
    }

    fn run(stream_text: &str, cell_text: &str) -> Result<(), Vec<String>> {
        run_with(stream_text, cell_text, CELL_DEF)
    }

    /// The checks on the cell, and on the portal of the process (`cell_def`
    /// is a `process.yaml`).
    fn run_with(stream_text: &str, cell_text: &str, cell_def: &str) -> Result<(), Vec<String>> {
        let s = stream::parse(stream_text, "stream")?;
        let c = reduction::parse(cell_text, "cell")?;
        let d = crate::config::ProcessDefinition::parse(cell_def, "process.yaml")?;
        let streams = [s];
        let mut errors = check(&streams, &c, &service()).err().unwrap_or_default();
        if let Some(p) = &d.portal {
            let paths = crate::channel::portal_intake_paths(&d);
            errors.extend(portal(&streams, &c, p, &service(), &paths));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn portal_fails_with(cell_def: &str, expected: &str) {
        let errors = run_with(STREAM, CELL, cell_def).unwrap_err();
        assert!(
            errors.iter().any(|f| f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    }

    fn register(cell_text: &str) -> Result<(), Vec<String>> {
        let s = stream::parse(REG_STREAM, "stream")?;
        let c = reduction::parse(cell_text, "cell")?;
        check(&[s], &c, &service())
    }

    fn register_fails_with(cell_text: &str, expected: &str) {
        let errors = register(cell_text).unwrap_err();
        assert!(
            errors.iter().any(|f| f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    }

    fn fails_with(stream_text: &str, cell_text: &str, expected: &str) {
        let errors = run(stream_text, cell_text).unwrap_err();
        assert!(
            errors.iter().any(|f| f.contains(expected)),
            "expected '{expected}' in {errors:?}"
        );
    }

    #[test]
    fn the_fixtures_pass() {
        run(STREAM, CELL).unwrap();
    }

    // 1. Schema.
    #[test]
    fn stream_schema_fails() {
        fails_with(
            &STREAM.replace("intake: portaal", "intake: Portaal"),
            CELL,
            "/events/0/intake",
        );
    }

    #[test]
    fn cell_config_schema_fails() {
        fails_with(
            STREAM,
            &CELL.replace("pick: latest", "pick: eerste"),
            "pick",
        );
    }

    // 2. A derivation points at something that exists.
    #[test]
    fn derivation_to_unknown_parameter() {
        let cell = CELL.replace("bevat_naam: {filled", "bevat_naampje: {filled");
        fails_with(
            STREAM,
            &cell,
            "derivation 'bevat_naampje': 'bevat_naampje' is not a parameter",
        );
    }

    #[test]
    fn derivation_to_unknown_field_path() {
        let cell = CELL.replace("{filled: content.naam}", "{filled: content.naam_x}");
        fails_with(STREAM, &cell, "field path 'content.naam_x' does not exist");
    }

    #[test]
    fn table_must_be_a_field() {
        let cell = CELL.replace(
            "{table: content.organen, one_row",
            "{table: content, one_row",
        );
        fails_with(STREAM, &cell, "field path 'content' is not a table field");
        // A plain field is not a table either.
        let cell = CELL.replace(
            "{table: content.organen, one_row",
            "{table: content.naam, one_row",
        );
        fails_with(
            STREAM,
            &cell,
            "field path 'content.naam' is not a table field",
        );
    }

    #[test]
    fn table_derivation_reads_a_declared_column() {
        let cell = CELL.replace("each_row: zetels}", "each_row: stoelen}");
        fails_with(
            STREAM,
            &cell,
            "column 'stoelen' is not among the columns of table 'content.organen'",
        );
        let cell = CELL.replace("only_where: samengevoegd}", "only_where: gebundeld}");
        fails_with(STREAM, &cell, "column 'gebundeld' is not among the columns");
    }

    #[test]
    fn filter_without_event() {
        let cell = CELL.replace("subtype: aanvraag, root", "subtype: melding, root");
        fails_with(STREAM, &cell, "the filter selects no event");
    }

    #[test]
    fn filter_with_unknown_input() {
        let cell = CELL.replace("root: $root}", "root: $zaak}");
        fails_with(STREAM, &cell, "'zaak' is not an input");
    }

    #[test]
    fn legal_basis_with_a_paragraph() {
        // Article 1 has paragraph 1 ("1. Een aanvraag ..."); the assessment
        // compares by article, even if the legal basis names a paragraph.
        let stream = STREAM.replace(
            "      - testregeling_aanvraag#1\n",
            "      - testregeling_aanvraag#1 lid 1\n",
        );
        assert_ne!(stream, STREAM);
        run(&stream, CELL).unwrap();
        let stream = STREAM.replace(
            "      - testregeling_aanvraag#1\n",
            "      - testregeling_aanvraag#1 lid 9\n",
        );
        fails_with(
            &stream,
            CELL,
            "legal basis 'testregeling_aanvraag#1 lid 9': article 1 has no paragraph 9",
        );
    }

    #[test]
    fn legal_basis_that_does_not_exist() {
        let stream = STREAM.replace(
            "- testregeling_aanvraag#1",
            "- testregeling_aanvraag#1\n      - testregeling_aanvraag#7",
        );
        fails_with(&stream, CELL, "has no article 7");
    }

    #[test]
    fn legal_basis_of_a_bound_effective_at_exists() {
        let stream = STREAM.replace(
            "legal_basis: [testregeling_aanvraag#1]",
            "legal_basis: [testregeling_aanvraag#8]",
        );
        assert_ne!(stream, STREAM);
        fails_with(&stream, CELL, "has no article 8");
    }

    // 3. No orphan field.
    #[test]
    fn orphan_field() {
        let stream = STREAM.replace(
            "      - {field: content.rekeningnummer, reason: voor de betaling}\n",
            "",
        );
        fails_with(&stream, CELL, "orphan field 'content.rekeningnummer'");
    }

    #[test]
    fn not_reduced_covers_a_branch() {
        // `core` is in not_reduced as a whole: no orphan field below it.
        let s = stream::parse(STREAM, "s").unwrap();
        assert!(s.events[0].not_reduced.iter().any(|n| n.field == "core"));
        run(STREAM, CELL).unwrap();
    }

    #[test]
    fn not_reduced_to_unknown_field() {
        let stream = STREAM.replace(
            "{field: content.rekeningnummer,",
            "{field: content.rekening,",
        );
        fails_with(&stream, CELL, "not_reduced names 'content.rekening'");
    }

    // 7. The name of the case state belongs to the runtime.
    #[test]
    fn the_case_state_belongs_to_the_runtime() {
        // The first definition is now named case_state.
        let i = CELL.find("- name: ").unwrap() + "- name: ".len();
        let end = CELL[i..].find('\n').unwrap() + i;
        let cell = format!("{}case_state{}", &CELL[..i], &CELL[end..]);
        fails_with(STREAM, &cell, "that name belongs to the runtime");
    }

    // 4. No name collision.
    #[test]
    fn duplicate_derivation_across_two_lexostatuses() {
        let extra = "  - name: tweede\n    inputs: [{name: root, type: string}]\n    reduction:\n      chronicle: test_kroniek\n      filter: {name: aanvraag_ontvangen, root: $root}\n      pick: latest\n      derivations:\n        bevat_naam: {filled: core.aanvrager.naam}\n";
        let cell = format!("{CELL}{extra}");
        fails_with(
            STREAM,
            &cell,
            "parameter 'bevat_naam' gets more than one derivation",
        );
    }

    #[test]
    fn duplicate_derivation_within_a_lexostatus() {
        let cell = CELL.replace(
            "        bevat_aanduiding: {filled: content.aanduiding}\n",
            "        bevat_aanduiding: {filled: content.aanduiding}\n        bevat_aanduiding: {filled: content.naam}\n",
        );
        // The YAML reader already rejects a duplicate key, and names it.
        fails_with(
            STREAM,
            &cell,
            "duplicate entry with key \"bevat_aanduiding\"",
        );
    }

    #[test]
    fn portal_with_output_outside_the_legal_basis() {
        let cell_def = CELL_DEF.replace(
            "regulation: testregeling_aanvraag\n    output: aanvraag_volledig",
            "regulation: testregeling_awb\n    output: in_verzuim",
        );
        portal_fails_with(&cell_def, "is not in the legal basis of event");
    }

    /// The cell definition with an offer from `testregeling_aanvraag`.
    fn with_offer(output: &str, deadline: Option<&str>) -> String {
        let deadline = deadline
            .map(|t| format!("    deadline: {t}\n"))
            .unwrap_or_default();
        CELL_DEF.replace(
            "  form:",
            &format!(
                "  offer:\n    regulation: testregeling_aanvraag\n    output: {output}\n{deadline}  form:"
            ),
        )
    }

    #[test]
    fn portal_with_offer_and_deadline_from_the_same_article() {
        run_with(
            STREAM,
            CELL,
            &with_offer("aanvraag_volledig", Some("aanvraag_tijdig")),
        )
        .unwrap();
        run_with(STREAM, CELL, &with_offer("aanvraag_volledig", None)).unwrap();
    }

    #[test]
    fn offer_with_unknown_output() {
        portal_fails_with(
            &with_offer("bestaat_niet", None),
            "portal.offer: regulation 'testregeling_aanvraag' has no output 'bestaat_niet'",
        );
    }

    #[test]
    fn offer_with_unknown_deadline() {
        portal_fails_with(
            &with_offer("aanvraag_volledig", Some("bestaat_niet")),
            "portal.offer: regulation 'testregeling_aanvraag' has no deadline output 'bestaat_niet'",
        );
    }

    /// The deadline goes along in the same run: another article would make
    /// the run of the output depend on what that article needs.
    #[test]
    fn offer_with_deadline_from_another_article() {
        portal_fails_with(
            &with_offer("aanvraag_volledig", Some("aanvraag_compleet")),
            "portal.offer: deadline 'aanvraag_compleet' comes from testregeling_aanvraag#2, the output 'aanvraag_volledig' from testregeling_aanvraag#1; they must come from the same article",
        );
    }

    #[test]
    fn portal_with_unknown_intake_path() {
        let stream = STREAM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        fails_with(&stream, CELL, "'$intake.eherkenning.bsn'");
    }

    #[test]
    fn no_portal_no_portal_check() {
        let cell_def = CELL_DEF.split("\nportal:").next().unwrap().to_string();
        run_with(STREAM, CELL, &cell_def).unwrap();
        // An intake path no portal delivers is then not an error either.
        let stream = STREAM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        run_with(&stream, CELL, &cell_def).unwrap();
    }

    #[test]
    fn portal_with_lexostatus_without_pick() {
        let cell = CELL.replace("      pick: latest\n", "");
        let errors = run(STREAM, &cell).unwrap_err();
        assert!(
            errors.iter().any(|f| f.contains("picks no gram (pick)")),
            "{errors:?}"
        );
        // And every derivation on the chosen gram reports it too.
        assert!(
            errors
                .iter()
                .any(|f| f.contains("derivation 'bevat_naam': reads the chosen gram")),
            "{errors:?}"
        );
    }

    // Derivations over a collection, with a filter per derivation.
    #[test]
    fn the_register_fixture_passes() {
        register(REG_CELL).unwrap();
    }

    /// The legal basis of a derivation makes a parameter valid that does not
    /// come from the legal basis of the event, and is itself checked like
    /// that of an event: article loaded, paragraph exists.
    #[test]
    fn the_legal_basis_of_a_derivation() {
        // A name the register's regulation does not know.
        let cell = REG_CELL.replace(
            "        jaar_van_mededeling:             # het jaartal van een datum in de kroniek\n",
            "        jaar:\n",
        );
        register_fails_with(
            &cell,
            "'jaar' is not a parameter of an article from the legal basis of event 'mededeling_gedaan' (testregeling_register#3)",
        );
        // With a legal basis that requires it, it is valid.
        let with = |legal_basis: &str| {
            cell.replace(
                "          year_of: datum\n",
                &format!("          year_of: datum\n          legal_basis: [{legal_basis}]\n"),
            )
        };
        register(&with("testregeling_afnemer#5")).unwrap();
        // An article that does not exist, or a paragraph the article does not have.
        register_fails_with(
            &with("testregeling_afnemer#9"),
            "derivation 'jaar': legal basis 'testregeling_afnemer#9': regulation 'testregeling_afnemer' has no article 9",
        );
        register_fails_with(
            &with("testregeling_afnemer#5 lid 7"),
            "derivation 'jaar': legal basis 'testregeling_afnemer#5 lid 7': article 5 has no paragraph 7",
        );
        // The legal basis is checked for an extra field too.
        let cell = REG_CELL.replace(
            "legal_basis: [testregeling_register#1]\n",
            "legal_basis: [testregeling_onbekend#1]\n",
        );
        register_fails_with(
            &cell,
            "derivation 'ingeschreven': legal basis 'testregeling_onbekend#1': regulation 'testregeling_onbekend' is not loaded",
        );
    }

    #[test]
    fn filter_per_derivation_selects_an_event() {
        let cell = REG_CELL.replace(
            "{name: aanduiding_geschrapt,",
            "{name: aanduiding_verloren,",
        );
        register_fails_with(
            &cell,
            "derivation 'is_geschrapt': the filter selects no event",
        );
    }

    #[test]
    fn filter_per_derivation_on_a_field_path_that_exists() {
        let cell = REG_CELL.replace(
            "{name: aanduiding_geschrapt, orgaan: $orgaan,",
            "{name: aanduiding_geschrapt, gebied: $orgaan,",
        );
        register_fails_with(
            &cell,
            "field path 'gebied' does not exist in event 'aanduiding_geschrapt'",
        );
        let cell = REG_CELL.replace(
            "aanduiding: $aanduiding, gebied: $gebied}",
            "aanduiding: $naam, gebied: $gebied}",
        );
        register_fails_with(&cell, "'naam' is not an input");
    }

    #[test]
    fn filter_and_sum_read_fields_so_no_orphan_field() {
        // Only the filter reads `lijst`, only the sum reads `zetels`.
        let cell = REG_CELL.replace(
            "        zetels_toegewezen: {filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, sum: zetels}\n",
            "",
        );
        register_fails_with(&cell, "orphan field 'lijst' in event 'uitslag_vastgesteld'");
        register_fails_with(
            &cell,
            "orphan field 'zetels' in event 'uitslag_vastgesteld'",
        );
    }

    #[test]
    fn derivation_on_the_gram_in_a_lexostatus_without_pick() {
        let cell = format!("{REG_CELL}        x: {{field: aanduiding}}\n");
        register_fails_with(
            &cell,
            "derivation 'x': reads the chosen gram, but the lexostatus picks none",
        );
    }

    #[test]
    fn extra_field_with_the_name_of_a_derivation() {
        let cell =
            format!("{CELL}      extra_fields:\n        bevat_naam: {{field: content.naam}}\n");
        fails_with(
            STREAM,
            &cell,
            "'bevat_naam' is both a derivation and an extra field",
        );
        let cell = format!("{CELL}      extra_fields:\n        name: {{field: content.naamx}}\n");
        fails_with(STREAM, &cell, "field path 'content.naamx' does not exist");
        // An extra field need not be a parameter.
        let cell = format!("{CELL}      extra_fields:\n        name: {{field: content.naam}}\n");
        run(STREAM, &cell).unwrap();
    }

    // 6. A list lexostatus.
    const LIST: &str = "  - name: werkvoorraad\n    inputs: []\n    reduction:\n      chronicle: test_kroniek\n      filter: {type: submission}\n      group_by: root\n      pick: latest\n      derivations:\n        naam: {field: content.naam}\n        aanvraagjaar: {field: content.aanvraagjaar}\n";

    #[test]
    fn list_has_columns_not_parameters() {
        // `naam` is not a parameter, and `aanvraagjaar` does not collide with
        // the derivation in aanvraag_inhoud: a list never goes to the engine.
        run(STREAM, &format!("{CELL}{LIST}")).unwrap();
    }

    #[test]
    fn without_selects_an_event() {
        let list = LIST.replace(
            "      pick: latest\n",
            "      without: {stage: BESLUIT}\n      pick: latest\n",
        );
        fails_with(
            STREAM,
            &format!("{CELL}{list}"),
            "lexostatus 'werkvoorraad': without selects no event in chronicle 'test_kroniek'",
        );
        // If it selects an event, it also reads its field paths.
        let list = LIST.replace("      pick: latest\n", "      without: {name: aanvraag_ontvangen, content.bestaat_niet: x}\n      pick: latest\n");
        fails_with(
            STREAM,
            &format!("{CELL}{list}"),
            "without filters on field path 'content.bestaat_niet'",
        );
    }

    #[test]
    fn portal_does_not_assess_a_list() {
        let cell = format!("{CELL}{LIST}");
        let cell_def = CELL_DEF.replace("lexostatus: aanvraag_inhoud", "lexostatus: werkvoorraad");
        let errors = run_with(STREAM, &cell, &cell_def).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|f| f.contains("'werkvoorraad' is a list")),
            "{errors:?}"
        );
    }
}
