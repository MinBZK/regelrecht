//! Tests of the origin check.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The fixture regulations, an adjustment to the consumer's regulation,
/// and extra regulations.
fn service(adjust: impl Fn(String) -> String, extra: &[&str]) -> Arc<LawExecutionService> {
    let mut s = LawExecutionService::new();
    for e in walkdir::WalkDir::new(fixtures().join("regulation")) {
        let e = e.unwrap();
        if e.file_type().is_file() {
            let mut text = std::fs::read_to_string(e.path()).unwrap();
            // The consumer's regulation, and the Awb with the procedure.
            if text.contains("$id: testregeling_afnemer") || text.contains("$id: testregeling_awb")
            {
                text = adjust(text);
            }
            s.load_law(&text).unwrap();
        }
    }
    for t in extra {
        s.load_law(t).unwrap();
    }
    Arc::new(s)
}

fn cells(s: &Arc<LawExecutionService>) -> BTreeMap<String, Arc<Cell>> {
    crate::config::cell_dirs(&fixtures().join("cells"))
        .unwrap()
        .iter()
        .map(|m| {
            let c = Cell::load(m, s.clone()).unwrap();
            (c.id().to_string(), Arc::new(c))
        })
        .collect()
}

/// A process of the fixtures as the policy gives it (RFC-047), after
/// `adjust`.
fn process(cell: &str, adjust: impl FnOnce(&mut ProcessDefinition)) -> ProcessDefinition {
    let mut d = crate::derive::tests::derived(cell);
    adjust(&mut d);
    d
}

/// The consumer's process as it is.
fn keep(_: &mut ProcessDefinition) {}

/// The check on the consumer's process.
fn consumer(
    regulation: impl Fn(String) -> String,
    adjust: impl FnOnce(&mut ProcessDefinition),
    extra: &[&str],
) -> Check {
    let s = service(regulation, extra);
    let c = cells(&s);
    let d = process("test_afnemer", adjust);
    check_with_state(d, &c, &s)
}

/// As when loading a process: prepare the actions (the
/// kind and what has not happened yet, from the procedure), then the check.
fn check_with_state(
    mut d: ProcessDefinition,
    c: &BTreeMap<String, Arc<Cell>>,
    s: &Arc<LawExecutionService>,
) -> Check {
    let authority = crate::authority::own(&d, s);
    let f = crate::action::prepare_for(&mut d, authority.as_deref(), s, &c["test_afnemer"]);
    assert!(f.is_empty(), "{f:?}");
    check(&d, &c["test_afnemer"], c, s)
}

fn same(t: String) -> String {
    t
}

/// Replace the origin of `jaar` (art. 3).
fn year_with(origin: &'static str) -> impl Fn(String) -> String {
    move |t: String| {
        t.replace(
                "origin: {waarde: REGISTER, register: testregeling_register, grondslag: testregeling_register#3}\n          - name: gebiedstabel",
                &format!("origin: {origin}\n          - name: gebiedstabel"),
            )
    }
}

