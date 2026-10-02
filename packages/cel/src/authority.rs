//! Actor identity: on behalf of which competent authority a process acts, and with
//! which mandate.
//!
//! RFC-022 §2 keeps three axes apart, and so does the runtime:
//!
//! - `recording_actor`: who records. The actor of the process, which is the
//!   `recording_actor` of every stream it records in (an id).
//! - `competent_authority`: the competent authority according to the law, of the article
//!   or otherwise of the regulation (a name).
//! - the acting actor: who acts in the process, in a role, through a
//!   channel, on behalf of an authority ([`crate::gram::ActingActor`]).
//!
//! What connects the actor to the authority is the policy (RFC-047): the
//! process acts on behalf of the competent authority of the implementing
//! policy that declares its channels (`on_behalf_of`), by the name a
//! regulation gives in `competent_authority`, or a regulation whose
//! competent authority it is. The runtime compares names literally; there is
//! no normalization that reads an id as a name. If the process also acts for
//! another authority, `mandates` in that policy names that authority with a
//! legal basis (Awb 10:1). Without a mandate
//! the decision is refused for another authority.

use std::collections::BTreeSet;

use regelrecht_engine::{ArticleBasedLaw, LawExecutionService};
use serde_json::Value;

use crate::config::{Mandate, OnBehalfOf, ProcessDefinition};
use crate::regulations;

