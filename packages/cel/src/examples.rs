//! Examples: default data per action, for a test setup.
//!
//! `examples.yaml` of the deployment can name a JSON file per action (see
//! [`crate::config::ExamplesDefinition`]): logins through a channel, an
//! application and, per action in a case, a form. The frontend offers them
//! to prefill a form or to perform the action with them directly.
//! They are ordinary input, not facts: the runtime does not record them and
//! the action checks them like any other input.
//!
//! A value `"$today"` in a form becomes today's date when requested
//! ([`Examples::op`]). A decision, a publication or a
//! payment in the future is not a fact (the cell refuses an `effective_at` after
//! the recording), and an example with a fixed date goes stale.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::channel::Routes;
use crate::config::{ExamplesDefinition, ProcessDefinition};

/// The loaded examples of a process. Without an `examples` block: empty.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Examples {
    pub logins: Vec<LoginExample>,
    /// The `external` of an application.
    pub application: Option<Map<String, Value>>,
    /// Per action the `form`.
    pub actions: BTreeMap<String, Map<String, Value>>,
}

/// The value that becomes today's date in an example.
pub const TODAY: &str = "$today";

impl Examples {
    /// The examples as the frontend receives them: `"$today"` becomes
    /// `today` (YYYY-MM-DD).
    pub fn on(&self, today: &str) -> Self {
        let mut out = self.clone();
        for f in out.actions.values_mut() {
            for w in f.values_mut() {
                if w.as_str() == Some(TODAY) {
                    *w = Value::String(today.to_string());
                }
            }
        }
        out
    }
}

/// A login, with the file name without extension as its label: the channel, the
/// role if the file names one, and the fields of the channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoginExample {
    pub label: String,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub fields: BTreeMap<String, String>,
}

