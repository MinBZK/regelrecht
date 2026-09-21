//! De controles bij het opstarten. De cel weigert te starten als er een
//! faalt, met een melding die het veld of de parameter noemt.
//!
//! 1. Stroom en celconfiguratie valideren tegen hun schema (bij het laden,
//!    zie [`crate::stroom::parse`] en [`crate::reductie::parse`]).
//! 2. Elke afleiding wijst naar iets dat bestaat: een parameter van een
//!    artikel uit de grondslag van het gefilterde event, en veldpaden van
//!    dat event.
//! 3. Geen weesveld: elk veld van een event wordt door een afleiding gelezen
//!    of staat met reden in `niet_gereduceerd`.
//! 4. Geen naamsbotsing: een parameter krijgt maar een afleiding.
//!
//! Daarnaast: het portaalblok wijst naar een bestaand event, een bestaande
//! lexostatus en een bestaande uitkomst, van een artikel uit de grondslag
//! van dat event.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::LawExecutionService;

use crate::eherkenning::INTAKE_PADEN;
use crate::reductie::{CelConfig, LexostatusDefinitie};
use crate::regelingen;
use crate::stroom::{Binding, Event, Stroom};

/// Een event met de stroom waar het in staat.
pub type StroomEvent<'a> = (&'a Stroom, &'a Event);

/// De events die het filter van een definitie kan aanwijzen: in de kroniek
/// van de reductie, en gelijk op elke vaste filterwaarde. Een waarde `$x`
/// hangt van de vraag af en telt hier als passend.
pub fn events_voor<'a>(def: &LexostatusDefinitie, strommen: &'a [Stroom]) -> Vec<StroomEvent<'a>> {
    let mut uit = Vec::new();
    for stroom in strommen
        .iter()
        .filter(|s| s.chronicle == def.reduction.kroniek)
    {
        for event in &stroom.events {
            let past = def.reduction.filter.iter().all(|(sleutel, waarde)| {
                if waarde.starts_with('$') {
                    return true;
                }
                let eigen = match sleutel.as_str() {
                    "name" => Some(event.name.as_str()),
                    "type" => Some(event.type_.as_str()),
                    "soort" => event.soort.as_deref(),
                    "recording_actor" => Some(stroom.recording_actor.as_str()),
                    "chronicle" => Some(stroom.chronicle.as_str()),
                    _ => return true,
                };
                eigen == Some(waarde.as_str())
            });
            if past {
                uit.push((stroom, event));
            }
        }
    }
    uit
}

/// Alle controles. `Ok` als de cel mag starten.
pub fn controleer(
    strommen: &[Stroom],
    config: &CelConfig,
    service: &LawExecutionService,
) -> Result<(), Vec<String>> {
    let mut fouten = Vec::new();
    uniek(strommen, config, &mut fouten);
    grondslagen(strommen, service, &mut fouten);
    verwijzingen(strommen, config, service, &mut fouten);
    weesvelden(strommen, config, &mut fouten);
    botsingen(strommen, config, &mut fouten);
    portaal(strommen, config, service, &mut fouten);
    if fouten.is_empty() {
        Ok(())
    } else {
        Err(fouten)
    }
}