/// A name of an authority from `competent_authority`: a text, or an
/// object with `name`. A reference (`#bevoegd_gezag`) is not a name.
fn name(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => s.strip_prefix('#').is_none().then(|| s.clone()),
        Value::Object(o) => o.get("name").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn name_of<T: serde::Serialize>(authority: &T) -> Option<String> {
    name(&serde_json::to_value(authority).ok()?)
}

/// The competent authority of a regulation itself, without that of an article.
pub fn authority_of_regulation(service: &LawExecutionService, regulation: &str) -> Option<String> {
    authority_of_law(service.resolver().get_law(regulation)?)
}

/// The competent authority of one version of a regulation itself.
pub fn authority_of_law(law: &ArticleBasedLaw) -> Option<String> {
    name_of(law.competent_authority.as_ref()?)
}

/// The competent authority according to the law: of the article itself, otherwise of the
/// regulation.
pub fn authority_of(
    service: &LawExecutionService,
    regulation: &str,
    article: &str,
) -> Option<String> {
    let law = service.resolver().get_law(regulation)?;
    let authority = law
        .find_article_by_number(article)
        .and_then(|a| a.machine_readable.as_ref())
        .and_then(|m| m.competent_authority.as_ref())
        .or(law.competent_authority.as_ref())?;
    name_of(authority)
}

/// Every authority a loaded regulation names, on the regulation or on an
/// article.
pub fn authorities(service: &LawExecutionService) -> BTreeSet<String> {
    service
        .list_laws()
        .into_iter()
        .flat_map(|id| authorities_of_regulation(service, id))
        .collect()
}

/// Every authority one regulation names, on itself or on an article. A
/// reference (`#bevoegd_gezag`) is not a name and does not count.
pub fn authorities_of_regulation(
    service: &LawExecutionService,
    regulation: &str,
) -> BTreeSet<String> {
    let Some(law) = service.resolver().get_law(regulation) else {
        return BTreeSet::new();
    };
    let mut out: BTreeSet<String> = authority_of_law(law).into_iter().collect();
    for a in &law.articles {
        out.extend(
            a.machine_readable
                .as_ref()
                .and_then(|m| m.competent_authority.as_ref())
                .and_then(name_of),
        );
    }
    out
}

/// The beschikkingen for which `authority` is competent, as (regulation, article):
/// every article that produces a `BESCHIKKING` and whose competent authority
/// (of the article, otherwise of the regulation) is `authority`. That way a process finds
/// its decision in the law, without the configuration designating it.
pub fn decision_orders_of(service: &LawExecutionService, authority: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in service.list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        for a in &law.articles {
            let decision_order = a
                .get_execution_spec()
                .and_then(|e| e.produces.as_ref())
                .and_then(|p| p.legal_character.as_deref())
                == Some("BESCHIKKING");
            if decision_order && authority_of(service, id, &a.number).as_deref() == Some(authority)
            {
                out.push((id.to_string(), a.number.clone()));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The authority the process acts for, from `on_behalf_of`, without checking:
/// `None` if the process names none or the regulation has none.
pub fn own(d: &ProcessDefinition, service: &LawExecutionService) -> Option<String> {
    match d.on_behalf_of.as_ref()? {
        OnBehalfOf::Authority { authority } => Some(authority.clone()),
        OnBehalfOf::Regulation { regulation } => authority_of_regulation(service, regulation),
    }
}

/// The authority the process acts for, from `on_behalf_of`, checked against
/// the law; and the check on the mandates. A name must be an authority that
/// a loaded regulation names; a regulation must be loaded and name an
/// authority. A mandate names such an authority, not the own authority, and a
/// legal basis that points to a loaded article.
pub fn loose_at(
    d: &ProcessDefinition,
    service: &LawExecutionService,
) -> Result<Option<String>, Vec<String>> {
    let known = authorities(service);
    let unknown = |g: &str| {
        format!(
            "no loaded regulation names '{g}' as competent authority (competent_authority); known: {}",
            if known.is_empty() {
                "none".to_string()
            } else {
                known.iter().cloned().collect::<Vec<_>>().join(", ")
            }
        )
    };
    let mut errors = Vec::new();
    let own = match &d.on_behalf_of {
        None => None,
        Some(OnBehalfOf::Authority { authority }) => {
            if !known.contains(authority) {
                errors.push(format!("on_behalf_of: {}", unknown(authority)));
            }
            Some(authority.clone())
        }
        Some(OnBehalfOf::Regulation { regulation }) => match service.resolver().get_law(regulation)
        {
            None => {
                errors.push(format!(
                    "on_behalf_of: regulation '{regulation}' is not loaded"
                ));
                None
            }
            Some(_) => match authority_of_regulation(service, regulation) {
                Some(g) => Some(g),
                None => {
                    errors.push(format!(
                        "on_behalf_of: regulation '{regulation}' names no competent authority (competent_authority)"
                    ));
                    None
                }
            },
        },
    };
    for m in &d.mandates {
        if !known.contains(&m.authority) {
            errors.push(format!("mandate: {}", unknown(&m.authority)));
        }
        if own.as_deref() == Some(m.authority.as_str()) {
            errors.push(format!(
                "mandate: '{}' is the authority the process itself acts for (on_behalf_of)",
                m.authority
            ));
        }
        if let Err(f) = regulations::valid(service, &m.legal_basis) {
            errors.push(format!("mandate of '{}': {f}", m.authority));
        }
    }
    if d.handling.is_some() && d.on_behalf_of.is_none() {
        errors.push(
            "handling without on_behalf_of: name the competent authority the process decides for (on_behalf_of: {authority: <name>} or {regulation: <$id>})".into(),
        );
    }
    if errors.is_empty() {
        Ok(own)
    } else {
        Err(errors)
    }
}

/// Why the process may take a decision of `law` (the authority the law designates):
/// as that authority itself, or under mandate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Competence<'a> {
    Own,
    Mandate(&'a Mandate),
}

/// Check the authority of the law against the own authority and the mandates. Another
/// authority without a mandate is a refusal, with the reason.
pub fn assessment<'a>(
    own: Option<&str>,
    mandates: &'a [Mandate],
    law: &str,
) -> Result<Competence<'a>, String> {
    if own == Some(law) {
        return Ok(Competence::Own);
    }
    if let Some(m) = mandates.iter().find(|m| m.authority == law) {
        return Ok(Competence::Mandate(m));
    }
    Err(match own {
        Some(e) => format!(
            "the law designates '{law}' as competent authority, and the process acts on behalf of '{e}' without a mandate from '{law}'"
        ),
        None => format!(
            "the law designates '{law}' as competent authority, and the process does not name on behalf of whom it acts"
        ),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// A fictitious regulation with a beschikking of "De Instantie van
    /// Voorbeeld", an article with its own authority, and a mandate regulation.
    const REGULATION: &str = r#"
$id: testregeling_bevoegd
regulatory_layer: WET
publication_date: '2025-01-01'
competent_authority:
  name: De Instantie van Voorbeeld
articles:
  - number: '1'
    text: Toets
    machine_readable:
      execution:
        produces: {legal_character: TOETS, decision_type: GEEN_BESLUIT}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: toets, type: boolean}]
        actions: [{output: toets, value: {operation: GREATER_THAN, subject: $x, value: 0}}]
  - number: '2'
    text: Besluit
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: $x}]
  - number: '3'
    text: Besluit van de raad
    machine_readable:
      competent_authority: De Raad van Voorbeeld
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: bedrag_raad, type: number}]
        actions: [{output: bedrag_raad, value: $x}]
  - number: '4'
    text: De raad verleent de instantie mandaat voor de besluiten van artikel 3.
