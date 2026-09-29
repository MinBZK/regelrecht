//! Channels and roles: how someone enters a process, and what they may do
//! there (`channels` and `roles` in `process.yaml`).
//!
//! A channel is a simulated login: a few identification fields, each
//! with a label and a shape check (a pattern, and optionally the
//! elfproef). The channel also says under which path of `$intake` its fields
//! arrive at the cell and which field designates the owner of a case. There is
//! no register and no certified login: whoever enters a valid number
//! is logged in for this PoC. Who may act on behalf of an organization is said by
//! a register via the synthesis, not by the login.
//!
//! A role names its channel and the route groups it may use
//! ([`Routes`]). Which routes are in a group is decided by the runtime; who
//! may use them, by the configuration.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regelrecht_engine::LawExecutionService;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::ProcessDefinition;
use crate::gram::set_path;
use crate::stream::{Binding, Case, Event};

/// A channel (`channels.<id>` in `process.yaml`).
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelDefinition {
    /// How the frontend names the channel, such as "Inloggen als medewerker".
    pub label: String,
    /// Explanation under the label on the login screen; the frontend itself adds
    /// that the login is simulated.
    #[serde(default)]
    pub explanation: Option<String>,
    /// The identification fields, in the order of the login screen.
    pub fields: Vec<IdentificationField>,
    /// The field that designates the owner of a case: an applicant who
    /// follows a case must have a gram in that case with their value of this
    /// field.
    #[serde(default)]
    pub owner: Option<String>,
    /// The path under `$intake` under which the fields arrive at the cell; without it:
    /// the id of the channel. A field `kvk` of channel `x` is then
    /// `$intake.x.kvk`.
    #[serde(default)]
    pub intake: Option<String>,
    /// What the channel and its owner rest on, such as the rule that says by
    /// which means and on behalf of whom someone logs in (`<regulation>#<article>`).
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

/// An identification field of a channel.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IdentificationField {
    pub name: String,
    pub label: String,
    /// A regular expression the whole value (after trimming) must match;
    /// without it: not empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// A check on top of the pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
    /// The message for a value that does not comply; without it a generic one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Digits only: the frontend shows a numeric keyboard.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub numeric: bool,
    /// What the field rests on: the rule that knows the data item and its shape,
    /// such as the number a register assigns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
    /// The pattern, compiled: at load time (see
    /// [`ChannelDefinition::check`]), not at every login.
    #[serde(skip)]
    regex: OnceLock<Regex>,
}

/// A check on an identification field that a pattern cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    /// The elfproef as for a burgerservicenummer: nine digits, the
    /// first eight weighted 9 through 2, the last with -1, and the sum
    /// divisible by 11.
    Elfproef,
}

/// The route groups of a process. A role names the groups it may
/// use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Routes {
    /// The portal: form, assessment, offer and submission, for whoever applies on behalf
    /// of themselves or their organization.
    Portal,
    /// The handling: worklist, case, trial decision and decision.
    Handling,
    /// The counter: entering an application that came in by another route
    /// on behalf of the applicant, with the day of receipt.
    Counter,
}

impl Routes {
    pub fn as_text(self) -> &'static str {
        match self {
            Routes::Portal => "portal",
            Routes::Handling => "handling",
            Routes::Counter => "counter",
        }
    }
}

/// A role (`roles.<id>` in `process.yaml`).
#[derive(Debug, Clone, Deserialize)]
pub struct RoleDefinition {
    /// The channel through which the role logs in.
    pub channel: String,
    /// The route groups the role may use.
    pub routes: Vec<Routes>,
    /// How the frontend names the role; without it: the id.
    #[serde(default)]
    pub label: Option<String>,
    /// Why this role may do this, such as a mandate regulation
    /// (`<regulation>#<article>`); a decision carries it along with the
    /// acting actor.
    #[serde(default)]
    pub legal_basis: Option<String>,
}

impl RoleDefinition {
    pub fn may(&self, r: Routes) -> bool {
        self.routes.contains(&r)
    }
}