#[test]
fn the_consumer_fixture_has_a_supplier_for_everything() {
    let c = consumer(same, keep, &[]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    let decision: Vec<(&str, OriginValue)> = c.parameters["besluit_genomen"]
        .iter()
        .map(|(b, g)| (b.name.as_str(), g.as_ref().unwrap().origin.waarde))
        .collect();
    assert!(decision.contains(&("feiten_vergaard", OriginValue::Oordeel)));
    assert!(decision.contains(&("jaar", OriginValue::Register)));
}

/// A required parameter without a supplier stops the runtime, with
/// parameter, origin and legal basis in the message.
#[test]
fn a_missing_supplier_is_an_error() {
    // The procedure no longer asks for the date of publication in a
    // later stage, and the lexostatus that reads it is not a source: then
    // nothing supplies it.
    let c = consumer(
        |t| t.replace("          - {name: datum_bekendmaking, type: date}\n", ""),
        |d| {
            only_the_decision(d);
            without_the_decision_source(d);
        },
        &[],
    );
    assert_eq!(
            c.errors,
            ["besluit_genomen: no supplier for parameter 'datum_bekendmaking' of testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2)"]
        );
}

/// The consumer's process with only the action of the decision.
fn only_the_decision(d: &mut ProcessDefinition) {
    d.handling
        .as_mut()
        .unwrap()
        .actions
        .retain(|a| a.name == "besluit_genomen");
}

/// Without the lexostatus `besluit` of the case as a source.
fn without_the_decision_source(d: &mut ProcessDefinition) {
    d.synthesis
        .retain(|b| !(b.case && b.lexostatus == "besluit"));
}

/// The decision of the consumer's process executes `regulation` with
/// `outputs` instead.
fn the_decision_executes(d: &mut ProcessDefinition, regulation: &str, outputs: &[&str]) {
    let a = &mut d.handling.as_mut().unwrap().actions[0];
    a.regulation = regulation.to_string();
    a.outputs = outputs.iter().map(|o| o.to_string()).collect();
    // The article follows from the first output again.
    a.article.clear();
}

/// With required: false and without a supplier the engine does not get the
/// value and computes with an unknown one (RFC-036): a warning.
#[test]
fn without_supplier_and_not_required_is_a_warning() {
    let c = consumer(
        |t| t.replace("          - {name: bekendgemaakt, type: boolean}\n", ""),
        keep,
        &[],
    );
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert_eq!(
            c.warnings,
            ["besluit_genomen: no supplier for parameter 'bekendgemaakt' of testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2); required: false, so the engine does not get it and computes with an unknown value (RFC-036)"]
        );
}

/// A supplier of the wrong kind is an error, and the message says
/// where the parameter comes from now.
#[test]
fn a_supplier_that_does_not_fit_the_origin() {
    let c = consumer(
        year_with("{waarde: DOSSIER, grondslag: 'testregeling_afnemer#3'}"),
        keep,
        &[],
    );
    assert_eq!(
            c.errors,
            ["besluit_genomen: wrong source for parameter 'jaar' of testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3): it comes from synthesis source test_register/registerstatus"]
        );
}

/// A wrong source is also an error if the parameter is required: false:
/// the engine would then compute with a value from the wrong party.
#[test]
fn a_wrong_source_is_an_error_even_with_required_false() {
    let c = consumer(
        |t| {
            t.replace(
                    "          - name: bekendgemaakt\n            type: boolean\n            required: false\n            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 2}",
                    "          - name: bekendgemaakt\n            type: boolean\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#3 lid 2}",
                )
        },
        keep,
        &[],
    );
    assert_eq!(
            c.errors,
            ["besluit_genomen: wrong source for parameter 'bekendgemaakt' of testregeling_afnemer#3 (BELANGHEBBENDE, grondslag testregeling_afnemer#3 lid 2): it comes from the state at decision"]
        );
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
}

/// What the applicant submits (a gram of type submission) is from the
/// interested party; what the own actor records otherwise (the course of the
/// case) is dossier. Whoever swaps them gets an error.
#[test]
fn interested_party_and_dossier_follow_from_what_the_derivation_reads() {
    let c = consumer(
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
        keep,
        &[],
    );
    assert!(c.errors.contains(
            &"assessment: wrong source for parameter 'aanvraagdatum' of testregeling_afnemer#1 (DOSSIER, grondslag testregeling_afnemer#1): it comes from own lexostatus aanvraag_inhoud (what the applicant submitted)".to_string()
        ), "{:?}", c.errors);
    assert!(c.errors.contains(
            &"besluit_genomen: wrong source for parameter 'datum_uitnodiging_aanvulling' of testregeling_afnemer#3 (BELANGHEBBENDE, grondslag testregeling_afnemer#3 lid 1): it comes from own lexostatus zaakverloop (the course of the case)".to_string()
        ), "{:?}", c.errors);
}

/// The handler gives a verdict in the decision form; a verdict
/// that also comes from a lexostatus has a wrong source.
#[test]
fn a_verdict_from_a_lexostatus_is_an_error() {
    let c = consumer(
        |t| {
            t.replace(
                    "origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: zetels_op_lijst",
                    "origin: {waarde: OORDEEL, grondslag: testregeling_afnemer#3 lid 1}\n          - name: zetels_op_lijst",
                )
        },
        keep,
        &[],
    );
    assert!(c.errors.contains(
            &"besluit_genomen: wrong source for parameter 'aanvraagdatum' of testregeling_afnemer#3 (OORDEEL, grondslag testregeling_afnemer#3 lid 1): the handler gives a verdict in the form of the action, but it comes from own lexostatus aanvraag_inhoud (what the applicant submitted)".to_string()
        ), "{:?}", c.errors);
}

#[test]
fn a_register_must_fit_the_source() {
    let c = consumer(
            year_with("{waarde: REGISTER, register: testregeling_afnemer, grondslag: testregeling_register#3}"),
            keep,
            &[],
        );
    assert_eq!(
            c.errors,
            ["besluit_genomen: wrong source for parameter 'jaar' of testregeling_afnemer#3 (REGISTER, register testregeling_afnemer, grondslag testregeling_register#3): synthesis source test_register/registerstatus keeps no chronicle with a legal basis in 'testregeling_afnemer'"]
        );
}

/// The register of an origin is a loaded regulation (an error); a
/// legal basis in a regulation that is not loaded cannot be verified (a
/// warning).
#[test]
fn register_and_legal_basis_are_loaded() {
    let c = consumer(
        year_with(
            "{waarde: REGISTER, register: een_onbekend_register, grondslag: 'een_onbekende_wet#1'}",
        ),
        keep,
        &[],
    );
    assert!(c.warnings.contains(
            &"provenance: parameter 'jaar' of testregeling_afnemer#3 (REGISTER, register een_onbekend_register, grondslag een_onbekende_wet#1): legal basis 'een_onbekende_wet#1': regulation 'een_onbekende_wet' is not loaded; cannot be verified".to_string()
        ), "{:?}", c.warnings);
    assert!(c.errors.contains(
            &"provenance: parameter 'jaar' of testregeling_afnemer#3 (REGISTER, register een_onbekend_register, grondslag een_onbekende_wet#1): register 'een_onbekend_register' is not a loaded regulation".to_string()
        ), "{:?}", c.errors);
}

/// A source with a url cannot be verified: it counts, with a
/// warning that says why.
#[test]
fn a_source_with_a_url_counts_with_a_warning() {
    let c = consumer(
        same,
        |d| {
            d.synthesis
                .iter_mut()
                .find(|b| b.lexostatus == "registerstatus")
                .unwrap()
                .url = Some("http://register.example".into());
        },
        &[],
    );
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert_eq!(
            c.warnings,
            ["origin of 'datum_mededeling', 'geblokkeerd_raad', 'jaar', 'zetels_op_lijst' cannot be verified: synthesis source test_register/registerstatus runs outside this runtime (http://register.example); whether its lexostatus keeps a chronicle with a legal basis in 'testregeling_register' cannot be seen at startup"]
        );
}

/// An internal source whose cell does not run in this runtime counts
/// too, with a warning.
#[test]
fn an_internal_source_that_does_not_run_counts_with_a_warning() {
    let s = service(same, &[]);
    let mut c = cells(&s);
    c.remove("test_register");
    let out = check_with_state(process("test_afnemer", keep), &c, &s);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(
            out.warnings,
            [
                "origin of 'is_geschrapt_raad', 'is_ingeschreven_raad' cannot be verified: synthesis source test_register/register has no url and does not run in this runtime; whether its lexostatus keeps a chronicle with a legal basis in 'testregeling_register' cannot be seen",
                "origin of 'datum_mededeling', 'geblokkeerd_raad', 'jaar', 'zetels_op_lijst' cannot be verified: synthesis source test_register/registerstatus has no url and does not run in this runtime; whether its lexostatus keeps a chronicle with a legal basis in 'testregeling_register' cannot be seen",
            ]
        );
}

/// The rows of the assessment supply the table to the assessment; those of the
/// decision do not.
#[test]
fn the_assessment_rows_supply_to_the_assessment() {
    let asks_table = |t: String| {
        t.replacen(
                "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: is_ingeschreven_raad",
                "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: gebiedstabel\n            type: array\n            nullable: true\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: is_ingeschreven_raad",
                1,
            )
    };
    let c = consumer(asks_table, keep, &[]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert_eq!(
            c.warnings,
            ["assessment: no supplier for parameter 'gebiedstabel' of testregeling_afnemer#1 (BELANGHEBBENDE, grondslag testregeling_afnemer#1); required: false, so the engine does not get it and computes with an unknown value (RFC-036)"]
        );
    let with_rows = |d: &mut ProcessDefinition| {
        d.portal.as_mut().unwrap().assessment.rows = serde_yaml_ng::from_str(
            "- parameter: gebiedstabel\n  table: {lexostatus: aanvraag_inhoud, field: gebieden}\n  columns: {gebied: gebied}\n",
        )
        .unwrap();
    };
    let c = consumer(asks_table, with_rows, &[]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
}

/// An interested-party parameter without required: false: a warning, no
/// error.
#[test]
fn warnings_about_the_origin() {
    let s = service(same, &[]);
    let c = cells(&s);
    let d = process("test_instantie", keep);
    let out = check(&d, &c["test_instantie"], &c, &s);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!(out.warnings.contains(
            &"provenance: parameter 'bevat_naam' of testregeling_aanvraag#1 comes from the interested party, but has no required: false (RFC-036)".to_string()
        ), "{:?}", out.warnings);
    // bevat_aantal_aanduidingen has required: false.
    assert!(!out
        .warnings
        .iter()
        .any(|w| w.contains("'bevat_aantal_aanduidingen'")));
}

/// A parameter without origin is an error: who supplies it cannot be
/// traced. The origin check is strict for every process (RFC-047).
#[test]
fn a_parameter_without_origin_is_an_error() {
    let without = |t: String| {
        t.replace("            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen", "          - name: opgeschorte_dagen")
    };
    let c = consumer(without, keep, &[]);
    assert_eq!(
            c.errors,
            ["provenance: parameter 'datum_uitnodiging_aanvulling' of testregeling_afnemer#3 has no origin; who supplies it cannot be traced"]
        );
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
}

/// The payment example: a regulation that asks for the determined and the
/// paid amount from the dossier, executed by a process
/// without a lexostatus that supplies the paid amount.
const PAYMENT: &str = r#"
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
fn the_payment_example_is_missing_a_supplier() {
    let c = consumer(
        same,
        |d| {
            only_the_decision(d);
            without_the_decision_source(d);
            the_decision_executes(d, "testregeling_betaling", &["nog_te_betalen"]);
        },
        &[PAYMENT],
    );
    assert_eq!(
            c.errors,
            [
                "besluit_genomen: no supplier for parameter 'vastgesteld_bedrag' of testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
                "besluit_genomen: no supplier for parameter 'betaald_bedrag' of testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
            ]
        );
}

/// Every output of the decision counts (RFC-043: "every outcome"), not
/// only the first: the parameters of a second article too.
#[test]
fn every_output_of_the_decision_counts() {
    let with = |outputs: &'static [&'static str]| {
        move |d: &mut ProcessDefinition| {
            only_the_decision(d);
            without_the_decision_source(d);
            the_decision_executes(d, "testregeling_betaling", outputs);
        }
    };
    // Only the second article: nothing to supply.
    let c = consumer(same, with(&["vermeldt_dag"]), &[PAYMENT]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    // The article without parameters first: the second still counts.
    let c = consumer(same, with(&["vermeldt_dag", "nog_te_betalen"]), &[PAYMENT]);
    assert_eq!(c.errors.len(), 2, "{:?}", c.errors);
    assert!(
        c.errors[0].contains("'vastgesteld_bedrag'"),
        "{:?}",
        c.errors
    );
}

/// The portal of the consumer's process offers `output` of its regulation.
fn offer(d: &mut ProcessDefinition, output: &str) {
    d.portal.as_mut().unwrap().offer = Some(crate::config::Offer {
        regulation: "testregeling_afnemer".into(),
        output: output.into(),
        deadline: None,
        windows: None,
        start: None,
        opening: None,
    });
}

/// An offer that asks for a dossier fact stops the runtime.
#[test]
fn an_offer_on_a_dossier_fact_is_an_error() {
    let c = consumer(same, |d| offer(d, "besluitdeadline"), &[]);
    assert!(c.errors.contains(
            &"offer: condition relies on 'opgeschorte_dagen' (DOSSIER, grondslag testregeling_afnemer#3 lid 1), which is not known beforehand".to_string()
        ), "{:?}", c.errors);
    assert!(c.errors.contains(
            &"offer: condition relies on 'aanvraagdatum' (BELANGHEBBENDE, grondslag testregeling_afnemer#1), which is not known beforehand".to_string()
        ), "{:?}", c.errors);
    // Register facts are allowed.
    assert!(
        !c.errors.iter().any(|f| f.contains("'jaar'")),
        "{:?}",
        c.errors
    );
}

/// An offer on register and login facts is allowed.
#[test]
fn an_offer_on_register_facts() {
    let c = consumer(same, |d| offer(d, "lijst_heeft_zetels"), &[]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
}

/// The window is a role, not a legal basis: the legal basis Awb 4:2 lid 1
/// without `rol: TIJDVAK` is a part of the application that is only known
/// after filling it in, so not a window and not known beforehand.
#[test]
fn the_window_is_a_role_not_a_legal_basis() {
    let with_offer = |d: &mut ProcessDefinition| {
        d.portal.as_mut().unwrap().offer = Some(crate::config::Offer {
            regulation: "testregeling_afnemer".into(),
            output: "aanvraag_aangeboden".into(),
            deadline: Some("aanvraagtermijn".into()),
            windows: Some("aangeboden_jaren".into()),
            start: Some("begin_aanvraagjaar".into()),
            opening: None,
        });
    };
    let c = consumer(same, with_offer, &[]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert_eq!(c.window.as_deref(), Some("aanvraagjaar"));

    let without_role = |t: String| t.replace(", rol: TIJDVAK}", "}");
    let c = consumer(without_role, with_offer, &[]);
    assert_eq!(c.window, None);
    assert_eq!(
            c.errors,
            [
                "offer: condition relies on 'aanvraagjaar' (BELANGHEBBENDE, grondslag algemene_wet_bestuursrecht#4:2 lid 1), which is not known beforehand",
                "offer: windows, but testregeling_afnemer asks for no window (a parameter with origin BELANGHEBBENDE and rol TIJDVAK)",
            ]
        );
}

/// The shape of an origin, when loading each regulation: a REGISTER
/// names its register, only a REGISTER does so, a window comes
/// from the interested party, the decision an action acts on from the
/// dossier, a legal basis can be parsed, and a value that cannot be read
/// is an error with article and parameter.
#[test]
fn the_shape_of_an_origin_at_load_time() {
    let law = |origin: &str| -> ArticleBasedLaw {
        serde_yaml_ng::from_str(&format!(
                "$id: een_wet\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Tekst.\n    machine_readable:\n      execution:\n        parameters:\n          - name: een_feit\n            type: boolean\n            origin: {origin}\n"
            ))
            .unwrap()
    };
    let error = |origin: &str| validate(&law(origin));
    assert!(
        error("{waarde: REGISTER, register: een_registerwet, grondslag: 'een_wet#1'}").is_empty()
    );
    assert_eq!(
            error("{waarde: REGISTER, grondslag: 'een_wet#1'}"),
            ["article 1, parameter 'een_feit': origin REGISTER without register: which regulation keeps the register cannot be traced"]
        );
    assert_eq!(
            error("{waarde: DOSSIER, register: een_registerwet, grondslag: 'een_wet#1'}"),
            ["article 1, parameter 'een_feit': origin DOSSIER with register 'een_registerwet': only REGISTER names a register"]
        );
    assert_eq!(
            error("{waarde: DOSSIER, grondslag: 'een_wet#1', rol: TIJDVAK}"),
            ["article 1, parameter 'een_feit': rol TIJDVAK with origin DOSSIER: the applicant chooses the window and the decision requested as part of the application (Awb 4:2 lid 1), so BELANGHEBBENDE"]
        );
    // The decision a parameter is about is a fact of the course of the case
    // (RFC-047, UB 15), not something the applicant chooses.
    assert!(error("{waarde: DOSSIER, grondslag: 'een_wet#1', rol: BESLUIT}").is_empty());
    assert_eq!(
            error("{waarde: BELANGHEBBENDE, grondslag: 'een_wet#1', rol: BESLUIT}"),
            ["article 1, parameter 'een_feit': rol BESLUIT with origin BELANGHEBBENDE: the runtime gives the decision an action acts on, a fact of the course of the case, so DOSSIER"]
        );
    assert_eq!(
            error("{waarde: BELANGHEBBENDE, grondslag: een_wet}"),
            ["article 1, parameter 'een_feit': legal basis 'een_wet' does not have the form <regulation>#<article>"]
        );
    let f = error("{waarde: KADER, grondslag: 'een_wet#1'}");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].starts_with(
            "article 1, parameter 'een_feit': invalid origin: unknown variant `KADER`"
        ),
        "{f:?}"
    );
}

/// `origins` is only allowed in implementing policy, and an override that
/// cannot be read is an error.
#[test]
fn the_shape_of_origins_at_load_time() {
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(&POLICY.replace(
        "regulatory_layer: UITVOERINGSBELEID",
        "regulatory_layer: WET",
    ))
    .unwrap();
    assert_eq!(
        validate(&law),
        ["article 1: origins is only allowed in implementing policy (RFC-043)"]
    );
    let policy: ArticleBasedLaw =
        serde_yaml_ng::from_str(&POLICY.replace("parameter: jaar", "parameter_: jaar")).unwrap();
    let f = validate(&policy);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].starts_with("article 1, origins[0]: invalid override:"),
        "{f:?}"
    );
    let policy: ArticleBasedLaw =
        serde_yaml_ng::from_str(&POLICY.replace("waarde: DOSSIER", "waarde: REGISTER")).unwrap();
    assert_eq!(
            validate(&policy),
            ["article 1, origins for 'jaar' of testregeling_afnemer: origin REGISTER without register: which regulation keeps the register cannot be traced"]
        );
}

/// An invalid origin stops loading the corpus, with the
/// file, the article and the parameter; the engine itself does load the
/// regulation.
#[test]
fn an_invalid_origin_names_file_and_parameter() {
    // Not with a leading dot: the loader skips hidden directories.
    let map = tempfile::Builder::new()
        .prefix("regelingen")
        .tempdir()
        .unwrap();
    let text =
        std::fs::read_to_string(fixtures().join("regulation/testregeling_afnemer/2025-01-01.yaml"))
            .unwrap()
            .replacen("waarde: OORDEEL", "waarde: OORDEL", 1);
    let path = map.path().join("afnemer.yaml");
    std::fs::write(&path, text).unwrap();
    let errors = regulations::load(map.path()).err().unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].starts_with(&format!(
            "{}: article 3, parameter 'besluitdatum': invalid origin: unknown variant `OORDEL`",
            path.display()
        )),
        "{errors:?}"
    );
}

