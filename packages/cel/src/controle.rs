//! De controles bij het opstarten. De cel weigert te starten als er een
//! faalt, met een melding die het veld of de parameter noemt.
//!
//! 1. Stroom, lexostatus-definities en celdefinitie valideren tegen hun
//!    schema (bij het laden, zie [`crate::stroom::parse`],
//!    [`crate::reductie::parse`] en [`crate::config::CelDefinitie::parse`]).
//! 2. Elke afleiding wijst naar iets dat bestaat: een parameter van een
//!    artikel uit de grondslag van een event dat haar filter aanwijst (of van
//!    een artikel uit `levert_aan`), en veldpaden van dat event. Een
//!    tabelafleiding wijst naar een tabelveld en leest alleen kolommen die de
//!    stroom voor dat veld declareert. Een afleiding op het gekozen gram
//!    vraagt een lexostatus die een gram kiest (`kies`).
//! 3. Geen weesveld: elk veld van een event wordt door een afleiding of een
//!    filter gelezen, of staat met reden in `niet_gereduceerd`.
//! 4. Geen naamsbotsing: een parameter krijgt maar een afleiding.
//! 5. Alleen een event met een zaak (`zaak: opent` of `volgt`) heeft een
//!    zaakkenmerk: een filter op `zaakkenmerk`, een input `zaakkenmerk` of
//!    `groepeer: zaakkenmerk` mag alleen events met een zaak aanwijzen, ook
//!    in `zonder`.
//! 6. Een lijst-lexostatus (`groepeer`) heeft kolommen, geen parameters: haar
//!    afleidingen hoeven geen parameter van een artikel te zijn en botsen niet
//!    met die van andere lexostatussen. Haar `zonder` wijst een event aan in
//!    haar kroniek, anders zou het nooit iets weglaten.
//!
//! Heeft de cel een portaal, dan wijst dat naar een bestaand event, een
//! bestaande lexostatus en een bestaande uitkomst, van een artikel uit de
//! grondslag van dat event. De controles op de synthese staan in
//! [`crate::synthese`].

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::LawExecutionService;

use crate::config::{Aanbod, Portaal};
use crate::eherkenning::INTAKE_PADEN;
use crate::reductie::{self, Filter, LexostatusDefinitie, Lexostatussen};
use crate::regelingen;
use crate::stroom::{Binding, Event, Stroom};

/// Een event met de stroom waar het in staat.
pub type StroomEvent<'a> = (&'a Stroom, &'a Event);

/// Of een event door een filter kan komen: gelijk op elke vaste waarde van
/// een sleutel van het gram zelf. Een veldpad of een waarde `$x` hangt van het
/// gram of de vraag af en telt hier als passend.
fn event_past(filter: &Filter, stroom: &Stroom, event: &Event) -> bool {
    filter.iter().all(|(sleutel, waarde)| {
        if waarde.starts_with('$') {
            return true;
        }
        let eigen = match sleutel.as_str() {
            "name" => Some(event.name.as_str()),
            "type" => Some(event.type_.as_str()),
            "soort" => event.soort.as_deref(),
            "stage" => event.stage.as_deref(),
            "recording_actor" => Some(stroom.recording_actor.as_str()),
            "chronicle" => Some(stroom.chronicle.as_str()),
            _ => return true,
        };
        eigen == Some(waarde.as_str())
    })
}

/// De events die een definitie kan aanwijzen: in de kroniek van de
/// reductie, door het filter van de lexostatus en, voor een afleiding over
/// een verzameling, ook door haar eigen filter.
pub fn events_voor<'a>(
    def: &LexostatusDefinitie,
    afleiding_filter: Option<&Filter>,
    strommen: &'a [Stroom],
) -> Vec<StroomEvent<'a>> {
    let mut uit = Vec::new();
    for stroom in strommen
        .iter()
        .filter(|s| s.chronicle == def.reduction.kroniek)
    {
        for event in &stroom.events {
            if event_past(&def.reduction.filter, stroom, event)
                && afleiding_filter.is_none_or(|f| event_past(f, stroom, event))
            {
                uit.push((stroom, event));
            }
        }
    }
    uit
}

/// De events in een kroniek die door een filter kunnen komen, los van het
/// filter van een lexostatus (voor `zonder`).
pub fn events_in<'a>(
    kroniek: &str,
    filter: &Filter,
    strommen: &'a [Stroom],
) -> Vec<StroomEvent<'a>> {
    strommen
        .iter()
        .filter(|s| s.chronicle == kroniek)
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(s, e)| event_past(filter, s, e))
        .collect()
}

/// Alle controles. `Ok` als de cel mag starten.
pub fn controleer(
    strommen: &[Stroom],
    lexostatussen: &Lexostatussen,
    portaal: Option<&Portaal>,
    service: &LawExecutionService,
) -> Result<(), Vec<String>> {
    let mut fouten = Vec::new();
    uniek(strommen, lexostatussen, &mut fouten);
    grondslagen(strommen, service, &mut fouten);
    verwijzingen(strommen, lexostatussen, service, &mut fouten);
    weesvelden(strommen, lexostatussen, &mut fouten);
    botsingen(strommen, lexostatussen, &mut fouten);
    zaakkenmerken(strommen, lexostatussen, &mut fouten);
    if let Some(p) = portaal {
        portaal_(strommen, lexostatussen, p, service, &mut fouten);
    }
    if fouten.is_empty() {
        Ok(())
    } else {
        Err(fouten)
    }
}

