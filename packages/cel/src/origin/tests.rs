//! Tests van de herkomstcontrole.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// De fixture-regelingen, een aanpassing op de regeling van de afnemer,
/// en extra regelingen.
fn service(pas_aan: impl Fn(String) -> String, extra: &[&str]) -> Arc<LawExecutionService> {
    let mut s = LawExecutionService::new();
    for e in walkdir::WalkDir::new(fixtures().join("regulation")) {
        let e = e.unwrap();
        if e.file_type().is_file() {
            let mut tekst = std::fs::read_to_string(e.path()).unwrap();
            // De regeling van de afnemer, en de Awb met de procedure.
            if tekst.contains("$id: testregeling_afnemer")
                || tekst.contains("$id: testregeling_awb")
            {
                tekst = pas_aan(tekst);
            }
            s.load_law(&tekst).unwrap();
        }
    }
    for t in extra {
        s.load_law(t).unwrap();
    }
    Arc::new(s)
}

fn cells(s: &Arc<LawExecutionService>) -> BTreeMap<String, Arc<Cell>> {
    crate::config::celmappen(&fixtures().join("cells"))
        .unwrap()
        .iter()
        .map(|m| {
            let c = Cell::laad(m, s.clone()).unwrap();
            (c.id().to_string(), Arc::new(c))
        })
        .collect()
}

fn proces(name: &str, pas_aan: impl Fn(String) -> String) -> ProcesDefinitie {
    let path = fixtures().join("processes").join(name).join("process.yaml");
    let tekst = pas_aan(std::fs::read_to_string(&path).unwrap());
    ProcesDefinitie::parse(&tekst, "t").unwrap()
}

/// De controle op het proces van de afnemer.
fn afnemer(
    regulation: impl Fn(String) -> String,
    pas_aan: impl Fn(String) -> String,
    extra: &[&str],
) -> Controle {
    let s = service(regulation, extra);
    let c = cells(&s);
    let d = proces("afnemer", pas_aan);
    controleer_met_stand(d, &c, &s)
}

/// Zoals bij het laden van een proces: de handelingen voorbereiden (de
/// soort en wat nog niet gebeurd is, uit de procedure), dan de controle.
fn controleer_met_stand(
    mut d: ProcesDefinitie,
    c: &BTreeMap<String, Arc<Cell>>,
    s: &Arc<LawExecutionService>,
) -> Controle {
    let authority = crate::gezag::eigen(&d, s);
    let f = crate::handeling::bereid_voor(&mut d, authority.as_deref(), s, &c["test_afnemer"]);
    assert!(f.is_empty(), "{f:?}");
    controleer(&d, &c["test_afnemer"], c, s)
}

fn zo(t: String) -> String {
    t
}

/// De origin van `jaar` (art. 3) vervangen.
fn jaar_met(origin: &'static str) -> impl Fn(String) -> String {
    move |t: String| {
        t.replace(
                "origin: {waarde: REGISTER, register: testregeling_register, grondslag: testregeling_register#3}\n          - name: gebiedstabel",
                &format!("origin: {origin}\n          - name: gebiedstabel"),
            )
    }
}

