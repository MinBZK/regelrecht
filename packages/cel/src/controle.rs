//! De controles op een cel bij het opstarten. De runtime weigert te starten
//! als er een faalt, met een melding die het veld of de parameter noemt.
//!
//! 1. Stroom, lexostatus-definities en celdefinitie valideren tegen hun
//!    schema (bij het laden, zie [`crate::stroom::parse`],
//!    [`crate::reductie::parse`] en [`crate::config::CelDefinitie::parse`]).
//! 2. Elke afleiding wijst naar iets dat bestaat: een parameter van een
//!    artikel uit de grondslag van een event dat haar filter aanwijst (of uit
//!    de eigen `grondslag` van de afleiding), en veldpaden van dat event. Elk
//!    artikel uit de grondslag van een afleiding is geladen en heeft het lid
//!    dat ze noemt, net als bij een event. Een
//!    tabelafleiding wijst naar een tabelveld en leest alleen kolommen die de
//!    stroom voor dat veld declareert. Een afleiding op het gekozen gram
//!    vraagt een lexostatus die een gram kiest (`kies`).
//! 3. Geen weesveld: elk veld van een event wordt door een afleiding of een
//!    filter gelezen, of staat met reden in `niet_gereduceerd`.
//! 4. Geen naamsbotsing: een parameter krijgt maar een afleiding.
//! 5. (Vervallen met chronolex v0.2.0: elk gram heeft een wortel, dus een
//!    filter op `wortel` of `groepeer: wortel` kan elk event aanwijzen.)
//! 6. Een lijst-lexostatus (`groepeer`) heeft kolommen, geen parameters: haar
//!    afleidingen hoeven geen parameter van een artikel te zijn en botsen niet
//!    met die van andere lexostatussen. Haar `zonder` wijst een event aan in
//!    haar kroniek, anders zou het nooit iets weglaten.
//! 7. Geen lexostatus heet [`crate::reductie::ZAAKSTAND`]: die biedt de
//!    runtime aan.
//!
//! Een proces met een portaal wijst naar een bestaand event van zijn cel, een
//! bestaande lexostatus en een bestaande uitkomst, van een artikel uit de
//! grondslag van dat event ([`portaal`], aangeroepen vanuit
//! [`crate::proces`]). De controles op de synthese staan in
//! [`crate::synthese`], die op de handelingen in [`crate::handeling`].

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::LawExecutionService;

use crate::config::{Aanbod, Portal};
use crate::reductie::{self, Filter, LexostatusDefinitie, Lexostatussen, Periode};
use crate::regelingen;
use crate::stroom::{Binding, Event, Eventkenmerk, Stroom};

/// Een event met de stroom waar het in staat.
pub type StroomEvent<'a> = (&'a Stroom, &'a Event);

/// Of een event door een filter kan komen: gelijk op elke vaste waarde van
/// een sleutel van het gram zelf. Een veldpad of een waarde `$x` hangt van het
/// gram of de vraag af en telt hier als passend.
fn event_past(filter: &Filter, stream: &Stroom, event: &Event) -> bool {
    filter.iter().all(|(sleutel, value)| {
        if value.starts_with('$') {
            return true;
        }
        // Alleen wat vast in de stroom staat, wijst hier events aan. Een
        // kenmerk dat een event nooit heeft (een zaakkenmerk zonder zaak)
        // meldt een eigen controle, en die moet het event dus zien.
        match event.kenmerk(stream, sleutel) {
            Some(Eventkenmerk::Vast(w)) => w == Some(value.as_str()),
            Some(Eventkenmerk::Vrij | Eventkenmerk::Nooit) | None => true,
        }
    })
}

/// De events die een definitie kan aanwijzen: in de kroniek van de
/// reductie, door het filter van de lexostatus en, voor een afleiding over
/// een verzameling, ook door haar eigen filter.
pub fn events_voor<'a>(
    def: &LexostatusDefinitie,
    afleiding_filter: Option<&Filter>,
    streams: &'a [Stroom],
) -> Vec<StroomEvent<'a>> {
    let mut uit = Vec::new();
    for stream in streams
        .iter()
        .filter(|s| s.chronicle == def.reduction.chronicle)
    {
        for event in &stream.events {
            if event_past(&def.reduction.filter, stream, event)
                && afleiding_filter.is_none_or(|f| event_past(f, stream, event))
            {
                uit.push((stream, event));
            }
        }
    }
    uit
}

/// De events in een kroniek die door een filter kunnen komen, los van het
/// filter van een lexostatus (voor `zonder`).
pub fn events_in<'a>(
    chronicle: &str,
    filter: &Filter,
    streams: &'a [Stroom],
) -> Vec<StroomEvent<'a>> {
    streams
        .iter()
        .filter(|s| s.chronicle == chronicle)
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(s, e)| event_past(filter, s, e))
        .collect()
}