fn uniek(strommen: &[Stroom], lexostatussen: &Lexostatussen, fouten: &mut Vec<String>) {
    let mut gezien = BTreeSet::new();
    for s in strommen {
        if !gezien.insert(&s.id) {
            fouten.push(format!("stroom '{}' staat er meer dan een keer", s.id));
        }
    }
    let mut gezien = BTreeSet::new();
    for d in &lexostatussen.lexostatus_definitions {
        if !gezien.insert(&d.name) {
            fouten.push(format!(
                "lexostatus '{}' staat er meer dan een keer",
                d.name
            ));
        }
    }
}

fn grondslagen(strommen: &[Stroom], service: &LawExecutionService, fouten: &mut Vec<String>) {
    for s in strommen {
        for e in &s.events {
            for g in &e.grondslag {
                if let Err(f) = regelingen::artikel(service, g) {
                    fouten.push(format!("event '{}' (stroom '{}'): {f}", e.name, s.id));
                }
            }
        }
    }
}

/// De parameters van de artikelen achter een lijst grondslagen.
fn parameters_van<'s>(
    grondslag: impl IntoIterator<Item = &'s String>,
    service: &LawExecutionService,
) -> BTreeSet<String> {
    grondslag
        .into_iter()
        .filter_map(|g| regelingen::artikel(service, g).ok())
        .flat_map(|a| a.get_parameters().iter().map(|p| p.name.clone()))
        .collect()
}

fn inputs_in_filter(def: &LexostatusDefinitie, filter: &Filter, fouten: &mut Vec<String>) {
    let inputs: BTreeSet<&str> = def.inputs.iter().map(|i| i.name.as_str()).collect();
    for (sleutel, waarde) in filter {
        if let Some(input) = waarde.strip_prefix('$') {
            if !inputs.contains(input) {
                fouten.push(format!(
                    "lexostatus '{}': filter '{sleutel}' gebruikt '${input}', maar '{input}' is geen input",
                    def.name
                ));
            }
        }
    }
}

fn verwijzingen(
    strommen: &[Stroom],
    lexostatussen: &Lexostatussen,
    service: &LawExecutionService,
    fouten: &mut Vec<String>,
) {
    for def in &lexostatussen.lexostatus_definitions {
        inputs_in_filter(def, &def.reduction.filter, fouten);
        for g in &def.levert_aan {
            if let Err(f) = regelingen::artikel(service, g) {
                fouten.push(format!("lexostatus '{}', levert_aan: {f}", def.name));
            }
        }
        let afnemer = parameters_van(&def.levert_aan, service);
        let events = events_voor(def, None, strommen);
        if events.is_empty() {
            fouten.push(format!(
                "lexostatus '{}': het filter wijst geen event aan in kroniek '{}'",
                def.name, def.reduction.kroniek
            ));
        }
        for (_, event) in &events {
            for pad in reductie::filter_paden(&def.reduction.filter) {
                if !event.heeft_pad(pad) {
                    fouten.push(format!(
                        "lexostatus '{}': filter op veldpad '{pad}', dat niet bestaat in event '{}'",
                        def.name, event.name
                    ));
                }
            }
        }
        if !def.reduction.zonder.is_empty() {
            inputs_in_filter(def, &def.reduction.zonder, fouten);
            let zonder = events_in(&def.reduction.kroniek, &def.reduction.zonder, strommen);
            if zonder.is_empty() {
                fouten.push(format!(
                    "lexostatus '{}': zonder wijst geen event aan in kroniek '{}', en zou dus nooit een zaak weglaten",
                    def.name, def.reduction.kroniek
                ));
            }
            for (_, event) in &zonder {
                for pad in reductie::filter_paden(&def.reduction.zonder) {
                    if !event.heeft_pad(pad) {
                        fouten.push(format!(
                            "lexostatus '{}': zonder filtert op veldpad '{pad}', dat niet bestaat in event '{}'",
                            def.name, event.name
                        ));
                    }
                }
            }
        }
        if def.reduction.afleidingen.is_empty() && def.reduction.extra_velden.is_empty() {
            fouten.push(format!(
                "lexostatus '{}': geen afleiding en geen extra veld, dus zij levert niets",
                def.name
            ));
        }
        for naam in def.reduction.extra_velden.keys() {
            if def.reduction.afleidingen.contains_key(naam) {
                fouten.push(format!(
                    "lexostatus '{}': '{naam}' is zowel een afleiding als een extra veld",
                    def.name
                ));
            }
        }
        for (param, afleiding) in def.alle_afleidingen() {
            // Een kolom van een lijst is geen parameter: ze gaat niet naar de engine.
            let is_parameter = def.reduction.afleidingen.contains_key(param) && !def.is_lijst();
            if afleiding.op_gekozen_gram() && def.reduction.kies.is_none() {
                fouten.push(format!(
                    "lexostatus '{}', afleiding '{param}': leest het gekozen gram, maar de lexostatus kiest er geen (kies)",
                    def.name
                ));
            }
            if let Some(f) = afleiding.filter() {
                inputs_in_filter(def, f, fouten);
            }
            let events = events_voor(def, afleiding.filter(), strommen);
            if events.is_empty() && afleiding.filter().is_some() {
                fouten.push(format!(
                    "lexostatus '{}', afleiding '{param}': het filter wijst geen event aan in kroniek '{}'",
                    def.name, def.reduction.kroniek
                ));
            }
            for (_, event) in events {
                if is_parameter {
                    let params = parameters_van(&event.grondslag, service);
                    if !params.contains(param) && !afnemer.contains(param) {
                        let mut waar = event.grondslag.join(", ");
                        if !def.levert_aan.is_empty() {
                            waar = format!("{waar}; levert_aan: {}", def.levert_aan.join(", "));
                        }
                        fouten.push(format!(
                            "lexostatus '{}', afleiding '{param}': '{param}' is geen parameter van een artikel uit de grondslag van event '{}' ({waar})",
                            def.name, event.name
                        ));
                    }
                }
                if let Some((tabel, gelezen)) = afleiding.tabel_kolommen() {
                    match event.kolommen(tabel) {
                        None => fouten.push(format!(
                            "lexostatus '{}', afleiding '{param}': veldpad '{tabel}' is geen tabelveld van event '{}'",
                            def.name, event.name
                        )),
                        Some(kolommen) => {
                            for kolom in gelezen.into_iter().filter(|k| !kolommen.iter().any(|c| c == k)) {
                                fouten.push(format!(
                                    "lexostatus '{}', afleiding '{param}': kolom '{kolom}' staat niet in de kolommen van tabel '{tabel}' van event '{}' ({})",
                                    def.name,
                                    event.name,
                                    kolommen.join(", ")
                                ));
                            }
                        }
                    }
                    continue;
                }
                for pad in afleiding.gelezen_paden() {
                    if !event.heeft_pad(pad) {
                        fouten.push(format!(
                            "lexostatus '{}', afleiding '{param}': veldpad '{pad}' bestaat niet in event '{}'",
                            def.name, event.name
                        ));
                    }
                }
            }
        }
    }
}

