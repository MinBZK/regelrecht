//! Who supplies a parameter, according to the law: `origin` on a parameter and
//! `origins` in implementing policy (RFC-043).
//!
//! The law gives the origin of each parameter with a legal basis. Implementing
//! policy of the process's actor can override it. At startup the
//! process checks that every parameter the caller of an executed
//! output (assessment, offer, every output of the decision) must supply has a
//! supplier that fits its origin:
//!
//! | origin | supplier |
//! |---|---|
//! | `BELANGHEBBENDE` | a derivation of an own lexostatus that reads only submissions (`type: submission`: what the applicant provides, its content or its login); the window (`rol: TIJDVAK`) for the offer also the choice in the portal |
//! | `DOSSIER` | a derivation of an own lexostatus that reads only other grams of the own actor (the course of the case), or the state at decision |
//! | `OORDEEL` | the decision form, only for the decision, and no one else |
//! | `REGISTER` | a synthesis source that supplies the parameter (under the name the process's synthesis gives it), and that keeps a chronicle with a legal basis in `register` |
//! | `KANAAL` | a derivation that reads only `$intake` |
//!
//! A parameter that has a supplier that does not fit its origin is
//! always an error, also next to a supplier that does fit: the source is then
//! wrong. If it has no supplier, that is an error, except with
//! `required: false`: then the engine does not get it, and computes with an
//! unknown value (RFC-036); that is a warning. A parameter without
//! `origin` is an error: who supplies it cannot be traced (RFC-047 made the
//! check strict for every process). A `BELANGHEBBENDE` parameter without `required: false` is a
//! warning (RFC-036), except the window.
//!
//! What the runtime cannot verify (a source with a url, an internal cell that
//! is not running) counts as a supplier, with a warning that says why it
//! cannot be verified.
//!
//! [`validate`] checks the shape of `origin` and `origins` in a regulation
//! at load time, with the file in the message: an invalid value does not stop
//! the engine (see `Declared` in law-model), but it does stop the runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use regelrecht_engine::{Article, LawExecutionService, RegulatoryLayer};
use regelrecht_law_model::{
    ArticleBasedLaw, Declared, Origin, OriginOverride, OriginRole, OriginValue, Parameter,
};

use crate::authority;
use crate::cell::Cell;
use crate::config::{ActionDefinition, ActionKind, ProcessDefinition, RowsDefinition, Verdict};
use crate::reduction::{Derivation, Filter, LexostatusDefinition};
use crate::regulations::{self, Required};
use crate::stream::{Binding, Event, EventAttribute, Stream};

mod check;
mod delivery;
mod shape;

#[cfg(test)]
mod tests;

pub use check::{check, group, label_from, verdicts};
pub use delivery::Execution;

// What the delivery check uses.
use delivery::{beforehand_known, supplier, SupplierOutcome, Suppliers};
pub use shape::{overwrites, parameter, validate, Overwrites};

/// The origin in force for a parameter, and where it is stated.
#[derive(Debug, Clone, PartialEq)]
pub struct InForce {
    pub origin: Origin,
    /// The article of the policy that overrides it; `None` if it comes from
    /// the law.
    pub policy: Option<String>,
}

impl InForce {
    /// For a message: `REGISTER, register een_registerwet, grondslag x#1`.
    pub fn description(&self) -> String {
        let mut s = self.origin.waarde.as_str().to_string();
        if let Some(r) = &self.origin.register {
            s.push_str(&format!(", register {r}"));
        }
        if let Some(r) = self.origin.rol {
            s.push_str(&format!(", rol {}", r.as_str()));
        }
        s.push_str(&format!(", grondslag {}", self.origin.grondslag));
        if let Some(b) = &self.policy {
            s.push_str(&format!(", from {b}"));
        }
        s
    }

    /// Whether the parameter is the window of the requested decision order
    /// (`rol: TIJDVAK`). Apart from the legal basis: Awb 4:2 lid 1 is also the
    /// legal basis of other parts of the application.
    pub fn is_window(&self) -> bool {
        self.origin.rol == Some(OriginRole::Tijdvak)
    }
}

/// What the check yields.
#[derive(Debug, Default)]
pub struct Check {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// Per execution the parameters with their origin in force, in the order
    /// of declaration; for an action over all outputs together,
    /// under the name of the action.
    pub parameters: BTreeMap<String, Vec<(Required, Option<InForce>)>>,
    /// The parameter of the offer article that is the window: `rol:
    /// TIJDVAK`.
    pub window: Option<String>,
}