#[test]
fn de_fixture_van_de_afnemer_heeft_voor_alles_een_leverancier() {
    let c = afnemer(zo, zo, &[]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    let decision: Vec<(&str, OriginValue)> = c.parameters["besluit"]
        .iter()
        .map(|(b, g)| (b.name.as_str(), g.as_ref().unwrap().origin.waarde))
        .collect();
    assert!(decision.contains(&("feiten_vergaard", OriginValue::Oordeel)));
    assert!(decision.contains(&("jaar", OriginValue::Register)));
}

/// Een verplichte parameter zonder leverancier houdt de runtime tegen, met
/// parameter, herkomst en grondslag in de melding.
#[test]
fn een_ontbrekende_leverancier_is_een_fout() {
    // De procedure vraagt de datum van bekendmaking niet meer in een
    // latere stage, en de lexostatus die haar leest is geen bron: dan
    // levert niets haar.
    let c = afnemer(
        |t| t.replace("          - {name: datum_bekendmaking, type: date}\n", ""),
        |t| {
            alleen_het_besluit(t).replace(
                "  - {cell: test_afnemer, lexostatus: besluit, case: true}\n",
                "",
            )
        },
        &[],
    );
    assert_eq!(
            c.fouten,
            ["besluit: geen leverancier voor parameter 'datum_bekendmaking' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2)"]
        );
}

/// Het proces van de afnemer met alleen de handeling van het besluit.
fn alleen_het_besluit(t: String) -> String {
    let start = t.find("    # De bekendmaking").unwrap();
    let eind = t.find("# Standaardgegevens").unwrap();
    format!("{}{}", &t[..start], &t[eind..])
}

/// Met required: false en zonder leverancier krijgt de engine de waarde
/// niet en rekent ze met een onbekende (RFC-036): een waarschuwing.
#[test]
fn zonder_leverancier_en_niet_verplicht_is_een_waarschuwing() {
    let c = afnemer(
        |t| t.replace("          - {name: bekendgemaakt, type: boolean}\n", ""),
        zo,
        &[],
    );
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert_eq!(
            c.warnings,
            ["besluit: geen leverancier voor parameter 'bekendgemaakt' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2); required: false, dus de engine krijgt hem niet en rekent met een onbekende waarde (RFC-036)"]
        );
}

/// Een leverancier van de verkeerde soort is een fout, en de melding zegt
/// waar de parameter nu vandaan komt.
#[test]
fn een_leverancier_die_niet_bij_de_herkomst_past() {
    let c = afnemer(
        jaar_met("{waarde: DOSSIER, grondslag: 'testregeling_afnemer#3'}"),
        zo,
        &[],
    );
    assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'jaar' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3): hij komt uit synthese-bron test_register/registerstatus"]
        );
}

/// Een verkeerde bron is ook een fout als de parameter required: false
/// is: de engine zou dan rekenen met een waarde van de verkeerde partij.
#[test]
fn een_verkeerde_bron_is_ook_bij_required_false_een_fout() {
    let c = afnemer(
        |t| {
            t.replace(
                    "          - name: bekendgemaakt\n            type: boolean\n            required: false\n            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 2}",
                    "          - name: bekendgemaakt\n            type: boolean\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#3 lid 2}",
                )
        },
        zo,
        &[],
    );
    assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'bekendgemaakt' van testregeling_afnemer#3 (BELANGHEBBENDE, grondslag testregeling_afnemer#3 lid 2): hij komt uit de stand bij besluit"]
        );
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
}

/// Wat de aanvrager indient (een gram van type indiening) is van de
/// belanghebbende; wat de eigen actor verder vastlegt (het verloop van de
/// zaak) is dossier. Wie ze verwisselt, krijgt een fout.
#[test]
fn belanghebbende_en_dossier_volgen_uit_wat_de_afleiding_leest() {
    let c = afnemer(
        |t| {
            t.replace(
                    "          - name: aanvraagdatum\n            type: date\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}",
                    "          - name: aanvraagdatum\n            type: date\n            required: false\n            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#1}",
                )
                .replace(
                    "            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen",
                    "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen",
                )
        },
        zo,
        &[],
    );
    assert!(c.fouten.contains(
            &"assessment: verkeerde bron voor parameter 'aanvraagdatum' van testregeling_afnemer#1 (DOSSIER, grondslag testregeling_afnemer#1): hij komt uit eigen lexostatus aanvraag_inhoud (wat de aanvrager indiende)".to_string()
        ), "{:?}", c.fouten);
    assert!(c.fouten.contains(
            &"besluit: verkeerde bron voor parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 (BELANGHEBBENDE, grondslag testregeling_afnemer#3 lid 1): hij komt uit eigen lexostatus zaakverloop (het verloop van de zaak)".to_string()
        ), "{:?}", c.fouten);
}

/// Een oordeel geeft de behandelaar in het besluitformulier; een oordeel
/// dat ook uit een lexostatus komt, heeft een verkeerde bron.
#[test]
fn een_oordeel_uit_een_lexostatus_is_een_fout() {
    let c = afnemer(
        |t| {
            t.replace(
                    "origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: zetels_op_lijst",
                    "origin: {waarde: OORDEEL, grondslag: testregeling_afnemer#3 lid 1}\n          - name: zetels_op_lijst",
                )
        },
        zo,
        &[],
    );
    assert!(c.fouten.contains(
            &"besluit: verkeerde bron voor parameter 'aanvraagdatum' van testregeling_afnemer#3 (OORDEEL, grondslag testregeling_afnemer#3 lid 1): een oordeel geeft de behandelaar in het formulier van de handeling, maar hij komt uit eigen lexostatus aanvraag_inhoud (wat de aanvrager indiende)".to_string()
        ), "{:?}", c.fouten);
}

