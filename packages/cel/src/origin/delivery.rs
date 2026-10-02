//! Who can supply a parameter: a derivation of an own lexostatus, a
//! synthesis source, the form, the state at decision or the choice in the
//! portal, and whether that supplier fits the origin.

use super::*;

/// An output the process executes.
#[derive(Debug, Clone, Copy)]
pub enum Execution<'a> {
    Assessment,
    Offer,
    /// An action in a case (not a follow-up: that computes on the input of
    /// the recorded decision).
    Action(&'a ActionDefinition),
}

impl Execution<'_> {
    pub(super) fn name(self) -> String {
        match self {
            Execution::Assessment => "assessment".into(),
            Execution::Offer => "offer".into(),
            Execution::Action(h) => h.name.clone(),
        }
    }

    pub(super) fn is_offer(self) -> bool {
        matches!(self, Execution::Offer)
    }

    pub(super) fn is_action(self) -> bool {
        matches!(self, Execution::Action(_))
    }
}

/// What a derivation of an own lexostatus reads, judged by the grams that can
/// pass its filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Read {
    /// Only `$intake` of a submission: the login.
    Channel,
    /// Only submissions: what the applicant provides.
    Submission,
    /// Only other grams of the own actor: the course of the case.
    Course,
    /// Both, or nothing that can be verified statically.
    Undetermined,
}

impl Read {
    pub(super) fn words(self) -> &'static str {
        match self {
            Read::Channel => "the login ($intake)",
            Read::Submission => "what the applicant submitted",
            Read::Course => "the course of the case",
            Read::Undetermined => "submissions and the course of the case mixed together",
        }
    }
}

/// A synthesis source that supplies a parameter.
#[derive(Debug, Clone)]
pub(super) struct SourceDelivery {
    cell: String,
    lexostatus: String,
    /// The url of a source outside this runtime; `None`: internal.
    url: Option<String>,
}

/// A supplier of a parameter for an execution.
#[derive(Debug, Clone)]
pub(super) enum Delivery {
    /// A derivation of an own lexostatus, or a table per row from an
    /// own extra field.
    Own {
        lexostatus: String,
        read: Read,
    },
    /// The state at decision.
    ActionStatus,
    /// The choice of the window in the portal.
    Choice,
    /// The id of the decision the action acts on.
    Decision,
    Source(SourceDelivery),
}

impl Delivery {
    pub(super) fn words(&self) -> String {
        match self {
            Delivery::Own { lexostatus, read } => {
                format!("own lexostatus {lexostatus} ({})", read.words())
            }
            Delivery::ActionStatus => "the state at decision".into(),
            Delivery::Choice => "the choice in the portal".into(),
            Delivery::Decision => "the decision the action acts on".into(),
            Delivery::Source(b) => format!("synthesis source {}/{}", b.cell, b.lexostatus),
        }
    }

    /// Whether this supplier fits the origin (a register source still
    /// without the check on the register).
    pub(super) fn fits(&self, g: &InForce, execution: Execution<'_>) -> bool {
        match (self, g.origin.waarde) {
            (
                Delivery::Own {
                    read: Read::Channel,
                    ..
                },
                OriginValue::Kanaal | OriginValue::Belanghebbende,
            )
            | (
                Delivery::Own {
                    read: Read::Submission,
                    ..
                },
                OriginValue::Belanghebbende,
            )
            | (
                Delivery::Own {
                    read: Read::Course, ..
                },
                OriginValue::Dossier,
            )
            | (Delivery::Source(_), OriginValue::Register) => true,
            (Delivery::ActionStatus | Delivery::Decision, OriginValue::Dossier) => {
                execution.is_action()
            }
            (Delivery::Choice, OriginValue::Belanghebbende) => {
                execution.is_offer() && g.is_window()
            }
            _ => false,
        }
    }
}

/// What the process can supply for an execution, per parameter.
#[derive(Debug, Default)]
pub(super) struct Suppliers {
    per_parameter: BTreeMap<String, Vec<Delivery>>,
    /// Whether the policy offers windows with the offer (`offer.windows`).
    choice: bool,
}

/// The choice of the window, as a supplier.
pub(super) static CHOICE: Delivery = Delivery::Choice;

impl Suppliers {
    pub(super) fn add(&mut self, name: &str, l: Delivery) {
        self.per_parameter
            .entry(name.to_string())
            .or_default()
            .push(l);
    }