/// The decision form: the OORDEEL parameters of the decision, with the
/// label after "Naam:" and the group from the legal basis.
#[test]
fn the_decision_form_follows_from_origin() {
    let s = service(same, &[]);
    let c = cells(&s);
    let out = check_with_state(process("test_afnemer", keep), &c, &s);
    let o = verdicts(&out, &s, "besluit_genomen");
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
    // Without "Naam:" the label is the description, without a description the
    // name.
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
    let out = check_with_state(process("test_afnemer", keep), &c, &s);
    assert_eq!(
        verdicts(&out, &s, "besluit_genomen")[0].label,
        "De dag van het besluit"
    );
    assert_eq!(
        crate::form::readable("een_regeling_zonder_naam"),
        "Een regeling zonder naam"
    );
}

/// Implementing policy of the actor gives `jaar` a different origin.
const POLICY: &str = r#"
$id: testbeleid_afnemer_herkomst
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
          origin: {waarde: DOSSIER, grondslag: 'testbeleid_afnemer_herkomst#1 lid 1'}
"#;

#[test]
fn an_override_in_policy_wins() {
    let c = consumer(same, keep, &[POLICY]);
    assert_eq!(
            c.errors,
            ["besluit_genomen: wrong source for parameter 'jaar' of testregeling_afnemer#3 (DOSSIER, grondslag testbeleid_afnemer_herkomst#1 lid 1, from testbeleid_afnemer_herkomst#1): it comes from synthesis source test_register/registerstatus"]
        );
    // Policy of another authority does not count.
    let other = POLICY.replace("name: Test afnemer", "name: Een ander");
    let c = consumer(same, keep, &[&other]);
    assert!(c.errors.is_empty(), "{:?}", c.errors);
}

