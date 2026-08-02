//! # regelrecht-law-model
//!
//! The canonical Rust representation of the law-YAML document model — the shape
//! of the files under `corpus/regulation/`. This crate is a dependency-light
//! leaf (serde + serde_json, no engine/runtime deps) that every consumer of the
//! law format can depend on, so the model is defined **once** instead of being
//! re-derived per crate.
//!
//! Scope: document structs/enums plus allocation-free accessors. It deliberately
//! contains no YAML loading, security limits or evaluation logic — the engine
//! owns those (`regelrecht_engine::article::LawLoad` for loading, the resolver
//! for execution) and re-exports these types so existing paths keep working.
//!
//! The JSON schema under `schema/` remains the authoritative specification
//! (RFC-013); this crate is one conforming representation of it.

mod header;
mod model;
mod value;

pub use header::{parse_law_header, LawHeader};
pub use model::{
    Action, ActionOperation, ActionValue, Article, ArticleBasedLaw, ArticleReference,
    ArticleRequirement, AuthorityType, Case, CompetentAuthority, Declaration, DeclaredProperty,
    Definition, Execution, FieldLegalBasis, HookDeclaration, HookFilter, HookPoint,
    ImplementsDeclaration, Input, LegalBasis, MachineReadable, Marking, MarkingResolution,
    OpenTerm, OpenTermDefault, Output, OverrideDeclaration, Parameter, Placement,
    PlacementContainer, Preamble, ProcedureAppliesTo, ProcedureDefinition, Produces, ResolveSpec,
    Source, Stage, StageRequirement, Temporal, TypeSpec, UntranslatableEntry,
};
pub use value::{Operation, ParameterType, RegulatoryLayer, Value};