    pub(super) fn of(d: &ProcessDefinition, cell: &Cell, execution: Execution<'_>) -> Self {
        let mut l = Suppliers::default();
        let mut own: Vec<&str> = d.case_sources().map(|b| b.lexostatus.as_str()).collect();
        if let Some(p) = &d.portal {
            l.choice =
                execution.is_offer() && p.offer.as_ref().is_some_and(|a| a.windows.is_some());
            own.push(&p.assessment.lexostatus);
        }
        own.sort_unstable();
        own.dedup();
        for name in own {
            let Some(def) = cell.lexostatuses.lexostatus(name) else {
                continue;
            };
            for (param, derivation) in &def.reduction.derivations {
                l.add(
                    param,
                    Delivery::Own {
                        lexostatus: name.to_string(),
                        read: read(cell, &d.actor, def, derivation),
                    },
                );
            }
        }
        for b in d.other_sources() {
            for p in &b.parameters {
                l.add(
                    p,
                    Delivery::Source(SourceDelivery {
                        cell: b.cell.clone(),
                        lexostatus: b.lexostatus.clone(),
                        url: b.url.clone(),
                    }),
                );
            }
        }
        if let Execution::Action(h) = execution {
            for name in h.not_yet.keys() {
                l.add(name, Delivery::ActionStatus);
            }
            if let Some(p) = &h.decision_parameter {
                l.add(p, Delivery::Decision);
            }
        }
        // The synthesis per row supplies only to the execution that
        // executes it: the assessment or the decision.
        let rows: &[RowsDefinition] = match execution {
            Execution::Assessment => d.portal.as_ref().map(|p| p.assessment.rows.as_slice()),
            Execution::Action(h) => Some(h.rows.as_slice()),
            Execution::Offer => None,
        }
        .unwrap_or_default();
        for r in rows {
            l.rows(d, cell, r);
        }
        l
    }

    /// The synthesis per row: from an extra field of a source a delivery
    /// by that source, from an own table an own delivery, judged by what the
    /// extra field reads.
    pub(super) fn rows(&mut self, d: &ProcessDefinition, cell: &Cell, r: &RowsDefinition) {
        let source = d.other_sources().find(|b| {
            b.lexostatus == r.table.lexostatus && b.extra_fields.contains(&r.table.field)
        });
        let delivery = match source {
            Some(b) => Delivery::Source(SourceDelivery {
                cell: b.cell.clone(),
                lexostatus: b.lexostatus.clone(),
                url: b.url.clone(),
            }),
            None => {
                let read = cell
                    .lexostatuses
                    .lexostatus(&r.table.lexostatus)
                    .and_then(|def| {
                        def.all_derivations()
                            .find(|(name, _)| **name == r.table.field)
                            .map(|(_, a)| read(cell, &d.actor, def, a))
                    })
                    .unwrap_or(Read::Undetermined);
                Delivery::Own {
                    lexostatus: r.table.lexostatus.clone(),
                    read,
                }
            }
        };
        self.add(&r.parameter, delivery);
    }

    /// Who supplies a parameter. The choice in the portal supplies only the
    /// window.
    pub(super) fn of_parameter(&self, name: &str, window: bool) -> Vec<&Delivery> {
        let mut out: Vec<&Delivery> = self.per_parameter.get(name).into_iter().flatten().collect();
        if self.choice && window {
            out.push(&CHOICE);
        }
        out
    }
}

/// Whether a gram of this event can pass a filter, as far as that is
/// fixed without the input: a `$` value always fits, a field path if
/// the event has the field.
pub(super) fn can_fit(filter: &Filter, stream: &Stream, event: &Event) -> bool {
    filter.iter().all(|(key, value)| {
        let input = value.starts_with('$');
        match event.attribute(stream, key) {
            None => event.has_path(key),
            Some(EventAttribute::Fixed(w)) => {
                if input {
                    w.is_some()
                } else {
                    w == Some(value.as_str())
                }
            }
            Some(EventAttribute::Free) => true,
            Some(EventAttribute::Never) => false,
        }
    })
}

/// What a derivation reads: the events of the lexostatus's chronicle that
/// can pass the filter of the lexostatus and that of the derivation.
/// If it reads only `$intake` of a submission, it is the login.
pub(super) fn read(cell: &Cell, actor: &str, def: &LexostatusDefinition, a: &Derivation) -> Read {
    let events: Vec<(&Stream, &Event)> = cell
        .streams
        .iter()
        .filter(|s| s.chronicle == def.reduction.chronicle)
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(s, e)| {
            can_fit(&def.reduction.filter, s, e) && a.filter().is_none_or(|f| can_fit(f, s, e))
        })
        .collect();
    if events.is_empty() {
        return Read::Undetermined;
    }
    if events.iter().all(|(_, e)| e.is_submission()) {
        let paths = a.read_paths();
        let only_intake = !paths.is_empty()
            && events.iter().all(|(_, e)| {
                let intake: BTreeSet<String> = e
                    .leaves()
                    .into_iter()
                    .filter(|b| matches!(b.binding, Binding::Intake(_)))
                    .map(|b| b.path)
                    .collect();
                paths.iter().all(|p| intake.contains(*p))
            });
        return if only_intake {
            Read::Channel
        } else {
            Read::Submission
        };
    }
    if events
        .iter()
        .all(|(s, e)| !e.is_submission() && s.recording_actor == actor)
    {
        return Read::Course;
    }
    Read::Undetermined
}