fn gedekt(pad: &str, door: &str) -> bool {
    pad == door || pad.starts_with(&format!("{door}."))
}

fn weesvelden(strommen: &[Stroom], lexostatussen: &Lexostatussen, fouten: &mut Vec<String>) {
    for stroom in strommen {
        for event in &stroom.events {
            let dit_event = |(s, e): &StroomEvent<'_>| s.id == stroom.id && e.name == event.name;
            let mut gelezen: Vec<&str> = Vec::new();
            for def in &lexostatussen.lexostatus_definitions {
                if events_voor(def, None, strommen).iter().any(dit_event) {
                    gelezen.extend(reductie::filter_paden(&def.reduction.filter));
                }
                if events_in(&def.reduction.kroniek, &def.reduction.zonder, strommen)
                    .iter()
                    .any(dit_event)
                    && !def.reduction.zonder.is_empty()
                {
                    gelezen.extend(reductie::filter_paden(&def.reduction.zonder));
                }
                for (_, a) in def.alle_afleidingen() {
                    if events_voor(def, a.filter(), strommen).iter().any(dit_event) {
                        gelezen.extend(a.gelezen_paden());
                    }
                }
            }
            for ng in &event.niet_gereduceerd {
                if !event.heeft_pad(&ng.veld) {
                    fouten.push(format!(
                        "niet_gereduceerd noemt '{}', maar event '{}' (stroom '{}') heeft dat veld niet",
                        ng.veld, event.name, stroom.id
                    ));
                }
            }
            for blad in event.bladeren() {
                let door_afleiding = gelezen.iter().any(|p| gedekt(&blad.pad, p));
                let uitgezonderd = event
                    .niet_gereduceerd
                    .iter()
                    .any(|n| gedekt(&blad.pad, &n.veld));
                if !door_afleiding && !uitgezonderd {
                    fouten.push(format!(
                        "weesveld '{}' in event '{}' (stroom '{}'): geen afleiding leest het en het staat niet in niet_gereduceerd",
                        blad.pad, event.name, stroom.id
                    ));
                }
            }
        }
    }
}

fn botsingen(strommen: &[Stroom], lexostatussen: &Lexostatussen, fouten: &mut Vec<String>) {
    // (stroom, event, parameter) -> lexostatussen die hem afleiden
    let mut per: BTreeMap<(String, String, String), Vec<&str>> = BTreeMap::new();
    for def in lexostatussen
        .lexostatus_definitions
        .iter()
        .filter(|d| !d.is_lijst())
    {
        for (param, a) in &def.reduction.afleidingen {
            for (s, e) in events_voor(def, a.filter(), strommen) {
                let defs = per
                    .entry((s.id.clone(), e.name.clone(), param.clone()))
                    .or_default();
                if !defs.contains(&def.name.as_str()) {
                    defs.push(&def.name);
                }
            }
        }
    }
    for ((_, event, param), defs) in per {
        if defs.len() > 1 {
            fouten.push(format!(
                "parameter '{param}' krijgt meer dan een afleiding voor event '{event}': lexostatus {}",
                defs.join(", ")
            ));
        }
    }
}

