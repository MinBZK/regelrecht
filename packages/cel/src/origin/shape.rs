//! The shape of `origin` and `origins` in a regulation, and the implementing
//! policy that overrides an origin.

use super::*;

/// The shape of an `origin` by itself, apart from the process: the legal basis
/// can be parsed, `REGISTER` names its register and only `REGISTER` does so,
/// a window and the decision requested (`TIJDVAK`, `GEVRAAGD_BESLUIT`) come
/// from the interested party (Awb 4:2 lid 1), and the decision an action acts
/// on (`BESLUIT`, RFC-047) comes from the dossier: the runtime gives the id
/// of that decision gram.
fn shape(o: &Origin) -> Vec<String> {
    let mut errors = Vec::new();
    if let Err(f) = regulations::parse(&o.legal_basis) {
        errors.push(f);
    }
    match (o.waarde, o.register.as_deref().map(str::trim)) {
        (OriginValue::Register, None | Some("")) => errors.push(
            "origin REGISTER without register: which regulation keeps the register cannot be traced"
                .into(),
        ),
        (OriginValue::Register, Some(_)) | (_, None) => {}
        (w, Some(r)) => errors.push(format!(
            "origin {} with register '{r}': only REGISTER names a register",
            w.as_str()
        )),
    }
    match o.rol {
        // The decision an action acts on is a fact of the course of the
        // case, which the runtime gives (RFC-047).
        Some(OriginRole::Besluit) if o.waarde != OriginValue::Dossier => errors.push(format!(
            "rol BESLUIT with origin {}: the runtime gives the decision an action acts on, a fact of the course of the case, so DOSSIER",
            o.waarde.as_str()
        )),
        Some(OriginRole::Besluit) | None => {}
        Some(rol) if o.waarde != OriginValue::Belanghebbende => errors.push(format!(
            "rol {} with origin {}: the applicant chooses the window and the decision requested as part of the application (Awb 4:2 lid 1), so BELANGHEBBENDE",
            rol.as_str(),
            o.waarde.as_str()
        )),
        Some(_) => {}
    }
    errors
}

/// Check `origin` on every parameter and `origins` on every article of a
/// loaded regulation: a value that cannot be read, or an origin that is
/// not right by itself (see `shape`). Every message names article and
/// parameter; the caller puts the file in front.
pub fn validate(law: &ArticleBasedLaw) -> Vec<String> {
    let mut errors = Vec::new();
    for a in &law.articles {
        for p in a.get_parameters() {
            let Some(o) = &p.origin else { continue };
            let where_ = format!("article {}, parameter '{}'", a.number, p.name);
            match o.valid() {
                Err(e) => errors.push(format!("{where_}: invalid origin: {e}")),
                Ok(o) => errors.extend(shape(o).into_iter().map(|f| format!("{where_}: {f}"))),
            }
        }
        let Some(origins) = a.machine_readable.as_ref().and_then(|m| m.origins.as_ref()) else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            errors.push(format!(
                "article {}: origins is only allowed in implementing policy (RFC-048)",
                a.number
            ));
        }
        for (i, o) in origins.iter().enumerate() {
            match o.valid() {
                Err(e) => errors.push(format!(
                    "article {}, origins[{i}]: invalid override: {e}",
                    a.number
                )),
                Ok(o) => errors.extend(shape(&o.origin).into_iter().map(|f| {
                    format!(
                        "article {}, origins for '{}' of {}: {f}",
                        a.number, o.parameter, o.regulation
                    )
                })),
            }
        }
    }
    errors
}

/// The overrides by the implementing policy of an actor, per
/// (regulation, parameter).
#[derive(Debug, Default)]
pub struct Overwrites(BTreeMap<(String, String), InForce>);

/// Read `origins` from every loaded implementing policy whose competent
/// authority (of the article, otherwise of the regulation) is the authority on
/// whose behalf the process acts (`on_behalf_of`, see [`crate::authority`]);
/// without that authority none. Two articles that give the same parameter a
/// different origin are an error.
pub fn overwrites(
    service: &LawExecutionService,
    authority: Option<&str>,
) -> Result<Overwrites, Vec<String>> {
    let mut out: BTreeMap<(String, String), InForce> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut ids: Vec<&str> = service.list_laws();
    ids.sort_unstable();
    for id in ids {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            continue;
        }
        for a in &law.articles {
            let Some(origins) = a.machine_readable.as_ref().and_then(|m| m.origins.as_ref()) else {
                continue;
            };
            let of_actor = authority.is_some()
                && authority::authority_of(service, id, &a.number).as_deref() == authority;
            if !of_actor {
                continue;
            }
            let article = format!("{id}#{}", a.number);
            for o in origins
                .iter()
                .filter_map(Declared::<OriginOverride>::as_valid)
            {
                if let Err(f) = regulations::parse(&o.origin.legal_basis) {
                    errors.push(format!("origins in {article}: {f}"));
                    continue;
                }
                if !declares(service, &o.regulation, &o.parameter) {
                    errors.push(format!(
                        "origins in {article}: regulation '{}' has no parameter '{}'",
                        o.regulation, o.parameter
                    ));
                    continue;
                }
                let new = InForce {
                    origin: o.origin.clone(),
                    policy: Some(article.clone()),
                };
                let key = (o.regulation.clone(), o.parameter.clone());
                match out.get(&key) {
                    Some(earlier) if earlier.origin != new.origin => errors.push(format!(
                        "origins: '{}' of {} gets two origins: {} and {}",
                        o.parameter,
                        o.regulation,
                        earlier.description(),
                        new.description()
                    )),
                    Some(_) => {}
                    None => {
                        out.insert(key, new);
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(Overwrites(out))
    } else {
        Err(errors)
    }
}

/// Whether a loaded regulation declares a parameter with this name anywhere.
fn declares(service: &LawExecutionService, regulation: &str, parameter: &str) -> bool {
    service.resolver().get_law(regulation).is_some_and(|l| {
        l.articles
            .iter()
            .any(|a| a.get_parameters().iter().any(|p| p.name == parameter))
    })
}

/// The parameter behind a [`Required`].
pub fn parameter<'s>(service: &'s LawExecutionService, b: &Required) -> Option<&'s Parameter> {
    regulations::article(service, &b.article)
        .ok()?
        .get_parameters()
        .iter()
        .find(|p| p.name == b.name)
}

impl Overwrites {
    /// The origin in force of a parameter of a regulation: the one from the
    /// policy, otherwise the one from the law. An origin that cannot be read
    /// counts as none; loading the regulation already reported it.
    pub fn in_force(&self, regulation: &str, p: &Parameter) -> Option<InForce> {
        if let Some(g) = self.0.get(&(regulation.to_string(), p.name.clone())) {
            return Some(g.clone());
        }
        p.origin
            .as_ref()
            .and_then(Declared::as_valid)
            .map(|origin| InForce {
                origin: origin.clone(),
                policy: None,
            })
    }
}
