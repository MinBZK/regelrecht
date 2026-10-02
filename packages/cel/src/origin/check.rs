//! The check at startup: per executed output every parameter with
//! its origin and supplier, and the verdicts of a decision.

use super::*;

/// The parameters the caller of an output must supply, in the
/// order in which the article (and then every called article) declares
/// them.
fn in_order(service: &LawExecutionService, regulation: &str, article: &Article) -> Vec<Required> {
    let required = regulations::required_parameters(service, regulation, article);
    let mut out: Vec<Required> = Vec::new();
    for p in article.get_parameters() {
        if let Some(b) = required.get(&p.name) {
            out.push(b.clone());
        }
    }
    for b in required.values() {
        if !out.iter().any(|u| u.name == b.name) {
            out.push(b.clone());
        }
    }
    out
}

/// The outputs the process executes: the assessment, the offer and every
/// output of the decision (RFC-043: "every outcome").
fn executions(d: &ProcessDefinition) -> Vec<(Execution<'_>, &str, &str)> {
    let mut out: Vec<(Execution<'_>, &str, &str)> = Vec::new();
    if let Some(p) = &d.portal {
        out.push((
            Execution::Assessment,
            &p.assessment.regulation,
            &p.assessment.output,
        ));
        if let Some(a) = &p.offer {
            out.push((Execution::Offer, &a.regulation, &a.output));
        }
    }
    for h in d.handling.iter().flat_map(|b| b.actions.iter()) {
        if matches!(h.kind, ActionKind::FollowUp { .. }) {
            continue;
        }
        for u in h.outputs.iter().chain(h.assessments.iter()) {
            out.push((Execution::Action(h), &h.regulation, u));
        }
    }
    out
}

/// Check the origin of every parameter of the outputs the
/// process executes. `cells` are the cells of the runtime, for the
/// register sources.
pub fn check(
    d: &ProcessDefinition,
    cell: &Cell,
    cells: &BTreeMap<String, Arc<Cell>>,
    service: &LawExecutionService,
) -> Check {
    let mut c = Check::default();
    let authority = authority::own(d, service);
    let overwrites = match overwrites(service, authority.as_deref()) {
        Ok(o) => o,
        Err(f) => {
            c.errors.extend(f);
            return c;
        }
    };
    // One message per parameter of an article, even if more executions
    // ask for it.
    let mut reported: BTreeSet<(String, String, &'static str)> = BTreeSet::new();
    // What cannot be verified, per source and reason: the parameters with it.
    let mut unverifiable: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (execution, regulation, output) in executions(d) {
        // An output that does not exist is reported by the check on the portal
        // or the decision.
        let Some(article) = service
            .resolver()
            .get_article_by_output(regulation, output, None)
        else {
            continue;
        };
        let suppliers = Suppliers::of(d, cell, execution);
        let list = c.parameters.entry(execution.name()).or_default();
        let mut new = Vec::new();
        for b in in_order(service, regulation, article) {
            if list.iter().any(|(earlier, _)| *earlier == b) {
                continue;
            }
            let Some(p) = parameter(service, &b) else {
                continue;
            };
            let law = b
                .article
                .split_once('#')
                .map(|(r, _)| r)
                .unwrap_or(regulation);
            let g = overwrites.in_force(law, p);
            new.push((b, p, g));
        }
        for (b, p, g) in new {
            let mut report = |kind: &'static str, text: String, error: bool| {
                if reported.insert((b.article.clone(), b.name.clone(), kind)) {
                    if error {
                        c.errors.push(text);
                    } else {
                        c.warnings.push(text);
                    }
                }
            };
            check_parameter(
                ParameterPlace {
                    execution,
                    b: &b,
                    p,
                    g: g.as_ref(),
                },
                &suppliers,
                cells,
                service,
                &mut report,
                &mut unverifiable,
            );
            c.parameters
                .entry(execution.name())
                .or_default()
                .push((b, g));
        }
    }
    for (reason, names) in unverifiable {
        let names: Vec<String> = names.into_iter().map(|n| format!("'{n}'")).collect();
        c.warnings.push(format!(
            "origin of {} cannot be verified: {reason}",
            names.join(", ")
        ));
    }
    window(d, &mut c);
    c
}

/// A parameter an execution asks for, with what the check knows
/// about it.
struct ParameterPlace<'a> {
    execution: Execution<'a>,
    b: &'a Required,
    p: &'a Parameter,
    g: Option<&'a InForce>,
}

