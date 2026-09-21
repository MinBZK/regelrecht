//! Voorwaarden op een actie: wat de wet over een actor zegt, naast de knop.
//!
//! Of een actor een actie volgens de wet *mag* of *kan*, is iets anders dan of
//! de wereld haar nu technisch kan uitvoeren. Dat laatste is
//! [`crate::ActionSnapshot::available`], en dat blijft precies wat het was: een
//! eigen feit dat ontbreekt, of een bekendmaking waar niets bekend te maken
//! valt. Dit is het eerste: een **voorwaarde**, uitgerekend door de cel van de
//! actor zelf, en getoond naast de actie in plaats van haar tegen te houden.
//!
//! Twee soorten, en ze staan met opzet apart in het beeld:
//!
//! - een **regelingsvoorwaarde** noemt een regeling, een uitkomst van die
//!   regeling en de parameters. De cel van de actor laadt die regeling en voert
//!   haar uit op wat zij zelf weet. De regel staat in de regeling en nooit in
//!   het wereldbestand; het wereldbestand noemt alleen de verwijzing;
//! - een **lexostatusvoorwaarde** noemt een lexostatus die de cel van de actor
//!   zelf publiceert, een uitkomst daarvan en de waarde die verwacht wordt. Dat
//!   is de eigen stand van de actor — "er loopt nog geen aanvraag" — en die komt
//!   uit haar eigen kroniek en niet uit een wet.
//!
//! Elke voorwaarde levert `waar`, `onwaar` of `onbekend`, met de reden erbij.
//! Wat de cel niet weet, is onbekend: ze vraagt het niet over haar grens. Dat
//! is invariant I1 en geen beperking van deze module — het beeld van de wereld
//! wordt bij elke stap opgevraagd, en een voorwaarde die over een grens reikte
//! zou bij elk scherm verkeer opleveren dat niemand vroeg. De rekenweg is die
//! van [`crate::Cell::reduce`]: een engine zonder cel-tier.
//!
//! **Tonen, niet blokkeren.** Een voorwaarde op `onwaar` of `onbekend` laat de
//! actie uitvoerbaar. Juridisch mag iedereen een aanvraag indienen (Awb art. 1:3
//! en 4:1); wie niet gerechtigd is, krijgt een afwijzing, en die tegenproef moet
//! in de opstelling te spelen blijven.

use crate::cell::{
    check_parameter_value, niet_geleverd_reden, Cell, DocumentedParameter, RegulationSurface,
};
use crate::error::{Result, SimulatorError, Subject};
use crate::journal::IndicatorParam;
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Eén voorwaarde op een actie, zoals het wereldbestand haar noemt.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "ConditionFields")]
pub struct ActionCondition {
    /// Wat een lezer van deze voorwaarde ziet. Casusdata, en uniek binnen de
    /// actie: een scenario wijst een voorwaarde met dit label aan.
    pub label: String,
    /// Vrije toelichting.
    pub doc: Option<String>,
    /// Waar de uitkomst vandaan komt.
    pub check: ConditionCheck,
    /// De parameters: `$veld` wijst een veld van het formulier van de actie aan,
    /// al het andere is een letterlijke waarde.
    pub params: BTreeMap<String, IndicatorParam>,
}

/// De twee soorten voorwaarde.
#[derive(Debug, Clone)]
pub enum ConditionCheck {
    /// Een uitkomst van een regeling die de cel van de actor laadt. De uitkomst
    /// is een ja-of-nee; `waar` is ja.
    Regulation {
        /// De regeling, bij `$id`.
        regulation: String,
        /// De uitkomst van die regeling.
        output: String,
    },
    /// Een uitkomst van een lexostatus die de cel van de actor zelf publiceert,
    /// tegen een verwachte waarde. `null` verwacht dat er niets is: geen
    /// vastlegging, of een vastlegging zonder deze uitkomst.
    Lexostatus {
        /// De gepubliceerde lexostatus.
        lexostatus: String,
        /// De uitkomst daarvan.
        output: String,
        /// De waarde waarbij de voorwaarde `waar` is.
        expect: Value,
    },
}