#[test]
fn een_register_moet_bij_de_bron_passen() {
    let c = afnemer(
            jaar_met("{waarde: REGISTER, register: testregeling_afnemer, grondslag: testregeling_register#3}"),
            zo,
            &[],
        );
    assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'jaar' van testregeling_afnemer#3 (REGISTER, register testregeling_afnemer, grondslag testregeling_register#3): synthese-bron test_register/registerstatus houdt geen kroniek bij met een grondslag in 'testregeling_afnemer'"]
        );
}

/// Het register van een herkomst is een geladen regeling (een fout); een
/// grondslag in een regeling die niet geladen is, is niet na te gaan (een
/// waarschuwing).
#[test]
fn register_en_grondslag_zijn_geladen() {
    let c = afnemer(
        jaar_met(
            "{waarde: REGISTER, register: een_onbekend_register, grondslag: 'een_onbekende_wet#1'}",
        ),
        zo,
        &[],
    );
    assert!(c.warnings.contains(
            &"provenance: parameter 'jaar' van testregeling_afnemer#3 (REGISTER, register een_onbekend_register, grondslag een_onbekende_wet#1): grondslag 'een_onbekende_wet#1': regeling 'een_onbekende_wet' is niet geladen; niet na te gaan".to_string()
        ), "{:?}", c.warnings);
    assert!(c.fouten.contains(
            &"provenance: parameter 'jaar' van testregeling_afnemer#3 (REGISTER, register een_onbekend_register, grondslag een_onbekende_wet#1): register 'een_onbekend_register' is geen geladen regeling".to_string()
        ), "{:?}", c.fouten);
}

/// Een bron met een url is niet na te gaan: ze telt, met een
/// waarschuwing die zegt waarom.
#[test]
fn een_bron_met_een_url_telt_met_een_waarschuwing() {
    let c = afnemer(
        zo,
        |t| {
            t.replace(
                    "  - cell: test_register\n    lexostatus: registerstatus\n",
                    "  - cell: test_register\n    url: http://register.example\n    lexostatus: registerstatus\n",
                )
        },
        &[],
    );
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert_eq!(
            c.warnings,
            ["herkomst van 'datum_mededeling', 'geblokkeerd_raad', 'jaar', 'zetels_op_lijst' niet na te gaan: synthese-bron test_register/registerstatus draait buiten deze runtime (http://register.example); of haar lexostatus een kroniek bijhoudt met een grondslag in 'testregeling_register', is bij het opstarten niet te zien"]
        );
}

/// Een interne bron waarvan de cel niet in deze runtime draait, telt
/// ook, met een waarschuwing.
#[test]
fn een_interne_bron_die_niet_draait_telt_met_een_waarschuwing() {
    let s = service(zo, &[]);
    let mut c = cells(&s);
    c.remove("test_register");
    let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
    assert!(uit.fouten.is_empty(), "{:?}", uit.fouten);
    assert_eq!(
            uit.warnings,
            [
                "herkomst van 'is_geschrapt_raad', 'is_ingeschreven_raad' niet na te gaan: synthese-bron test_register/register heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in 'testregeling_register', is niet te zien",
                "herkomst van 'datum_mededeling', 'geblokkeerd_raad', 'jaar', 'zetels_op_lijst' niet na te gaan: synthese-bron test_register/registerstatus heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in 'testregeling_register', is niet te zien",
            ]
        );
}