fn uniek(strommen: &[Stroom], config: &CelConfig, fouten: &mut Vec<String>) {
    let mut gezien = BTreeSet::new();
    for s in strommen {
        if !gezien.insert(&s.id) {
            fouten.push(format!("stroom '{}' staat er meer dan een keer", s.id));
        }
    }
    let mut gezien = BTreeSet::new();
    for d in &config.lexostatus_definitions {
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

/// De parameters van de artikelen uit de grondslag van een event.
fn parameters(event: &Event, service: &LawExecutionService) -> BTreeSet<String> {
    event
        .grondslag
        .iter()
        .filter_map(|g| regelingen::artikel(service, g).ok())
        .flat_map(|a| a.get_parameters().iter().map(|p| p.name.clone()))
        .collect()
}

fn verwijzingen(
    strommen: &[Stroom],
    config: &CelConfig,
    service: &LawExecutionService,
    fouten: &mut Vec<String>,
) {
    for def in &config.lexostatus_definitions {
        let inputs: BTreeSet<&str> = def.inputs.iter().map(|i| i.name.as_str()).collect();
        for (sleutel, waarde) in &def.reduction.filter {
            if let Some(input) = waarde.strip_prefix('$') {
                if !inputs.contains(input) {
                    fouten.push(format!(
                        "lexostatus '{}': filter '{sleutel}' gebruikt '${input}', maar '{input}' is geen input",
                        def.name
                    ));
                }
            }
        }
        let events = events_voor(def, strommen);
        if events.is_empty() {
            fouten.push(format!(
                "lexostatus '{}': het filter wijst geen event aan in kroniek '{}'",
                def.name, def.reduction.kroniek
            ));
        }
        for (_, event) in events {
            let params = parameters(event, service);
            for (param, afleiding) in &def.reduction.afleidingen {
                if !params.contains(param) {
                    fouten.push(format!(
                        "lexostatus '{}', afleiding '{param}': '{param}' is geen parameter van een artikel uit de grondslag van event '{}' ({})",
                        def.name,
                        event.name,
                        event.grondslag.join(", ")
                    ));
                }
                let tabel = matches!(
                    afleiding,
                    crate::reductie::Afleiding::ElkeRegel { .. }
                        | crate::reductie::Afleiding::EenRegel { .. }
                );
                for pad in afleiding.gelezen_paden() {
                    let bestaat = if tabel {
                        event.heeft_blad(pad)
                    } else {
                        event.heeft_pad(pad)
                    };
                    if !bestaat {
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

fn weesvelden(strommen: &[Stroom], config: &CelConfig, fouten: &mut Vec<String>) {
    for stroom in strommen {
        for event in &stroom.events {
            let mut gelezen: Vec<&str> = Vec::new();
            for def in &config.lexostatus_definitions {
                if events_voor(def, strommen)
                    .iter()
                    .any(|(s, e)| s.id == stroom.id && e.name == event.name)
                {
                    for a in def.reduction.afleidingen.values() {
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

fn botsingen(strommen: &[Stroom], config: &CelConfig, fouten: &mut Vec<String>) {
    // (stroom, event, parameter) -> lexostatussen die hem afleiden
    let mut per: BTreeMap<(String, String, String), Vec<&str>> = BTreeMap::new();
    for def in &config.lexostatus_definitions {
        for (s, e) in events_voor(def, strommen) {
            for param in def.reduction.afleidingen.keys() {
                per.entry((s.id.clone(), e.name.clone(), param.clone()))
                    .or_default()
                    .push(&def.name);
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

fn portaal(
    strommen: &[Stroom],
    config: &CelConfig,
    service: &LawExecutionService,
    fouten: &mut Vec<String>,
) {
    let Some(p) = &config.portaal else {
        fouten.push("de celconfiguratie mist het blok 'portaal'".into());
        return;
    };
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
    match config.lexostatus(&p.toets.lexostatus) {
        None => fouten.push(format!(
            "portaal: lexostatus '{}' bestaat niet",
            p.toets.lexostatus
        )),
        Some(def) => {
            if !events_voor(def, strommen)
                .iter()
                .any(|(s, e)| s.id == stroom.id && e.name == event.name)
            {
                fouten.push(format!(
                    "portaal: lexostatus '{}' leest event '{}' niet",
                    def.name, event.name
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{reductie, stroom};
    use std::path::Path;

    const STROOM: &str = include_str!("../tests/fixtures/chronicles/test_aanvragen.yaml");
    const CEL: &str = include_str!("../tests/fixtures/cel/lexostatussen.yaml");

    fn service() -> LawExecutionService {
        regelingen::laad(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation"))
            .unwrap()
    }

    fn draai(stroom_tekst: &str, cel_tekst: &str) -> Result<(), Vec<String>> {
        let s = stroom::parse(stroom_tekst, "stroom")?;
        let c = reductie::parse(cel_tekst, "cel")?;
        controleer(&[s], &c, &service())
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
        faalt_met(STROOM, &cel, "veldpad 'inhoud' bestaat niet");
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
        let extra = "  - name: tweede\n    inputs: [{name: zaakkenmerk, type: string}]\n    reduction:\n      kroniek: test_kroniek\n      filter: {name: aanvraag_ontvangen, zaakkenmerk: $zaakkenmerk}\n      kies: laatste\n      afleidingen:\n        bevat_naam: {gevuld: kern.aanvrager.naam}\nportaal:";
        let cel = CEL.replacen("portaal:", extra, 1);
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

    // Portaal.
    #[test]
    fn portaal_met_onbekende_uitkomst() {
        let cel = CEL.replace("uitkomst: aanvraag_volledig", "uitkomst: bestaat_niet");
        faalt_met(STROOM, &cel, "geen uitkomst 'bestaat_niet'");
    }

    #[test]
    fn portaal_met_uitkomst_buiten_de_grondslag() {
        let cel = CEL.replace(
            "regeling: testregeling_aanvraag\n    uitkomst: aanvraag_volledig",
            "regeling: testregeling_awb\n    uitkomst: in_verzuim",
        );
        faalt_met(STROOM, &cel, "staat niet in de grondslag van event");
    }

    #[test]
    fn portaal_met_onbekend_intakepad() {
        let stroom = STROOM.replace("$intake.eherkenning.persoon", "$intake.eherkenning.bsn");
        faalt_met(&stroom, CEL, "'$intake.eherkenning.bsn'");
    }

    #[test]
    fn portaal_ontbreekt() {
        let cel = CEL.split("portaal:").next().unwrap().to_string();
        faalt_met(STROOM, &cel, "mist het blok 'portaal'");
    }
}