/// Het YAML-oppervlak van een voorwaarde: beide soorten, los.
///
/// Dezelfde keuze als bij een actie: de vorm valt hieronder en niet in serde, zodat
/// de melding zegt wat er mis is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConditionFields {
    label: String,
    #[serde(default)]
    doc: Option<String>,
    #[serde(default)]
    regulation: Option<String>,
    #[serde(default)]
    lexostatus: Option<String>,
    output: String,
    #[serde(default)]
    params: BTreeMap<String, IndicatorParam>,
    /// Aanwezig-met-`null` en afwezig zijn hier twee dingen: het eerste verwacht
    /// dat er niets is, het tweede is een vergeten verwachting.
    #[serde(default, deserialize_with = "present")]
    expect: Option<Value>,
}

/// Een veld dat er staat, ook als het `null` is.
fn present<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

impl TryFrom<ConditionFields> for ActionCondition {
    type Error = String;

    fn try_from(fields: ConditionFields) -> std::result::Result<Self, Self::Error> {
        let label = fields.label;
        let check = match (fields.regulation, fields.lexostatus, fields.expect) {
            (Some(_), Some(_), _) => {
                return Err(format!(
                    "voorwaarde '{label}' noemt zowel `regulation` als `lexostatus`; een \
                     voorwaarde komt uit een regeling óf uit een eigen lexostatus"
                ))
            }
            (None, None, _) => {
                return Err(format!(
                    "voorwaarde '{label}' noemt geen `regulation` en geen `lexostatus`"
                ))
            }
            (Some(_), None, Some(_)) => {
                return Err(format!(
                    "voorwaarde '{label}' uit een regeling draagt geen `expect`: de uitkomst \
                     is een ja-of-nee, en `waar` is ja"
                ))
            }
            (None, Some(_), None) => {
                return Err(format!(
                    "voorwaarde '{label}' op een lexostatus mist `expect`: bij welke waarde \
                     is ze waar? (`null` verwacht dat er niets is)"
                ))
            }
            (Some(regulation), None, None) => ConditionCheck::Regulation {
                regulation,
                output: fields.output,
            },
            (None, Some(lexostatus), Some(expect)) => ConditionCheck::Lexostatus {
                lexostatus,
                output: fields.output,
                expect,
            },
        };
        Ok(Self {
            label,
            doc: fields.doc,
            check,
            params: fields.params,
        })
    }
}

/// Wat een voorwaarde op dit moment zegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOutcome {
    /// De voorwaarde is vervuld.
    Waar,
    /// De voorwaarde is niet vervuld.
    Onwaar,
    /// De cel van de actor weet het niet.
    Onbekend,
}

impl std::fmt::Display for ConditionOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Waar => "waar",
            Self::Onwaar => "onwaar",
            Self::Onbekend => "onbekend",
        })
    }
}

/// Eén voorwaarde op een actie, met wat ze nu zegt en waarom.
#[derive(Debug, Clone, Serialize)]
pub struct ConditionSnapshot {
    /// Wat een lezer ziet.
    pub label: String,
    /// Vrije toelichting uit het wereldbestand.
    pub doc: Option<String>,
    /// De cel die de voorwaarde uitrekende: altijd de actor.
    pub cell: String,
    /// Waar de uitkomst vandaan komt.
    #[serde(flatten)]
    pub source: ConditionSource,
    /// De parameters waarmee gerekend is, zoals ze uit het formulier kwamen.
    ///
    /// Alleen wat er een waarde had: een veld dat nog leeg is, staat er niet in,
    /// en dan is de uitkomst `onbekend`.
    pub params: BTreeMap<String, Value>,
    /// Wat de voorwaarde zegt.
    pub outcome: ConditionOutcome,
    /// De waarde die gevonden werd; `None` als er niets uit te rekenen viel.
    pub value: Option<Value>,
    /// Waarom, in woorden: het artikel en de uitkomst, de gevonden stand, of
    /// wat de cel niet wist.
    pub reason: String,
}