#[test]
fn two_clashing_overrides_are_an_error() {
    let second = format!(
            "{POLICY}  - number: '2'\n    text: Tweede.\n    machine_readable:\n      origins:\n        - regulation: testregeling_afnemer\n          parameter: jaar\n          origin: {{waarde: REGISTER, register: testregeling_register, grondslag: 'testbeleid_afnemer_herkomst#2'}}\n"
        );
    let c = consumer(same, keep, &[&second]);
    assert_eq!(
            c.errors,
            ["origins: 'jaar' of testregeling_afnemer gets two origins: DOSSIER, grondslag testbeleid_afnemer_herkomst#1 lid 1, from testbeleid_afnemer_herkomst#1 and REGISTER, register testregeling_register, grondslag testbeleid_afnemer_herkomst#2, from testbeleid_afnemer_herkomst#2"]
        );
    // An override of a parameter that does not exist.
    let unknown = POLICY.replace("parameter: jaar", "parameter: bestaat_niet");
    let c = consumer(same, keep, &[&unknown]);
    assert_eq!(
            c.errors,
            ["origins in testbeleid_afnemer_herkomst#1: regulation 'testregeling_afnemer' has no parameter 'bestaat_niet'"]
        );
}

/// Give `bekendgemaakt` of the decision article origin role BESLUIT (RFC-047)
/// and prepare the consumer's actions: the parameter that gets the decision
/// id follows from the law, or the errors that `prepare_for` reports.
fn prepared_with_besluit(
    regulation: impl Fn(String) -> String,
    adjust: impl FnOnce(&mut ProcessDefinition),
) -> (ProcessDefinition, Vec<String>) {
    let s = service(
        |t| {
            regulation(t.replace(
                "origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 2}\n          - name: datum_uitnodiging_aanvulling",
                "origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 2, rol: BESLUIT}\n          - name: datum_uitnodiging_aanvulling",
            ))
        },
        &[],
    );
    let c = cells(&s);
    let mut d = process("test_afnemer", adjust);
    let authority = crate::authority::own(&d, &s);
    let errors = crate::action::prepare_for(&mut d, authority.as_deref(), &s, &c["test_afnemer"]);
    (d, errors)
}

