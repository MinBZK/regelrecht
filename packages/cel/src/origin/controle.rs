//! De controle bij het opstarten: per uitgevoerde uitkomst elke parameter met
//! zijn herkomst en leverancier, en de oordelen van een besluit.

use super::*;

/// De parameters die de aanroeper van een uitkomst moet leveren, in de
/// volgorde waarin het artikel (en daarna elk aangeroepen artikel) ze
/// declareert.
fn in_volgorde(
    service: &LawExecutionService,
    regulation: &str,
    article: &Article,
) -> Vec<Benodigd> {
    let benodigd = regelingen::benodigde_parameters(service, regulation, article);
    let mut uit: Vec<Benodigd> = Vec::new();
    for p in article.get_parameters() {
        if let Some(b) = benodigd.get(&p.name) {
            uit.push(b.clone());
        }
    }
    for b in benodigd.values() {
        if !uit.iter().any(|u| u.name == b.name) {
            uit.push(b.clone());
        }
    }
    uit
}

/// De uitkomsten die het proces uitvoert: de toets, het aanbod en elke
/// uitkomst van het besluit (RFC-043: "every outcome").
fn uitvoeringen(d: &ProcesDefinitie) -> Vec<(Uitvoering<'_>, &str, &str)> {
    let mut uit: Vec<(Uitvoering<'_>, &str, &str)> = Vec::new();
    if let Some(p) = &d.portal {
        uit.push((
            Uitvoering::Toets,
            &p.assessment.regulation,
            &p.assessment.output,
        ));
        if let Some(a) = &p.offer {
            uit.push((Uitvoering::Aanbod, &a.regulation, &a.output));
        }
    }
    for h in d.handling.iter().flat_map(|b| b.actions.iter()) {
        if matches!(h.soort, Handelingsoort::FollowUp { .. }) {
            continue;
        }
        for u in h.outputs.iter().chain(h.assessments.iter()) {
            uit.push((Uitvoering::Handeling(h), &h.regulation, u));
        }
    }
    uit
}

/// Controleer de herkomst van elke parameter van de uitkomsten die het
/// proces uitvoert. `cellen` zijn de cellen van de runtime, voor de
/// register-bronnen.
pub fn controleer(
    d: &ProcesDefinitie,
    cell: &Cell,
    cells: &BTreeMap<String, Arc<Cell>>,
    service: &LawExecutionService,
) -> Controle {
    let mut c = Controle::default();
    let authority = gezag::eigen(d, service);
    let overschrijvingen = match overschrijvingen(service, authority.as_deref()) {
        Ok(o) => o,
        Err(f) => {
            c.fouten.extend(f);
            return c;
        }
    };
    // Een melding per parameter van een artikel, ook als meer uitvoeringen
    // hem vragen.
    let mut gemeld: BTreeSet<(String, String, &'static str)> = BTreeSet::new();
    // Wat niet na te gaan is, per bron en reden: de parameters erbij.
    let mut niet_na_te_gaan: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (uitvoering, regulation, output) in uitvoeringen(d) {
        // Een uitkomst die niet bestaat, meldt de controle op portaal of
        // besluit.
        let Some(article) = service
            .resolver()
            .get_article_by_output(regulation, output, None)
        else {
            continue;
        };
        let leveranciers = Leveranciers::van(d, cell, uitvoering);
        let list = c.parameters.entry(uitvoering.name()).or_default();
        let mut nieuw = Vec::new();
        for b in in_volgorde(service, regulation, article) {
            if list.iter().any(|(eerder, _)| *eerder == b) {
                continue;
            }
            let Some(p) = parameter(service, &b) else {
                continue;
            };
            let law = b
                .article
                .split_once('#')
                .map(|(r, _)| r)
                .unwrap_or(regulation);
            let g = overschrijvingen.geldend(law, p);
            nieuw.push((b, p, g));
        }
        for (b, p, g) in nieuw {
            let mut meld = |soort: &'static str, tekst: String, error: bool| {
                if gemeld.insert((b.article.clone(), b.name.clone(), soort)) {
                    if error {
                        c.fouten.push(tekst);
                    } else {
                        c.warnings.push(tekst);
                    }
                }
            };
            controleer_parameter(
                Parameterplek {
                    d,
                    uitvoering,
                    b: &b,
                    p,
                    g: g.as_ref(),
                },
                &leveranciers,
                cells,
                service,
                &mut meld,
                &mut niet_na_te_gaan,
            );
            c.parameters
                .entry(uitvoering.name())
                .or_default()
                .push((b, g));
        }
    }
    for (reason, namen) in niet_na_te_gaan {
        let namen: Vec<String> = namen.into_iter().map(|n| format!("'{n}'")).collect();
        c.warnings.push(format!(
            "herkomst van {} niet na te gaan: {reason}",
            namen.join(", ")
        ));
    }
    window(d, &mut c);
    c
}

/// Een parameter die een uitvoering vraagt, met wat de controle erover
/// weet.
struct Parameterplek<'a> {
    d: &'a ProcesDefinitie,
    uitvoering: Uitvoering<'a>,
    b: &'a Benodigd,
    p: &'a Parameter,
    g: Option<&'a Geldend>,
}

