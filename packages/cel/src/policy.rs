//! The process of an actor from its policy (RFC-047): which channels there
//! are, who logs in through them, what each supplies and which submission the
//! portal records. Read from `produces.extensions.chronolex` of implementing
//! policy (`UITVOERINGSBELEID`) that names a competent authority; elsewhere
//! it is an error. The technique of a channel is in the deployment.
//!
//! Also the startup check on `executes`: every entry is valid, the target
//! exists, and a policy only works out a competence of its own authority
//! (Awb 4:81).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_engine::{LawExecutionService, RegulatoryLayer};
use serde::Deserialize;

use crate::authority;
use crate::channel::Routes;
use crate::config::{Mandate, Offer};

/// What a channel is for; it decides the routes of its role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelKind {
    Portal,
    Handling,
    Counter,
}

impl ChannelKind {
    pub fn routes(self) -> Routes {
        match self {
            ChannelKind::Portal => Routes::Portal,
            ChannelKind::Handling => Routes::Handling,
            ChannelKind::Counter => Routes::Counter,
        }
    }
}

/// What the login of a channel shows: names, or names with the legal basis
/// that knows the data item.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Identifies {
    Names(Vec<String>),
    WithBasis(BTreeMap<String, Vec<String>>),
}

impl Identifies {
    pub fn fields(&self) -> BTreeMap<String, Vec<String>> {
        match self {
            Identifies::Names(n) => n.iter().map(|n| (n.clone(), Vec::new())).collect(),
            Identifies::WithBasis(m) => m.clone(),
        }
    }
}

/// One legal basis or more.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OneOrMore {
    One(String),
    More(Vec<String>),
}

impl OneOrMore {
    pub fn list(&self) -> Vec<String> {
        match self {
            OneOrMore::One(s) => vec![s.clone()],
            OneOrMore::More(v) => v.clone(),
        }
    }
}

/// The output of the submitted article that the portal assesses.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assesses {
    pub output: String,
}

/// The working document of the actor that gives the form its order, groups
/// and labels; `document` relative to the corpus root.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyForm {
    pub document: String,
    pub screen: String,
}

/// A channel as the policy names it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyChannel {
    pub kind: ChannelKind,
    /// The role that logs in through it; without: the channel id.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub identifies: Option<Identifies>,
    #[serde(default)]
    pub owner: Option<String>,
    /// The article whose submission this portal records.
    #[serde(default)]
    pub submits: Option<String>,
    #[serde(default)]
    pub assesses: Option<Assesses>,
    #[serde(default)]
    pub offers: Option<Offer>,
    #[serde(default)]
    pub form: Option<PolicyForm>,
    #[serde(default)]
    pub legal_basis: Option<OneOrMore>,
}

/// A channel with the article that declares it.
#[derive(Debug, Clone)]
pub struct DeclaredChannel {
    pub id: String,
    /// `<regulation>#<article>`.
    pub article: String,
    pub def: PolicyChannel,
}

/// The policy of one competent authority.
#[derive(Debug, Clone, Default)]
pub struct ActorPolicy {
    pub authority: String,
    pub channels: Vec<DeclaredChannel>,
    /// Per channel: the article that says it, and what it supplies.
    pub supplies: BTreeMap<String, (String, BTreeMap<String, String>)>,
    pub mandates: Vec<Mandate>,
}