/// A logged-in user: in which role, through which channel, with which
/// values of the identification fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Session {
    pub role: String,
    pub channel: String,
    pub fields: BTreeMap<String, String>,
}

impl ChannelDefinition {
    /// The path under `$intake` of the fields of this channel.
    pub fn intake_prefix<'a>(&'a self, id: &'a str) -> &'a str {
        self.intake.as_deref().unwrap_or(id)
    }

    /// The paths under `$intake` this channel supplies.
    pub fn intake_paths(&self, id: &str) -> Vec<String> {
        let p = self.intake_prefix(id);
        self.fields
            .iter()
            .map(|v| format!("{p}.{}", v.name))
            .collect()
    }

    /// The path under `$intake` of the owner field, if the channel names
    /// one.
    pub fn owner_path(&self, id: &str) -> Option<String> {
        self.owner
            .as_ref()
            .map(|e| format!("{}.{e}", self.intake_prefix(id)))
    }

    /// Validate the input of a login: every field is present, as text, and
    /// matches its shape. Other keys do not count.
    pub fn validate(&self, input: &Map<String, Value>) -> Result<BTreeMap<String, String>, String> {
        let mut out = BTreeMap::new();
        for v in &self.fields {
            let value = match input.get(&v.name) {
                Some(Value::String(s)) => s.trim().to_string(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
            if value.is_empty() {
                return Err(format!("{} ontbreekt", v.label));
            }
            if !v.satisfies(&value) {
                return Err(v
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("{} is ongeldig", v.label)));
            }
            out.insert(v.name.clone(), value);
        }
        Ok(out)
    }

    /// Check the channel itself, at startup: fields with a unique
    /// name, patterns that can be parsed, and an owner that is a field.
    pub fn check(&self, id: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut names: Vec<&str> = Vec::new();
        for v in &self.fields {
            if names.contains(&v.name.as_str()) {
                errors.push(format!("channel '{id}': field '{}' appears twice", v.name));
            }
            names.push(&v.name);
            if let Some(p) = &v.pattern {
                if let Some(Err(e)) = v.regex() {
                    errors.push(format!(
                        "channel '{id}': field '{}' has an invalid pattern '{p}': {e}",
                        v.name
                    ));
                }
            }
        }
        if let Some(e) = &self.owner {
            if !names.contains(&e.as_str()) {
                errors.push(format!(
                    "channel '{id}': owner '{e}' is not a field of the channel ({})",
                    names.join(", ")
                ));
            }
        }
        errors
    }
}

impl IdentificationField {
    /// The pattern as a regular expression for the whole field (not a part
    /// of it), compiled once. `None` without a pattern.
    fn regex(&self) -> Option<Result<&Regex, regex::Error>> {
        let p = self.pattern.as_ref()?;
        if let Some(r) = self.regex.get() {
            return Some(Ok(r));
        }
        Some(Regex::new(&format!("^(?:{p})$")).map(|r| self.regex.get_or_init(|| r)))
    }

    fn satisfies(&self, value: &str) -> bool {
        let pattern = match self.regex() {
            Some(r) => r.is_ok_and(|r| r.is_match(value)),
            None => true,
        };
        pattern
            && match self.check {
                Some(Check::Elfproef) => elfproef(value),
                None => true,
            }
    }
}

/// The elfproef of a burgerservicenummer: nine digits, weighted 9, 8, ...,
/// 2 and -1, and the sum divisible by 11.
pub fn elfproef(number: &str) -> bool {
    let digits: Vec<i64> = number
        .chars()
        .filter_map(|c| c.to_digit(10).map(i64::from))
        .collect();
    if digits.len() != 9 || number.len() != 9 {
        return false;
    }
    let sum: i64 = digits[..8]
        .iter()
        .zip((2..=9).rev())
        .map(|(c, w)| c * w)
        .sum::<i64>()
        - digits[8];
    sum % 11 == 0
}

/// What a channel passes to the cell under `$intake`: `channel` and, for every
/// channel in `channels`, its fields: with the values of the logged-in
/// user for their own channel, and empty (`null`) for the others. That way
/// every channel of a portal supplies every path the event binds, and
/// what another channel would supply stays empty.
pub fn intake<'a>(
    channel: &str,
    channels: impl IntoIterator<Item = (&'a str, &'a ChannelDefinition)>,
    user: Option<(&str, &BTreeMap<String, String>)>,
) -> Value {
    let mut out = Map::new();
    out.insert("channel".into(), Value::String(channel.to_string()));
    for (id, k) in channels {
        let own = user.filter(|(g, _)| *g == id).map(|(_, v)| v);
        let fields: Map<String, Value> = k
            .fields
            .iter()
            .map(|v| {
                let w = own
                    .and_then(|e| e.get(&v.name))
                    .map_or(Value::Null, |w| Value::String(w.clone()));
                (v.name.clone(), w)
            })
            .collect();
        set_path(&mut out, k.intake_prefix(id), Value::Object(fields));
    }
    Value::Object(out)
}

