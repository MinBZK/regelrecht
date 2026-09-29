//! Reading configuration from disk, in one place: a file with its name in
//! every message, a YAML document validated against its schema, and the
//! files and subdirectories of a directory. An error while reading a
//! directory is never skipped: a file that cannot be read there is a
//! message, not silence.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::schema::{self, Kind};

/// The text of a file, with the name under which messages refer to it.
pub fn read(path: &Path) -> Result<(String, String), String> {
    let source = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{source}: {e}"))?;
    Ok((text, source))
}

/// Read a file and convert it with `parse(text, source)`.
pub fn load<T>(
    path: &Path,
    parse: impl FnOnce(&str, &str) -> Result<T, Vec<String>>,
) -> Result<T, Vec<String>> {
    let (text, source) = read(path).map_err(|e| vec![e])?;
    parse(&text, &source)
}

/// A YAML document without a schema, converted to `T`. Every message names
/// `source`.
pub fn yaml<T: DeserializeOwned>(text: &str, source: &str) -> Result<T, Vec<String>> {
    serde_yaml_ng::from_str(text).map_err(|e| vec![format!("{source}: not valid YAML: {e}")])
}

/// A YAML document, validated against its schema: the YAML tree (which keeps
/// the order of the document) and the same as JSON. Every message names
/// `source`.
pub fn yaml_document(
    text: &str,
    source: &str,
    kind: Kind,
) -> Result<(serde_yaml_ng::Value, Value), Vec<String>> {
    let yaml: serde_yaml_ng::Value = self::yaml(text, source)?;
    let document: Value =
        serde_json::to_value(&yaml).map_err(|e| vec![format!("{source}: {e}")])?;
    schema::validate(kind, &document).map_err(|f| {
        f.into_iter()
            .map(|f| format!("{source}: {f}"))
            .collect::<Vec<_>>()
    })?;
    Ok((yaml, document))
}

/// Read a YAML definition, validate it against its schema and convert it.
pub fn definition<T: DeserializeOwned>(
    text: &str,
    source: &str,
    kind: Kind,
) -> Result<T, Vec<String>> {
    let (_, document) = yaml_document(text, source, kind)?;
    serde_json::from_value(document).map_err(|e| vec![format!("{source}: {e}")])
}

/// The paths in a directory, sorted. An entry that cannot be read is an
/// error.
fn content(map: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for item in std::fs::read_dir(map).map_err(|e| format!("{}: {e}", map.display()))? {
        paths.push(item.map_err(|e| format!("{}: {e}", map.display()))?.path());
    }
    paths.sort();
    Ok(paths)
}

/// The `.yaml` and `.yml` files in a directory, sorted.
pub fn yaml_files(map: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(content(map)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
        .collect())
}

/// The subdirectories of a directory that contain `file`, sorted.
pub fn dirs_with(map: &Path, file: &str) -> Result<Vec<PathBuf>, String> {
    Ok(content(map)?
        .into_iter()
        .filter(|p| p.join(file).is_file())
        .collect())
}