/// Zet de periode van elke periode-afleiding zonder `periode` uit de
/// regeling: de `temporal.period_type` (RFC-001) van de parameter met de naam
/// van de afleiding, in een artikel uit de grondslag van de afleiding of van
/// een event dat zij leest. Welke periode een tijdvak is, zegt zo de wet (een
/// maand, een jaar), niet de celconfiguratie. Geen of twee verschillende
/// perioden is een fout: noem de periode dan in de afleiding.
pub fn perioden(
    streams: &[Stroom],
    lexostatuses: &mut Lexostatussen,
    service: &LawExecutionService,
) -> Vec<String> {
    let mut fouten = Vec::new();
    for def in &mut lexostatuses.lexostatus_definitions {
        let kopie = def.clone();
        let derivations = def
            .reduction
            .derivations
            .iter_mut()
            .chain(def.reduction.extra_fields.iter_mut());
        for (name, a) in derivations {
            if a.derivation.periode_mut().is_none_or(|p| p.is_some()) {
                continue;
            }
            let events = events_voor(&kopie, a.derivation.filter(), streams);
            let legal_basis: Vec<String> = a
                .legal_basis
                .iter()
                .chain(events.iter().flat_map(|(_, e)| e.legal_basis.iter()))
                .cloned()
                .collect();
            let Some(period) = a.derivation.periode_mut().filter(|p| p.is_none()) else {
                continue;
            };
            let gevonden: BTreeSet<String> = legal_basis
                .iter()
                .filter_map(|g| regelingen::article(service, g).ok())
                .flat_map(|art| art.get_parameters().iter())
                .filter(|p| &p.name == name)
                .filter_map(|p| p.temporal.as_ref()?.period_type.clone())
                .collect();
            let perioden: Vec<Periode> = gevonden
                .iter()
                .filter_map(|t| Periode::uit_period_type(t))
                .collect();
            match perioden.as_slice() {
                [p] if gevonden.len() == 1 => *period = Some(*p),
                _ => fouten.push(format!(
                    "lexostatus '{}', afleiding '{name}': periode_van zonder periode, en {}; noem de periode (jaar, kwartaal, maand)",
                    kopie.name,
                    if gevonden.is_empty() {
                        format!("geen parameter '{name}' in de grondslag ({}) noemt een temporal.period_type", legal_basis.join(", "))
                    } else {
                        format!("de grondslag noemt voor '{name}' de period_type {}", gevonden.into_iter().collect::<Vec<_>>().join(", "))
                    }
                )),
            }
        }
    }
    fouten
}

/// Alle controles op een cel. `Ok` als de cel mag starten.
pub fn controleer(
    streams: &[Stroom],
    lexostatuses: &Lexostatussen,
    service: &LawExecutionService,
) -> Result<(), Vec<String>> {
    let mut fouten = Vec::new();
    uniek(streams, lexostatuses, &mut fouten);
    grondslagen(streams, service, &mut fouten);
    verwijzingen(streams, lexostatuses, service, &mut fouten);
    weesvelden(streams, lexostatuses, &mut fouten);
    botsingen(streams, lexostatuses, &mut fouten);
    if fouten.is_empty() {
        Ok(())
    } else {
        Err(fouten)
    }
}

fn uniek(streams: &[Stroom], lexostatuses: &Lexostatussen, fouten: &mut Vec<String>) {
    let mut gezien = BTreeSet::new();
    for s in streams {
        if !gezien.insert(&s.id) {
            fouten.push(format!("stroom '{}' staat er meer dan een keer", s.id));
        }
    }
    let mut gezien = BTreeSet::new();
    for d in &lexostatuses.lexostatus_definitions {
        if d.name == reductie::ZAAKSTAND {
            fouten.push(format!(
                "lexostatus '{}': die naam is van de runtime, die haar voor elke cel met een zaak aanbiedt",
                d.name
            ));
        }
        if !gezien.insert(&d.name) {
            fouten.push(format!(
                "lexostatus '{}' staat er meer dan een keer",
                d.name
            ));
        }
    }
}

/// Elke grondslag wijst een geladen artikel aan, en een lid dat de
/// artikeltekst heeft (een regel die met `<n>.` of `<n> ` begint). Ook de
/// grondslag van een gebonden `op_moment`.
fn grondslagen(streams: &[Stroom], service: &LawExecutionService, fouten: &mut Vec<String>) {
    for s in streams {
        for e in &s.events {
            let van_moment = e.effective_at.iter().flat_map(|b| &b.legal_basis);
            for g in e.legal_basis.iter().chain(van_moment) {
                if let Err(f) = regelingen::geldig(service, g) {
                    fouten.push(format!("event '{}' (stroom '{}'): {f}", e.name, s.id));
                }
            }
        }
    }
}

/// De parameters van de artikelen achter een lijst grondslagen.
fn parameters_van<'s>(
    legal_basis: impl IntoIterator<Item = &'s String>,
    service: &LawExecutionService,
) -> BTreeSet<String> {
    legal_basis
        .into_iter()
        .filter_map(|g| regelingen::article(service, g).ok())
        .flat_map(|a| a.get_parameters().iter().map(|p| p.name.clone()))
        .collect()
}