/// De rijen van de toets leveren de tabel aan de toets; die van het
/// besluit niet.
#[test]
fn de_rijen_van_de_toets_leveren_aan_de_toets() {
    let vraagt_tabel = |t: String| {
        t.replacen(
                "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: is_ingeschreven_raad",
                "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: gebiedstabel\n            type: array\n            nullable: true\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: is_ingeschreven_raad",
                1,
            )
    };
    let c = afnemer(vraagt_tabel, zo, &[]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert_eq!(
            c.warnings,
            ["assessment: geen leverancier voor parameter 'gebiedstabel' van testregeling_afnemer#1 (BELANGHEBBENDE, grondslag testregeling_afnemer#1); required: false, dus de engine krijgt hem niet en rekent met een onbekende waarde (RFC-036)"]
        );
    let met_rijen = |t: String| {
        t.replace(
                "    output: aanvraag_toelaatbaar\n",
                "    output: aanvraag_toelaatbaar\n    rows:\n      - parameter: gebiedstabel\n        table: {lexostatus: aanvraag_inhoud, field: gebieden}\n        columns: {gebied: gebied}\n",
            )
    };
    let c = afnemer(vraagt_tabel, met_rijen, &[]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
}

/// Zonder origin, en een belanghebbende-parameter zonder required: false:
/// waarschuwingen, geen fouten.
#[test]
fn waarschuwingen_over_de_herkomst() {
    let s = service(zo, &[]);
    let c = cells(&s);
    let d = proces("instantie", zo);
    let uit = controleer(&d, &c["test_instantie"], &c, &s);
    assert!(uit.fouten.is_empty(), "{:?}", uit.fouten);
    assert!(uit.warnings.contains(
            &"provenance: parameter 'bevat_naam' van testregeling_aanvraag#1 komt van de belanghebbende, maar heeft geen required: false (RFC-036)".to_string()
        ), "{:?}", uit.warnings);
    // bevat_aantal_aanduidingen heeft required: false.
    assert!(!uit
        .warnings
        .iter()
        .any(|w| w.contains("'bevat_aantal_aanduidingen'")));

    let without = |t: String| {
        t.replace("            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen", "          - name: opgeschorte_dagen")
    };
    let s = service(without, &[]);
    let c = cells(&s);
    let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
    assert_eq!(
            uit.warnings,
            ["provenance: parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 heeft geen origin; wie hem levert is niet na te gaan"]
        );
}

/// In een proces met `herkomst: streng` is een parameter zonder origin
/// een fout.
#[test]
fn strikte_herkomst_maakt_een_parameter_zonder_origin_een_fout() {
    let without = |t: String| {
        t.replace("            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen", "          - name: opgeschorte_dagen")
    };
    let streng = |t: String| {
        t.replace(
            "actor: test_afnemer\n",
            "actor: test_afnemer\norigin_check: strict\n",
        )
    };
    let c = afnemer(without, streng, &[]);
    assert_eq!(
            c.fouten,
            ["provenance: parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 heeft geen origin; wie hem levert is niet na te gaan (herkomst: streng)"]
        );
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    // Zonder streng blijft het een waarschuwing (zie hierboven), en
    // `ruim` is hetzelfde als niets.
    let ruim = |t: String| {
        t.replace(
            "actor: test_afnemer\n",
            "actor: test_afnemer\norigin_check: lenient\n",
        )
    };
    let c = afnemer(without, ruim, &[]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert_eq!(c.warnings.len(), 1, "{:?}", c.warnings);
}

/// Het betalingsvoorbeeld: een regeling die het vastgestelde en het
/// betaalde bedrag uit het dossier vraagt, uitgevoerd door een proces
/// zonder lexostatus die het betaalde bedrag levert.
const BETALING: &str = r#"
$id: testregeling_betaling
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: |-
      1. Het bedrag wordt overeenkomstig de vaststelling betaald.
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters:
          - name: vastgesteld_bedrag
            type: number
            origin: {waarde: DOSSIER, grondslag: 'testregeling_betaling#1 lid 1'}
          - name: betaald_bedrag
            type: number
            origin: {waarde: DOSSIER, grondslag: 'testregeling_betaling#1 lid 1'}
        output:
          - name: nog_te_betalen
            type: number
        actions:
          - output: nog_te_betalen
            value:
              operation: MAX
              values:
                - 0
                - operation: SUBTRACT
                  values: [$vastgesteld_bedrag, $betaald_bedrag]
  - number: '2'
    text: |-
      1. Een besluit vermeldt de dag waarop het is genomen.
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        output:
          - name: vermeldt_dag
            type: boolean
        actions:
          - output: vermeldt_dag
            value: true
"#;

#[test]
fn het_betalingsvoorbeeld_mist_een_leverancier() {
    let c = afnemer(
        zo,
        |t| {
            alleen_het_besluit(t)
                    .replace("  - {cell: test_afnemer, lexostatus: besluit, case: true}\n", "")
                    .replace("      regulation: testregeling_afnemer\n", "      regulation: testregeling_betaling\n")
                    .replace(
                        "outputs: [vastgesteld_bedrag, gebiedsbedrag, besluit_tijdig, besluitdeadline, zorgvuldig]",
                        "outputs: [nog_te_betalen]",
                    )
        },
        &[BETALING],
    );
    assert_eq!(
            c.fouten,
            [
                "besluit: geen leverancier voor parameter 'vastgesteld_bedrag' van testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
                "besluit: geen leverancier voor parameter 'betaald_bedrag' van testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
            ]
        );
}

/// Elke uitkomst van het besluit telt (RFC-043: "every outcome"), niet
/// alleen de eerste: de parameters van een tweede artikel ook.
#[test]
fn elke_uitkomst_van_het_besluit_telt() {
    let met = |outputs: &'static str| {
        move |t: String| {
            alleen_het_besluit(t)
                    .replace("  - {cell: test_afnemer, lexostatus: besluit, case: true}\n", "")
                    .replace("      regulation: testregeling_afnemer\n", "      regulation: testregeling_betaling\n")
                    .replace(
                        "outputs: [vastgesteld_bedrag, gebiedsbedrag, besluit_tijdig, besluitdeadline, zorgvuldig]",
                        outputs,
                    )
        }
    };
    // Alleen het tweede artikel: niets te leveren.
    let c = afnemer(zo, met("outputs: [vermeldt_dag]"), &[BETALING]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    // Het artikel zonder parameters eerst: het tweede telt nog steeds.
    let c = afnemer(
        zo,
        met("outputs: [vermeldt_dag, nog_te_betalen]"),
        &[BETALING],
    );
    assert_eq!(c.fouten.len(), 2, "{:?}", c.fouten);
    assert!(
        c.fouten[0].contains("'vastgesteld_bedrag'"),
        "{:?}",
        c.fouten
    );
}

/// Een aanbod dat een dossierfeit vraagt, houdt de runtime tegen.
#[test]
fn een_aanbod_op_een_dossierfeit_is_een_fout() {
    let c = afnemer(
        zo,
        |t| {
            t.replace(
                    "    output: aanvraag_toelaatbaar\n",
                    "    output: aanvraag_toelaatbaar\n  offer: {regulation: testregeling_afnemer, output: besluitdeadline}\n",
                )
        },
        &[],
    );
    assert!(c.fouten.contains(
            &"offer: voorwaarde leunt op 'opgeschorte_dagen' (DOSSIER, grondslag testregeling_afnemer#3 lid 1), dat vooraf niet bekend is".to_string()
        ), "{:?}", c.fouten);
    assert!(c.fouten.contains(
            &"offer: voorwaarde leunt op 'aanvraagdatum' (BELANGHEBBENDE, grondslag testregeling_afnemer#1), dat vooraf niet bekend is".to_string()
        ), "{:?}", c.fouten);
    // Registerfeiten mogen.
    assert!(
        !c.fouten.iter().any(|f| f.contains("'jaar'")),
        "{:?}",
        c.fouten
    );
}

/// Een aanbod op register- en loginfeiten mag.
#[test]
fn een_aanbod_op_registerfeiten() {
    let c = afnemer(
        zo,
        |t| {
            t.replace(
                    "    output: aanvraag_toelaatbaar\n",
                    "    output: aanvraag_toelaatbaar\n  offer: {regulation: testregeling_afnemer, output: lijst_heeft_zetels}\n",
                )
        },
        &[],
    );
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
}

/// Het tijdvak is een rol, geen grondslag: de grondslag Awb 4:2 lid 1
/// zonder `rol: TIJDVAK` is een deel van de aanvraag dat pas na het
/// invullen bekend is, dus geen tijdvak en niet vooraf bekend.
#[test]
fn het_tijdvak_is_een_rol_geen_grondslag() {
    let met_aanbod = |t: String| {
        t.replace(
                "    output: aanvraag_toelaatbaar\n",
                "    output: aanvraag_toelaatbaar\n  offer:\n    regulation: testregeling_afnemer\n    output: aanvraag_aangeboden\n    deadline: aanvraagtermijn\n    windows: aangeboden_jaren\n    start: begin_aanvraagjaar\n",
            )
    };
    let c = afnemer(zo, met_aanbod, &[]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    assert_eq!(c.window.as_deref(), Some("aanvraagjaar"));

    let zonder_rol = |t: String| t.replace(", rol: TIJDVAK}", "}");
    let c = afnemer(zonder_rol, met_aanbod, &[]);
    assert_eq!(c.window, None);
    assert_eq!(
            c.fouten,
            [
                "offer: voorwaarde leunt op 'aanvraagjaar' (BELANGHEBBENDE, grondslag algemene_wet_bestuursrecht#4:2 lid 1), dat vooraf niet bekend is",
                "offer: tijdvakken, maar testregeling_afnemer vraagt geen tijdvak (een parameter met origin BELANGHEBBENDE en rol TIJDVAK)",
            ]
        );
}

/// De vorm van een origin, bij het laden van elke regeling: een REGISTER
/// noemt zijn register, alleen een REGISTER doet dat, een tijdvak komt
/// van de belanghebbende, een grondslag is te ontleden, en een waarde
/// die niet te lezen is, is een fout met artikel en parameter.
#[test]
fn de_vorm_van_een_origin_bij_het_laden() {
    let law = |origin: &str| -> ArticleBasedLaw {
        serde_yaml_ng::from_str(&format!(
                "$id: een_wet\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Tekst.\n    machine_readable:\n      execution:\n        parameters:\n          - name: een_feit\n            type: boolean\n            origin: {origin}\n"
            ))
            .unwrap()
    };
    let error = |origin: &str| valideer(&law(origin));
    assert!(
        error("{waarde: REGISTER, register: een_registerwet, grondslag: 'een_wet#1'}").is_empty()
    );
    assert_eq!(
            error("{waarde: REGISTER, grondslag: 'een_wet#1'}"),
            ["artikel 1, parameter 'een_feit': origin REGISTER zonder register: welke regeling het register houdt, is niet na te gaan"]
        );
    assert_eq!(
            error("{waarde: DOSSIER, register: een_registerwet, grondslag: 'een_wet#1'}"),
            ["artikel 1, parameter 'een_feit': origin DOSSIER met register 'een_registerwet': alleen REGISTER noemt een register"]
        );
    assert_eq!(
            error("{waarde: DOSSIER, grondslag: 'een_wet#1', rol: TIJDVAK}"),
            ["artikel 1, parameter 'een_feit': rol TIJDVAK bij origin DOSSIER: het tijdvak kiest de aanvrager als deel van de gevraagde beschikking (Awb 4:2 lid 1), dus BELANGHEBBENDE"]
        );
    assert_eq!(
            error("{waarde: BELANGHEBBENDE, grondslag: een_wet}"),
            ["artikel 1, parameter 'een_feit': grondslag 'een_wet' heeft niet de vorm <regeling>#<artikel>"]
        );
    let f = error("{waarde: KADER, grondslag: 'een_wet#1'}");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].starts_with(
            "artikel 1, parameter 'een_feit': ongeldige origin: unknown variant `KADER`"
        ),
        "{f:?}"
    );
}

/// `origins` staat alleen in uitvoeringsbeleid, en een overschrijving die
/// niet te lezen is, is een fout.
#[test]
fn de_vorm_van_origins_bij_het_laden() {
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(&BELEID.replace(
        "regulatory_layer: UITVOERINGSBELEID",
        "regulatory_layer: WET",
    ))
    .unwrap();
    assert_eq!(
        valideer(&law),
        ["artikel 1: origins staat alleen in uitvoeringsbeleid (RFC-043)"]
    );
    let beleid: ArticleBasedLaw =
        serde_yaml_ng::from_str(&BELEID.replace("parameter: jaar", "parameter_: jaar")).unwrap();
    let f = valideer(&beleid);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].starts_with("artikel 1, origins[0]: ongeldige overschrijving:"),
        "{f:?}"
    );
    let beleid: ArticleBasedLaw =
        serde_yaml_ng::from_str(&BELEID.replace("waarde: DOSSIER", "waarde: REGISTER")).unwrap();
    assert_eq!(
            valideer(&beleid),
            ["artikel 1, origins voor 'jaar' van testregeling_afnemer: origin REGISTER zonder register: welke regeling het register houdt, is niet na te gaan"]
        );
}

