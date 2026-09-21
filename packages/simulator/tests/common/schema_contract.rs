//! De JSON-schema's van de chronolexografie als toets: `schema/v0.6.0/gram.json`,
//! `chronicle.json` en `world-snapshot.json`.
//!
//! Eén plek voor het laden en valideren, gedeeld door de tests van deze crate en
//! door die van de HTTP-laag (`packages/chrono-poc-web/tests/api.rs` neemt dit
//! bestand op met `#[path]`). Beide crates staan onder `packages/`, dus de weg
//! naar `schema/` is vanuit elk `CARGO_MANIFEST_DIR` dezelfde.
//!
//! De schema's verwijzen naar elkaar (`world-snapshot.json` naar `gram.json` en
//! `chronicle.json`) op hun `$id`. Die verwijzingen worden hier uit de checkout
//! beantwoord en niet over het netwerk: een test hoort te toetsen tegen het
//! schema in déze pull request, niet tegen wat er ooit gepubliceerd is.

#![allow(dead_code)]

use jsonschema::{Retrieve, Uri, Validator};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Het `$id`-voorvoegsel van de schema's in `schema/v0.6.0/`.
const BASE: &str =
    "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.6.0/schema/v0.6.0/";

/// De map met de schema's, in deze checkout.
pub fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("schema")
        .join("v0.6.0")
}

/// Lees één schema als JSON.
fn load(file: &str) -> Value {
    let path = schema_dir().join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is geen geldige JSON: {e}", path.display()))
}

/// Beantwoordt een verwijzing naar een zusterschema uit de checkout.
struct LocalSchemas;

impl Retrieve for LocalSchemas {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let file = uri
            .as_str()
            .strip_prefix(BASE)
            .ok_or_else(|| format!("verwijzing buiten schema/v0.6.0: {}", uri.as_str()))?;
        Ok(load(file))
    }
}

/// Bouw de validator voor één schemabestand.
fn build(file: &str) -> Validator {
    jsonschema::options()
        .with_retriever(LocalSchemas)
        .build(&load(file))
        .unwrap_or_else(|e| panic!("{file} is geen geldig JSON-schema: {e}"))
}

/// De validator voor één gram (`gram.json`).
pub fn gram() -> &'static Validator {
    static VALIDATOR: OnceLock<Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| build("gram.json"))
}

/// De validator voor één kroniekstroom-definitie (`chronicle.json`).
pub fn chronicle() -> &'static Validator {
    static VALIDATOR: OnceLock<Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| build("chronicle.json"))
}

/// De validator voor het beeld van de wereld (`world-snapshot.json`).
pub fn world_snapshot() -> &'static Validator {
    static VALIDATOR: OnceLock<Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| build("world-snapshot.json"))
}

/// De fouten van `value` tegen `validator`, elk als `pad: melding`. Leeg is
/// geldig.
pub fn errors(validator: &Validator, value: &Value) -> Vec<String> {
    validator
        .iter_errors(value)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect()
}

/// Faal met elke fout erbij als `value` niet valideert.
pub fn assert_valid(validator: &Validator, value: &Value, what: &str) {
    let errors = errors(validator, value);
    assert!(
        errors.is_empty(),
        "{what} valideert niet tegen het schema:\n  {}\n\n{}",
        errors.join("\n  "),
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// Elk gram in een beeld van de wereld, met waar het staat.
pub fn grams(snapshot: &Value) -> Vec<(String, &Value)> {
    let mut found = Vec::new();
    for cell in snapshot["cells"].as_array().into_iter().flatten() {
        for chronicle in cell["chronicles"].as_array().into_iter().flatten() {
            for (index, gram) in chronicle["grams"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                found.push((
                    format!(
                        "{}|{}|{index}",
                        cell["id"].as_str().unwrap_or("?"),
                        chronicle["stream"].as_str().unwrap_or("?")
                    ),
                    gram,
                ));
            }
        }
    }
    found
}

/// Toets een beeld van de wereld: het geheel tegen `world-snapshot.json`, en
/// elk gram erin los tegen `gram.json`.
///
/// Het tweede zit al in het eerste — het beeld verwijst voor zijn grammen naar
/// `gram.json` — maar los getoetst noemt een fout het gram en niet alleen een
/// pad diep in het beeld.
pub fn assert_snapshot_valid(snapshot: &Value, what: &str) {
    for (id, gram) in grams(snapshot) {
        assert_valid(self::gram(), gram, &format!("{what}: gram {id}"));
    }
    assert_valid(world_snapshot(), snapshot, &format!("{what}: het beeld"));
}
