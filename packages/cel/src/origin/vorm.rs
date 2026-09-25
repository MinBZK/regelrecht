//! De vorm van `origin` en `origins` in een regeling, en het uitvoeringsbeleid
//! dat een herkomst overschrijft.

use super::*;

/// De vorm van een `origin` op zichzelf, los van het proces: de grondslag is
/// te ontleden, `REGISTER` noemt zijn register en alleen `REGISTER` doet dat,
/// en een tijdvak komt van de belanghebbende.
fn vorm(o: &Origin) -> Vec<String> {
    let mut fouten = Vec::new();
    if let Err(f) = regelingen::ontleed(&o.grondslag) {
        fouten.push(f);
    }
    match (o.waarde, o.register.as_deref().map(str::trim)) {
        (OriginValue::Register, None | Some("")) => fouten.push(
            "origin REGISTER zonder register: welke regeling het register houdt, is niet na te gaan"
                .into(),
        ),
        (OriginValue::Register, Some(_)) | (_, None) => {}
        (w, Some(r)) => fouten.push(format!(
            "origin {} met register '{r}': alleen REGISTER noemt een register",
            w.as_str()
        )),
    }
    if o.rol == Some(OriginRole::Tijdvak) && o.waarde != OriginValue::Belanghebbende {
        fouten.push(format!(
            "rol TIJDVAK bij origin {}: het tijdvak kiest de aanvrager als deel van de gevraagde beschikking (Awb 4:2 lid 1), dus BELANGHEBBENDE",
            o.waarde.as_str()
        ));
    }
    fouten
}

/// Controleer `origin` op elke parameter en `origins` op elk artikel van een
/// geladen regeling: een waarde die niet te lezen is, of een origin die op
/// zichzelf niet klopt (zie `vorm`). Elke melding noemt artikel en
/// parameter; de aanroeper zet het bestand ervoor.
pub fn valideer(law: &ArticleBasedLaw) -> Vec<String> {
    let mut fouten = Vec::new();
    for a in &law.articles {
        for p in a.get_parameters() {
            let Some(o) = &p.origin else { continue };
            let waar = format!("artikel {}, parameter '{}'", a.number, p.name);
            match o.valid() {
                Err(e) => fouten.push(format!("{waar}: ongeldige origin: {e}")),
                Ok(o) => fouten.extend(vorm(o).into_iter().map(|f| format!("{waar}: {f}"))),
            }
        }
        let Some(origins) = a.machine_readable.as_ref().and_then(|m| m.origins.as_ref()) else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            fouten.push(format!(
                "artikel {}: origins staat alleen in uitvoeringsbeleid (RFC-043)",
                a.number
            ));
        }
        for (i, o) in origins.iter().enumerate() {
            match o.valid() {
                Err(e) => fouten.push(format!(
                    "artikel {}, origins[{i}]: ongeldige overschrijving: {e}",
                    a.number
                )),
                Ok(o) => fouten.extend(vorm(&o.origin).into_iter().map(|f| {
                    format!(
                        "artikel {}, origins voor '{}' van {}: {f}",
                        a.number, o.parameter, o.regulation
                    )
                })),
            }
        }
    }
    fouten
}

/// De overschrijvingen door het uitvoeringsbeleid van een actor, per
/// (regeling, parameter).
#[derive(Debug, Default)]
pub struct Overschrijvingen(BTreeMap<(String, String), Geldend>);

/// Lees `origins` uit elk geladen uitvoeringsbeleid waarvan het bevoegd
/// gezag (van het artikel, anders van de regeling) het gezag is waarvoor het
/// proces handelt (`namens`, zie [`crate::gezag`]); zonder dat gezag geen. Twee
/// artikelen die dezelfde parameter een andere herkomst geven, zijn een fout.
pub fn overschrijvingen(
    service: &LawExecutionService,
    gezag: Option<&str>,
) -> Result<Overschrijvingen, Vec<String>> {
    let mut uit: BTreeMap<(String, String), Geldend> = BTreeMap::new();
    let mut fouten = Vec::new();
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
            let van_actor =
                gezag.is_some() && gezag::gezag_van(service, id, &a.number).as_deref() == gezag;
            if !van_actor {
                continue;
            }
            let artikel = format!("{id}#{}", a.number);
            for o in origins
                .iter()
                .filter_map(Declared::<OriginOverride>::as_valid)
            {
                if let Err(f) = regelingen::ontleed(&o.origin.grondslag) {
                    fouten.push(format!("origins in {artikel}: {f}"));
                    continue;
                }
                if !declareert(service, &o.regulation, &o.parameter) {
                    fouten.push(format!(
                        "origins in {artikel}: regeling '{}' heeft geen parameter '{}'",
                        o.regulation, o.parameter
                    ));
                    continue;
                }
                let nieuw = Geldend {
                    origin: o.origin.clone(),
                    beleid: Some(artikel.clone()),
                };
                let sleutel = (o.regulation.clone(), o.parameter.clone());
                match uit.get(&sleutel) {
                    Some(eerder) if eerder.origin != nieuw.origin => fouten.push(format!(
                        "origins: '{}' van {} krijgt twee herkomsten: {} en {}",
                        o.parameter,
                        o.regulation,
                        eerder.beschrijving(),
                        nieuw.beschrijving()
                    )),
                    Some(_) => {}
                    None => {
                        uit.insert(sleutel, nieuw);
                    }
                }
            }
        }
    }
    if fouten.is_empty() {
        Ok(Overschrijvingen(uit))
    } else {
        Err(fouten)
    }
}

/// Of een geladen regeling ergens een parameter met deze naam declareert.
fn declareert(service: &LawExecutionService, regeling: &str, parameter: &str) -> bool {
    service.resolver().get_law(regeling).is_some_and(|l| {
        l.articles
            .iter()
            .any(|a| a.get_parameters().iter().any(|p| p.name == parameter))
    })
}

/// De parameter achter een [`Benodigd`].
pub fn parameter<'s>(service: &'s LawExecutionService, b: &Benodigd) -> Option<&'s Parameter> {
    regelingen::artikel(service, &b.artikel)
        .ok()?
        .get_parameters()
        .iter()
        .find(|p| p.name == b.naam)
}

impl Overschrijvingen {
    /// De geldende herkomst van een parameter van een regeling: die uit het
    /// beleid, anders die uit de wet. Een origin die niet te lezen is, telt
    /// als geen; het laden van de regeling heeft hem al gemeld.
    pub fn geldend(&self, regeling: &str, p: &Parameter) -> Option<Geldend> {
        if let Some(g) = self.0.get(&(regeling.to_string(), p.name.clone())) {
            return Some(g.clone());
        }
        p.origin
            .as_ref()
            .and_then(Declared::as_valid)
            .map(|origin| Geldend {
                origin: origin.clone(),
                beleid: None,
            })
    }
}