/// Waar de uitkomst van een voorwaarde vandaan komt, zoals een lezer het ziet.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConditionSource {
    /// Een uitkomst van een regeling die de actor laadt.
    Regulation {
        /// De regeling, bij `$id`.
        regulation: String,
        /// De uitkomst.
        output: String,
        /// Het artikel dat de uitkomst voortbrengt, in de versie die nu geldt.
        article: Option<String>,
        /// De versie van de regeling die nu geldt.
        valid_from: Option<String>,
    },
    /// Een uitkomst van een eigen lexostatus, tegen een verwachte waarde.
    Lexostatus {
        /// De gepubliceerde lexostatus.
        lexostatus: String,
        /// De uitkomst.
        output: String,
        /// De waarde waarbij de voorwaarde waar is.
        expected: Value,
    },
}

/// Toets de voorwaarden van één actie tegen de cel van haar actor en haar
/// formulier.
///
/// Wat hier faalt, faalt luid bij het optuigen: een voorwaarde die nooit uit te
/// rekenen is, zou anders voorgoed "onbekend" tonen, en dat valt niet te
/// onderscheiden van een cel die het echt niet weet.
pub(crate) fn check(
    action: &str,
    conditions: &[ActionCondition],
    actor: &Cell,
    form: &[DocumentedParameter],
) -> Result<()> {
    let fail = |condition: &ActionCondition, reason: String| SimulatorError::ActionCondition {
        action: action.to_string(),
        condition: condition.label.clone(),
        reason,
    };
    let fields: BTreeSet<&str> = form.iter().map(|field| field.name.as_str()).collect();
    let mut labels: BTreeSet<&str> = BTreeSet::new();

    for condition in conditions {
        if !labels.insert(condition.label.as_str()) {
            return Err(fail(
                condition,
                "twee voorwaarden met hetzelfde label; een label wijst er één aan".to_string(),
            ));
        }
        for (name, param) in &condition.params {
            if let IndicatorParam::FromEvent(field) = param {
                if !fields.contains(field.as_str()) {
                    return Err(fail(
                        condition,
                        format!(
                            "parameter '{name}' verwijst naar '${field}', maar het formulier \
                             van deze actie kent alleen: {}",
                            joined(fields.iter().copied())
                        ),
                    ));
                }
            }
        }

        match &condition.check {
            ConditionCheck::Regulation { regulation, output } => {
                let Some(RegulationSurface {
                    outputs,
                    parameters,
                    required,
                }) = actor.regulation_surface(regulation)
                else {
                    return Err(fail(
                        condition,
                        format!(
                            "cel '{}' laadt regeling '{regulation}' niet. Een voorwaarde \
                             rekent in de cel van de actor, dus noem de regeling onder \
                             `laws:` van die cel (nu: {})",
                            actor.id(),
                            joined(actor.laws().iter().map(String::as_str))
                        ),
                    ));
                };
                let Some(types) = outputs.get(output) else {
                    return Err(fail(
                        condition,
                        format!(
                            "regeling '{regulation}' kent geen uitkomst '{output}' (wel: {})",
                            joined(outputs.keys().map(String::as_str))
                        ),
                    ));
                };
                if types.iter().any(|kind| kind != "boolean") {
                    return Err(fail(
                        condition,
                        format!(
                            "uitkomst '{output}' van '{regulation}' is geen ja-of-nee maar \
                             {}; een voorwaarde uit een regeling is waar of onwaar",
                            joined(types.iter().map(String::as_str))
                        ),
                    ));
                }
                if let Some(unknown) = condition
                    .params
                    .keys()
                    .find(|name| !parameters.contains(name.as_str()))
                {
                    return Err(fail(
                        condition,
                        format!(
                            "regeling '{regulation}' kent geen parameter '{unknown}' (wel: {})",
                            joined(parameters.iter().map(String::as_str))
                        ),
                    ));
                }
                // Een verplichte parameter die de voorwaarde niet vult, geeft bij
                // elke uitrekening een enginefout: voorgoed onbekend.
                if let Some(missing) = required
                    .get(output)
                    .into_iter()
                    .flatten()
                    .find(|name| !condition.params.contains_key(name.as_str()))
                {
                    return Err(fail(
                        condition,
                        format!(
                            "uitkomst '{output}' van '{regulation}' vraagt parameter \
                             '{missing}', maar de voorwaarde vult die niet"
                        ),
                    ));
                }
            }
            ConditionCheck::Lexostatus {
                lexostatus, output, ..
            } => {
                let Some(definition) = actor
                    .published_definitions()
                    .find(|definition| &definition.name == lexostatus)
                else {
                    return Err(fail(
                        condition,
                        format!(
                            "cel '{}' publiceert geen lexostatus '{lexostatus}' (wel: {}); \
                             een voorwaarde leest alleen de eigen stand van de actor",
                            actor.id(),
                            joined(actor.published_names())
                        ),
                    ));
                };
                let published = definition.published_outputs();
                if !published.contains(output.as_str()) {
                    return Err(fail(
                        condition,
                        format!(
                            "lexostatus '{lexostatus}' publiceert geen uitkomst '{output}' \
                             (wel: {})",
                            joined(published.iter().copied())
                        ),
                    ));
                }
                let expected: BTreeSet<&str> = definition
                    .inputs
                    .iter()
                    .map(|input| input.name.as_str())
                    .collect();
                let given: BTreeSet<&str> = condition.params.keys().map(String::as_str).collect();
                if expected != given {
                    return Err(fail(
                        condition,
                        format!(
                            "lexostatus '{lexostatus}' vraagt precies {}, de voorwaarde vult {}",
                            joined(expected.iter().copied()),
                            joined(given.iter().copied())
                        ),
                    ));
                }
                // Een letterlijke waarde door dezelfde typetoets als wat er
                // straks over de draad komt, net als bij een statusindicator:
                // anders is ze bij elke meting stil onbekend.
                for input in &definition.inputs {
                    let Some(IndicatorParam::Literal(value)) = condition.params.get(&input.name)
                    else {
                        continue;
                    };
                    check_parameter_value(
                        actor.id(),
                        Subject::Lexostatus,
                        lexostatus,
                        input,
                        value,
                    )
                    .map_err(|error| fail(condition, error.to_string()))?;
                }
            }
        }
    }
    Ok(())
}