/// The checks on a parameter: the offer rule, the origin itself and its
/// supplier.
fn check_parameter(
    place: ParameterPlace<'_>,
    l: &Suppliers,
    cells: &BTreeMap<String, Arc<Cell>>,
    service: &LawExecutionService,
    report: &mut impl FnMut(&'static str, String, bool),
    unverifiable: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let ParameterPlace { execution, b, p, g } = place;
    if execution.is_offer() && !beforehand_known(g) {
        let provenance = g
            .map(InForce::description)
            .unwrap_or_else(|| "no origin".into());
        report(
            "aanbod-regel",
            format!(
                "offer: condition relies on '{}' ({provenance}), which is not known beforehand",
                b.name
            ),
            true,
        );
    }
    let Some(g) = g else {
        report(
            "zonder",
            format!(
                "provenance: parameter '{}' of {} has no origin; who supplies it cannot be traced",
                b.name, b.article
            ),
            true,
        );
        return;
    };
    let where_ = format!(
        "parameter '{}' of {} ({})",
        b.name,
        b.article,
        g.description()
    );
    // A legal basis in a regulation that is not loaded may be right; the
    // runtime just cannot verify it.
    if let Err(f) = regulations::article(service, &g.origin.grondslag) {
        report(
            "grondslag",
            format!("provenance: {where_}: {f}; cannot be verified"),
            false,
        );
    }
    if let Some(r) = &g.origin.register {
        if service.resolver().get_law(r).is_none() {
            report(
                "register",
                format!("provenance: {where_}: register '{r}' is not a loaded regulation"),
                true,
            );
        }
    }
    if g.origin.waarde == OriginValue::Belanghebbende && !g.is_window() && p.required != Some(false)
    {
        report(
            "required",
            format!(
                "provenance: parameter '{}' of {} comes from the interested party, but has no required: false (RFC-036)",
                b.name, b.article
            ),
            false,
        );
    }
    match supplier(execution, &b.name, g, l, cells) {
        SupplierOutcome::Fits { warnings } => {
            for w in warnings {
                unverifiable.entry(w).or_default().insert(b.name.clone());
            }
        }
        SupplierOutcome::Wrong(reason) => report(
            "leverancier",
            format!("{}: wrong source for {where_}: {reason}", execution.name()),
            true,
        ),
        SupplierOutcome::Missing(reason) => {
            let text = format!("{}: no supplier for {where_}{reason}", execution.name());
            if p.required == Some(false) {
                report(
                    "leverancier",
                    format!(
                        "{text}; required: false, so the engine does not get it and computes with an unknown value (RFC-036)"
                    ),
                    false,
                );
            } else {
                report("leverancier", text, true);
            }
        }
    }
}

/// The window of the offer: at most one parameter with `rol: TIJDVAK`,
/// and that one asks for `offer.windows`; windows without such a parameter are
/// an error too.
fn window(d: &ProcessDefinition, c: &mut Check) {
    let Some(offer) = d.portal.as_ref().and_then(|p| p.offer.as_ref()) else {
        return;
    };
    let names: Vec<String> = c
        .parameters
        .get(&Execution::Offer.name())
        .into_iter()
        .flatten()
        .filter(|(_, g)| g.as_ref().is_some_and(InForce::is_window))
        .map(|(b, _)| b.name.clone())
        .collect();
    match (names.as_slice(), offer.windows.is_some()) {
        ([], false) => {}
        ([], true) => c.errors.push(format!(
            "offer: windows, but {} asks for no window (a parameter with origin BELANGHEBBENDE and rol TIJDVAK)",
            offer.regulation
        )),
        ([name], true) => c.window = Some(name.clone()),
        ([name], false) => c.errors.push(format!(
            "offer: the window '{name}' (rol TIJDVAK) asks for offer.windows: the output of the policy with the windows the portal offers"
        )),
        (more, _) => c.errors.push(format!(
            "offer: more than one window ({}); the portal offers one",
            more.join(", ")
        )),
    }
}

/// The decision form: the parameters of the decision with origin
/// `OORDEEL`, in the order of declaration. The label is the description of
/// the parameter, or the part after "Naam:" if that is there, and otherwise the name;
/// the group is the article of the legal basis.
pub fn verdicts(c: &Check, service: &LawExecutionService, action: &str) -> Vec<Verdict> {
    c.parameters
        .get(action)
        .into_iter()
        .flatten()
        .filter_map(|(b, g)| {
            let g = g
                .as_ref()
                .filter(|g| g.origin.waarde == OriginValue::Oordeel)?;
            let p = parameter(service, b)?;
            Some(Verdict {
                parameter: b.name.clone(),
                label: label(p),
                group: group(service, &g.origin.grondslag),
                explanation: None,
            })
        })
        .collect()
}

/// The label of a parameter: the part of the description after "Naam:",
/// otherwise the whole description, otherwise the name.
fn label(p: &Parameter) -> String {
    let text = label_from(p.description.as_deref().unwrap_or_default());
    if text.is_empty() {
        p.name.clone()
    } else {
        text
    }
}

/// The label from a description: the part after "Naam:", otherwise the whole
/// description, without a period at the end.
pub fn label_from(description: &str) -> String {
    let text = description.trim();
    let text = match text.rsplit_once("Naam:") {
        Some((_, name)) => name.trim(),
        None => text,
    };
    let text = text.strip_suffix('.').unwrap_or(text).trim();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The group of a verdict: the regulation and the article of its legal basis.
pub fn group(service: &LawExecutionService, legal_basis: &str) -> Option<String> {
    let g = regulations::parse(legal_basis).ok()?;
    let name = service
        .resolver()
        .get_law(g.regulation)
        .and_then(|l| l.name.clone())
        .unwrap_or_else(|| crate::form::readable(g.regulation));
    Some(format!("{name}, artikel {}", g.article))
}