fn inputs_in_filter(def: &LexostatusDefinitie, filter: &Filter, fouten: &mut Vec<String>) {
    let inputs: BTreeSet<&str> = def.inputs.iter().map(|i| i.name.as_str()).collect();
    for (sleutel, value) in filter {
        if let Some(input) = value.strip_prefix('$') {
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
    streams: &[Stroom],
    lexostatuses: &Lexostatussen,
    service: &LawExecutionService,
    fouten: &mut Vec<String>,
) {
    for def in &lexostatuses.lexostatus_definitions {
        inputs_in_filter(def, &def.reduction.filter, fouten);
        let events = events_voor(def, None, streams);
        if events.is_empty() {
            fouten.push(format!(
                "lexostatus '{}': het filter wijst geen event aan in kroniek '{}'",
                def.name, def.reduction.chronicle
            ));
        }
        for (_, event) in &events {
            for path in reductie::filter_paden(&def.reduction.filter) {
                if !event.heeft_pad(path) {
                    fouten.push(format!(
                        "lexostatus '{}': filter op veldpad '{path}', dat niet bestaat in event '{}'",
                        def.name, event.name
                    ));
                }
            }
        }
        if !def.reduction.without.is_empty() {
            inputs_in_filter(def, &def.reduction.without, fouten);
            let without = events_in(&def.reduction.chronicle, &def.reduction.without, streams);
            if without.is_empty() {
                fouten.push(format!(
                    "lexostatus '{}': zonder wijst geen event aan in kroniek '{}', en zou dus nooit een zaak weglaten",
                    def.name, def.reduction.chronicle
                ));
            }
            for (_, event) in &without {
                for path in reductie::filter_paden(&def.reduction.without) {
                    if !event.heeft_pad(path) {
                        fouten.push(format!(
                            "lexostatus '{}': zonder filtert op veldpad '{path}', dat niet bestaat in event '{}'",
                            def.name, event.name
                        ));
                    }
                }
            }
        }
        if def.reduction.derivations.is_empty() && def.reduction.extra_fields.is_empty() {
            fouten.push(format!(
                "lexostatus '{}': geen afleiding en geen extra veld, dus zij levert niets",
                def.name
            ));
        }
        for name in def.reduction.extra_fields.keys() {
            if def.reduction.derivations.contains_key(name) {
                fouten.push(format!(
                    "lexostatus '{}': '{name}' is zowel een afleiding als een extra veld",
                    def.name
                ));
            }
        }
        for (param, derivation) in def.alle_afleidingen() {
            // Een kolom van een lijst is geen parameter: ze gaat niet naar de engine.
            let is_parameter = def.reduction.derivations.contains_key(param) && !def.is_lijst();
            // De eigen grondslag van de afleiding: geladen, met het lid.
            for g in &derivation.legal_basis {
                if let Err(f) = regelingen::geldig(service, g) {
                    fouten.push(format!(
                        "lexostatus '{}', afleiding '{param}': {f}",
                        def.name
                    ));
                }
            }
            let eigen_grondslag = parameters_van(&derivation.legal_basis, service);
            if derivation.op_gekozen_gram() && def.reduction.pick.is_none() {
                fouten.push(format!(
                    "lexostatus '{}', afleiding '{param}': leest het gekozen gram, maar de lexostatus kiest er geen (kies)",
                    def.name
                ));
            }
            if let Some(f) = derivation.filter() {
                inputs_in_filter(def, f, fouten);
            }
            let events = events_voor(def, derivation.filter(), streams);
            if events.is_empty() && derivation.filter().is_some() {
                fouten.push(format!(
                    "lexostatus '{}', afleiding '{param}': het filter wijst geen event aan in kroniek '{}'",
                    def.name, def.reduction.chronicle
                ));
            }
            for (_, event) in events {
                if is_parameter {
                    let params = parameters_van(&event.legal_basis, service);
                    if !params.contains(param) && !eigen_grondslag.contains(param) {
                        let mut waar = event.legal_basis.join(", ");
                        if !derivation.legal_basis.is_empty() {
                            waar = format!(
                                "{waar}; grondslag van de afleiding: {}",
                                derivation.legal_basis.join(", ")
                            );
                        }
                        fouten.push(format!(
                            "lexostatus '{}', afleiding '{param}': '{param}' is geen parameter van een artikel uit de grondslag van event '{}' ({waar})",
                            def.name, event.name
                        ));
                    }
                }
                if let Some((table, gelezen)) = derivation.tabel_kolommen() {
                    match event.columns(table) {
                        None => fouten.push(format!(
                            "lexostatus '{}', afleiding '{param}': veldpad '{table}' is geen tabelveld van event '{}'",
                            def.name, event.name
                        )),
                        Some(columns) => {
                            for column in gelezen.into_iter().filter(|k| !columns.iter().any(|c| c == k)) {
                                fouten.push(format!(
                                    "lexostatus '{}', afleiding '{param}': kolom '{column}' staat niet in de kolommen van tabel '{table}' van event '{}' ({})",
                                    def.name,
                                    event.name,
                                    columns.join(", ")
                                ));
                            }
                        }
                    }
                    continue;
                }
                for path in derivation.gelezen_paden() {
                    if !event.heeft_pad(path) {
                        fouten.push(format!(
                            "lexostatus '{}', afleiding '{param}': veldpad '{path}' bestaat niet in event '{}'",
                            def.name, event.name
                        ));
                    }
                }
            }
        }
    }
}

fn gedekt(path: &str, door: &str) -> bool {
    path == door || path.starts_with(&format!("{door}."))
}

fn weesvelden(streams: &[Stroom], lexostatuses: &Lexostatussen, fouten: &mut Vec<String>) {
    for stream in streams {
        for event in &stream.events {
            let dit_event = |(s, e): &StroomEvent<'_>| s.id == stream.id && e.name == event.name;
            let mut gelezen: Vec<&str> = Vec::new();
            for def in &lexostatuses.lexostatus_definitions {
                if events_voor(def, None, streams).iter().any(dit_event) {
                    gelezen.extend(reductie::filter_paden(&def.reduction.filter));
                }
                if events_in(&def.reduction.chronicle, &def.reduction.without, streams)
                    .iter()
                    .any(dit_event)
                    && !def.reduction.without.is_empty()
                {
                    gelezen.extend(reductie::filter_paden(&def.reduction.without));
                }
                for (_, a) in def.alle_afleidingen() {
                    if events_voor(def, a.filter(), streams).iter().any(dit_event) {
                        gelezen.extend(a.gelezen_paden());
                    }
                }
            }
            for ng in &event.not_reduced {
                if !event.heeft_pad(&ng.field) {
                    fouten.push(format!(
                        "niet_gereduceerd noemt '{}', maar event '{}' (stroom '{}') heeft dat veld niet",
                        ng.field, event.name, stream.id
                    ));
                }
            }
            for blad in event.bladeren() {
                let door_afleiding = gelezen.iter().any(|p| gedekt(&blad.path, p));
                let uitgezonderd = event
                    .not_reduced
                    .iter()
                    .any(|n| gedekt(&blad.path, &n.field));
                if !door_afleiding && !uitgezonderd {
                    fouten.push(format!(
                        "weesveld '{}' in event '{}' (stroom '{}'): geen afleiding leest het en het staat niet in niet_gereduceerd",
                        blad.path, event.name, stream.id
                    ));
                }
            }
        }
    }
}

fn botsingen(streams: &[Stroom], lexostatuses: &Lexostatussen, fouten: &mut Vec<String>) {
    // (stroom, event, parameter) -> lexostatussen die hem afleiden
    let mut per: BTreeMap<(String, String, String), Vec<&str>> = BTreeMap::new();
    for def in lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| !d.is_lijst())
    {
        for (param, a) in &def.reduction.derivations {
            for (s, e) in events_voor(def, a.filter(), streams) {
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

/// De controles op het portaal van een proces, tegen de stromen en
/// lexostatussen van de cel waarin het vastlegt: het event bestaat, bindt
/// alleen `$intake`-paden die de kanalen van het portaal leveren
/// (`intake_paden`, zie [`crate::kanaal`]), de toets-lexostatus leest het
/// event en kiest een gram, de toetsuitkomst komt uit de grondslag van het
/// event, en het aanbod klopt.
pub fn portal(
    streams: &[Stroom],
    lexostatuses: &Lexostatussen,
    p: &Portal,
    service: &LawExecutionService,
    intake_paden: &[String],
) -> Vec<String> {
    let mut fouten = Vec::new();
    portaal_(streams, lexostatuses, p, service, intake_paden, &mut fouten);
    fouten
}

fn portaal_(
    streams: &[Stroom],
    lexostatuses: &Lexostatussen,
    p: &Portal,
    service: &LawExecutionService,
    intake_paden: &[String],
    fouten: &mut Vec<String>,
) {
    let Some(stream) = streams.iter().find(|s| s.id == p.stream) else {
        fouten.push(format!(
            "portal: stroom '{}' bestaat niet in cel '{}'",
            p.stream, p.cell
        ));
        return;
    };
    let Some(event) = stream.event(&p.event) else {
        fouten.push(format!(
            "portal: stroom '{}' heeft geen event '{}'",
            p.stream, p.event
        ));
        return;
    };
    for blad in event.bladeren() {
        if let Binding::Intake(path) = &blad.binding {
            if !intake_paden.contains(path) {
                fouten.push(format!(
                    "portal: veld '{}' bindt aan '$intake.{path}', maar de kanalen van het portaal leveren alleen {}",
                    blad.path,
                    intake_paden.join(", ")
                ));
            }
        }
    }
    match lexostatuses.lexostatus(&p.assessment.lexostatus) {
        None => fouten.push(format!(
            "portal: lexostatus '{}' bestaat niet",
            p.assessment.lexostatus
        )),
        Some(def) => {
            if !events_voor(def, None, streams)
                .iter()
                .any(|(s, e)| s.id == stream.id && e.name == event.name)
            {
                fouten.push(format!(
                    "portal: lexostatus '{}' leest event '{}' niet",
                    def.name, event.name
                ));
            }
            if def.reduction.pick.is_none() {
                fouten.push(format!(
                    "portal: lexostatus '{}' kiest geen gram (kies), en de toets reduceert een concept",
                    def.name
                ));
            }
            if def.is_lijst() {
                fouten.push(format!(
                    "portal: lexostatus '{}' is een lijst (groepeer), en een lijst gaat nooit naar de engine",
                    def.name
                ));
            }
            for i in def.inputs.iter().filter(|i| i.name != "root") {
                fouten.push(format!(
                    "portal: lexostatus '{}' vraagt input '{}'; de toets geeft alleen 'wortel' mee",
                    def.name, i.name
                ));
            }
        }
    }
    match service.resolver().get_article_by_output(
        &p.assessment.regulation,
        &p.assessment.output,
        None,
    ) {
        None => fouten.push(format!(
            "portal: regeling '{}' heeft geen uitkomst '{}'",
            p.assessment.regulation, p.assessment.output
        )),
        // De toets geeft de lexostatus als parameters aan dit artikel; die
        // zijn afgeleid voor de artikelen uit de grondslag van het event.
        // Op artikel, ook als de grondslag een lid noemt.
        Some(article) => {
            let legal_basis = format!("{}#{}", p.assessment.regulation, article.number);
            let in_grondslag = event.legal_basis.iter().any(|g| {
                regelingen::ontleed(g).is_ok_and(|o| {
                    o.regulation == p.assessment.regulation && o.article == article.number
                })
            });
            if !in_grondslag {
                fouten.push(format!(
                    "portal: uitkomst '{}' komt uit {legal_basis}, en dat staat niet in de grondslag van event '{}' ({})",
                    p.assessment.output,
                    event.name,
                    event.legal_basis.join(", ")
                ));
            }
        }
    }
    if let Some(a) = &p.offer {
        aanbod_(a, service, fouten);
    }
}

/// Het aanbod: de uitkomst bestaat, en een termijn komt uit hetzelfde
/// artikel. Uitkomst en termijn gaan in een run; een termijn uit een ander
/// artikel, of een die niet bestaat, zou die run laten mislukken en daarmee
/// ook het oordeel over de uitkomst. De tijdvakken zijn een uitkomst van
/// dezelfde regeling, in een eigen run zonder parameters.
fn aanbod_(a: &Aanbod, service: &LawExecutionService, fouten: &mut Vec<String>) {
    let resolver = service.resolver();
    if let Some(t) = &a.windows {
        match resolver.get_article_by_output(&a.regulation, t, None) {
            None => fouten.push(format!(
                "portaal.aanbod: regeling '{}' heeft geen tijdvakken-uitkomst '{t}'",
                a.regulation
            )),
            Some(art) if art.get_parameters().iter().any(|p| p.required != Some(false)) => {
                fouten.push(format!(
                    "portaal.aanbod: tijdvakken '{t}' komt uit {}#{}, en dat artikel vraagt een parameter; de tijdvakken staan vast voordat iemand iets invult",
                    a.regulation, art.number
                ))
            }
            Some(_) => {}
        }
    }
    // Het begin en de openstelling van een tijdvak: een uitkomst van
    // dezelfde regeling, met het tijdvak als parameter.
    for (soort, u) in [("begin", &a.start), ("openstelling", &a.opening)] {
        let Some(u) = u else {
            continue;
        };
        if a.windows.is_none() {
            fouten.push(format!("portaal.aanbod: {soort} zonder tijdvakken"));
        }
        if resolver
            .get_article_by_output(&a.regulation, u, None)
            .is_none()
        {
            fouten.push(format!(
                "portaal.aanbod: regeling '{}' heeft geen {soort}-uitkomst '{u}'",
                a.regulation
            ));
        }
    }
    let Some(article) = resolver.get_article_by_output(&a.regulation, &a.output, None) else {
        fouten.push(format!(
            "portaal.aanbod: regeling '{}' heeft geen uitkomst '{}'",
            a.regulation, a.output
        ));
        return;
    };
    let Some(t) = &a.deadline else {
        return;
    };
    match resolver.get_article_by_output(&a.regulation, t, None) {
        None => fouten.push(format!(
            "portaal.aanbod: regeling '{}' heeft geen termijn-uitkomst '{t}'",
            a.regulation
        )),
        Some(ander) if ander.number != article.number => fouten.push(format!(
            "portaal.aanbod: termijn '{t}' komt uit {r}#{}, de uitkomst '{}' uit {r}#{}; ze moeten uit hetzelfde artikel komen",
            ander.number,
            a.output,
            article.number,
            r = a.regulation,
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
    const CEL: &str = include_str!("../tests/fixtures/cells/instantie/lexostatuses.yaml");
    const CELDEF: &str = include_str!("../tests/fixtures/processes/instantie/process.yaml");
    const REG_STROOM: &str = include_str!("../tests/fixtures/chronicles/test_registers.yaml");
    const REG_CEL: &str = include_str!("../tests/fixtures/cells/register/lexostatuses.yaml");

    fn service() -> LawExecutionService {
        regelingen::laad(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation"))
            .unwrap()
            .service
    }

    fn draai(stroom_tekst: &str, cel_tekst: &str) -> Result<(), Vec<String>> {
        draai_met(stroom_tekst, cel_tekst, CELDEF)
    }

    /// De controles op de cel, en op het portaal van het proces (`celdef` is
    /// een `proces.yaml`).
    fn draai_met(stroom_tekst: &str, cel_tekst: &str, celdef: &str) -> Result<(), Vec<String>> {
        let s = stroom::parse(stroom_tekst, "stroom")?;
        let c = reductie::parse(cel_tekst, "cel")?;
        let d = crate::config::ProcesDefinitie::parse(celdef, "process.yaml")?;
        let streams = [s];
        let mut fouten = controleer(&streams, &c, &service())
            .err()
            .unwrap_or_default();
        if let Some(p) = &d.portal {
            let paden = crate::kanaal::portaal_intake_paden(&d);
            fouten.extend(portal(&streams, &c, p, &service(), &paden));
        }
        if fouten.is_empty() {
            Ok(())
        } else {
            Err(fouten)
        }
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
        controleer(&[s], &c, &service())
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
        faalt_met(STROOM, &CEL.replace("pick: latest", "pick: eerste"), "pick");
    }

    // 2. Afleiding wijst naar iets dat bestaat.
    #[test]
    fn afleiding_naar_onbekende_parameter() {
        let cell = CEL.replace("bevat_naam: {filled", "bevat_naampje: {filled");
        faalt_met(
            STROOM,
            &cell,
            "afleiding 'bevat_naampje': 'bevat_naampje' is geen parameter",
        );
    }

    #[test]
    fn afleiding_naar_onbekend_veldpad() {
        let cell = CEL.replace("{filled: content.naam}", "{filled: content.naam_x}");
        faalt_met(STROOM, &cell, "veldpad 'content.naam_x' bestaat niet");
    }

    #[test]
    fn tabel_moet_een_veld_zijn() {
        let cell = CEL.replace(
            "{table: content.organen, one_row",
            "{table: content, one_row",
        );
        faalt_met(STROOM, &cell, "veldpad 'content' is geen tabelveld");
        // Een gewoon veld is ook geen tabel.
        let cell = CEL.replace(
            "{table: content.organen, one_row",
            "{table: content.naam, one_row",
        );
        faalt_met(STROOM, &cell, "veldpad 'content.naam' is geen tabelveld");
    }

    #[test]
    fn tabelafleiding_leest_een_gedeclareerde_kolom() {
        let cell = CEL.replace("each_row: zetels}", "each_row: stoelen}");
        faalt_met(
            STROOM,
            &cell,
            "kolom 'stoelen' staat niet in de kolommen van tabel 'content.organen'",
        );
        let cell = CEL.replace("only_where: samengevoegd}", "only_where: gebundeld}");
        faalt_met(STROOM, &cell, "kolom 'gebundeld' staat niet in de kolommen");
    }

    #[test]
    fn filter_zonder_event() {
        let cell = CEL.replace("subtype: aanvraag, root", "subtype: melding, root");
        faalt_met(STROOM, &cell, "het filter wijst geen event aan");
    }

    #[test]
    fn filter_met_onbekende_input() {
        let cell = CEL.replace("root: $root}", "root: $zaak}");
        faalt_met(STROOM, &cell, "'zaak' is geen input");
    }

    #[test]
    fn grondslag_met_een_lid() {
        // Artikel 1 heeft lid 1 ("1. Een aanvraag ..."); de toets vergelijkt
        // op artikel, ook als de grondslag een lid noemt.
        let stream = STROOM.replace(
            "      - testregeling_aanvraag#1\n",
            "      - testregeling_aanvraag#1 lid 1\n",
        );
        assert_ne!(stream, STROOM);
        draai(&stream, CEL).unwrap();
        let stream = STROOM.replace(
            "      - testregeling_aanvraag#1\n",
            "      - testregeling_aanvraag#1 lid 9\n",
        );
        faalt_met(
            &stream,
            CEL,
            "grondslag 'testregeling_aanvraag#1 lid 9': artikel 1 heeft geen lid 9",
        );
    }

    #[test]
    fn grondslag_die_niet_bestaat() {
        let stream = STROOM.replace(
            "- testregeling_aanvraag#1",
            "- testregeling_aanvraag#1\n      - testregeling_aanvraag#7",
        );
        faalt_met(&stream, CEL, "heeft geen artikel 7");
    }

    #[test]
    fn grondslag_van_een_gebonden_op_moment_bestaat() {
        let stream = STROOM.replace(
            "legal_basis: [testregeling_aanvraag#1]",
            "legal_basis: [testregeling_aanvraag#8]",
        );
        assert_ne!(stream, STROOM);
        faalt_met(&stream, CEL, "heeft geen artikel 8");
    }

    // 3. Geen weesveld.
    #[test]
    fn weesveld() {
        let stream = STROOM.replace(
            "      - {field: content.rekeningnummer, reason: voor de betaling}\n",
            "",
        );
        faalt_met(&stream, CEL, "weesveld 'content.rekeningnummer'");
    }

    #[test]
    fn niet_gereduceerd_dekt_een_tak() {
        // `kern` staat als geheel in niet_gereduceerd: geen weesveld eronder.
        let s = stroom::parse(STROOM, "s").unwrap();
        assert!(s.events[0].not_reduced.iter().any(|n| n.field == "core"));
        draai(STROOM, CEL).unwrap();
    }

    #[test]
    fn niet_gereduceerd_naar_onbekend_veld() {
        let stream = STROOM.replace(
            "{field: content.rekeningnummer,",
            "{field: content.rekening,",
        );
        faalt_met(&stream, CEL, "niet_gereduceerd noemt 'content.rekening'");
    }

    // 7. De naam van de zaakstand is van de runtime.
    #[test]
    fn de_zaakstand_is_van_de_runtime() {
        // De eerste definitie heet nu zaakstand.
        let i = CEL.find("- name: ").unwrap() + "- name: ".len();
        let eind = CEL[i..].find('\n').unwrap() + i;
        let cell = format!("{}case_state{}", &CEL[..i], &CEL[eind..]);
        faalt_met(STROOM, &cell, "die naam is van de runtime");
    }

    // 4. Geen naamsbotsing.
    #[test]
    fn dubbele_afleiding_over_twee_lexostatussen() {
        let extra = "  - name: tweede\n    inputs: [{name: root, type: string}]\n    reduction:\n      chronicle: test_kroniek\n      filter: {name: aanvraag_ontvangen, root: $root}\n      pick: latest\n      derivations:\n        bevat_naam: {filled: core.aanvrager.naam}\n";
        let cell = format!("{CEL}{extra}");
        faalt_met(
            STROOM,
            &cell,
            "parameter 'bevat_naam' krijgt meer dan een afleiding",
        );
    }

    #[test]
    fn dubbele_afleiding_in_een_lexostatus() {
        let cell = CEL.replace(
            "        bevat_aanduiding: {filled: content.aanduiding}\n",
            "        bevat_aanduiding: {filled: content.aanduiding}\n        bevat_aanduiding: {filled: content.naam}\n",
        );
        // De YAML-lezer weigert een dubbele sleutel al, en noemt hem.
        faalt_met(
            STROOM,
            &cell,
            "duplicate entry with key \"bevat_aanduiding\"",
        );
    }

    #[test]
    fn portaal_met_uitkomst_buiten_de_grondslag() {
        let celdef = CELDEF.replace(
            "regulation: testregeling_aanvraag\n    output: aanvraag_volledig",
            "regulation: testregeling_awb\n    output: in_verzuim",
        );
        portaal_faalt_met(&celdef, "staat niet in de grondslag van event");
    }

    /// De cel-definitie met een aanbod uit `testregeling_aanvraag`.
    fn met_aanbod(output: &str, deadline: Option<&str>) -> String {
        let deadline = deadline
            .map(|t| format!("    deadline: {t}\n"))
            .unwrap_or_default();
        CELDEF.replace(
            "  form:",
            &format!(
                "  offer:\n    regulation: testregeling_aanvraag\n    output: {output}\n{deadline}  form:"
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
        let stream = STROOM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        faalt_met(&stream, CEL, "'$intake.eherkenning.bsn'");
    }

    #[test]
    fn zonder_portaal_geen_portaalcontrole() {
        let celdef = CELDEF.split("\nportal:").next().unwrap().to_string();
        draai_met(STROOM, CEL, &celdef).unwrap();
        // Ook een intake-pad dat geen portaal levert, is dan geen fout.
        let stream = STROOM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        draai_met(&stream, CEL, &celdef).unwrap();
    }

    #[test]
    fn portaal_met_lexostatus_zonder_kies() {
        let cell = CEL.replace("      pick: latest\n", "");
        let fouten = draai(STROOM, &cell).unwrap_err();
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

    /// De grondslag van een afleiding maakt een parameter geldig die niet uit
    /// de grondslag van het event komt, en wordt zelf gecontroleerd zoals die
    /// van een event: artikel geladen, lid bestaat.
    #[test]
    fn de_grondslag_van_een_afleiding() {
        // Een naam die de regeling van het register niet kent.
        let cell = REG_CEL.replace(
            "        jaar_van_mededeling:             # het jaartal van een datum in de kroniek\n",
            "        jaar:\n",
        );
        register_faalt_met(
            &cell,
            "'jaar' is geen parameter van een artikel uit de grondslag van event 'mededeling_gedaan' (testregeling_register#3)",
        );
        // Met een grondslag die hem vraagt, is hij geldig.
        let met = |legal_basis: &str| {
            cell.replace(
                "          year_of: datum\n",
                &format!("          year_of: datum\n          legal_basis: [{legal_basis}]\n"),
            )
        };
        register(&met("testregeling_afnemer#5")).unwrap();
        // Een artikel dat niet bestaat, of een lid dat het artikel niet heeft.
        register_faalt_met(
            &met("testregeling_afnemer#9"),
            "afleiding 'jaar': grondslag 'testregeling_afnemer#9': regeling 'testregeling_afnemer' heeft geen artikel 9",
        );
        register_faalt_met(
            &met("testregeling_afnemer#5 lid 7"),
            "afleiding 'jaar': grondslag 'testregeling_afnemer#5 lid 7': artikel 5 heeft geen lid 7",
        );
        // Ook bij een extra veld wordt de grondslag gecontroleerd.
        let cell = REG_CEL.replace(
            "legal_basis: [testregeling_register#1]\n",
            "legal_basis: [testregeling_onbekend#1]\n",
        );
        register_faalt_met(
            &cell,
            "afleiding 'ingeschreven': grondslag 'testregeling_onbekend#1': regeling 'testregeling_onbekend' is niet geladen",
        );
    }

    #[test]
    fn filter_per_afleiding_wijst_een_event_aan() {
        let cell = REG_CEL.replace(
            "{name: aanduiding_geschrapt,",
            "{name: aanduiding_verloren,",
        );
        register_faalt_met(
            &cell,
            "afleiding 'is_geschrapt': het filter wijst geen event aan",
        );
    }

    #[test]
    fn filter_per_afleiding_op_een_veldpad_dat_bestaat() {
        let cell = REG_CEL.replace(
            "{name: aanduiding_geschrapt, orgaan: $orgaan,",
            "{name: aanduiding_geschrapt, gebied: $orgaan,",
        );
        register_faalt_met(
            &cell,
            "veldpad 'gebied' bestaat niet in event 'aanduiding_geschrapt'",
        );
        let cell = REG_CEL.replace(
            "aanduiding: $aanduiding, gebied: $gebied}",
            "aanduiding: $naam, gebied: $gebied}",
        );
        register_faalt_met(&cell, "'naam' is geen input");
    }

    #[test]
    fn filter_en_som_lezen_velden_dus_geen_weesveld() {
        // `lijst` leest alleen het filter, `zetels` alleen de som.
        let cell = REG_CEL.replace(
            "        zetels_toegewezen: {filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, sum: zetels}\n",
            "",
        );
        register_faalt_met(&cell, "weesveld 'lijst' in event 'uitslag_vastgesteld'");
        register_faalt_met(&cell, "weesveld 'zetels' in event 'uitslag_vastgesteld'");
    }

    #[test]
    fn afleiding_op_het_gram_in_een_lexostatus_zonder_kies() {
        let cell = format!("{REG_CEL}        x: {{field: aanduiding}}\n");
        register_faalt_met(
            &cell,
            "afleiding 'x': leest het gekozen gram, maar de lexostatus kiest er geen",
        );
    }

    #[test]
    fn extra_veld_met_de_naam_van_een_afleiding() {
        let cell =
            format!("{CEL}      extra_fields:\n        bevat_naam: {{field: content.naam}}\n");
        faalt_met(
            STROOM,
            &cell,
            "'bevat_naam' is zowel een afleiding als een extra veld",
        );
        let cell = format!("{CEL}      extra_fields:\n        name: {{field: content.naamx}}\n");
        faalt_met(STROOM, &cell, "veldpad 'content.naamx' bestaat niet");
        // Een extra veld hoeft geen parameter te zijn.
        let cell = format!("{CEL}      extra_fields:\n        name: {{field: content.naam}}\n");
        draai(STROOM, &cell).unwrap();
    }

    // 6. Een lijst-lexostatus.
    const LIJST: &str = "  - name: werkvoorraad\n    inputs: []\n    reduction:\n      chronicle: test_kroniek\n      filter: {type: submission}\n      group_by: root\n      pick: latest\n      derivations:\n        naam: {field: content.naam}\n        aanvraagjaar: {field: content.aanvraagjaar}\n";

    #[test]
    fn lijst_heeft_kolommen_geen_parameters() {
        // `naam` is geen parameter, en `aanvraagjaar` botst niet met de
        // afleiding in aanvraag_inhoud: een lijst gaat nooit naar de engine.
        draai(STROOM, &format!("{CEL}{LIJST}")).unwrap();
    }

    #[test]
    fn zonder_wijst_een_event_aan() {
        let list = LIJST.replace(
            "      pick: latest\n",
            "      without: {stage: BESLUIT}\n      pick: latest\n",
        );
        faalt_met(
            STROOM,
            &format!("{CEL}{list}"),
            "lexostatus 'werkvoorraad': zonder wijst geen event aan in kroniek 'test_kroniek'",
        );
        // Wijst het een event aan, dan leest het ook zijn veldpaden.
        let list = LIJST.replace("      pick: latest\n", "      without: {name: aanvraag_ontvangen, content.bestaat_niet: x}\n      pick: latest\n");
        faalt_met(
            STROOM,
            &format!("{CEL}{list}"),
            "zonder filtert op veldpad 'content.bestaat_niet'",
        );
    }

    #[test]
    fn portaal_toetst_geen_lijst() {
        let cell = format!("{CEL}{LIJST}");
        let celdef = CELDEF.replace("lexostatus: aanvraag_inhoud", "lexostatus: werkvoorraad");
        let fouten = draai_met(STROOM, &cell, &celdef).unwrap_err();
        assert!(
            fouten
                .iter()
                .any(|f| f.contains("'werkvoorraad' is een lijst")),
            "{fouten:?}"
        );
    }
}