/// Read the examples; paths are relative to the directory of the process. Every error
/// is returned and names the path.
pub fn load(
    map: &Path,
    definition: &ExamplesDefinition,
    process: &ProcessDefinition,
) -> Result<Examples, Vec<String>> {
    let mut errors = Vec::new();
    let mut out = Examples::default();
    // The label is the key by which the frontend picks a login: unique.
    let mut labels: Vec<(String, &str)> = Vec::new();
    for path in &definition.logins {
        match login(map, path, process) {
            Ok(v) => {
                if let Some((_, earlier)) = labels.iter().find(|(l, _)| *l == v.label) {
                    errors.push(format!(
                        "examples {earlier} and {path} have the same label '{}'",
                        v.label
                    ));
                    continue;
                }
                labels.push((v.label.clone(), path));
                out.logins.push(v);
            }
            Err(f) => errors.push(f),
        }
    }
    if let Some(path) = &definition.application {
        out.application = object_under(map, path, "external")
            .map_err(|f| errors.push(f))
            .ok();
    }
    for (name, path) in &definition.actions {
        if let Ok(f) = object_under(map, path, "form").map_err(|f| errors.push(f)) {
            out.actions.insert(name.clone(), f);
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

fn read(map: &Path, path: &str) -> Result<Value, String> {
    let text =
        std::fs::read_to_string(map.join(path)).map_err(|e| format!("example {path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("example {path}: not valid JSON: {e}"))
}

/// A login as a channel accepts it, and valid. The channel is in
/// the file (`channel`), or is the only channel of the portal.
fn login(map: &Path, path: &str, process: &ProcessDefinition) -> Result<LoginExample, String> {
    let Value::Object(input) = read(map, path)? else {
        return Err(format!(
            "example {path}: expected an object with the fields of a channel"
        ));
    };
    let portal = process.channels_with(Routes::Portal);
    let channel = match input.get("channel").and_then(Value::as_str) {
        Some(k) => k.to_string(),
        None => match portal.as_slice() {
            [(id, _)] => id.to_string(),
            _ => {
                return Err(format!(
                    "example {path}: name the channel; the portal has {}",
                    portal.len()
                ))
            }
        },
    };
    let k = process.channels.get(&channel).ok_or_else(|| {
        format!("example {path}: channel '{channel}' is not listed under channels")
    })?;
    let fields = k
        .validate(&input)
        .map_err(|e| format!("example {path}: {e}"))?;
    let role = input
        .get("role")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(r) = &role {
        if process.roles.get(r).is_none_or(|d| d.channel != channel) {
            return Err(format!(
                "example {path}: role '{r}' does not log in through channel '{channel}'"
            ));
        }
    }
    let label = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    Ok(LoginExample {
        label,
        channel,
        role,
        fields,
    })
}

/// The object under `key` in a JSON object.
fn object_under(map: &Path, path: &str, key: &str) -> Result<Map<String, Value>, String> {
    match read(map, path)? {
        Value::Object(mut o) => match o.remove(key) {
            Some(Value::Object(content)) => Ok(content),
            _ => Err(format!(
                "example {path}: expected an object with '{key}' as an object"
            )),
        },
        _ => Err(format!(
            "example {path}: expected an object with '{key}' as an object"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn dir_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            std::fs::write(dir.path().join(name), content).unwrap();
        }
        dir
    }

    /// A process with a portal channel: an organization number of eight
    /// digits and a name.
    fn process() -> ProcessDefinition {
        let org = crate::channel::tests::channel(
            "Organisatie",
            "- {name: kvk, label: Nummer, pattern: '[0-9]{8}', message: een nummer heeft acht cijfers}\n- {name: persoon, label: Naam}\n",
            None,
            None,
        );
        let aanvrager = crate::channel::RoleDefinition {
            channel: "org".into(),
            routes: vec![crate::channel::Routes::Portal],
            label: None,
            legal_basis: None,
        };
        ProcessDefinition {
            id: "p".into(),
            actor: "a".into(),
            channels: [("org".to_string(), org)].into(),
            roles: [("aanvrager".to_string(), aanvrager)].into(),
            ..Default::default()
        }
    }

    fn definition(
        logins: &[&str],
        application: Option<&str>,
        decision: Option<&str>,
    ) -> ExamplesDefinition {
        ExamplesDefinition {
            logins: logins.iter().map(|s| s.to_string()).collect(),
            application: application.map(str::to_string),
            actions: decision
                .map(|b| BTreeMap::from([("besluit".to_string(), b.to_string())]))
                .unwrap_or_default(),
        }
    }

    #[test]
    fn valid_examples() {
        let dir = dir_with(&[
            (
                "login-een.json",
                r#"{"kvk": " 12345678 ", "persoon": "A. Tester"}"#,
            ),
            ("aanvraag.json", r#"{"external": {"naam": "Voorbeeld"}}"#),
            (
                "besluit.json",
                r#"{"form": {"feiten_vergaard": true, "besluitdatum": "$today"}}"#,
            ),
        ]);
        let v = load(
            dir.path(),
            &definition(
                &["login-een.json"],
                Some("aanvraag.json"),
                Some("besluit.json"),
            ),
            &process(),
        )
        .unwrap();
        assert_eq!(
            v.logins,
            [LoginExample {
                label: "login-een".into(),
                channel: "org".into(),
                role: None,
                fields: [
                    ("kvk".to_string(), "12345678".to_string()),
                    ("persoon".to_string(), "A. Tester".to_string())
                ]
                .into(),
            }]
        );
        assert_eq!(v.application.as_ref().unwrap()["naam"], "Voorbeeld");
        assert_eq!(v.actions["besluit"]["feiten_vergaard"], true);
        // "$today" becomes today's date when requested.
        assert_eq!(v.actions["besluit"]["besluitdatum"], "$today");
        assert_eq!(
            v.on("2025-03-20").actions["besluit"]["besluitdatum"],
            "2025-03-20"
        );
    }

    #[test]
    fn without_examples_everything_is_empty() {
        let dir = dir_with(&[]);
        let v = load(dir.path(), &ExamplesDefinition::default(), &process()).unwrap();
        assert!(v.logins.is_empty() && v.application.is_none() && v.actions.is_empty());
    }

    #[test]
    fn missing_file_names_the_path() {
        let dir = dir_with(&[]);
        let errors = load(
            dir.path(),
            &definition(&["weg.json"], Some("ook-weg.json"), None),
            &process(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("weg.json"), "{errors:?}");
        assert!(errors[1].contains("ook-weg.json"), "{errors:?}");
    }

    #[test]
    fn wrong_shape_is_refused() {
        let dir = dir_with(&[
            ("geen-json.json", "{"),
            ("zonder-persoon.json", r#"{"kvk": "12345678"}"#),
            ("korte-kvk.json", r#"{"kvk": "123", "persoon": "A"}"#),
            ("aanvraag.json", r#"{"form": {}}"#),
            ("besluit.json", r#"[{"form": {}}]"#),
        ]);
        let errors = load(
            dir.path(),
            &definition(
                &["geen-json.json", "zonder-persoon.json", "korte-kvk.json"],
                Some("aanvraag.json"),
                Some("besluit.json"),
            ),
            &process(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 5, "{errors:?}");
        assert!(
            errors[0].contains("geen-json.json: not valid JSON"),
            "{errors:?}"
        );
        assert!(
            errors[1].contains("zonder-persoon.json: Naam ontbreekt"),
            "{errors:?}"
        );
        assert!(
            errors[2].contains("korte-kvk.json: een nummer heeft acht cijfers"),
            "{errors:?}"
        );
        assert!(
            errors[3].contains("aanvraag.json: expected an object with 'external'"),
            "{errors:?}"
        );
        assert!(
            errors[4].contains("besluit.json: expected an object with 'form'"),
            "{errors:?}"
        );
    }

    #[test]
    fn two_logins_with_the_same_label() {
        let dir = dir_with(&[("login.json", r#"{"kvk": "12345678", "persoon": "A"}"#)]);
        std::fs::create_dir_all(dir.path().join("ander")).unwrap();
        std::fs::write(
            dir.path().join("ander/login.json"),
            r#"{"kvk": "87654321", "persoon": "B"}"#,
        )
        .unwrap();
        let errors = load(
            dir.path(),
            &definition(&["login.json", "ander/login.json"], None, None),
            &process(),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("login.json"), "{errors:?}");
        assert!(errors[0].contains("ander/login.json"), "{errors:?}");
        assert!(errors[0].contains("label 'login'"), "{errors:?}");
    }
}