"#;

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(REGULATION).unwrap();
        s
    }

    fn process(on_behalf_of: Option<OnBehalfOf>, mandates: &[(&str, &str)]) -> ProcessDefinition {
        ProcessDefinition {
            id: "p".into(),
            actor: "de_instantie".into(),
            on_behalf_of,
            mandates: mandates
                .iter()
                .map(|(authority, legal_basis)| Mandate {
                    authority: authority.to_string(),
                    legal_basis: legal_basis.to_string(),
                })
                .collect(),
            ..Default::default()
        }
    }

    fn authority(name: &str) -> Option<OnBehalfOf> {
        Some(OnBehalfOf::Authority {
            authority: name.into(),
        })
    }

    fn regulation(name: &str) -> Option<OnBehalfOf> {
        Some(OnBehalfOf::Regulation {
            regulation: name.into(),
        })
    }

    #[test]
    fn the_beschikking_of_the_competent_authority_is_found() {
        let s = service();
        assert_eq!(
            decision_orders_of(&s, "De Instantie van Voorbeeld"),
            vec![("testregeling_bevoegd".to_string(), "2".to_string())]
        );
        // Literally: an id is not a name.
        assert!(decision_orders_of(&s, "de_instantie_van_voorbeeld").is_empty());
        assert!(decision_orders_of(&s, "Een ander orgaan").is_empty());
    }

    #[test]
    fn on_behalf_of_an_authority_or_a_regulation() {
        let s = service();
        let d = process(authority("De Raad van Voorbeeld"), &[]);
        assert_eq!(
            loose_at(&d, &s).unwrap().as_deref(),
            Some("De Raad van Voorbeeld")
        );
        let d = process(regulation("testregeling_bevoegd"), &[]);
        assert_eq!(
            loose_at(&d, &s).unwrap().as_deref(),
            Some("De Instantie van Voorbeeld")
        );
        let d = process(authority("de_instantie_van_voorbeeld"), &[]);
        let f = loose_at(&d, &s).unwrap_err();
        assert!(
            f[0].contains("no loaded regulation names 'de_instantie_van_voorbeeld'"),
            "{f:?}"
        );
        let d = process(regulation("bestaat_niet"), &[]);
        assert!(loose_at(&d, &s).unwrap_err()[0].contains("not loaded"));
        assert_eq!(loose_at(&process(None, &[]), &s).unwrap(), None);
    }

    #[test]
    fn a_mandate_names_a_known_authority_and_a_legal_basis() {
        let s = service();
        let ok = process(
            regulation("testregeling_bevoegd"),
            &[("De Raad van Voorbeeld", "testregeling_bevoegd#4")],
        );
        assert!(loose_at(&ok, &s).is_ok());
        let error = process(
            regulation("testregeling_bevoegd"),
            &[
                ("De Instantie van Voorbeeld", "testregeling_bevoegd#9"),
                ("Niemand", "testregeling_bevoegd#4"),
            ],
        );
        let f = loose_at(&error, &s).unwrap_err();
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f[0].contains("the authority the process itself acts for"));
        assert!(f[1].contains("testregeling_bevoegd#9"));
        assert!(f[2].contains("'Niemand'"));
    }

    #[test]
    fn own_authority_mandate_or_refusal() {
        let m = [Mandate {
            authority: "De Raad van Voorbeeld".into(),
            legal_basis: "testregeling_bevoegd#4".into(),
        }];
        let own = Some("De Instantie van Voorbeeld");
        assert_eq!(
            assessment(own, &m, "De Instantie van Voorbeeld"),
            Ok(Competence::Own)
        );
        assert_eq!(
            assessment(own, &m, "De Raad van Voorbeeld"),
            Ok(Competence::Mandate(&m[0]))
        );
        assert!(assessment(own, &m, "Een ander")
            .unwrap_err()
            .contains("without a mandate from 'Een ander'"));
        assert!(assessment(own, &[], "De Raad van Voorbeeld").is_err());
        assert!(assessment(None, &m, "Een ander")
            .unwrap_err()
            .contains("does not name on behalf of whom"));
    }
}