/// Een ongeldige origin houdt het laden van het corpus tegen, met het
/// bestand, het artikel en de parameter; de engine zelf laadt de
/// regeling wel.
#[test]
fn een_ongeldige_origin_noemt_bestand_en_parameter() {
    // Niet met een punt vooraan: de lader slaat verborgen mappen over.
    let map = tempfile::Builder::new()
        .prefix("regelingen")
        .tempdir()
        .unwrap();
    let tekst =
        std::fs::read_to_string(fixtures().join("regulation/testregeling_afnemer/2025-01-01.yaml"))
            .unwrap()
            .replacen("waarde: OORDEEL", "waarde: OORDEL", 1);
    let path = map.path().join("afnemer.yaml");
    std::fs::write(&path, tekst).unwrap();
    let fouten = regelingen::laad(map.path()).err().unwrap();
    assert_eq!(fouten.len(), 1, "{fouten:?}");
    assert!(
        fouten[0].starts_with(&format!(
            "{}: artikel 3, parameter 'besluitdatum': ongeldige origin: unknown variant `OORDEL`",
            path.display()
        )),
        "{fouten:?}"
    );
}

/// Het besluitformulier: de OORDEEL-parameters van het besluit, met het
/// label na "Naam:" en de groep uit de grondslag.
#[test]
fn het_besluitformulier_volgt_uit_origin() {
    let s = service(zo, &[]);
    let c = cells(&s);
    let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
    let o = verdicts(&uit, &s, "besluit");
    let fields: Vec<(&str, &str, Option<&str>)> = o
        .iter()
        .map(|o| (o.parameter.as_str(), o.label.as_str(), o.group.as_deref()))
        .collect();
    assert_eq!(
        fields,
        [
            (
                "besluitdatum",
                "Besluitdatum",
                Some("Testregeling afnemer, artikel 3")
            ),
            (
                "feiten_vergaard",
                "De relevante feiten zijn vergaard",
                Some("Testregeling afnemer, artikel 3")
            ),
        ]
    );
    // Zonder "Naam:" is het label de omschrijving, zonder omschrijving de
    // naam.
    let s = service(
        |t| {
            t.replace(
                "'Het oordeel van de instantie bij het besluiten. Naam: Besluitdatum.'",
                "De dag van het besluit.",
            )
        },
        &[],
    );
    let c = cells(&s);
    let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
    assert_eq!(
        verdicts(&uit, &s, "besluit")[0].label,
        "De dag van het besluit"
    );
    assert_eq!(
        crate::formulier::leesbaar("een_regeling_zonder_naam"),
        "Een regeling zonder naam"
    );
}

