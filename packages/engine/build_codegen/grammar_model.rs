// Shared grammar model + translation. include!'d by build scripts and the
// codegen binary. No external deps beyond serde/serde_yaml_ng.
//
// NOTE: the workspace pins `serde_yaml_ng` (a maintained fork of the
// unmaintained `serde_yaml`) — its API is identical, so it is used here.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Grammar {
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub id: String,
    pub action: String,
    pub keyword: String, // given|when|then
    #[serde(default)]
    pub tier: String,
    pub text: String,
    #[serde(default)]
    pub args: Vec<Arg>,
    #[serde(default)]
    pub datatable: bool,
    #[serde(default)]
    pub literals: Vec<serde_yaml_ng::Value>,
}

#[derive(Debug, Deserialize)]
pub struct Arg {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String, // string|number
}

#[allow(dead_code)]
pub fn load_grammar(path: &std::path::Path) -> Grammar {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read grammar {}: {e}", path.display()));
    serde_yaml_ng::from_str(&raw)
        .unwrap_or_else(|e| panic!("invalid grammar yaml {}: {e}", path.display()))
}

/// True when the step has at least one numeric arg (forces a regex binding).
pub fn needs_regex(step: &Step) -> bool {
    step.args.iter().any(|a| a.ty == "number")
}

/// Translate canonical `text` into a Cucumber Expression (quoted args -> {string}).
/// Only valid when !needs_regex(step).
pub fn to_cucumber_expr(step: &Step) -> String {
    // Replace each "{name}" (quoted) with {string}; bare {name} not allowed here.
    let mut out = step.text.clone();
    for a in &step.args {
        let quoted = format!("\"{{{}}}\"", a.name); // "{name}"
        out = out.replace(&quoted, "{string}");
    }
    out
}

/// Translate canonical `text` into an anchored regex with one capture per arg.
///
/// Quoted string arg `"{name}"` -> `"([^"]*)"`, numeric `{name}` ->
/// `(-?\d+(?:\.\d+)?)`. Literal text (including the quote chars around string
/// args) is regex-escaped; the capture groups themselves are NOT escaped.
///
/// The `\u{0}STR\u{0}` / `\u{0}NUM\u{0}` NUL sentinels mark capture positions
/// before escaping, so `regex_escape` only touches literal text. They are then
/// swapped for the real, unescaped capture groups.
pub fn to_regex(step: &Step) -> String {
    let mut working = step.text.clone();
    for a in &step.args {
        if a.ty == "string" {
            let ph = format!("\"{{{}}}\"", a.name);
            working = working.replacen(&ph, "\u{0}STR\u{0}", 1);
        } else {
            let ph = format!("{{{}}}", a.name);
            working = working.replacen(&ph, "\u{0}NUM\u{0}", 1);
        }
    }
    let body = regex_escape(&working)
        .replace("\u{0}STR\u{0}", "\"([^\"]*)\"")
        .replace("\u{0}NUM\u{0}", "(-?\\d+(?:\\.\\d+)?)");
    format!("^{body}$")
}

fn regex_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

// Verifies the expr/regex translation against representative grammar steps.
// This module is only compiled when grammar_model.rs is include!'d into a test
// context (build scripts cannot host #[test]s). build.rs carries an equivalent
// runtime assert! so the same invariants are checked at build time too.
#[cfg(test)]
mod grammar_model_tests {
    use super::*;

    fn step(text: &str, args: Vec<(&str, &str)>) -> Step {
        Step {
            id: "t".into(),
            action: "t".into(),
            keyword: "then".into(),
            tier: "core".into(),
            text: text.into(),
            args: args
                .into_iter()
                .map(|(n, t)| Arg {
                    name: n.into(),
                    ty: t.into(),
                })
                .collect(),
            datatable: false,
            literals: vec![],
        }
    }

    #[test]
    fn quoted_string_step_becomes_cucumber_expr() {
        let s = step("the calculation date is \"{date}\"", vec![("date", "string")]);
        assert!(!needs_regex(&s));
        assert_eq!(to_cucumber_expr(&s), "the calculation date is {string}");
    }

    #[test]
    fn numeric_step_becomes_anchored_regex() {
        let s = step(
            "output \"{output}\" equals {value}",
            vec![("output", "string"), ("value", "number")],
        );
        assert!(needs_regex(&s));
        assert_eq!(
            to_regex(&s),
            r#"^output "([^"]*)" equals (-?\d+(?:\.\d+)?)$"#
        );
    }

    #[test]
    fn two_numeric_args_each_get_capture_groups() {
        let s = step(
            "the note hints article \"{article}\" at position {start} to {end}",
            vec![
                ("article", "string"),
                ("start", "number"),
                ("end", "number"),
            ],
        );
        assert_eq!(
            to_regex(&s),
            r#"^the note hints article "([^"]*)" at position (-?\d+(?:\.\d+)?) to (-?\d+(?:\.\d+)?)$"#
        );
    }
}