/// De controles op een parameter: de aanbodregel, de herkomst zelf en zijn
/// leverancier.
fn controleer_parameter(
    plek: Parameterplek<'_>,
    l: &Leveranciers,
    cells: &BTreeMap<String, Arc<Cell>>,
    service: &LawExecutionService,
    meld: &mut impl FnMut(&'static str, String, bool),
    niet_na_te_gaan: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let Parameterplek {
        d,
        uitvoering,
        b,
        p,
        g,
    } = plek;
    if uitvoering.is_aanbod() && !vooraf_bekend(g) {
        let provenance = g
            .map(Geldend::beschrijving)
            .unwrap_or_else(|| "geen origin".into());
        meld(
            "aanbod-regel",
            format!(
                "offer: voorwaarde leunt op '{}' ({provenance}), dat vooraf niet bekend is",
                b.name
            ),
            true,
        );
    }
    let Some(g) = g else {
        let streng = d.origin_check == Herkomstcontrole::Strict;
        meld(
            "zonder",
            format!(
                "provenance: parameter '{}' van {} heeft geen origin; wie hem levert is niet na te gaan{}",
                b.name,
                b.article,
                if streng { " (herkomst: streng)" } else { "" }
            ),
            streng,
        );
        return;
    };
    let waar = format!(
        "parameter '{}' van {} ({})",
        b.name,
        b.article,
        g.beschrijving()
    );
    // Een grondslag in een regeling die niet geladen is, kan kloppen; de
    // runtime kan het alleen niet nagaan.
    if let Err(f) = regelingen::article(service, &g.origin.grondslag) {
        meld(
            "grondslag",
            format!("provenance: {waar}: {f}; niet na te gaan"),
            false,
        );
    }
    if let Some(r) = &g.origin.register {
        if service.resolver().get_law(r).is_none() {
            meld(
                "register",
                format!("provenance: {waar}: register '{r}' is geen geladen regeling"),
                true,
            );
        }
    }
    if g.origin.waarde == OriginValue::Belanghebbende
        && !g.is_tijdvak()
        && p.required != Some(false)
    {
        meld(
            "required",
            format!(
                "provenance: parameter '{}' van {} komt van de belanghebbende, maar heeft geen required: false (RFC-036)",
                b.name, b.article
            ),
            false,
        );
    }
    match leverancier(uitvoering, &b.name, g, l, cells) {
        Uitslag::Past { warnings } => {
            for w in warnings {
                niet_na_te_gaan.entry(w).or_default().insert(b.name.clone());
            }
        }
        Uitslag::Verkeerd(reason) => meld(
            "leverancier",
            format!(
                "{}: verkeerde bron voor {waar}: {reason}",
                uitvoering.name()
            ),
            true,
        ),
        Uitslag::Standalone(reason) => {
            let tekst = format!(
                "{}: geen leverancier voor {waar}{reason}",
                uitvoering.name()
            );
            if p.required == Some(false) {
                meld(
                    "leverancier",
                    format!(
                        "{tekst}; required: false, dus de engine krijgt hem niet en rekent met een onbekende waarde (RFC-036)"
                    ),
                    false,
                );
            } else {
                meld("leverancier", tekst, true);
            }
        }
    }
}

/// Het tijdvak van het aanbod: hoogstens een parameter met `rol: TIJDVAK`,
/// en die vraagt `aanbod.tijdvakken`; tijdvakken zonder zo'n parameter zijn
/// ook een fout.
fn window(d: &ProcesDefinitie, c: &mut Controle) {
    let Some(offer) = d.portal.as_ref().and_then(|p| p.offer.as_ref()) else {
        return;
    };
    let namen: Vec<String> = c
        .parameters
        .get(&Uitvoering::Aanbod.name())
        .into_iter()
        .flatten()
        .filter(|(_, g)| g.as_ref().is_some_and(Geldend::is_tijdvak))
        .map(|(b, _)| b.name.clone())
        .collect();
    match (namen.as_slice(), offer.windows.is_some()) {
        ([], false) => {}
        ([], true) => c.fouten.push(format!(
            "offer: tijdvakken, maar {} vraagt geen tijdvak (een parameter met origin BELANGHEBBENDE en rol TIJDVAK)",
            offer.regulation
        )),
        ([name], true) => c.window = Some(name.clone()),
        ([name], false) => c.fouten.push(format!(
            "offer: het tijdvak '{name}' (rol TIJDVAK) vraagt aanbod.tijdvakken: de uitkomst van het beleid met de tijdvakken die het portaal aanbiedt"
        )),
        (meer, _) => c.fouten.push(format!(
            "offer: meer dan een tijdvak ({}); het portaal biedt er een aan",
            meer.join(", ")
        )),
    }
}

/// Het besluitformulier: de parameters van het besluit met origin
/// `OORDEEL`, in de volgorde van declaratie. Het label is de omschrijving van
/// de parameter, of het deel na "Naam:" als dat er staat, en anders de naam;
/// de groep is het artikel van de grondslag.
pub fn verdicts(c: &Controle, service: &LawExecutionService, action: &str) -> Vec<Oordeel> {
    c.parameters
        .get(action)
        .into_iter()
        .flatten()
        .filter_map(|(b, g)| {
            let g = g
                .as_ref()
                .filter(|g| g.origin.waarde == OriginValue::Oordeel)?;
            let p = parameter(service, b)?;
            Some(Oordeel {
                parameter: b.name.clone(),
                label: label(p),
                group: group(service, &g.origin.grondslag),
                explanation: None,
            })
        })
        .collect()
}

/// Het label van een parameter: het deel van de omschrijving na "Naam:",
/// anders de hele omschrijving, anders de naam.
fn label(p: &Parameter) -> String {
    let tekst = label_uit(p.description.as_deref().unwrap_or_default());
    if tekst.is_empty() {
        p.name.clone()
    } else {
        tekst
    }
}

/// Het label uit een omschrijving: het deel na "Naam:", anders de hele
/// omschrijving, zonder punt aan het eind.
pub fn label_uit(description: &str) -> String {
    let tekst = description.trim();
    let tekst = match tekst.rsplit_once("Naam:") {
        Some((_, name)) => name.trim(),
        None => tekst,
    };
    let tekst = tekst.strip_suffix('.').unwrap_or(tekst).trim();
    tekst.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// De groep van een oordeel: de regeling en het artikel van zijn grondslag.
fn group(service: &LawExecutionService, legal_basis: &str) -> Option<String> {
    let g = regelingen::ontleed(legal_basis).ok()?;
    let name = service
        .resolver()
        .get_law(g.regulation)
        .and_then(|l| l.name.clone())
        .unwrap_or_else(|| crate::formulier::leesbaar(g.regulation));
    Some(format!("{name}, artikel {}", g.article))
}