/// Een filter op `zaakkenmerk`, of een input `zaakkenmerk`, vraagt dat elk
/// event dat het aanwijst een zaak heeft: zonder zaak heeft een gram geen
/// zaakkenmerk en komt het nooit door zo'n filter.
fn zaakkenmerken(strommen: &[Stroom], lexostatussen: &Lexostatussen, fouten: &mut Vec<String>) {
    let zonder_zaak = |events: Vec<StroomEvent<'_>>| -> Vec<String> {
        events
            .into_iter()
            .filter(|(_, e)| !e.zaak.heeft_kenmerk())
            .map(|(s, e)| format!("'{}' (stroom '{}')", e.name, s.id))
            .collect()
    };
    let melding = |waar: &str, events: Vec<String>| {
        format!(
            "{waar}, maar event {} heeft geen zaak (zaak: geen) en dus geen zaakkenmerk; geef het event zaak: opent of volgt, of gebruik geen zaakkenmerk",
            events.join(", ")
        )
    };
    for def in &lexostatussen.lexostatus_definitions {
        if def.is_lijst() {
            let mut events = zonder_zaak(events_voor(def, None, strommen));
            if !def.reduction.zonder.is_empty() {
                events.extend(zonder_zaak(events_in(
                    &def.reduction.kroniek,
                    &def.reduction.zonder,
                    strommen,
                )));
            }
            if !events.is_empty() {
                fouten.push(melding(
                    &format!("lexostatus '{}': groepeer op 'zaakkenmerk'", def.name),
                    events,
                ));
            }
        }
        let input = def.inputs.iter().any(|i| i.name == "zaakkenmerk");
        let filter = def.reduction.filter.contains_key("zaakkenmerk");
        if input || filter {
            let events = zonder_zaak(events_voor(def, None, strommen));
            if !events.is_empty() {
                let wat = match (input, filter) {
                    (true, true) => "input en filter op 'zaakkenmerk'",
                    (true, false) => "input 'zaakkenmerk'",
                    _ => "filter op 'zaakkenmerk'",
                };
                fouten.push(melding(
                    &format!("lexostatus '{}': {wat}", def.name),
                    events,
                ));
            }
        }
        for (naam, a) in def.alle_afleidingen() {
            let Some(f) = a.filter().filter(|f| f.contains_key("zaakkenmerk")) else {
                continue;
            };
            let events = zonder_zaak(events_voor(def, Some(f), strommen));
            if !events.is_empty() {
                fouten.push(melding(
                    &format!(
                        "lexostatus '{}', afleiding '{naam}': filter op 'zaakkenmerk'",
                        def.name
                    ),
                    events,
                ));
            }
        }
    }
}

fn portaal_(
    strommen: &[Stroom],
    lexostatussen: &Lexostatussen,
    p: &Portaal,
    service: &LawExecutionService,
    fouten: &mut Vec<String>,
) {
    let Some(stroom) = strommen.iter().find(|s| s.id == p.stroom) else {
        fouten.push(format!("portaal: stroom '{}' bestaat niet", p.stroom));
        return;
    };
    let Some(event) = stroom.event(&p.event) else {
        fouten.push(format!(
            "portaal: stroom '{}' heeft geen event '{}'",
            p.stroom, p.event
        ));
        return;
    };
    for blad in event.bladeren() {
        if let Binding::Intake(pad) = &blad.binding {
            if !INTAKE_PADEN.contains(&pad.as_str()) {
                fouten.push(format!(
                    "portaal: veld '{}' bindt aan '$intake.{pad}', maar het portaal levert alleen {}",
                    blad.pad,
                    INTAKE_PADEN.join(", ")
                ));
            }
        }
    }
    match lexostatussen.lexostatus(&p.toets.lexostatus) {
        None => fouten.push(format!(
            "portaal: lexostatus '{}' bestaat niet",
            p.toets.lexostatus
        )),
        Some(def) => {
            if !events_voor(def, None, strommen)
                .iter()
                .any(|(s, e)| s.id == stroom.id && e.name == event.name)
            {
                fouten.push(format!(
                    "portaal: lexostatus '{}' leest event '{}' niet",
                    def.name, event.name
                ));
            }
            if def.reduction.kies.is_none() {
                fouten.push(format!(
                    "portaal: lexostatus '{}' kiest geen gram (kies), en de toets reduceert een concept",
                    def.name
                ));
            }
            if def.is_lijst() {
                fouten.push(format!(
                    "portaal: lexostatus '{}' is een lijst (groepeer), en een lijst gaat nooit naar de engine",
                    def.name
                ));
            }
            for i in def.inputs.iter().filter(|i| i.name != "zaakkenmerk") {
                fouten.push(format!(
                    "portaal: lexostatus '{}' vraagt input '{}'; de toets geeft alleen 'zaakkenmerk' mee",
                    def.name, i.name
                ));
            }
        }
    }
    match service
        .resolver()
        .get_article_by_output(&p.toets.regeling, &p.toets.uitkomst, None)
    {
        None => fouten.push(format!(
            "portaal: regeling '{}' heeft geen uitkomst '{}'",
            p.toets.regeling, p.toets.uitkomst
        )),
        // De toets geeft de lexostatus als parameters aan dit artikel; die
        // zijn afgeleid voor de artikelen uit de grondslag van het event.
        Some(artikel) => {
            let grondslag = format!("{}#{}", p.toets.regeling, artikel.number);
            if !event.grondslag.contains(&grondslag) {
                fouten.push(format!(
                    "portaal: uitkomst '{}' komt uit {grondslag}, en dat staat niet in de grondslag van event '{}' ({})",
                    p.toets.uitkomst,
                    event.name,
                    event.grondslag.join(", ")
                ));
            }
        }
    }
    if let Some(a) = &p.aanbod {
        aanbod_(a, service, fouten);
    }
}