/// Reken één voorwaarde uit, in de cel van de actor, op de waarden van het
/// formulier en de stand van de klok.
///
/// Nooit een fout: wat niet uit te rekenen valt, is `onbekend` met de reden
/// erbij. Een voorwaarde blokkeert niets, dus ze hoort ook het beeld van de
/// wereld niet te laten omvallen.
pub(crate) fn evaluate(
    condition: &ActionCondition,
    actor: &Cell,
    values: &BTreeMap<String, Value>,
    clock: NaiveDate,
) -> ConditionSnapshot {
    let mut params = BTreeMap::new();
    let mut empty = Vec::new();
    for (name, param) in &condition.params {
        match param {
            IndicatorParam::Literal(value) => {
                params.insert(name.clone(), value.clone());
            }
            IndicatorParam::FromEvent(field) => match values.get(field) {
                Some(value) => {
                    params.insert(name.clone(), value.clone());
                }
                None => empty.push(field.as_str()),
            },
        }
    }

    let (source, found) = match &condition.check {
        ConditionCheck::Regulation { regulation, output } => {
            let (article, valid_from) = actor.own_output_article(regulation, output, clock);
            let found = if empty.is_empty() {
                regulation_outcome(
                    actor,
                    regulation,
                    output,
                    article.as_deref(),
                    &params,
                    clock,
                )
            } else {
                Found::onbekend(None, not_filled(&empty))
            };
            let source = ConditionSource::Regulation {
                regulation: regulation.clone(),
                output: output.clone(),
                article,
                valid_from,
            };
            (source, found)
        }
        ConditionCheck::Lexostatus {
            lexostatus,
            output,
            expect,
        } => {
            let found = if empty.is_empty() {
                lexostatus_outcome(actor, lexostatus, output, expect, &params, clock)
            } else {
                Found::onbekend(None, not_filled(&empty))
            };
            let source = ConditionSource::Lexostatus {
                lexostatus: lexostatus.clone(),
                output: output.clone(),
                expected: expect.clone(),
            };
            (source, found)
        }
    };

    ConditionSnapshot {
        label: condition.label.clone(),
        doc: condition.doc.clone(),
        cell: actor.id().to_string(),
        source,
        params,
        outcome: found.outcome,
        value: found.value,
        reason: found.reason,
    }
}

