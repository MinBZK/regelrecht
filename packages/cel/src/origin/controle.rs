//! De controle bij het opstarten: per uitgevoerde uitkomst elke parameter met
//! zijn herkomst en leverancier, en de oordelen van een besluit.

use super::*;

/// De parameters die de aanroeper van een uitkomst moet leveren, in de
/// volgorde waarin het artikel (en daarna elk aangeroepen artikel) ze
/// declareert.
fn in_volgorde(service: &LawExecutionService, regeling: &str, artikel: &Article) -> Vec<Benodigd> {
    let benodigd = regelingen::benodigde_parameters(service, regeling, artikel);
    let mut uit: Vec<Benodigd> = Vec::new();
    for p in artikel.get_parameters() {
        if let Some(b) = benodigd.get(&p.name) {
            uit.push(b.clone());
        }
    }
    for b in benodigd.values() {
        if !uit.iter().any(|u| u.naam == b.naam) {
            uit.push(b.clone());
        }
    }
    uit
}

/// De uitkomsten die het proces uitvoert: de toets, het aanbod en elke
/// uitkomst van het besluit (RFC-043: "every outcome").
fn uitvoeringen(d: &ProcesDefinitie) -> Vec<(Uitvoering<'_>, &str, &str)> {
    let mut uit: Vec<(Uitvoering<'_>, &str, &str)> = Vec::new();
    if let Some(p) = &d.portaal {
        uit.push((Uitvoering::Toets, &p.toets.regeling, &p.toets.uitkomst));
        if let Some(a) = &p.aanbod {
            uit.push((Uitvoering::Aanbod, &a.regeling, &a.uitkomst));
        }
    }
    for h in d.behandeling.iter().flat_map(|b| b.handelingen.iter()) {
        if matches!(h.soort, Handelingsoort::Vervolg { .. }) {
            continue;
        }
        for u in h.uitkomsten.iter().chain(h.toetsen.iter()) {
            uit.push((Uitvoering::Handeling(h), &h.regeling, u));
        }
    }
    uit
}

/// Controleer de herkomst van elke parameter van de uitkomsten die het
/// proces uitvoert. `cellen` zijn de cellen van de runtime, voor de
/// register-bronnen.
pub fn controleer(
    d: &ProcesDefinitie,
    cel: &Cel,
    cellen: &BTreeMap<String, Arc<Cel>>,
    service: &LawExecutionService,
) -> Controle {
    let mut c = Controle::default();
    let gezag = gezag::eigen(d, service);
    let overschrijvingen = match overschrijvingen(service, gezag.as_deref()) {
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
    for (uitvoering, regeling, uitkomst) in uitvoeringen(d) {
        // Een uitkomst die niet bestaat, meldt de controle op portaal of
        // besluit.
        let Some(artikel) = service
            .resolver()
            .get_article_by_output(regeling, uitkomst, None)
        else {
            continue;
        };
        let leveranciers = Leveranciers::van(d, cel, uitvoering);
        let lijst = c.parameters.entry(uitvoering.naam()).or_default();
        let mut nieuw = Vec::new();
        for b in in_volgorde(service, regeling, artikel) {
            if lijst.iter().any(|(eerder, _)| *eerder == b) {
                continue;
            }
            let Some(p) = parameter(service, &b) else {
                continue;
            };
            let law = b
                .artikel
                .split_once('#')
                .map(|(r, _)| r)
                .unwrap_or(regeling);
            let g = overschrijvingen.geldend(law, p);
            nieuw.push((b, p, g));
        }
        for (b, p, g) in nieuw {
            let mut meld = |soort: &'static str, tekst: String, fout: bool| {
                if gemeld.insert((b.artikel.clone(), b.naam.clone(), soort)) {
                    if fout {
                        c.fouten.push(tekst);
                    } else {
                        c.waarschuwingen.push(tekst);
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
                cellen,
                service,
                &mut meld,
                &mut niet_na_te_gaan,
            );
            c.parameters
                .entry(uitvoering.naam())
                .or_default()
                .push((b, g));
        }
    }
    for (reden, namen) in niet_na_te_gaan {
        let namen: Vec<String> = namen.into_iter().map(|n| format!("'{n}'")).collect();
        c.waarschuwingen.push(format!(
            "herkomst van {} niet na te gaan: {reden}",
            namen.join(", ")
        ));
    }
    tijdvak(d, &mut c);
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
    cellen: &BTreeMap<String, Arc<Cel>>,
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
        let herkomst = g
            .map(Geldend::beschrijving)
            .unwrap_or_else(|| "geen origin".into());
        meld(
            "aanbod-regel",
            format!(
                "aanbod: voorwaarde leunt op '{}' ({herkomst}), dat vooraf niet bekend is",
                b.naam
            ),
            true,
        );
    }
    let Some(g) = g else {
        let streng = d.herkomst == Herkomstcontrole::Streng;
        meld(
            "zonder",
            format!(
                "herkomst: parameter '{}' van {} heeft geen origin; wie hem levert is niet na te gaan{}",
                b.naam,
                b.artikel,
                if streng { " (herkomst: streng)" } else { "" }
            ),
            streng,
        );
        return;
    };
    let waar = format!(
        "parameter '{}' van {} ({})",
        b.naam,
        b.artikel,
        g.beschrijving()
    );
    // Een grondslag in een regeling die niet geladen is, kan kloppen; de
    // runtime kan het alleen niet nagaan.
    if let Err(f) = regelingen::artikel(service, &g.origin.grondslag) {
        meld(
            "grondslag",
            format!("herkomst: {waar}: {f}; niet na te gaan"),
            false,
        );
    }
    if let Some(r) = &g.origin.register {
        if service.resolver().get_law(r).is_none() {
            meld(
                "register",
                format!("herkomst: {waar}: register '{r}' is geen geladen regeling"),
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
                "herkomst: parameter '{}' van {} komt van de belanghebbende, maar heeft geen required: false (RFC-036)",
                b.naam, b.artikel
            ),
            false,
        );
    }
    match leverancier(uitvoering, &b.naam, g, l, cellen) {
        Uitslag::Past { waarschuwingen } => {
            for w in waarschuwingen {
                niet_na_te_gaan.entry(w).or_default().insert(b.naam.clone());
            }
        }
        Uitslag::Verkeerd(reden) => meld(
            "leverancier",
            format!("{}: verkeerde bron voor {waar}: {reden}", uitvoering.naam()),
            true,
        ),
        Uitslag::Geen(reden) => {
            let tekst = format!("{}: geen leverancier voor {waar}{reden}", uitvoering.naam());
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
fn tijdvak(d: &ProcesDefinitie, c: &mut Controle) {
    let Some(aanbod) = d.portaal.as_ref().and_then(|p| p.aanbod.as_ref()) else {
        return;
    };
    let namen: Vec<String> = c
        .parameters
        .get(&Uitvoering::Aanbod.naam())
        .into_iter()
        .flatten()
        .filter(|(_, g)| g.as_ref().is_some_and(Geldend::is_tijdvak))
        .map(|(b, _)| b.naam.clone())
        .collect();
    match (namen.as_slice(), aanbod.tijdvakken.is_some()) {
        ([], false) => {}
        ([], true) => c.fouten.push(format!(
            "aanbod: tijdvakken, maar {} vraagt geen tijdvak (een parameter met origin BELANGHEBBENDE en rol TIJDVAK)",
            aanbod.regeling
        )),
        ([naam], true) => c.tijdvak = Some(naam.clone()),
        ([naam], false) => c.fouten.push(format!(
            "aanbod: het tijdvak '{naam}' (rol TIJDVAK) vraagt aanbod.tijdvakken: de uitkomst van het beleid met de tijdvakken die het portaal aanbiedt"
        )),
        (meer, _) => c.fouten.push(format!(
            "aanbod: meer dan een tijdvak ({}); het portaal biedt er een aan",
            meer.join(", ")
        )),
    }
}

/// Het besluitformulier: de parameters van het besluit met origin
/// `OORDEEL`, in de volgorde van declaratie. Het label is de omschrijving van
/// de parameter, of het deel na "Naam:" als dat er staat, en anders de naam;
/// de groep is het artikel van de grondslag.
pub fn oordelen(c: &Controle, service: &LawExecutionService, handeling: &str) -> Vec<Oordeel> {
    c.parameters
        .get(handeling)
        .into_iter()
        .flatten()
        .filter_map(|(b, g)| {
            let g = g
                .as_ref()
                .filter(|g| g.origin.waarde == OriginValue::Oordeel)?;
            let p = parameter(service, b)?;
            Some(Oordeel {
                parameter: b.naam.clone(),
                label: label(p),
                groep: groep(service, &g.origin.grondslag),
                uitleg: None,
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
pub fn label_uit(omschrijving: &str) -> String {
    let tekst = omschrijving.trim();
    let tekst = match tekst.rsplit_once("Naam:") {
        Some((_, naam)) => naam.trim(),
        None => tekst,
    };
    let tekst = tekst.strip_suffix('.').unwrap_or(tekst).trim();
    tekst.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// De groep van een oordeel: de regeling en het artikel van zijn grondslag.
fn groep(service: &LawExecutionService, grondslag: &str) -> Option<String> {
    let g = regelingen::ontleed(grondslag).ok()?;
    let naam = service
        .resolver()
        .get_law(g.regeling)
        .and_then(|l| l.name.clone())
        .unwrap_or_else(|| crate::formulier::leesbaar(g.regeling));
    Some(format!("{naam}, artikel {}", g.artikel))
}