/// Uitvoeringsbeleid van de actor geeft `jaar` een andere herkomst.
const BELEID: &str = r#"
$id: testbeleid_afnemer
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
competent_authority: {name: Test afnemer}
articles:
  - number: '1'
    text: |-
      1. De afnemer stelt het jaar zelf vast.
    machine_readable:
      origins:
        - regulation: testregeling_afnemer
          parameter: jaar
          origin: {waarde: DOSSIER, grondslag: 'testbeleid_afnemer#1 lid 1'}
"#;

#[test]
fn een_overschrijving_in_beleid_wint() {
    let c = afnemer(zo, zo, &[BELEID]);
    assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'jaar' van testregeling_afnemer#3 (DOSSIER, grondslag testbeleid_afnemer#1 lid 1, uit testbeleid_afnemer#1): hij komt uit synthese-bron test_register/registerstatus"]
        );
    // Beleid van een ander gezag telt niet.
    let ander = BELEID.replace("name: Test afnemer", "name: Een ander");
    let c = afnemer(zo, zo, &[&ander]);
    assert!(c.fouten.is_empty(), "{:?}", c.fouten);
}

#[test]
fn twee_botsende_overschrijvingen_zijn_een_fout() {
    let tweede = format!(
            "{BELEID}  - number: '2'\n    text: Tweede.\n    machine_readable:\n      origins:\n        - regulation: testregeling_afnemer\n          parameter: jaar\n          origin: {{waarde: REGISTER, register: testregeling_register, grondslag: 'testbeleid_afnemer#2'}}\n"
        );
    let c = afnemer(zo, zo, &[&tweede]);
    assert_eq!(
            c.fouten,
            ["origins: 'jaar' van testregeling_afnemer krijgt twee herkomsten: DOSSIER, grondslag testbeleid_afnemer#1 lid 1, uit testbeleid_afnemer#1 en REGISTER, register testregeling_register, grondslag testbeleid_afnemer#2, uit testbeleid_afnemer#2"]
        );
    // Een overschrijving van een parameter die niet bestaat.
    let onbekend = BELEID.replace("parameter: jaar", "parameter: bestaat_niet");
    let c = afnemer(zo, zo, &[&onbekend]);
    assert_eq!(
            c.fouten,
            ["origins in testbeleid_afnemer#1: regeling 'testregeling_afnemer' heeft geen parameter 'bestaat_niet'"]
        );
}