/// Wat een uitrekening opleverde.
struct Found {
    outcome: ConditionOutcome,
    value: Option<Value>,
    reason: String,
}

impl Found {
    fn onbekend(value: Option<Value>, reason: String) -> Self {
        Self {
            outcome: ConditionOutcome::Onbekend,
            value,
            reason,
        }
    }
}

/// De reden bij een formulier waarin nog iets ontbreekt.
fn not_filled(fields: &[&str]) -> String {
    format!(
        "onbekend, want het formulier heeft nog geen waarde voor: {}",
        fields.join(", ")
    )
}

/// Een regelingsvoorwaarde: laat de eigen engine de uitkomst uitrekenen.
fn regulation_outcome(
    actor: &Cell,
    regulation: &str,
    output: &str,
    article: Option<&str>,
    params: &BTreeMap<String, Value>,
    clock: NaiveDate,
) -> Found {
    let waar_of_onwaar = match actor.evaluate_own_output(regulation, output, params, clock) {
        Err(reason) => return Found::onbekend(None, format!("onbekend: {reason}")),
        Ok(None) => {
            return Found::onbekend(
                None,
                format!("onbekend, want '{regulation}' leverde geen '{output}' op"),
            )
        }
        Ok(Some(value)) => value,
    };
    if let Some(reason) = niet_geleverd_reden(&waar_of_onwaar) {
        return Found::onbekend(Some(waar_of_onwaar), reason);
    }
    let Some(ja) = waar_of_onwaar.as_bool() else {
        return Found::onbekend(
            Some(waar_of_onwaar.clone()),
            format!("onbekend, want '{output}' is geen ja of nee maar {waar_of_onwaar}"),
        );
    };
    let waar = if ja { "waar" } else { "onwaar" };
    let bron = match article {
        Some(article) => format!("art. {article} van {regulation}"),
        None => regulation.to_string(),
    };
    Found {
        outcome: if ja {
            ConditionOutcome::Waar
        } else {
            ConditionOutcome::Onwaar
        },
        value: Some(waar_of_onwaar),
        reason: format!("volgens {bron} is '{output}' {waar}"),
    }
}

/// Een lexostatusvoorwaarde: reduceer de eigen lexostatus en vergelijk.
fn lexostatus_outcome(
    actor: &Cell,
    lexostatus: &str,
    output: &str,
    expect: &Value,
    params: &BTreeMap<String, Value>,
    clock: NaiveDate,
) -> Found {
    let answer = match actor.reduce(lexostatus, params, clock) {
        Ok(answer) => answer,
        Err(error) => return Found::onbekend(None, format!("onbekend: {error}")),
    };
    let (actual, stand) = match answer.values() {
        Some(values) => {
            let actual = values.get(output).cloned().unwrap_or(Value::Null);
            let stand = format!("'{output}' is {}", shown(&actual));
            (actual, stand)
        }
        None => (
            Value::Null,
            format!(
                "niets vastgesteld ({})",
                answer.not_established().unwrap_or("geen vastlegging")
            ),
        ),
    };
    if let Some(reason) = niet_geleverd_reden(&actual) {
        return Found::onbekend(Some(actual), reason);
    }
    let outcome = if &actual == expect {
        ConditionOutcome::Waar
    } else {
        ConditionOutcome::Onwaar
    };
    Found {
        outcome,
        reason: format!(
            "'{lexostatus}' bij {}: {stand}; {}",
            actor.id(),
            match expect {
                Value::Null => "waar als er niets ligt".to_string(),
                expect => format!("waar bij {}", shown(expect)),
            }
        ),
        value: Some(actual),
    }
}

/// Een waarde zoals een lezer haar in een reden leest: `null` is "niets".
fn shown(value: &Value) -> String {
    match value {
        Value::Null => "niets".to_string(),
        Value::String(text) => format!("'{text}'"),
        other => other.to_string(),
    }
}

/// Een komma-gescheiden lijst, of "niets" als ze leeg is.
fn joined<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let names: Vec<&str> = names.into_iter().collect();
    if names.is_empty() {
        "niets".to_string()
    } else {
        names.join(", ")
    }
}