fn decision_parameter(d: &ProcessDefinition, action: &str) -> Option<String> {
    d.handling
        .as_ref()
        .and_then(|h| h.actions.iter().find(|a| a.name == action))
        .and_then(|a| a.decision_parameter.clone())
}

#[test]
fn prepare_for_fills_in_the_decision_parameter_from_the_law() {
    let (d, errors) = prepared_with_besluit(same, keep);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        decision_parameter(&d, "besluit_genomen").as_deref(),
        Some("bekendgemaakt")
    );
    // Without the role nothing is filled in.
    let s = service(same, &[]);
    let c = cells(&s);
    let mut d = process("test_afnemer", keep);
    let authority = crate::authority::own(&d, &s);
    let errors = crate::action::prepare_for(&mut d, authority.as_deref(), &s, &c["test_afnemer"]);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(decision_parameter(&d, "besluit_genomen"), None);
}

#[test]
fn a_configured_decision_parameter_that_contradicts_the_law_is_an_error() {
    let (_, errors) = prepared_with_besluit(same, |d| {
        d.handling
            .as_mut()
            .unwrap()
            .actions
            .iter_mut()
            .find(|a| a.name == "besluit_genomen")
            .unwrap()
            .decision_parameter = Some("jaar".into());
    });
    assert!(
        errors.iter().any(|e| e.contains("action 'besluit_genomen'")
            && e.contains("'jaar'")
            && e.contains("'bekendgemaakt'")),
        "{errors:?}"
    );
}

#[test]
fn more_than_one_besluit_parameter_is_an_error() {
    let (_, errors) = prepared_with_besluit(
        |t| {
            t.replace(
                "origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: jaar",
                "origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1, rol: BESLUIT}\n          - name: jaar",
            )
        },
        keep,
    );
    assert!(
        errors
            .iter()
            .any(|e| e.contains("more than one parameter with origin role BESLUIT")),
        "{errors:?}"
    );
}