/// The paths under `$intake` the portal supplies: `channel` and the fields
/// of every channel of a role with routes `portal` or `counter`. The counter
/// identifies the applicant with the fields of a portal channel.
pub fn portal_intake_paths(d: &ProcessDefinition) -> Vec<String> {
    let mut out = vec!["channel".to_string()];
    for (id, k) in d.channels_with(Routes::Portal) {
        out.extend(k.intake_paths(id));
    }
    out
}

/// The path under `$intake` to which the event binds its `effective_at`, if it
/// does: that is where the counter puts the day of receipt.
pub fn receipt_path(event: &Event) -> Option<String> {
    match event.effective_at.as_ref()?.binding() {
        Binding::Intake(path) => Some(path),
        _ => None,
    }
}

/// The checks on the `channels` and `roles` of a process at startup:
///
/// - every channel is in order ([`ChannelDefinition::check`]), and every
///   legal basis of a channel or of a field points to a loaded article,
///   with the paragraph it names;
/// - every role names a channel that exists, and a legal basis that points to a loaded
///   article;
/// - a portal requires a role with routes `portal`, and such a role a
///   portal; a handling and routes `handling` likewise;
/// - routes `counter` require a portal whose event binds its `effective_at`
///   to `$intake`: that is where the day of receipt goes;
/// - if the portal event follows a case, every portal channel names an
///   owner: whoever follows a case must know it.
pub fn check_process(
    d: &ProcessDefinition,
    portal_event: Option<&Event>,
    service: &LawExecutionService,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (id, k) in &d.channels {
        errors.extend(k.check(id));
        let fields = k.fields.iter().flat_map(|v| {
            v.legal_basis
                .iter()
                .map(move |g| (format!("channel '{id}', field '{}'", v.name), g))
        });
        for (where_, g) in k
            .legal_basis
            .iter()
            .map(|g| (format!("channel '{id}'"), g))
            .chain(fields)
        {
            if let Err(f) = crate::regulations::valid(service, g) {
                errors.push(format!("{where_}: {f}"));
            }
        }
    }
    for (id, role) in &d.roles {
        if !d.channels.contains_key(&role.channel) {
            errors.push(format!(
                "role '{id}': channel '{}' is not listed under channels",
                role.channel
            ));
        }
        if let Some(g) = &role.legal_basis {
            if let Err(f) = crate::regulations::valid(service, g) {
                errors.push(format!("role '{id}': {f}"));
            }
        }
    }
    let has = |r: Routes| d.roles_with(r).next().is_some();
    for (r, block, present) in [
        (Routes::Portal, "portal", d.portal.is_some()),
        (Routes::Handling, "handling", d.handling.is_some()),
    ] {
        match (present, has(r)) {
            (true, false) => errors.push(format!(
                "{block} without a role that may use it: give a role routes: [{}]",
                r.as_text()
            )),
            (false, true) => errors.push(format!(
                "a role with routes {} and no {block}: that role has nothing to do",
                r.as_text()
            )),
            _ => {}
        }
    }
    if has(Routes::Counter) {
        match (d.portal.is_some(), portal_event) {
            (false, _) => errors.push(
                "a role with routes counter and no portal: the counter enters an application in the event of the portal".into(),
            ),
            (true, Some(e)) if receipt_path(e).is_none() => errors.push(format!(
                "counter: event '{}' does not bind effective_at to $intake; the counter provides the day of receipt (Awb 4:13)",
                e.name
            )),
            _ => {}
        }
        if !has(Routes::Portal) {
            errors.push(
                "counter: no portal channel to identify the applicant with; give a role routes: [portal]".into(),
            );
        }
    }
    if portal_event.is_some_and(|e| e.case == Case::Follows) {
        for (id, k) in d.channels_with(Routes::Portal) {
            if k.owner.is_none() {
                errors.push(format!(
                    "channel '{id}': the portal follows a case, and the channel names no owner"
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn channel(yaml: &str) -> ChannelDefinition {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    fn organisation() -> ChannelDefinition {
        channel(
            "label: Organisatie\nfields:\n  - {name: nummer, label: Organisatienummer, pattern: '[0-9]{8}', message: een organisatienummer heeft acht cijfers}\n  - {name: persoon, label: Naam}\nowner: nummer\n",
        )
    }

    fn citizen() -> ChannelDefinition {
        channel(
            "label: Burger\nfields:\n  - {name: nummer, label: Burgernummer, pattern: '[0-9]{9}', check: elfproef}\nowner: nummer\nintake: burger\n",
        )
    }

    fn input(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn a_login_matches_the_fields_of_the_channel() {
        let k = organisation();
        let ok = k
            .validate(&input(
                json!({"nummer": " 12345678 ", "persoon": "A. Tester", "machtiging": 1}),
            ))
            .unwrap();
        assert_eq!(ok["nummer"], "12345678");
        assert_eq!(ok["persoon"], "A. Tester");
        assert_eq!(
            k.validate(&input(json!({"nummer": "1234567", "persoon": "A"}))),
            Err("een organisatienummer heeft acht cijfers".into())
        );
        // The pattern applies to the whole value.
        assert!(k
            .validate(&input(json!({"nummer": "123456789", "persoon": "A"})))
            .is_err());
        assert_eq!(
            k.validate(&input(json!({"nummer": "12345678", "persoon": "  "}))),
            Err("Naam ontbreekt".into())
        );
    }

    #[test]
    fn the_elfproef() {
        assert!(elfproef("111222333"));
        assert!(elfproef("123456782"));
        assert!(!elfproef("123456789"));
        assert!(!elfproef("12345678"));
        assert!(!elfproef("12345678a"));
        let k = citizen();
        assert!(k.validate(&input(json!({"nummer": "111222333"}))).is_ok());
        assert_eq!(
            k.validate(&input(json!({"nummer": "111222334"}))),
            Err("Burgernummer is ongeldig".into())
        );
    }

    #[test]
    fn the_intake_supplies_every_path_of_every_channel() {
        let (o, b) = (organisation(), citizen());
        let fields: BTreeMap<String, String> =
            [("nummer".to_string(), "111222333".to_string())].into();
        let i = intake(
            "portaal",
            [("organisatie", &o), ("burgerlogin", &b)],
            Some(("burgerlogin", &fields)),
        );
        assert_eq!(
            i,
            json!({"channel": "portaal", "organisatie": {"nummer": null, "persoon": null}, "burger": {"nummer": "111222333"}})
        );
        assert_eq!(
            o.intake_paths("organisatie"),
            ["organisatie.nummer", "organisatie.persoon"]
        );
        assert_eq!(b.owner_path("burgerlogin").unwrap(), "burger.nummer");
    }

    #[test]
    fn a_channel_is_checked_at_startup() {
        let k = channel(
            "label: X\nfields:\n  - {name: a, label: A, pattern: '[0-9'}\n  - {name: a, label: B}\nowner: c\n",
        );
        let f = k.check("x");
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f[0].contains("invalid pattern"));
        assert!(f[1].contains("appears twice"));
        assert!(f[2].contains("owner 'c'"));

        assert!(organisation().check("o").is_empty());
    }
}