/// The policies with channels, per competent authority, in the version that
/// applies on `date` (the newest without one).
pub fn read(
    service: &LawExecutionService,
    date: Option<NaiveDate>,
) -> Result<BTreeMap<String, ActorPolicy>, Vec<String>> {
    let articles = crate::law::articles(service, date)?;
    let mut out: BTreeMap<String, ActorPolicy> = BTreeMap::new();
    let mut errors = Vec::new();
    for (reference, a) in &articles {
        let c = &a.chronolex;
        if c.channels.is_empty() && c.supplies.is_empty() && c.mandates.is_empty() {
            continue;
        }
        if a.regulation.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            errors.push(format!(
                "{reference}: channels, supplies and mandates belong in implementing policy (regulatory_layer UITVOERINGSBELEID)"
            ));
            continue;
        }
        let Some(authority) = authority::authority_of_regulation(service, &a.regulation.id) else {
            errors.push(format!(
                "{reference}: the policy names no competent_authority; a channel belongs to an authority"
            ));
            continue;
        };
        let p = out.entry(authority.clone()).or_insert_with(|| ActorPolicy {
            authority,
            ..ActorPolicy::default()
        });
        for (id, def) in &c.channels {
            if let Some(d) = p.channels.iter().find(|d| &d.id == id) {
                errors.push(format!(
                    "{reference}: channel '{id}' is already declared in {}",
                    d.article
                ));
                continue;
            }
            p.channels.push(DeclaredChannel {
                id: id.clone(),
                article: reference.clone(),
                def: def.clone(),
            });
        }
        for (id, s) in &c.supplies {
            if let Some((by, _)) = p.supplies.get(id) {
                errors.push(format!(
                    "{reference}: what channel '{id}' supplies is already said in {by}"
                ));
                continue;
            }
            p.supplies
                .insert(id.clone(), (reference.clone(), s.clone()));
        }
        p.mandates.extend(c.mandates.iter().cloned());
    }
    for p in out.values() {
        let known: BTreeSet<&str> = p.channels.iter().map(|c| c.id.as_str()).collect();
        for (id, (by, _)) in &p.supplies {
            if !known.contains(id.as_str()) {
                errors.push(format!(
                    "{by}: supplies for channel '{id}', which the policy of '{}' does not declare",
                    p.authority
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// Every `executes` of every loaded regulation: each entry is valid (the
/// engine skips an invalid one, the runtime reports it), the target article
/// exists, and the policy only works out a competence of its own authority
/// (Awb 4:81). The target, or else its regulation, names the authority of
/// the policy, or names none and its regulation names no other authority
/// (general law, such as the Awb). Each message names both articles.
pub fn check_executes(service: &LawExecutionService) -> Vec<String> {
    let resolver = service.resolver();
    let mut errors = Vec::new();
    for id in service.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for a in &law.articles {
            let from = format!("{id}#{}", a.number);
            for reason in a.get_invalid_executes() {
                errors.push(format!(
                    "{from}: an entry of executes is not valid: {reason}"
                ));
            }
            for e in a.get_executes() {
                errors.extend(check_one(service, id, &from, &e.article).err());
            }
        }
    }
    errors
}

/// One `executes` target of the article `from` in `regulation`.
fn check_one(
    service: &LawExecutionService,
    regulation: &str,
    from: &str,
    article: &str,
) -> Result<(), String> {
    let target = crate::regulations::parse(article).map_err(|_| {
        format!("{from}: executes '{article}', which does not have the form <regulation>#<article>")
    })?;
    if target.paragraph.is_some() {
        return Err(format!(
            "{from}: executes '{article}', a paragraph; executes names the article itself"
        ));
    }
    if crate::regulations::article(service, article).is_err() {
        return Err(format!(
            "{from}: executes {article}, which is not a loaded article"
        ));
    }
    let own = authority::authority_of_regulation(service, regulation).ok_or_else(|| {
        format!("{from}: executes {article}, but the policy names no competent_authority")
    })?;
    let of_target = authority::authority_of(service, target.regulation, target.article);
    let named = authorities_in(service, target.regulation);
    let fits = match &of_target {
        Some(t) => t == &own,
        None => named.is_empty() || named.contains(&own),
    };
    if fits {
        return Ok(());
    }
    let theirs = of_target.unwrap_or_else(|| named.iter().cloned().collect::<Vec<_>>().join(", "));
    Err(format!(
        "{from}: executes {article}, a competence of '{theirs}', and the policy is of '{own}' (Awb 4:81)"
    ))
}

/// Every authority a regulation names, on itself or on an article.
fn authorities_in(service: &LawExecutionService, regulation: &str) -> BTreeSet<String> {
    let Some(law) = service.resolver().get_law(regulation) else {
        return BTreeSet::new();
    };
    let mut out: BTreeSet<String> = authority::authority_of_regulation(service, regulation)
        .into_iter()
        .collect();
    for a in &law.articles {
        if a.machine_readable
            .as_ref()
            .and_then(|m| m.competent_authority.as_ref())
            .is_some()
        {
            out.extend(authority::authority_of(service, regulation, &a.number));
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use regelrecht_engine::LawExecutionService;

    const LAW: &str = r#"
$id: wet_t
regulatory_layer: WET
publication_date: '2025-01-01'
competent_authority: {name: Instantie T}
articles:
  - number: '1'
    text: Een partij kan een bijdrage aanvragen.
    machine_readable:
      execution:
        parameters: [{name: jaar, type: number, required: true}]
        output: [{name: aangevraagd, type: boolean}]
        actions: [{output: aangevraagd, value: true}]
"#;

    const POLICY: &str = r#"
$id: beleid_t
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
competent_authority: {name: Instantie T}
articles:
  - number: '1'
    text: Het portaal is het kanaal eherkenning; het behandelportaal het kanaal medewerker.
    machine_readable:
      executes: [{article: 'wet_t#1', as: procedure}]
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          extensions:
            chronolex:
              channels:
                eherkenning: {kind: portal, role: aanvrager, identifies: [kvk, persoon], owner: kvk, submits: 'wet_t#1', assesses: {output: aangevraagd}}
                medewerker: {kind: handling, role: behandelaar, legal_basis: 'beleid_t#1'}
              mandates: [{authority: Een ander, legal_basis: 'beleid_t#1'}]
  - number: '2'
    text: Het kanaal eherkenning levert de ondertekening.
    machine_readable:
      execution:
        produces:
          extensions:
            chronolex:
              supplies:
                eherkenning: {ondertekening: persoon, kanaal: $channel}
"#;

    fn service(texts: &[&str]) -> LawExecutionService {
        let mut s = LawExecutionService::new();
        for t in texts {
            s.load_law(t).unwrap();
        }
        s
    }

    #[test]
    fn channels_supplies_and_mandates_come_from_the_policy() {
        let s = service(&[LAW, POLICY]);
        let p = read(&s, None).unwrap();
        let a = &p["Instantie T"];
        let ids: Vec<&str> = a.channels.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["eherkenning", "medewerker"]);
        assert_eq!(a.channels[0].article, "beleid_t#1");
        assert_eq!(a.channels[0].def.kind, ChannelKind::Portal);
        assert_eq!(a.channels[0].def.submits.as_deref(), Some("wet_t#1"));
        let (by, supplies) = &a.supplies["eherkenning"];
        assert_eq!(by, "beleid_t#2");
        assert_eq!(supplies["ondertekening"], "persoon");
        assert_eq!(a.mandates[0].authority, "Een ander");
    }

    #[test]
    fn a_channel_outside_implementing_policy_is_an_error() {
        let wrong = POLICY.replace(
            "regulatory_layer: UITVOERINGSBELEID",
            "regulatory_layer: MINISTERIELE_REGELING",
        );
        let s = service(&[LAW, &wrong]);
        let e = read(&s, None).unwrap_err();
        assert!(
            e.iter()
                .any(|f| f.contains("beleid_t#1") && f.contains("UITVOERINGSBELEID")),
            "{e:?}"
        );
        let nobody = POLICY.replace("competent_authority: {name: Instantie T}\n", "");
        let s = service(&[LAW, &nobody]);
        let e = read(&s, None).unwrap_err();
        assert!(e.iter().any(|f| f.contains("competent_authority")), "{e:?}");
    }

    #[test]
    fn supplies_for_an_unknown_channel_is_an_error() {
        let s = service(&[
            LAW,
            &POLICY.replace(
                "                eherkenning: {ondertekening",
                "                portaal: {ondertekening",
            ),
        ]);
        let e = read(&s, None).unwrap_err();
        assert!(
            e.iter()
                .any(|f| f.contains("beleid_t#2") && f.contains("'portaal'")),
            "{e:?}"
        );
    }

    #[test]
    fn executes_points_to_an_existing_article_of_the_own_authority() {
        let s = service(&[LAW, POLICY]);
        assert_eq!(check_executes(&s), Vec::<String>::new());
        let s = service(&[
            LAW,
            &POLICY.replace("article: 'wet_t#1'", "article: 'wet_t#9'"),
        ]);
        let e = check_executes(&s);
        assert!(
            e.iter()
                .any(|f| f.contains("beleid_t#1") && f.contains("wet_t#9")),
            "{e:?}"
        );
        let other = LAW.replace(
            "competent_authority: {name: Instantie T}",
            "competent_authority: {name: Instantie U}",
        );
        let s = service(&[&other, POLICY]);
        let e = check_executes(&s);
        assert!(
            e.iter().any(|f| f.contains("beleid_t#1")
                && f.contains("wet_t#1")
                && f.contains("Instantie U")),
            "{e:?}"
        );
        // A general law without any authority may be executed (Awb, BW).
        let general = LAW.replace("competent_authority: {name: Instantie T}\n", "");
        let s = service(&[&general, POLICY]);
        assert_eq!(check_executes(&s), Vec::<String>::new());
    }

    #[test]
    fn an_invalid_executes_entry_is_reported() {
        // The engine skips an invalid entry; the runtime does not.
        let s = service(&[
            LAW,
            &POLICY.replace(
                "executes: [{article: 'wet_t#1', as: procedure}]",
                "executes: [{article: 'wet_t#1', as: procedure}, {article: 'wet_t#1', as: zomaar}]",
            ),
        ]);
        let e = check_executes(&s);
        assert!(
            e.iter()
                .any(|f| f.contains("beleid_t#1") && f.contains("zomaar")),
            "{e:?}"
        );
        // A paragraph is not an article (`articleReference`).
        let s = service(&[
            LAW,
            &POLICY.replace("article: 'wet_t#1'", "article: 'wet_t#1 lid 1'"),
        ]);
        let e = check_executes(&s);
        assert!(
            e.iter()
                .any(|f| f.contains("beleid_t#1") && f.contains("lid 1")),
            "{e:?}"
        );
    }
}