/// Whether the offer may rely on a parameter: what is fixed beforehand is who
/// logs in (`KANAAL`), what a register knows (`REGISTER`) and which window the
/// applicant chooses (`BELANGHEBBENDE` with `rol: TIJDVAK`). Whether an application
/// is complete is only known after filling it in.
pub(super) fn beforehand_known(g: Option<&InForce>) -> bool {
    g.is_some_and(|g| {
        matches!(g.origin.waarde, OriginValue::Kanaal | OriginValue::Register) || g.is_window()
    })
}

/// How things stand with the supplier of a parameter.
#[derive(Debug)]
pub(super) enum SupplierOutcome {
    /// A supplier fits, possibly with what cannot be verified.
    Fits { warnings: Vec<String> },
    /// A supplier does not fit the origin, even if another one does
    /// fit.
    Wrong(String),
    /// No supplier; the text starts with `: ` or is empty.
    Missing(String),
}

/// Whether a parameter has a supplier that fits its origin, and
/// none that does not fit it.
pub(super) fn supplier(
    execution: Execution<'_>,
    name: &str,
    g: &InForce,
    l: &Suppliers,
    cells: &BTreeMap<String, Arc<Cell>>,
) -> SupplierOutcome {
    let deliveries = l.of_parameter(name, g.is_window());
    if g.origin.waarde == OriginValue::Oordeel {
        if !deliveries.is_empty() {
            let who: Vec<String> = deliveries.iter().map(|lv| lv.words()).collect();
            return SupplierOutcome::Wrong(format!(
                "the handler gives a verdict in the form of the action, but it comes from {}",
                who.join(" and ")
            ));
        }
        return if execution.is_action() {
            SupplierOutcome::Fits {
                warnings: Vec::new(),
            }
        } else {
            SupplierOutcome::Missing(
                ": the handler gives a verdict only at an action in the case".into(),
            )
        };
    }
    let mut wrong = Vec::new();
    let mut fits = false;
    let mut warnings = Vec::new();
    for lv in deliveries {
        if !lv.fits(g, execution) {
            wrong.push(format!("it comes from {}", lv.words()));
            continue;
        }
        match lv {
            Delivery::Source(source) => match register_source(source, g, cells) {
                Ok(w) => {
                    fits = true;
                    warnings.extend(w);
                }
                Err(r) => wrong.push(r),
            },
            _ => fits = true,
        }
    }
    if !wrong.is_empty() {
        SupplierOutcome::Wrong(wrong.join("; "))
    } else if fits {
        SupplierOutcome::Fits { warnings }
    } else {
        SupplierOutcome::Missing(String::new())
    }
}

/// Whether a synthesis source may supply a register parameter: its lexostatus
/// keeps a chronicle with a legal basis in the register of the origin.
/// Under which name the consumer asks for the fact is said by the synthesis of the
/// process (the translation belongs to the consumer). `Ok` with
/// a warning if that cannot be verified: a source with a url, or an
/// internal cell that does not run in this runtime.
pub(super) fn register_source(
    source: &SourceDelivery,
    g: &InForce,
    cells: &BTreeMap<String, Arc<Cell>>,
) -> Result<Option<String>, String> {
    let who = format!("synthesis source {}/{}", source.cell, source.lexostatus);
    let register = g.origin.register.as_deref().unwrap_or_default();
    if let Some(url) = &source.url {
        return Ok(Some(format!(
            "{who} runs outside this runtime ({url}); whether its lexostatus keeps a chronicle with a legal basis in '{register}' cannot be seen at startup"
        )));
    }
    let Some(cell) = cells.get(&source.cell) else {
        return Ok(Some(format!(
            "{who} has no url and does not run in this runtime; whether its lexostatus keeps a chronicle with a legal basis in '{register}' cannot be seen"
        )));
    };
    let Some(def) = cell.lexostatuses.lexostatus(&source.lexostatus) else {
        return Err(format!("{who} does not exist"));
    };
    // `shape` already stops a REGISTER without register.
    if let Some(register) = &g.origin.register {
        let tracks = cell
            .streams
            .iter()
            .filter(|s| s.chronicle == def.reduction.chronicle)
            .flat_map(|s| s.events.iter())
            .flat_map(|e| e.legal_basis.iter())
            .any(|gr| regulations::parse(gr).is_ok_and(|gr| gr.regulation == register));
        if !tracks {
            return Err(format!(
                "{who} keeps no chronicle with a legal basis in '{register}'"
            ));
        }
    }
    Ok(None)
}
