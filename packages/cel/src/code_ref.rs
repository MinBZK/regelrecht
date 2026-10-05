//! Where the code decides what the law or the policy should say: a place in
//! the runtime that carries knowledge of a stage, an origin or a case
//! (spec "Opbouw"). The map colours a node that rests on one, and points to
//! the code instead of to a configuration. [`code_ref!`] records the file and
//! line where it is written, so the reference cannot drift from the code.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CodeRef {
    /// The file, relative to the repository root.
    #[serde(serialize_with = "repository_path")]
    pub file: &'static str,
    pub line: u32,
    /// What the code decides there that is not in the law or the policy.
    pub reason: &'static str,
}

/// `file!()` is relative to the cargo workspace (`packages/`); the map names
/// files from the repository root.
fn repository_path<S: serde::Serializer>(file: &&'static str, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("packages/{file}"))
}

/// A [`CodeRef`] to the line it is written on.
#[macro_export]
macro_rules! code_ref {
    ($reason:expr) => {
        $crate::code_ref::CodeRef {
            file: file!(),
            line: line!(),
            reason: $reason,
        }
    };
}