/// Het aanbod: de uitkomst bestaat, en een termijn komt uit hetzelfde
/// artikel. Uitkomst en termijn gaan in een run; een termijn uit een ander
/// artikel, of een die niet bestaat, zou die run laten mislukken en daarmee
/// ook het oordeel over de uitkomst.
fn aanbod_(a: &Aanbod, service: &LawExecutionService, fouten: &mut Vec<String>) {
    let resolver = service.resolver();
    let Some(artikel) = resolver.get_article_by_output(&a.regeling, &a.uitkomst, None) else {
        fouten.push(format!(
            "portaal.aanbod: regeling '{}' heeft geen uitkomst '{}'",
            a.regeling, a.uitkomst
        ));
        return;
    };
    let Some(t) = &a.termijn else {
        return;
    };
    match resolver.get_article_by_output(&a.regeling, t, None) {
        None => fouten.push(format!(
            "portaal.aanbod: regeling '{}' heeft geen termijn-uitkomst '{t}'",
            a.regeling
        )),
        Some(ander) if ander.number != artikel.number => fouten.push(format!(
            "portaal.aanbod: termijn '{t}' komt uit {r}#{}, de uitkomst '{}' uit {r}#{}; ze moeten uit hetzelfde artikel komen",
            ander.number,
            a.uitkomst,
            artikel.number,
            r = a.regeling,
        )),
        Some(_) => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{reductie, stroom};
    use std::path::Path;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");
    const CEL: &str = include_str!("../tests/fixtures/cellen/instantie/lexostatussen.yaml");
    const CELDEF: &str = include_str!("../tests/fixtures/cellen/instantie/cel.yaml");
    const REG_STROOM: &str = include_str!("../tests/fixtures/chronicles/test_registers.yaml");
    const REG_CEL: &str = include_str!("../tests/fixtures/cellen/register/lexostatussen.yaml");

    fn service() -> LawExecutionService {
        regelingen::laad(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation"))
            .unwrap()
            .service
    }

    fn draai(stroom_tekst: &str, cel_tekst: &str) -> Result<(), Vec<String>> {
        draai_met(stroom_tekst, cel_tekst, CELDEF)
    }

    fn draai_met(stroom_tekst: &str, cel_tekst: &str, celdef: &str) -> Result<(), Vec<String>> {
        let s = stroom::parse(stroom_tekst, "stroom")?;
        let c = reductie::parse(cel_tekst, "cel")?;
        let d = crate::config::CelDefinitie::parse(celdef, "cel.yaml")?;
        controleer(&[s], &c, d.portaal.as_ref(), &service())
    }

    fn portaal_faalt_met(celdef: &str, verwacht: &str) {
        let fouten = draai_met(STROOM, CEL, celdef).unwrap_err();
        assert!(
            fouten.iter().any(|f| f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    }

    fn register(cel_tekst: &str) -> Result<(), Vec<String>> {
        let s = stroom::parse(REG_STROOM, "stroom")?;
        let c = reductie::parse(cel_tekst, "cel")?;
        controleer(&[s], &c, None, &service())
    }

    fn register_faalt_met(cel_tekst: &str, verwacht: &str) {
        let fouten = register(cel_tekst).unwrap_err();
        assert!(
            fouten.iter().any(|f| f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    }

    fn faalt_met(stroom_tekst: &str, cel_tekst: &str, verwacht: &str) {
        let fouten = draai(stroom_tekst, cel_tekst).unwrap_err();
        assert!(
            fouten.iter().any(|f| f.contains(verwacht)),
            "verwacht '{verwacht}' in {fouten:?}"
        );
    }

    #[test]
    fn de_fixtures_slagen() {
        draai(STROOM, CEL).unwrap();
    }

    // 1. Schema.
    #[test]
    fn schema_stroom_faalt() {
        faalt_met(
            &STROOM.replace("intake: portaal", "intake: Portaal"),
            CEL,
            "/events/0/intake",
        );
    }

    #[test]
    fn schema_celconfig_faalt() {
        faalt_met(
            STROOM,
            &CEL.replace("kies: laatste", "kies: eerste"),
            "kies",
        );
    }

    // 2. Afleiding wijst naar iets dat bestaat.
    #[test]
    fn afleiding_naar_onbekende_parameter() {
        let cel = CEL.replace("bevat_naam: {gevuld", "bevat_naampje: {gevuld");
        faalt_met(
            STROOM,
            &cel,
            "afleiding 'bevat_naampje': 'bevat_naampje' is geen parameter",
        );
    }

    #[test]
    fn afleiding_naar_onbekend_veldpad() {
        let cel = CEL.replace("{gevuld: inhoud.naam}", "{gevuld: inhoud.naam_x}");
        faalt_met(STROOM, &cel, "veldpad 'inhoud.naam_x' bestaat niet");
    }

    #[test]
    fn tabel_moet_een_veld_zijn() {
        let cel = CEL.replace(
            "{tabel: inhoud.organen, een_regel",
            "{tabel: inhoud, een_regel",
        );
        faalt_met(STROOM, &cel, "veldpad 'inhoud' is geen tabelveld");
        // Een gewoon veld is ook geen tabel.
        let cel = CEL.replace(
            "{tabel: inhoud.organen, een_regel",
            "{tabel: inhoud.naam, een_regel",
        );
        faalt_met(STROOM, &cel, "veldpad 'inhoud.naam' is geen tabelveld");
    }

    #[test]
    fn tabelafleiding_leest_een_gedeclareerde_kolom() {
        let cel = CEL.replace("elke_regel: zetels}", "elke_regel: stoelen}");
        faalt_met(
            STROOM,
            &cel,
            "kolom 'stoelen' staat niet in de kolommen van tabel 'inhoud.organen'",
        );
        let cel = CEL.replace("alleen_waar: samengevoegd}", "alleen_waar: gebundeld}");
        faalt_met(STROOM, &cel, "kolom 'gebundeld' staat niet in de kolommen");
    }

    #[test]
    fn filter_zonder_event() {
        let cel = CEL.replace(
            "soort: aanvraag, zaakkenmerk",
            "soort: melding, zaakkenmerk",
        );
        faalt_met(STROOM, &cel, "het filter wijst geen event aan");
    }

    #[test]
    fn filter_met_onbekende_input() {
        let cel = CEL.replace("zaakkenmerk: $zaakkenmerk}", "zaakkenmerk: $zaak}");
        faalt_met(STROOM, &cel, "'zaak' is geen input");
    }

    #[test]
    fn grondslag_die_niet_bestaat() {
        let stroom = STROOM.replace(
            "- testregeling_aanvraag#1",
            "- testregeling_aanvraag#1\n      - testregeling_aanvraag#7",
        );
        faalt_met(&stroom, CEL, "heeft geen artikel 7");
    }

    // 3. Geen weesveld.
    #[test]
    fn weesveld() {
        let stroom = STROOM.replace(
            "      - {veld: inhoud.rekeningnummer, reden: voor de betaling}\n",
            "",
        );
        faalt_met(&stroom, CEL, "weesveld 'inhoud.rekeningnummer'");
    }

    #[test]
    fn niet_gereduceerd_dekt_een_tak() {
        // `kern` staat als geheel in niet_gereduceerd: geen weesveld eronder.
        let s = stroom::parse(STROOM, "s").unwrap();
        assert!(s.events[0]
            .niet_gereduceerd
            .iter()
            .any(|n| n.veld == "kern"));
        draai(STROOM, CEL).unwrap();
    }

    #[test]
    fn niet_gereduceerd_naar_onbekend_veld() {
        let stroom = STROOM.replace("{veld: inhoud.rekeningnummer,", "{veld: inhoud.rekening,");
        faalt_met(&stroom, CEL, "niet_gereduceerd noemt 'inhoud.rekening'");
    }

    // 4. Geen naamsbotsing.
    #[test]
    fn dubbele_afleiding_over_twee_lexostatussen() {
        let extra = "  - name: tweede\n    inputs: [{name: zaakkenmerk, type: string}]\n    reduction:\n      kroniek: test_kroniek\n      filter: {name: aanvraag_ontvangen, zaakkenmerk: $zaakkenmerk}\n      kies: laatste\n      afleidingen:\n        bevat_naam: {gevuld: kern.aanvrager.naam}\n";
        let cel = format!("{CEL}{extra}");
        faalt_met(
            STROOM,
            &cel,
            "parameter 'bevat_naam' krijgt meer dan een afleiding",
        );
    }

    #[test]
    fn dubbele_afleiding_in_een_lexostatus() {
        let cel = CEL.replace(
            "        bevat_aanduiding: {gevuld: inhoud.aanduiding}\n",
            "        bevat_aanduiding: {gevuld: inhoud.aanduiding}\n        bevat_aanduiding: {gevuld: inhoud.naam}\n",
        );
        // De YAML-lezer weigert een dubbele sleutel al, en noemt hem.
        faalt_met(
            STROOM,
            &cel,
            "duplicate entry with key \"bevat_aanduiding\"",
        );
    }

    // 5. Zaakkenmerk alleen bij een event met een zaak.
    #[test]
    fn filter_op_zaakkenmerk_vraagt_een_event_met_een_zaak() {
        // De fixture: aanvraag_ontvangen opent een zaak.
        assert!(STROOM.contains("zaak: opent"));
        let volgt = STROOM.replace("zaak: opent", "zaak: volgt");
        draai(&volgt, CEL).unwrap();
        for stroom in [
            STROOM.replace("    zaak: opent\n", ""),
            STROOM.replace("zaak: opent", "zaak: geen"),
        ] {
            faalt_met(
                &stroom,
                CEL,
                "lexostatus 'aanvraag_inhoud': input en filter op 'zaakkenmerk', maar event 'aanvraag_ontvangen' (stroom 'test_aanvragen') heeft geen zaak",
            );
        }
        // Alleen de input, zonder filter.
        let stroom = STROOM.replace("zaak: opent", "zaak: geen");
        let cel = CEL.replace(", zaakkenmerk: $zaakkenmerk}", "}");
        faalt_met(
            &stroom,
            &cel,
            "lexostatus 'aanvraag_inhoud': input 'zaakkenmerk', maar",
        );
    }

    #[test]
    fn filter_per_afleiding_op_zaakkenmerk_zonder_zaak() {
        let cel = REG_CEL.replace(
            "inputs: [{name: aanduiding, type: string}]",
            "inputs: [{name: aanduiding, type: string}, {name: zaakkenmerk, type: string}]",
        );
        register_faalt_met(
            &cel,
            "lexostatus 'registerstatus': input 'zaakkenmerk', maar event",
        );
        let cel = REG_CEL.replace(
            "{filter: {name: aanduiding_geschrapt, orgaan: raad,",
            "{filter: {name: aanduiding_geschrapt, zaakkenmerk: 00000000-0000-4000-8000-000000000001, orgaan: raad,",
        );
        register_faalt_met(
            &cel,
            "afleiding 'is_geschrapt_raad': filter op 'zaakkenmerk', maar event 'aanduiding_geschrapt' (stroom 'test_registers') heeft geen zaak",
        );
    }

    // Portaal.
    #[test]
    fn portaal_met_onbekende_uitkomst() {
        let celdef = CELDEF.replace("uitkomst: aanvraag_volledig", "uitkomst: bestaat_niet");
        portaal_faalt_met(&celdef, "geen uitkomst 'bestaat_niet'");
    }

    #[test]
    fn portaal_met_uitkomst_buiten_de_grondslag() {
        let celdef = CELDEF.replace(
            "regeling: testregeling_aanvraag\n    uitkomst: aanvraag_volledig",
            "regeling: testregeling_awb\n    uitkomst: in_verzuim",
        );
        portaal_faalt_met(&celdef, "staat niet in de grondslag van event");
    }

    /// De cel-definitie met een aanbod uit `testregeling_aanvraag`.
    fn met_aanbod(uitkomst: &str, termijn: Option<&str>) -> String {
        let termijn = termijn
            .map(|t| format!("    termijn: {t}\n"))
            .unwrap_or_default();
        CELDEF.replace(
            "  formulier:",
            &format!(
                "  aanbod:\n    regeling: testregeling_aanvraag\n    uitkomst: {uitkomst}\n{termijn}  formulier:"
            ),
        )
    }

    #[test]
    fn portaal_met_aanbod_en_termijn_uit_hetzelfde_artikel() {
        draai_met(
            STROOM,
            CEL,
            &met_aanbod("aanvraag_volledig", Some("aanvraag_tijdig")),
        )
        .unwrap();
        draai_met(STROOM, CEL, &met_aanbod("aanvraag_volledig", None)).unwrap();
    }

    #[test]
    fn aanbod_met_onbekende_uitkomst() {
        portaal_faalt_met(
            &met_aanbod("bestaat_niet", None),
            "portaal.aanbod: regeling 'testregeling_aanvraag' heeft geen uitkomst 'bestaat_niet'",
        );
    }

    #[test]
    fn aanbod_met_onbekende_termijn() {
        portaal_faalt_met(
            &met_aanbod("aanvraag_volledig", Some("bestaat_niet")),
            "portaal.aanbod: regeling 'testregeling_aanvraag' heeft geen termijn-uitkomst 'bestaat_niet'",
        );
    }

    /// De termijn gaat in dezelfde run mee: een ander artikel zou de run van
    /// de uitkomst laten afhangen van wat dat artikel nodig heeft.
    #[test]
    fn aanbod_met_termijn_uit_een_ander_artikel() {
        portaal_faalt_met(
            &met_aanbod("aanvraag_volledig", Some("aanvraag_compleet")),
            "portaal.aanbod: termijn 'aanvraag_compleet' komt uit testregeling_aanvraag#2, de uitkomst 'aanvraag_volledig' uit testregeling_aanvraag#1; ze moeten uit hetzelfde artikel komen",
        );
    }

    #[test]
    fn portaal_met_onbekend_intakepad() {
        let stroom = STROOM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        faalt_met(&stroom, CEL, "'$intake.eherkenning.bsn'");
    }

    #[test]
    fn zonder_portaal_geen_portaalcontrole() {
        let celdef = CELDEF.split("\nportaal:").next().unwrap().to_string();
        draai_met(STROOM, CEL, &celdef).unwrap();
        // Ook een intake-pad dat geen portaal levert, is dan geen fout.
        let stroom = STROOM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        draai_met(&stroom, CEL, &celdef).unwrap();
    }

    #[test]
    fn portaal_met_lexostatus_zonder_kies() {
        let cel = CEL.replace("      kies: laatste\n", "");
        let fouten = draai(STROOM, &cel).unwrap_err();
        assert!(
            fouten.iter().any(|f| f.contains("kiest geen gram (kies)")),
            "{fouten:?}"
        );
        // En elke afleiding op het gekozen gram meldt het ook.
        assert!(
            fouten
                .iter()
                .any(|f| f.contains("afleiding 'bevat_naam': leest het gekozen gram")),
            "{fouten:?}"
        );
    }

    // Afleidingen over een verzameling, met een filter per afleiding.
    #[test]
    fn de_registerfixture_slaagt() {
        register(REG_CEL).unwrap();
    }

    #[test]
    fn levert_aan_maakt_een_parameter_van_de_afnemer_geldig() {
        let cel = REG_CEL.replace(
            "    levert_aan: [testregeling_afnemer#1, testregeling_afnemer#2, testregeling_afnemer#3]\n",
            "",
        );
        register_faalt_met(
            &cel,
            "'is_ingeschreven_raad' is geen parameter van een artikel uit de grondslag van event 'aanduiding_ingeschreven' (testregeling_register#1)",
        );
        // Zonder levert_aan blijft een parameter uit de grondslag geldig.
        let fouten = register(&cel).unwrap_err();
        assert!(
            !fouten
                .iter()
                .any(|f| f.contains("'datum_mededeling' is geen")),
            "{fouten:?}"
        );
        // levert_aan naar een artikel dat niet bestaat.
        let cel = REG_CEL.replace("testregeling_afnemer#3]", "testregeling_afnemer#9]");
        register_faalt_met(&cel, "levert_aan: grondslag 'testregeling_afnemer#9'");
    }

    #[test]
    fn filter_per_afleiding_wijst_een_event_aan() {
        let cel = REG_CEL.replace(
            "{name: aanduiding_geschrapt,",
            "{name: aanduiding_verloren,",
        );
        register_faalt_met(
            &cel,
            "afleiding 'is_geschrapt_raad': het filter wijst geen event aan",
        );
    }

    #[test]
    fn filter_per_afleiding_op_een_veldpad_dat_bestaat() {
        let cel = REG_CEL.replace(
            "{name: aanduiding_geschrapt, orgaan: raad,",
            "{name: aanduiding_geschrapt, gebied: raad,",
        );
        register_faalt_met(
            &cel,
            "veldpad 'gebied' bestaat niet in event 'aanduiding_geschrapt'",
        );
        let cel = REG_CEL.replace(
            "aanduiding: $aanduiding}, bestaat",
            "aanduiding: $naam}, bestaat",
        );
        register_faalt_met(&cel, "'naam' is geen input");
    }

    #[test]
    fn filter_en_som_lezen_velden_dus_geen_weesveld() {
        // `lijst` leest alleen het filter, `zetels` alleen de som.
        let cel = REG_CEL.replace(
            "        zetels_op_lijst: {filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, som: zetels}\n",
            "",
        );
        register_faalt_met(&cel, "weesveld 'lijst' in event 'uitslag_vastgesteld'");
        register_faalt_met(&cel, "weesveld 'zetels' in event 'uitslag_vastgesteld'");
    }

    #[test]
    fn afleiding_op_het_gram_in_een_lexostatus_zonder_kies() {
        let cel = format!("{REG_CEL}        x: {{veld: aanduiding}}\n");
        register_faalt_met(
            &cel,
            "afleiding 'x': leest het gekozen gram, maar de lexostatus kiest er geen",
        );
    }

    #[test]
    fn extra_veld_met_de_naam_van_een_afleiding() {
        let cel = format!("{CEL}      extra_velden:\n        bevat_naam: {{veld: inhoud.naam}}\n");
        faalt_met(
            STROOM,
            &cel,
            "'bevat_naam' is zowel een afleiding als een extra veld",
        );
        let cel = format!("{CEL}      extra_velden:\n        naam: {{veld: inhoud.naamx}}\n");
        faalt_met(STROOM, &cel, "veldpad 'inhoud.naamx' bestaat niet");
        // Een extra veld hoeft geen parameter te zijn.
        let cel = format!("{CEL}      extra_velden:\n        naam: {{veld: inhoud.naam}}\n");
        draai(STROOM, &cel).unwrap();
    }

    // 6. Een lijst-lexostatus.
    const LIJST: &str = "  - name: werkvoorraad\n    inputs: []\n    reduction:\n      kroniek: test_kroniek\n      filter: {type: indiening}\n      groepeer: zaakkenmerk\n      kies: laatste\n      afleidingen:\n        naam: {veld: inhoud.naam}\n        aanvraagjaar: {veld: inhoud.aanvraagjaar}\n";

    #[test]
    fn lijst_heeft_kolommen_geen_parameters() {
        // `naam` is geen parameter, en `aanvraagjaar` botst niet met de
        // afleiding in aanvraag_inhoud: een lijst gaat nooit naar de engine.
        draai(STROOM, &format!("{CEL}{LIJST}")).unwrap();
    }

    #[test]
    fn groepeer_vraagt_events_met_een_zaak() {
        let stroom = STROOM.replace("zaak: opent", "zaak: geen");
        let cel = CEL
            .replace("inputs: [{name: zaakkenmerk, type: string}]", "inputs: []")
            .replace(", zaakkenmerk: $zaakkenmerk", "");
        faalt_met(
            &stroom,
            &format!("{cel}{LIJST}"),
            "lexostatus 'werkvoorraad': groepeer op 'zaakkenmerk', maar event 'aanvraag_ontvangen'",
        );
    }

    #[test]
    fn zonder_wijst_een_event_aan() {
        let lijst = LIJST.replace(
            "      kies: laatste\n",
            "      zonder: {stage: BESLUIT}\n      kies: laatste\n",
        );
        faalt_met(
            STROOM,
            &format!("{CEL}{lijst}"),
            "lexostatus 'werkvoorraad': zonder wijst geen event aan in kroniek 'test_kroniek'",
        );
        // Wijst het een event aan, dan leest het ook zijn veldpaden.
        let lijst = LIJST.replace("      kies: laatste\n", "      zonder: {name: aanvraag_ontvangen, inhoud.bestaat_niet: x}\n      kies: laatste\n");
        faalt_met(
            STROOM,
            &format!("{CEL}{lijst}"),
            "zonder filtert op veldpad 'inhoud.bestaat_niet'",
        );
    }

    #[test]
    fn portaal_toetst_geen_lijst() {
        let cel = format!("{CEL}{LIJST}");
        let celdef = CELDEF.replace("lexostatus: aanvraag_inhoud", "lexostatus: werkvoorraad");
        let fouten = draai_met(STROOM, &cel, &celdef).unwrap_err();
        assert!(
            fouten
                .iter()
                .any(|f| f.contains("'werkvoorraad' is een lijst")),
            "{fouten:?}"
        );
    }
}
